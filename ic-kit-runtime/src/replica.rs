use crate::canister::Canister;



/// A local replica that contains one or several canisters.
pub struct Replica {
    canisters: Vec<Canister>,
}

#[test]
fn use_replica() {
    let id = CanisterId(1);
    let _replica = Replica {
        canisters: vec![Canister::new(id)],
    };
}
