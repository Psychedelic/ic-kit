//! A simple example of a counter service that can proxy an increment call to a list of counter
//! canister.

pub mod canister;
pub use canister::FactoryCounterCanister;
