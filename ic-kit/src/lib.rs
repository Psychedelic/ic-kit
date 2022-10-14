#![warn(missing_docs)]
#![doc = include_str ! ("../../README.md")]

// re-exports.
pub use candid::{self, CandidType, Nat, Principal};

// The KitCanister derive macro.
pub use canister::KitCanister;
#[cfg(feature = "http")]
pub use ic_kit_http as http;
pub use ic_kit_macros as macros;
/// The IC-kit runtime, which can be used for testing the canister in non-wasm environments.
#[cfg(not(target_family = "wasm"))]
pub use ic_kit_runtime as rt;
pub use macros::KitCanister;
pub use setup::setup_hooks;

mod canister;
mod futures;
mod setup;
mod storage;

/// System APIs for the Internet Computer.
pub mod ic;

/// Helper methods around the stable storage.
pub mod stable;

/// Internal utility methods to deal with reading data.
pub mod utils;

/// The famous prelude module which re exports the most useful methods.
pub mod prelude {
    pub use serde::{Deserialize, Serialize};

    pub use super::candid::{CandidType, Nat, Principal};
    pub use super::canister::KitCanister;
    /// Enabled with the `http` feature. This re-exports the http module and macros
    #[cfg(feature = "http")]
    pub use super::http::*;
    pub use super::ic::{
        self, balance, caller, id, maybe_with, maybe_with_mut, spawn, swap, take, with, with_mut,
        CallBuilder, Cycles, StableSize,
    };
    pub use super::macros::*;
    #[cfg(not(target_family = "wasm"))]
    pub use super::rt::{self, prelude::*};
}
