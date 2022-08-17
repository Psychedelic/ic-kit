pub type BlockAddress = u64;

pub type BlockSize = u32;

/// The internal minimum allocation size (includes size header)
/// size : u32 = 4 bytes
/// next : u64 = 8 bytes
/// If the node is used then next is overwritten by content.
pub const MIN_ALLOCATION_SIZE: BlockSize = 12;

mod hole;
