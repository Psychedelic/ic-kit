use ic_types::Principal;

/// A canister ID.
pub struct CanisterId(pub u64);

impl From<CanisterId> for Principal {
    fn from(id: CanisterId) -> Self {
        let mut vec = vec![];
        vec.extend_from_slice(&id.0.to_be_bytes());
        vec.push(1);
        Principal::from_slice(vec.as_slice())
    }
}
