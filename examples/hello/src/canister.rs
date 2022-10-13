use ic_kit::prelude::*;

#[query]
fn hello() -> String {
    "Hello, World!".to_string()
}

// When the http feature is enabled, the `http_request` function is generated, with a single index route.
// This index route returns the balance of the canister in cycles, in JSON.

#[derive(KitCanister)]
#[candid_path("candid.did")]
struct HelloCanister;
