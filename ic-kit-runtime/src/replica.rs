use crate::canister::Canister;
use crate::canister_id::CanisterId;
use ic_types::Principal;

/// A local replica that contains one or several canisters.
pub struct Replica {
    canisters: Vec<Canister>,
}

#[test]
fn use_replica() {
    let id = CanisterId(1);
    let replica = Replica {
        canisters: vec![Canister::new(id)],
    };

    // from replica pov every method is sync.
    replica.upgrade(id, "method", ("hello", "x"));
}
