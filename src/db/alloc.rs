use crate::ic::{stable_grow, StableWriter};
use crate::StableMemoryError;
use futures::executor::BlockingStream;
use serde::{Deserialize, Serialize};

/// A memory allocator that works with stable storage's space, there
/// should only be one Allocator in a canister.
pub struct Allocator {
    blocks: BlockList,
}

/// A compact list of block ranges.
#[derive(Default, Serialize, Deserialize)]
struct BlockList {
    list: Vec<Block>,
}

#[derive(Debug, PartialOrd, PartialEq, Serialize, Deserialize)]
pub struct Block {
    offset: usize,
    size: usize,
}

impl Allocator {
    pub fn alloc(&mut self, size: usize) -> Result<Block, StableMemoryError> {
        if let Some(block) = self.blocks.allocate(size) {
            return Ok(block);
        }

        let new_pages = (size >> 16) as u32 + 1;
        let old_pages = stable_grow(new_pages)? as usize;
        let offset = old_pages << 16;
        let new_pages_size = (new_pages << 16) as usize;
        self.blocks.insert(Block::new(offset, new_pages_size));

        self.alloc(size)
    }

    pub fn free(&mut self, block: Block) {
        self.blocks.insert(block);
    }
}

impl Block {
    pub fn new(offset: usize, size: usize) -> Self {
        Self { offset, size }
    }

    pub fn write(&mut self, data: &[u8]) -> Result<usize, StableMemoryError> {
        let mut writer = StableWriter::new(self.offset);
        let to_write = if data.len() > self.size {
            &data[0..self.size]
        } else {
            data
        };
        writer.write(to_write)
    }
}

impl BlockList {
    pub fn insert(&mut self, block: Block) {
        if self.list.is_empty() {
            self.list.push(block);
            return;
        }

        let (mut block, right) = match self.list.binary_search_by(|b| b.offset.cmp(&block.offset)) {
            Ok(index) if block.size <= self.list[index].size => {
                return;
            }
            Ok(index) => {
                self.list[index].size = block.size;
                let right = self.list.split_off(index + 1);
                let block = self.list.pop().unwrap();
                (block, right)
            }
            Err(index) => {
                let right = self.list.split_off(index);
                (block, right)
            }
        };

        let offset = block.offset;
        let end = offset + block.size;
        while self.list.len() > 0 {
            let b = &self.list[self.list.len() - 1];
            let e = b.offset + b.size;

            if offset <= e {
                block = self.list.pop().unwrap();
                block.size = end.max(e) - block.offset;
            } else {
                break;
            }
        }

        self.list.push(block);

        let index = self.list.len() - 1;
        let block = &self.list[index];
        let offset = block.offset;
        let end = offset + block.size;

        for block in right {
            if block.offset <= end {
                let b_end = block.offset + block.size;
                self.list[index].size = end.max(b_end) - offset;
                continue;
            }

            self.list.push(block);
        }
    }

    /// Tries to find and allocate a block with the given size, returns `None` if there is not
    /// enough room.
    pub fn allocate(&mut self, size: usize) -> Option<Block> {
        let mut result = None;
        let mut maybe_remove_index = None;

        for (i, block) in self.list.iter_mut().enumerate() {
            if block.size >= size {
                result = Some(Block::new(block.offset, size));
                block.offset += size;
                block.size -= size;

                if block.size == 0 {
                    maybe_remove_index = Some(i);
                }

                break;
            }
        }

        if let Some(index) = maybe_remove_index {
            self.list.remove(index);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_list_insert() {
        let mut list = BlockList::default();
        list.insert(Block::new(0, 5));
        list.insert(Block::new(7, 2));
        list.insert(Block::new(3, 1));
        assert_eq!(list.list, vec![Block::new(0, 5), Block::new(7, 2)]);

        let mut list = BlockList::default();
        list.insert(Block::new(0, 3));
        list.insert(Block::new(7, 2));
        list.insert(Block::new(3, 2));
        assert_eq!(list.list, vec![Block::new(0, 5), Block::new(7, 2)]);

        let mut list = BlockList::default();
        list.insert(Block::new(0, 3));
        list.insert(Block::new(7, 2));
        list.insert(Block::new(2, 2));
        assert_eq!(list.list, vec![Block::new(0, 4), Block::new(7, 2)]);

        let mut list = BlockList::default();
        list.insert(Block::new(0, 3));
        list.insert(Block::new(7, 2));
        list.insert(Block::new(2, 5));
        assert_eq!(list.list, vec![Block::new(0, 9),]);

        let mut list = BlockList::default();
        list.insert(Block::new(0, 3));
        list.insert(Block::new(7, 2));
        list.insert(Block::new(0, 5));
        assert_eq!(list.list, vec![Block::new(0, 5), Block::new(7, 2)]);

        let mut list = BlockList::default();
        list.insert(Block::new(0, 3));
        list.insert(Block::new(7, 2));
        list.insert(Block::new(0, 7));
        assert_eq!(list.list, vec![Block::new(0, 9)]);

        let mut list = BlockList::default();
        list.insert(Block::new(0, 3));
        list.insert(Block::new(7, 2));
        list.insert(Block::new(0, 2));
        assert_eq!(list.list, vec![Block::new(0, 3), Block::new(7, 2)]);

        let mut list = BlockList::default();
        list.insert(Block::new(0, 1));
        list.insert(Block::new(1, 1));
        assert_eq!(list.list, vec![Block::new(0, 2),]);

        let mut list = BlockList::default();
        list.insert(Block::new(0, 1));
        list.insert(Block::new(2, 1));
        assert_eq!(list.list, vec![Block::new(0, 1), Block::new(2, 1),]);

        let mut list = BlockList::default();
        list.insert(Block::new(0, 1));
        list.insert(Block::new(2, 1));
        list.insert(Block::new(0, 2));
        assert_eq!(list.list, vec![Block::new(0, 3),]);
    }

    #[test]
    fn block_list_allocate() {
        let mut list = BlockList::default();
        list.insert(Block::new(0, 3));
        list.insert(Block::new(7, 2));
        assert_eq!(list.allocate(2), Some(Block::new(0, 2)));
        assert_eq!(list.list, vec![Block::new(2, 1), Block::new(7, 2)]);
        assert_eq!(list.allocate(2), Some(Block::new(7, 2)));
        assert_eq!(list.list, vec![Block::new(2, 1)]);
        assert_eq!(list.allocate(2), None);
        assert_eq!(list.list, vec![Block::new(2, 1)]);
        assert_eq!(list.allocate(1), Some(Block::new(2, 1)));
        assert_eq!(list.list, vec![]);
    }
}
