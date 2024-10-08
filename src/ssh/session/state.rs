use std::str;

use russh::keys::key::PublicKey;

use crate::{identity::Identity, openid};

#[derive(Debug, strum_macros::AsRefStr)]
pub enum State {
    Unauthenticated,
    KeyOffered(PublicKey),
    CodeSent(openid::DeviceCode, Option<PublicKey>),
    InvalidIdentity(Identity, Option<PublicKey>),
    // Once an authenticated state is reached, the user can really go do
    // whatever they want. For example, a dashboard and port-forwarding can
    // happen. This is intended to be the final state.
    Authenticated(Identity),
}

impl Default for State {
    fn default() -> Self {
        Self::Unauthenticated
    }
}

impl State {
    pub fn key_offered(&mut self, key: &PublicKey) {
        *self = State::KeyOffered(key.clone());
    }

    pub fn code_sent(&mut self, code: &openid::DeviceCode) {
        let key = match self {
            State::KeyOffered(key) => Some(key.clone()),
            State::InvalidIdentity(_, key) => key.clone(),
            _ => None,
        };

        *self = State::CodeSent(code.clone(), key);
    }

    pub fn code_used(&mut self) {
        let State::CodeSent(_, key) = self else {
            *self = State::Unauthenticated;

            return;
        };

        match key {
            Some(key) => {
                *self = State::KeyOffered(key.clone());
            }
            None => {
                *self = State::Unauthenticated;
            }
        }
    }

    pub fn invalid_identity(&mut self, identity: Identity) {
        let key = match self {
            State::KeyOffered(key) => Some(key.clone()),
            _ => None,
        };

        *self = State::InvalidIdentity(identity, key);
    }

    pub fn authenticated(&mut self, identity: Identity) {
        *self = State::Authenticated(identity);
    }
}
