pub type BlockAddress = u64;
pub type BlockSize = u64;

/// The internal minimum allocation size (includes size header)
/// size : u64 = 8 bytes
/// next : u64 = 8 bytes
/// If the node is used then next is overwritten by content.
pub const MIN_ALLOCATION_SIZE: BlockSize = 16;

mod allocator;
mod checksum;
mod hole;

pub use allocator::StableAllocator;
