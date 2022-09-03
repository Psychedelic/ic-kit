# IC Kit Examples

This directory contains some example canister's implemented using the IC Kit. Each example tries
to be as simple as possible to demonstrate one aspect of the IC Kit and a possible design pattern
you can also use to develop canisters.

# Testing the examples

```shell
cd ./fib
cargo test
```

# Build example

```shell
cd ./fib
cargo build --target wasm32-unknown-unknown --release --features kit-wasm-export
```

> Without `kit-wasm-export` the canister method will not be part of the public interface of the generated WASM. In simple
> terms, the code will compile but the methods won't be there!

# About Cargo.toml

Since the canister's developed by IC-Kit are intended to be imported as normal Rust crates, we don't want to generate
the wasm bindings by default, so we should explicitly specify the `kit-wasm-export` feature when building the crate as a
canister.

This prevents any unintentional name collisions, which can result in catastrophic events if you're building a canister.
For example if you depend on the interface of a `Counter` if that crate implements a `increment` method, now your
canister will also have the `increment` method, resulting in unintended use cases.

```toml
[features]
kit-wasm-export = []

[lib]
crate-type = ["cdylib", "lib"]
```

The `cdylib` is used, so we can target for `wasm32-unknown-unknown`, the `lib` is used to make it possible to import the
canister as a crate.
