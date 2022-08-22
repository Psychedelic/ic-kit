//! A Least-Recently-Used (LRU) cache implementation for the stable storage block access.

use crate::allocator::{BlockAddress, BlockSize};
use crate::checksum::CheckedU40;
use crate::free;
use crate::memory::DefaultMemory;
use crate::utils::read_struct;
use crate::Memory;
use std::collections::hash_map;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
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
    ref_count: HashMap<BlockAddress, usize>,
    /// All of the modified blocks that we need to flush to the stable storage.
    modified: HashSet<BlockAddress>,
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
pub struct LruCacheConfig {
    /// Only keep this many non-flushed blocks in the LRU cache.  
    /// Default: 1_000 WebAssembly pages. (i.e 62.5MB)
    pub modified_capacity: u64,
    /// Total size of the blocks allowed to be contained in this LRU cache.  
    /// Default: 30_000 WebAssembly pages. (i.e 1875MB)
    pub total_capacity: u64,
}

pub(crate) struct BlockEntry {
    address: BlockAddress,
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
            let size = block.size();
            self.size += size;
            Box::leak(Box::new(block))
        });

        unsafe {
            // SAFETY: We just allocated this block so we know it's not null.
            let block = block_ptr.as_mut().unwrap();
            block.prev = ptr::null_mut();
            block.next = self.head;

            if self.tail.is_null() {
                self.tail = block_ptr;
            } else {
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
                self.modified_size += unsafe { entry.as_ref().unwrap().size() };
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
    pub fn free(&mut self, address: BlockAddress) {}

    #[inline]
    fn maybe_flush(&mut self) {
        if self.config.total_capacity < self.size {
            // remove least recently used items.
        }

        if self.config.modified_capacity < self.modified_size {
            // write the modified items to the stable storage.
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

    /// Return the size of this block.
    pub fn size(&self) -> BlockSize {
        unsafe { (&*(self.data as *mut CheckedU40)).unchecked() }
    }

    /// Return the data section of this block, it does not contain the block header.
    pub fn data(&self) -> &[u8] {
        let size = self.size() as usize;
        unsafe { &core::slice::from_raw_parts(self.data as *const _, size)[8..] }
    }

    /// Returns a mutable reference to the data section of this block.
    pub fn data_mut(&mut self) -> &mut [u8] {
        let size = self.size() as usize;
        unsafe { &mut core::slice::from_raw_parts_mut(self.data, size)[8..] }
    }

    /// Free this block and give it back to the allocator.
    pub fn free(mut self) {
        free(self.address + 8);
    }
}

impl Drop for BlockEntry {
    fn drop(&mut self) {
        // Deallocate the allocated data vector.
        let size = self.size() as usize;
        let _ = unsafe { Vec::from_raw_parts(self.data, size, size) };
    }
}

fn load_block<M: Memory>(address: BlockAddress) -> BlockEntry {
    // TODO(qti3e) Handle the error here if the address is not a valid block beginning.
    let address = address - 8;
    let size = read_struct::<M, CheckedU40>(address).verify().expect("X");

    let data = unsafe {
        let mut data = Vec::<u8>::with_capacity(size as usize);
        data.set_len(size as usize);
        M::stable_read(address, data.as_mut_slice());
        data.leak().as_mut_ptr()
    };

    BlockEntry {
        address,
        data,
        next: ptr::null_mut(),
        prev: ptr::null_mut(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::global::*;
    use crate::memory::mock::MockMemory;
    use crate::StableAllocator;

    #[test]
    fn block_entry() {
        set_global_allocator(StableAllocator::new());

        for size in (16..256).step_by(4) {
            let address = allocate(size).unwrap();
            let block = BlockEntry::new(address);
            assert_eq!(block.size(), size + 8);
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
