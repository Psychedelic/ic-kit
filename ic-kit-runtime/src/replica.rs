use crate::canister::Canister;

use actix::prelude::*;

/// A local replica that contains one or several canisters.
#[derive(Default)]
pub struct Replica {
    canisters: Vec<Canister>,
}

impl Actor for Replica {
    type Context = Context<Self>;

    fn stopping(&mut self, _ctx: &mut Self::Context) -> Running {
        Running::Continue
    }
}

impl Replica {
    pub fn send() {}
}
