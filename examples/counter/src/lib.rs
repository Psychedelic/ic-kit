use ic_kit::ic;
use ic_kit::macros::{inspect_message, update};

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
