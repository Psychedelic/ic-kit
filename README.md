# IC Kit

[![Docs](https://docs.rs/ic-kit/badge.svg)](https://docs.rs/ic-kit)

This library provides an alternative to `ic-cdk` that can help developers write canisters
and unit test them in their Rust code.

Blog post: [IC Kit: Rust Canister Testing Kit we Used to Improve Code Coverage at Fleek](https://blog.fleek.co/posts/ickit-rust-canister-testing)

# THIS IS ALPHA SOFTWARE

IC-Kit v0.5 is still alpha and a developer preview. we're working towards making sure it's feature complete, and advise
you to not use it in any sensitive canister before it comes out of the alpha.

## Install

Add this to your `Cargo.toml`

```toml
[dependencies]
ic-kit = "0.5.0-alpha.4"
candid = "0.7"
```

## Example Usage

See [the examples](./examples) directory.

## What's different?

IC-Kit 0.5 is breaking drop-in replacement guarantee of the CDK, which allows us to go one step further in improving
canister development experience.

### Fully Simulated Replica

Now we have a `#[kit_test]` macro which gives you access to a replica simulator that you can use for testing your
canister.

```rust
use ic_kit::prelude::*;

#[kit_test]
async fn test(replica: Replica) {
    // let handle = replica.add_canister(Canister::anonymous());
}
```

### Inspect Message

It makes it easier to use the `inspect_message` feature of the Interest Computer, your function only
needs to return a `bool` and we take care of the rest.

```rust
use ic_kit::prelude::*;

#[inspect_message]
fn inspect_message() -> bool {
    // Only accept ingress message calls which have a payload
    // smaller than 1000 bytes.
    ic_kit::arg_data_size::arg_data_size() <= 1000
}
```

### Secure Memory Helpers

No need to use `thread_local!` to get rid of the `get_mut` anymore, we have deprecated `get[/_mut` method
and now have `with` variation.

```rust
let count = with(|counter: &Counter| {
    *counter.get()
});

let count = with_mut(|counter: &mut Counter| {
    *counter.get()
});
```

### Dependency Injection

Now your sync (non-async) methods can be simplified, we wrap them in the appropriate `with` and `with_mut` methods for you
so you don't have to think about it.

```rust
use ic_kit::prelude::*;
use std::collections::HashMap;

#[derive(Default)]
struct Registry {
    names: HashMap<Principal, String>,
}

#[update]
fn register(registry: &mut Registry, name: String) {
    registry.names.insert(caller(), name);
}

#[query]
fn get_name(registry: &Registry, user: Principal) -> Option<&String> {
    registry.names.get(&user)
}
```

### Native Macros

Now we no longer rely on the `ic-cdk-macros` allowing us to host our version of macros and innovate even more. 

### Importable Canisters

The `KitCanister` derive macro allows you to export a canister to be imported from other canisters.

### Easy Auto Candid Generation

```rust
#[derive(KitCanister)]
#[candid_path("candid.did")]
pub struct NamingSystemCanister;
```
