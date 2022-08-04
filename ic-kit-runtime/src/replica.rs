use crate::canister::Canister;
use crate::types::CanisterId;
use actix::prelude::*;

/// A local replica that contains one or several canisters.
#[derive(Default)]
pub struct Replica {
    canisters: Vec<Canister>,
}

impl Actor for Replica {
    type Context = Context<Self>;

    fn stopping(&mut self, ctx: &mut Self::Context) -> Running {
        Running::Continue
    }
}

impl Replica {
    pub fn send() {}
}

#[test]
fn sample() {
    System::new().block_on(|| {
        let replica = Replica::start_default();
        let canister = Canister::new(CanisterId::from_u64(1));
    });
}
