#[cfg(target_family = "wasm")]
mod ic;
mod inject;
mod interface;
mod mock;

#[cfg(target_family = "wasm")]
pub use ic::*;
pub use interface::*;
pub use mock::*;

pub use ic_cdk::export::candid;
pub use ic_cdk::export::Principal;

pub mod macros {
    /// Re-export async_std test to be used for async tests when not targeting WASM.
    #[cfg(not(target_family = "wasm"))]
    pub use async_std::test;

    /// Re-export ic_cdk_macros.
    pub use ic_cdk_macros::*;
}

/// The type definition of common canisters on the Internet Computer.
pub mod interfaces;

/// Return the IC context depending on the build target.
#[inline(always)]
pub fn get_context() -> &'static mut impl Context {
    #[cfg(not(target_family = "wasm"))]
    return inject::get_context();
    #[cfg(target_family = "wasm")]
    return IcContext::context();
}
