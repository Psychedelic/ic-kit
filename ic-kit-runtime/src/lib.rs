// We normally wouldn't have to do this, but since most of ic-kit users will build for wasm, we
// should handle this and print a nice compiler error to not confuse the users with 177 errors
// printed on their screen.
cfg_if::cfg_if! {
    if #[cfg(target_family = "wasm")] {
        compile_error!("IC-Kit runtime does not support builds for WASM.");
    } else {
        pub mod canister;
        pub mod replica;
        pub mod stable;
        pub mod types;

        pub use canister::CanisterMethod;
        pub use tokio::sync::oneshot;
        pub use tokio::spawn;
    }
}
