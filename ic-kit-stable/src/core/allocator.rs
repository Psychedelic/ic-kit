use crate::core::checksum::CheckedU40;
use crate::core::hole::HoleList;
use crate::core::memory::{DefaultMemory, IcMemory, Memory};
use crate::core::utils::read_struct;
use ic_kit::stable::StableMemoryError;

/// An address to a block.
pub type BlockAddress = u64;

/// Size of a block.
pub type BlockSize = u64;

/// The internal minimum allocation size (includes size header)
/// size : u64 = 8 bytes
/// next : u64 = 8 bytes
/// If the node is used then next is overwritten by content.
pub const MIN_ALLOCATION_SIZE: BlockSize = 16;

// TODO(qti3e) next steps:
// write the HoleList root to stable storage at the first block.
// load the HoleList from stable storage if present.

/// An allocator over the stable storage. This allocator assumes that it owns the entire stable
/// storage if there are already data in the stable storage.
pub struct StableAllocator<M: Memory = DefaultMemory> {
    hole_list: HoleList<M>,
}

impl<M: Memory> StableAllocator<M> {
    pub fn new() -> Self {
        Self {
            hole_list: HoleList::new(),
        }
    }

    /// Allocate a stable storage block with the given size.
    pub fn allocate(&mut self, size: BlockSize) -> Result<BlockAddress, StableMemoryError> {
        // we need 8 more bytes to store the CheckedU40 for the block size.
        let size = size + 8;

        if let Some((addr, _)) = self.hole_list.find(size) {
            // skip the block's size which is inserted into the first 8 bytes of the block.
            return Ok(addr + 8);
        }

        // number of pages we need to grow in order to fit this size. this is a ceiling division.
        // by 1 WebAssembly page.
        let new_pages = (size + (1 << 16) - 1) >> 16;
        let start = M::stable_grow(new_pages);

        // we couldn't allocate anymore.
        if start == -1 {
            return Err(StableMemoryError::OutOfMemory);
        }

        let addr = (start as u64) << 16;
        self.hole_list.insert(addr, new_pages << 16);

        let addr = self
            .hole_list
            .find(size)
            .expect("unreachable allocation condition.")
            .0;

        Ok(addr + 8)
    }

    /// Free the stable storage block at the given address. The address must be an address returned
    /// by a previous invocation to the [`allocate`] method.
    pub fn free(&mut self, addr: BlockAddress) {
        if addr < 8 {
            return;
        }

        let addr = addr - 8;

        // guard the api misuse by checking the checksum.
        if let Some(size) = read_struct::<M, CheckedU40>(addr).verify() {
            self.hole_list.insert(addr, size);
        } else {
            #[cfg(test)]
            panic!("Invalid pointer passed to free().")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic]
    fn free_misuse() {
        let mut allocator = StableAllocator::<DefaultMemory>::new();
        assert_eq!(allocator.allocate(100), Ok(8));
        assert_eq!(allocator.allocate(100), Ok(116));
        allocator.free(100);
    }

    #[test]
    fn allocate_after_free() {
        let mut allocator = StableAllocator::<DefaultMemory>::new();
        assert_eq!(allocator.allocate(100), Ok(8));
        assert_eq!(allocator.allocate(100), Ok(116));
        allocator.free(8);
        assert_eq!(allocator.allocate(100), Ok(8));
    }
}
