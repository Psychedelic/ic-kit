use ic_kit::prelude::*;
use ic_kit_example_counter::CounterCanister;

#[update]
async fn deploy_counter() -> Principal {
    // TODO(qti3e) we should make it possible to deploy an instance of CounterCanister.
    // It should work in the IC and the ic-kit-runtime environment.
    todo!()
}
