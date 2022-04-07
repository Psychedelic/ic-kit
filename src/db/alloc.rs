use futures::executor::BlockingStream;
use serde::{Deserialize, Serialize};

struct Allocator {}

/// A compact list of block ranges.
#[derive(Default, Serialize, Deserialize)]
struct BlockList {
    list: Vec<Block>,
}

#[derive(Debug, Copy, Clone, PartialOrd, PartialEq, Serialize, Deserialize)]
pub struct Block {
    offset: usize,
    size: usize,
}

impl Default for Allocator {
    fn default() -> Self {
        Allocator {}
    }
}

impl Allocator {
    pub fn alloc(&mut self, size: usize) -> Block {
        // 1. first search in free_blocks
        // 2. then try to find a block
        todo!()
    }

    pub fn free() {}
}

impl Block {
    pub fn new(offset: usize, size: usize) -> Self {
        Self { offset, size }
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
            let b = self.list[self.list.len() - 1];
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
        let block = self.list[index];
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_list() {
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
}
