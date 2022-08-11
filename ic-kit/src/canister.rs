use crate::Principal;

/// A canister.
pub trait KitCanister {
    /// Create a new instance of this canister using the provided canister id.
    #[cfg(not(target_family = "wasm"))]
    fn build(canister_id: Principal) -> ic_kit_runtime::Canister;

    /// Create a new instance of this canister with the anonymous principal id.
    #[cfg(not(target_family = "wasm"))]
    fn anonymous() -> ic_kit_runtime::Canister {
        Self::build(Principal::anonymous())
    }

    /// The candid description of the canister.
    fn candid() -> String;
}
