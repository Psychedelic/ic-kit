mod setup;

mod futures;
/// APIs/Methods to work with the Internet Computer.
pub mod ic;
/// The APIs for StableReader/StableWriter.
// pub mod stable;
/// Internal storage abstraction for singletons.
pub mod storage;

pub use candid::{self, CandidType, Principal};
pub use ic_cdk::api::call::{CallResult, RejectionCode};
pub use ic_kit_macros as macros;
pub use ic_kit_runtime as rt;
pub use setup::*;

/// ic_cdk APIs to be used with ic-kit-macros only, please don't use this directly
/// we may decide to change it anytime and break compatability.
pub mod ic_call_api_v0_ {
    pub use ic_cdk::api::call::arg_data;
    pub use ic_cdk::api::call::reject;
    pub use ic_cdk::api::call::reply;
}

pub use ic_cdk::api::stable::StableMemoryError;
