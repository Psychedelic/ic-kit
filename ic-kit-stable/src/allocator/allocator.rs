use crate::allocator::hole::HoleList;
use crate::allocator::{BlockAddress, BlockSize};
use crate::memory::Memory;

pub struct StableAllocator<M: Memory> {
    hole_list: HoleList<M>,
}

impl<M: Memory> StableAllocator<M> {
    pub fn allocate(&mut self, size: BlockSize) -> Option<BlockAddress> {
        let x = self.hole_list.find(size as BlockSize);
        todo!()
    }

    pub fn free(&mut self, addr: BlockAddress) {
        // let header_addr = addr - 8;
        // self.hole_list.insert(addr);
    }
}
