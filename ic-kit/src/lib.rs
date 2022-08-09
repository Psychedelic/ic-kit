mod futures;
mod setup;
mod storage;

/// System APIs for the Internet Computer.
pub mod ic;

/// Helper methods around the stable storage.
pub mod stable;

/// Internal utility methods to deal with reading data.
pub mod utils;

// re-exports.
pub use candid::{self, CandidType, Nat, Principal};
pub use ic_kit_macros as macros;
pub use setup::setup_hooks;

/// The IC-kit runtime, which can be used for testing the canister in non-wasm environments.
#[cfg(not(target_family = "wasm"))]
pub use ic_kit_runtime as rt;

/// The famous prelude module which re exports the most useful methods.
pub mod prelude {
    pub use super::ic;
    pub use super::ic::CallBuilder;
    pub use super::ic::{balance, caller, id, spawn};
    pub use super::ic::{maybe_with, maybe_with_mut, swap, take, with, with_mut};
    pub use super::ic::{Cycles, StableSize};
    pub use candid::{CandidType, Nat, Principal};
    pub use serde::{Deserialize, Serialize};

    pub use ic_kit_macros::*;

    #[cfg(not(target_family = "wasm"))]
    pub use ic_kit_runtime as rt;

    #[cfg(not(target_family = "wasm"))]
    pub use ic_kit_runtime::prelude::*;
}
