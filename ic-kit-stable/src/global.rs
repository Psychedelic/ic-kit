use crate::allocator::{BlockAddress, BlockSize};
use crate::lru::LruCache;
use crate::StableAllocator;
use ic_kit::stable::StableMemoryError;
use std::borrow::BorrowMut;
use std::cell::RefCell;

thread_local! {
    static ALLOCATOR: RefCell<Option<StableAllocator>> = RefCell::new(None);
    static LRU: RefCell<Option<LruCache>> = RefCell::new(None);
}

/// Set the stable storage allocator instance to be used for the canister.
///
/// # Panics
///
/// If called more than once throughout the canister's lifetime.
pub fn set_global_allocator(allocator: StableAllocator) {
    ALLOCATOR.with(|cell| {
        let mut option = cell.borrow_mut();

        if option.is_some() {
            panic!("set_global_allocator is only supposed to be called once.");
        }

        option.replace(allocator);
    });
}

/// Set a custom LRU cache for the canister.
///
/// # Panics
///
/// This method must only be called once during the initialization of the canister, either during
/// `init` or `post_upgrade`.
pub fn set_global_lru(lru: LruCache) {
    LRU.with(|cell| {
        let mut option = cell.borrow_mut();

        if option.is_some() {
            panic!("set_global_lru is only supposed to be called once during initialization.");
        }

        option.replace(lru);
    });
}

/// Allocate a block with the given size from the global stable storage allocator.
pub fn allocate(size: BlockSize) -> Result<BlockAddress, StableMemoryError> {
    ALLOCATOR.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .expect("ic_kit_stable::set_global_allocator must have been called.")
            .allocate(size)
    })
}

/// Free the block at the given address, the address must be the one you've retrieved earlier using
/// a call to [`allocate`].
pub fn free(address: BlockSize) {
    ALLOCATOR.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .expect("ic_kit_stable::set_global_allocator must have been called.")
            .free(address)
    })
}

/// Use the LRU cache instance to the callback.
#[inline]
pub(crate) fn with_lru<U, F: FnOnce(&mut LruCache) -> U>(f: F) -> U {
    LRU.with(|cell| {
        let mut lru = cell.borrow_mut();
        let lru_mut = lru.get_or_insert_with(|| LruCache::default());
        f(lru_mut)
    })
}
