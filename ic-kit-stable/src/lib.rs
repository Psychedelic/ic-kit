mod allocator;
mod checksum;
mod global;
mod hole;
mod lru;
mod memory;
mod pointer;
mod utils;

use crate::memory::DefaultMemory;

// Re-export anything from the global methods.
pub use global::*;

pub use allocator::StableAllocator;
pub use memory::Memory;

pub type LruCache = lru::LruCache<DefaultMemory>;
