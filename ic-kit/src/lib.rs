pub use handler::*;
pub use interface::*;
pub use mock::*;
pub use setup::*;

mod handler;
mod inject;
mod interface;
mod mock;
mod setup;
#[cfg(target_family = "wasm")]
mod wasm;

/// A set of mock principal IDs useful for testing.
#[cfg(not(target_family = "wasm"))]
#[deprecated(since = "0.4.7", note = "mock_principals is deprecated.")]
pub mod mock_principals {
    use crate::Principal;

    #[inline]
    pub fn alice() -> Principal {
        Principal::from_text("sgymv-uiaaa-aaaaa-aaaia-cai").unwrap()
    }

    #[inline]
    pub fn bob() -> Principal {
        Principal::from_text("ai7t5-aibaq-aaaaa-aaaaa-c").unwrap()
    }

    #[inline]
    pub fn john() -> Principal {
        Principal::from_text("hozae-racaq-aaaaa-aaaaa-c").unwrap()
    }

    #[inline]
    pub fn xtc() -> Principal {
        Principal::from_text("aanaa-xaaaa-aaaah-aaeiq-cai").unwrap()
    }
}

/// Return the IC context depending on the build target.
#[inline(always)]
#[deprecated(note = "get_context is deprecated use ic_kit::ic::*")]
pub fn get_context() -> &'static impl Context {
    #[cfg(not(target_family = "wasm"))]
    return inject::get_context();
    #[cfg(target_family = "wasm")]
    return wasm::IcContext::context();
}

/// APIs/Methods to work with the Internet Computer.
pub mod ic;
/// The type definition of common canisters on the Internet Computer.
pub mod interfaces;
/// The APIs for StableReader/StableWriter.
pub mod stable;
/// Internal storage abstraction for singletons.
pub mod storage;

/// async_std::test to be used for async tests when not targeting WASM.
#[cfg(not(target_family = "wasm"))]
pub use async_std::test as async_test;
pub use ic_cdk::api::call::{CallResult, RejectionCode};
pub use ic_cdk::export::candid;
pub use ic_cdk::export::Principal;
pub use ic_kit_macros as macros;

/// ic_cdk APIs to be used with ic-kit-macros only, please don't use this directly
/// we may decide to change it anytime and break compatability.
pub mod ic_call_api_v0_ {
    pub use ic_cdk::api::call::arg_data;
    pub use ic_cdk::api::call::reject;
    pub use ic_cdk::api::call::reply;
}
