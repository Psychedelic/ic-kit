//! A Least-Recently-Used (LRU) cache implementation for the stable storage block access.

use crate::core::allocator::{BlockAddress, BlockSize};
use crate::core::checksum::CheckedU40;
use crate::core::global::free;
use crate::core::memory::{DefaultMemory, Memory};
use crate::core::utils::read_struct;
use std::collections::hash_map;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::marker::PhantomData;
use std::ptr;

/// An specific LRU cache implementation for keeping stable storage data.
pub struct LruCache<M: Memory = DefaultMemory> {
    /// The configurations for this cache instance.
    config: LruCacheConfig,
    /// Map the address of each block to the LinkedList entry for non-linear lookups.
    map: BTreeMap<BlockAddress, *mut BlockEntry>,
    /// The number of alive references to this address. Any of StableRef or StableRefMut references
    /// are counted here so we do not drop the data in case there is an active reference to it.
    ///
    /// The associated value from an address to reference count must never be zero.
    ref_count: HashMap<BlockAddress, usize>,
    /// All of the modified blocks that we need to flush to the stable storage.
    modified: BTreeSet<BlockAddress>,
    /// Sum of the block size of all the blocks currently in this LRU cache.
    size: u64,
    /// Sum of the block size of all the modified blocks that we need to write back to the stable
    /// storage.
    modified_size: u64,
    /// The most recently accessed block.
    head: *mut BlockEntry,
    /// The least recently accessed block.
    tail: *mut BlockEntry,
    _mem: PhantomData<M>,
}

/// Configuration values for an LRU cache.
#[derive(Copy, Clone)]
pub struct LruCacheConfig {
    /// Only keep this many non-flushed blocks in the LRU cache.  
    /// Default: 1_000 WebAssembly pages. (i.e 62.5MB)
    pub modified_capacity: u64,
    /// Total size of the blocks allowed to be contained in this LRU cache.  
    /// Default: 30_000 WebAssembly pages. (i.e 1875MB)
    pub total_capacity: u64,
}

#[derive(Debug)]
pub(crate) struct BlockEntry {
    address: BlockAddress,
    size: usize,
    data: *mut u8,
    next: *mut BlockEntry,
    prev: *mut BlockEntry,
}

impl<M: Memory> LruCache<M> {
    /// Return a new instance of the LRU-cache
    fn new(config: LruCacheConfig) -> Self {
        Self {
            config,
            map: Default::default(),
            ref_count: HashMap::with_capacity(32),
            modified: Default::default(),
            size: 0,
            modified_size: 0,
            head: ptr::null_mut(),
            tail: ptr::null_mut(),
            _mem: Default::default(),
        }
    }

    /// Load the content of a block at the given address and move it to the head of the LruCache.
    fn load_internal(&mut self, address: BlockAddress) -> *mut BlockEntry {
        let block_ptr = *self.map.entry(address).or_insert_with(|| unsafe {
            let block = BlockEntry::new(address);
            let size = block.size as BlockSize;
            self.size += size;
            Box::leak(Box::new(block))
        });

        unsafe {
            let is_head = self.head == block_ptr;

            // SAFETY: We just allocated this block so we know it's not null.
            let block = block_ptr.as_mut().unwrap();
            block.prev = ptr::null_mut();
            if !is_head {
                block.next = self.head;
            }

            if self.tail.is_null() {
                self.tail = block_ptr;
            } else if !is_head {
                // SAFETY: If the tail is not null, that means neither is the head.
                self.head.as_mut().unwrap().prev = block_ptr;
            }

            self.head = block_ptr;
        }

        block_ptr
    }

    /// Return the data at the given address.
    pub fn get(&mut self, address: BlockAddress) -> *mut u8 {
        unsafe {
            self.load_internal(address)
                .as_ref()
                .unwrap()
                .data()
                .as_ptr() as *mut u8
        }
    }

    /// Mark the block at the given address as modified so we know to flush it to the stable storage.
    pub fn mark_modified(&mut self, address: BlockAddress) {
        if let Some(&entry) = self.map.get(&address) {
            if self.modified.insert(address) {
                self.modified_size += unsafe { entry.as_ref().unwrap().size } as BlockSize;
                self.maybe_flush();
            }
        }
    }

    /// Increment the reference count for a block, so we don't accidentally drop it.
    pub fn pin(&mut self, address: BlockAddress) {
        *self.ref_count.entry(address).or_default() += 1;
    }

    /// Decrement the reference count of a block address, so we can free it.
    pub fn unpin(&mut self, address: BlockAddress) {
        if let hash_map::Entry::Occupied(mut o) = self.ref_count.entry(address) {
            if *o.get() == 0 {
                o.remove();
            } else {
                *o.get_mut() -= 1;
            }
        }
    }

    /// Free the given block address.
    pub fn free(&mut self, address: BlockAddress) {
        // 1. Remove it from the linked list.
    }

