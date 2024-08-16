use std::{borrow::BorrowMut, io::Read, iter::Iterator, ops::Deref, os::fd::AsRawFd, pin::Pin};

use cata::{Command, Container};
use clap::Parser;
use crossterm::event::EventStream;
use eyre::{eyre, Result};
use futures::{channel::mpsc::Sender, FutureExt, SinkExt, StreamExt};
use k8s_openapi::{api::core::v1::Pod, apimachinery::pkg::apis::meta::v1::Status};
use kube::api::{AttachParams, TerminalSize};
use mio::{unix::SourceFd, Events, Interest, Poll};
use ratatui::{backend::CrosstermBackend, prelude::Backend, widgets::Clear, Terminal};
use replace_with::replace_with_or_abort;
use tokio::{
    io::AsyncWriteExt,
    signal,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    task::JoinSet,
    time::Duration,
};
use tokio_util::bytes::Bytes;
use tracing::info;

use crate::{
    events::{Broadcast, Event, Keypress},
    widget::{
        pod::{self},
        Raw, Widget,
    },
};

#[derive(Parser, Container)]
pub struct Dashboard {
    #[arg(long, default_value = "100ms")]
    ticks: humantime::Duration,

    #[arg(long)]
    route: Vec<String>,
}

fn poll_stdin(tx: &UnboundedSender<Bytes>) -> Result<()> {
    let mut poll = Poll::new()?;
    let mut events = Events::with_capacity(1024);

    let fd = std::io::stdin().as_raw_fd();
    let mut fd = SourceFd(&fd);

    poll.registry()
        .register(&mut fd, mio::Token(0), Interest::READABLE)?;

    loop {
        if tx.is_closed() {
            break;
        }

        poll.poll(&mut events, Some(Duration::from_millis(100)))?;

        for event in &events {
            if event.token() == mio::Token(0) {
                let mut buf = [0; 1024];
                let n = std::io::stdin().read(&mut buf)?;

                tx.send(Bytes::copy_from_slice(&buf[..n]))?;
            }
        }
    }

    Ok(())
}

async fn events(tick: Duration, sender: UnboundedSender<Bytes>) -> Result<()> {
    let mut tick = tokio::time::interval(tick);

    let (tx, mut rx) = unbounded_channel::<Bytes>();

    // While blocking tasks cannot be aborted, this *should* exit when this function
    // drops rx. Spawning the mio polling via spawn results in rx.recv() being
    // blocked without a yield happening.
    tokio::task::spawn_blocking(move || poll_stdin(&tx).unwrap());

    loop {
        tokio::select! {
            message = rx.recv() => {
                let Some(message) = message else {
                    break;
                };

                sender.send(message)?;
            }
            _ = tick.tick() => {
                sender.send(Bytes::new())?;
            }
        }
    }

    drop(rx);

    Ok(())
}

enum Mode {
    UI(Box<dyn Widget>),
    Raw(Box<dyn Raw>, Box<dyn Widget>),
}

impl Mode {
    fn raw(&mut self, raw: Box<dyn Raw>) {
        replace_with_or_abort(self, |self_| match self_ {
            Self::UI(previous) | Self::Raw(_, previous) => Self::Raw(raw, previous),
        });
    }

    fn ui(&mut self) {
        replace_with_or_abort(self, |self_| match self_ {
            Self::Raw(_, previous) => Self::UI(previous),
            _ => self_,
        });
    }
}

fn dispatch(mode: &mut Mode, term: &mut Terminal<impl Backend>, ev: &Event) -> Result<Broadcast> {
    let Mode::UI(widget) = mode else {
        return Err(eyre!("expected UI mode"));
    };

    match ev {
        Event::Render => {}
        Event::Keypress(key) => {
            if matches!(key, Keypress::EndOfText) {
                return Ok(Broadcast::Exited);
            }

            return widget.dispatch(ev);
        }
        _ => {
            return Ok(Broadcast::Ignored);
        }
    }

    term.draw(|frame| {
        let area = frame.size();

        widget.draw(frame, area);
    })?;

    Ok(Broadcast::Ignored)
}

async fn raw(
    term: &mut Terminal<impl Backend>,
    widget: &mut Box<dyn Raw>,
    input: &mut UnboundedReceiver<Bytes>,
) -> Result<()> {
    term.clear()?;
    term.reset_cursor()?;

    widget
        .start(input, Pin::new(Box::new(tokio::io::stdout())))
        .await?;

    term.clear()?;

    Ok(())
}

async fn ui<W>(route: Vec<String>, mut rx: UnboundedReceiver<Bytes>, tx: W) -> Result<()>
where
    W: std::io::Write + Send + 'static,
{
    let mut term = Terminal::new(CrosstermBackend::new(tx))?;

    term.clear()?;

    // kube::Client ends up being cloned by ~every widget, it'd be nice to Arc<> it
    // so that there's not a bunch of copying. Unfortunately, the Api interface
    // doesn't like Arc<>.
    let mut root = pod::List::new(kube::Client::try_default().await?);
    root.dispatch(&Event::Goto(route.clone()))?;

    let mut state = Mode::UI(Box::new(root));

    while let Some(ev) = rx.recv().await {
        let event = ev.try_into()?;

        let result = match state {
            Mode::UI(_) => dispatch(&mut state, &mut term, &event)?,
            Mode::Raw(ref mut widget, _) => {
                raw(&mut term, widget, &mut rx).await?;

                state.ui();

                Broadcast::Ignored
            }
        };

        match result {
            Broadcast::Exited => {
                break;
            }
            Broadcast::Raw(widget) => {
                state.raw(widget);
            }
            _ => {}
        }
    }

    Ok(())
}

trait ClearScreen {
    fn clear(&mut self) -> Result<()>;
    fn reset_cursor(&mut self) -> Result<()>;
}

impl<B> ClearScreen for Terminal<B>
where
    B: Backend,
{
    fn clear(&mut self) -> Result<()> {
        self.draw(|frame| {
            frame.render_widget(Clear, frame.size());
        })?;

        Ok(())
    }

    fn reset_cursor(&mut self) -> Result<()> {
        self.draw(|frame| {
            frame.set_cursor(0, 0);
        })?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl Command for Dashboard {
    async fn run(&self) -> Result<()> {
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(std::io::stderr(), crossterm::terminal::EnterAlternateScreen)?;

        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<Bytes>();

        let mut background = JoinSet::new();

        background.spawn(events(self.ticks.into(), sender.clone()));
        background.spawn(ui(self.route.clone(), receiver, std::io::stdout()));

        // Exit when *anything* ends (on error or otherwise).
        while let Some(res) = background.join_next().await {
            res??;

            background.shutdown().await;
        }

        Ok(())
    }
}

impl Drop for Dashboard {
    fn drop(&mut self) {
        crossterm::terminal::disable_raw_mode().unwrap();
        crossterm::execute!(std::io::stderr(), crossterm::terminal::LeaveAlternateScreen).unwrap();
    }
}