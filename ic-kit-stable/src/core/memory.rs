/// The memory interface. temp remove this once ic-kit-runtime has stable* support.
pub trait Memory {
    fn stable_size() -> u64;
    fn stable_grow(new_pages: u64) -> i64;
    fn stable_read(offset: u64, buf: &mut [u8]);
    fn stable_write(offset: u64, buf: &[u8]);
}

#[cfg(not(target_family = "wasm"))]
pub mod mock {
    use super::Memory;
    use ic_kit::rt::stable::{HeapStableMemory, StableMemoryBackend};
    use std::cell::RefCell;

    thread_local! {
        static MEMORY: RefCell<HeapStableMemory> = RefCell::new(HeapStableMemory::default());
    }

    // A memory interface that uses ic-kit's HeapStableMemory.
    pub struct MockMemory;

    impl Memory for MockMemory {
        fn stable_size() -> u64 {
            MEMORY.with(|c| c.borrow_mut().stable_size())
        }

        fn stable_grow(new_pages: u64) -> i64 {
            MEMORY.with(|c| c.borrow_mut().stable_grow(new_pages))
        }

        fn stable_read(offset: u64, buf: &mut [u8]) {
            MEMORY.with(|c| c.borrow_mut().stable_read(offset, buf))
        }

        fn stable_write(offset: u64, buf: &[u8]) {
            MEMORY.with(|c| c.borrow_mut().stable_write(offset, buf))
        }
    }
}

/// A memory backend using the IC.
pub struct IcMemory;

impl Memory for IcMemory {
    fn stable_size() -> u64 {
        ic_kit::ic::stable_size() as u64
    }

    fn stable_grow(new_pages: u64) -> i64 {
        match ic_kit::ic::stable_grow(new_pages as ic_kit::ic::StableSize) {
            Ok(s) => s as i64,
            Err(_) => -1,
        }
    }

    fn stable_read(offset: u64, buf: &mut [u8]) {
        ic_kit::ic::stable_read(offset as ic_kit::ic::StableSize, buf)
    }

    fn stable_write(offset: u64, buf: &[u8]) {
        ic_kit::ic::stable_write(offset as ic_kit::ic::StableSize, buf)
    }
}

#[cfg(test)]
pub type DefaultMemory = mock::MockMemory;

#[cfg(not(test))]
pub type DefaultMemory = IcMemory;