    /// Forcefully clear the LRU cache, write all of the data to the stable storage and clear
    /// the cache.
    pub fn clear(&mut self) {
        let config = self.config;

        self.config = LruCacheConfig {
            modified_capacity: 0,
            total_capacity: 0,
        };

        self.write_modified();
        self.drop_least_recently_used();

        self.config = config;

        assert_eq!(self.modified_size, 0);
    }

    #[inline]
    fn maybe_flush(&mut self) {
        if self.config.total_capacity < self.size {
            self.drop_least_recently_used();
        }

        if self.config.modified_capacity < self.modified_size {
            self.write_modified();
        }
    }

    fn drop_least_recently_used(&mut self) {
        let mut curr = self.tail;

        while !curr.is_null() && self.config.total_capacity < self.size {
            let curr_mut = unsafe { &mut *curr };

            // skip it.
            if self.ref_count.get(&curr_mut.address).is_some() {
                curr = curr_mut.prev;
                continue;
            }

            let size = curr_mut.size as u64;

            // write the data if it's modified.
            if self.modified.remove(&curr_mut.address) {
                let buf = curr_mut.data();
                M::stable_write(curr_mut.address, buf);
                self.modified_size -= size;
            }

            // Remove the block in the map.
            self.map.remove(&curr_mut.address);
            self.size -= size;

            if !curr_mut.prev.is_null() {
                let prev_mut = unsafe { &mut *(*curr).prev };
                prev_mut.next = curr_mut.next;
            }

            if !curr_mut.next.is_null() {
                let next_mut = unsafe { &mut *(*curr).next };
                next_mut.prev = curr_mut.prev;
            }

            curr_mut.prev = ptr::null_mut();
            curr_mut.next = ptr::null_mut();
            let tmp = curr_mut.prev;

            // Drop the current node.
            unsafe {
                let _ = Box::from_raw(curr);
            }

            curr = tmp;
        }

        self.tail = curr;
        if self.tail.is_null() {
            self.head = curr;
        }
    }

    fn write_modified(&mut self) {
        let mut flushed = Vec::new();

        for addr in self.modified.iter() {
            if self.config.modified_capacity >= self.modified_size {
                break;
            }

            let entry = unsafe { &(**self.map.get(addr).unwrap()) };
            let buf = entry.data();
            let size = entry.size as u64;

            M::stable_write(entry.address, buf);
            self.modified_size -= size;

            flushed.push(*addr);
        }

        for addr in flushed {
            self.modified.remove(&addr);
        }
    }
}

impl Default for LruCache {
    fn default() -> Self {
        Self::new(LruCacheConfig::default())
    }
}

impl Default for LruCacheConfig {
    #[inline]
    fn default() -> Self {
        Self {
            modified_capacity: 1_000 << 16,
            total_capacity: 30_000 << 16,
        }
    }
}

impl BlockEntry {
    /// Create a new BlockEntry by loading the content from the given stable storage address.
    pub fn new(address: BlockAddress) -> Self {
        load_block::<DefaultMemory>(address)
    }

    /// Return the data section of this block, it does not contain the block header.
    pub fn data(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.data as *const _, self.size) }
    }

    /// Returns a mutable reference to the data section of this block.
    pub fn data_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.data, self.size) }
    }

    /// Free this block and give it back to the allocator.
    pub fn free(mut self) {
        free(self.address);
    }
}

impl Drop for BlockEntry {
    fn drop(&mut self) {
        // Deallocate the allocated data vector.
        let _ = unsafe { Vec::from_raw_parts(self.data, self.size, self.size) };
    }
}

fn load_block<M: Memory>(address: BlockAddress) -> BlockEntry {
    // TODO(qti3e) Handle the error here if the address is not a valid block beginning.

    let size = read_struct::<M, CheckedU40>(address - 8)
        .verify()
        .expect("X");
    let size = (size - 8) as usize;

    let data = unsafe {
        let mut data = Vec::<u8>::with_capacity(size);
        data.set_len(size);
        M::stable_read(address, data.as_mut_slice());
        data.leak().as_mut_ptr()
    };

    BlockEntry {
        address,
        size,
        data,
        next: ptr::null_mut(),
        prev: ptr::null_mut(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::allocator::StableAllocator;
    use crate::core::global::*;
    use crate::core::memory::mock::MockMemory;

    #[test]
    fn block_entry() {
        set_global_allocator(StableAllocator::new());

        for size in (16..256).step_by(4) {
            let address = allocate(size).unwrap();
            let block = BlockEntry::new(address);
            assert_eq!(block.size as BlockSize, size);
            assert_eq!(address, 8);
            block.free();
        }
    }

    #[test]
    fn block_entry_data() {
        set_global_allocator(StableAllocator::new());
        let content = b"Hello Dfinity World!";
        let address = allocate(content.len() as BlockSize).unwrap();
        MockMemory::stable_write(address, content.as_slice());
        let block = BlockEntry::new(address);
        assert_eq!(block.data(), content);
    }
}
