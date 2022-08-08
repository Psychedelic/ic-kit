use ic_kit::candid::encode_one;
use ic_kit::ic::{print, trap};
use ic_kit::prelude::*;
use ic_kit::utils::{reject, reply};

#[derive(Default)]
pub struct Counter {
    number: u64,
}

impl Counter {
    /// Increment the counter by one.
    pub fn increment(&mut self) -> u64 {
        self.number += 1;
        self.number
    }

    /// Increment the counter by the provided value.
    pub fn increment_by(&mut self, n: u8) -> u64 {
        self.number += n as u64;
        self.number
    }
}

#[inspect_message]
pub fn inspect_message() -> bool {
    if ic_kit::utils::arg_data_size() > 5000 {
        return false;
    }

    true
}

#[update]
pub fn increment() -> u64 {
    ic::with_mut(Counter::increment)
}

#[export_name = "canister_update test"]
fn test_methods() {
    use ic_kit_sys::ic0;

    spawn(async {
        print("spawn1: call");
        let msg = CallBuilder::new(id(), "increment").perform_raw().await;
        print("spawn1: reply");
        print(format!("{:?}", msg));

        // reply(&encode_one("Reply from s1").unwrap());
        reject("Reject message");
        // trap("X");
    });

    spawn(async {
        print("spawn2: call");
        let msg = CallBuilder::new(id(), "increment").perform_raw().await;
        print("spawn2: reply");
        print(format!("{:?}", msg));

        reply(&encode_one("Reply from s2").unwrap());
        // trap("J");
    });
}
