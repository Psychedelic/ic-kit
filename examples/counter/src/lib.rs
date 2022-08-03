use ic_kit::macros::{query, update};
use ic_kit::{ic, Canister};

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

#[update]
pub fn increment() -> u64 {
    ic::with_mut(Counter::increment)
}

#[test]
fn test() {
    let rt = ic_kit::rt::Canister::new(vec![0]).with_method::<increment>();
}
