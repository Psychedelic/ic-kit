mod futures;
mod setup;

/// System APIs for the Internet Computer.
pub mod ic;

/// Easy to use and secure methods to manage the canister's global state.
pub mod storage;

/// Helper methods around the stable storage.
pub mod stable;

/// Internal utility methods to deal with reading data.
pub mod utils;

// re-exports.
pub use candid::{self, CandidType, Principal};
pub use ic_kit_macros as macros;
pub use setup::setup_hooks;

/// The IC-kit runtime, which can be used for testing the canister in non-wasm environments.
#[cfg(not(target_family = "wasm"))]
pub use ic_kit_runtime as rt;
