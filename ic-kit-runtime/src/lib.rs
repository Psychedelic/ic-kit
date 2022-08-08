pub mod canister;
pub mod replica;
pub mod stable;
pub mod types;

pub use canister::CanisterMethod;

#[cfg(target_family = "wasm")]
compile_error!("IC-Kit runtime does not support builds for WASM.");
