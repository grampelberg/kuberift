use std::collections::HashMap;

use eyre::Result;
use ratatui::{layout::Rect, Frame};

use super::{
    table::{Content, Table},
    Widget,
};
use crate::{
    events::{Broadcast, Event},
    resources,
    widget::TableRow,
};

pub struct Tunnel {
    items: HashMap<resources::Tunnel, resources::Tunnel>,
    table: Table,
}

impl Default for Tunnel {
    fn default() -> Self {
        Self {
            items: HashMap::new(),
            table: Table::default().title("Tunnels").no_highlight(),
        }
    }
}

impl Tunnel {
    pub fn height(&self) -> u16 {
        u16::try_from(self.items.len())
            .expect("no truncation")
            .saturating_add(2)
    }
}

impl Widget for Tunnel {
    fn dispatch(&mut self, event: &Event) -> Result<Broadcast> {
        Ok(match event {
            Event::Tunnel(Err(err)) => {
                let tun = err.tunnel.clone().into_error();

                self.items.insert(tun.clone(), tun);

                Broadcast::Ignored
            }
            Event::Tunnel(Ok(ev)) => {
                self.items.insert(ev.clone(), ev.clone());

                Broadcast::Consumed
            }
            _ => Broadcast::Ignored,
        })
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        self.table
            .draw::<HashMap<resources::Tunnel, resources::Tunnel>, resources::Tunnel>(
                frame,
                area,
                &self.items,
            )
    }
}

impl<'a, K> Content<'a, K> for HashMap<resources::Tunnel, resources::Tunnel>
where
    K: TableRow<'a>,
{
    fn items(&self, _: Option<String>) -> Vec<impl TableRow<'a>> {
        self.iter().map(|(_, v)| v.clone()).collect()
    }
}