//! This crate provides the bindings to the Internet Computer runtime that can work in wasm
//! environments, and also provides ability to mock this methods for non-wasm envs, this is
//! part of the Psychedelic's Canister Development kit, [IC-Kit](https://github.com/psychedelic/ic-kit).
//!
//! This is a low level crate, and we don't recommend you to use this directly, and encourage you
//! to look at ic-kit itself.

/// System APIs exposed by the Internet Computer's WASM runtime.
pub mod ic0;
