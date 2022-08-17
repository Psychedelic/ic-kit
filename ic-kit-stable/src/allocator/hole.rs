use super::{BlockAddress, BlockSize, MIN_ALLOCATION_SIZE};
use crate::memory::Memory;
use crate::utils::write_struct;
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::ptr::NonNull;

// used for testing if holes are properly dropped or not.
#[cfg(test)]
thread_local! {
    static ACTIVE_HOLE: std::cell::RefCell<usize> = std::cell::RefCell::new(0);
}

pub type Delta = BlockSize;

/// A data structure to keep a list of memory holes that uses a combination of power-of-two linked
/// lists and uses best-fit/worst-fit lookup through the linked lists to find a free hole, it is also
/// capable of merging freed holes to form larger holes and prevent fragmentation.
pub struct HoleList<M: Memory> {
    // assert(map[A].address = A)
    map: BTreeMap<BlockAddress, NonNull<Hole>>,
    // the largest empty hole can be 2^(36 + 4) bytes = 1TB.
    // assert(ceil(log2(roots[i].size) == i)
    roots: [Option<NonNull<Hole>>; 36],
    // minimum S such that:
    //      for all `i >= S` -> roots[i] == Null
    roots_right_boundary: usize,
    // maximum S such that:
    //      for all `i < S` -> roots[i] == Null
    // assert(roots_left_boundary == 36 || roots[roots_left_boundary].is_some())
    roots_left_boundary: usize,
    _memory: PhantomData<M>,
}

// On heap memory allocators this usually is stored within the hole itself, but we're doing this
// for a secondary storage.
#[derive(Debug)]
struct Hole {
    size: BlockSize,
    address: BlockAddress,
    prev: Option<NonNull<Hole>>,
    next: Option<NonNull<Hole>>,
}

#[repr(packed)]
struct HoleHeader {
    size: BlockSize,
    next: BlockAddress,
}

impl<M: Memory> Default for HoleList<M> {
    fn default() -> Self {
        HoleList {
            map: BTreeMap::new(),
            roots: [None; 36],
            roots_right_boundary: 0,
            roots_left_boundary: 36,
            _memory: PhantomData::default(),
        }
    }
}

impl<M: Memory> HoleList<M> {
    /// Create a new empty [`HoleList`]
    pub fn new() -> Self {
        Self::default()
    }

    /// Find and return a block that can
    pub fn find(&mut self, size: BlockSize) -> Option<(BlockAddress, BlockSize)> {
        let size = size.max(MIN_ALLOCATION_SIZE);
        let mut i = get_log2_index(size).max(self.roots_left_boundary);

        // Best case  = O(n)
        // Worst case = O(n + m)
        //      n: the number of holes in first root (self.roots[i])
        //      m: the number of holes in second non-empty root.
        let (addr, delta) = loop {
            if i >= self.roots_right_boundary {
                break None;
            }

            if let Some((addr, delta)) = self.iter(i).find(size) {
                break Some((addr, delta));
            }

            i += 1;
        }?;

        // We found a hole big enough for this data so let's remove it from the hole list.
        unsafe {
            self.remove_hole(addr);
        }

        // If the delta can form a valuable hole put it back to use.
        if delta >= MIN_ALLOCATION_SIZE {
            // We already know this hole does not have a neighbour so use `raw_insert` instead of
            // the `insert` method.
            self.raw_insert(addr + size, delta, false);
            Some((addr, size))
        } else {
            // we are using the extra bytes for this allocation hence +delta in size.
            Some((addr, size + delta))
        }
    }

    /// Insert the given hole to this list without attempting to merge with neighbouring nodes. Only
    /// use this method when you are SURE the block does not have a neighbour, for example when
    /// attempting to form a HoleList from a serialization.
    pub fn raw_insert(&mut self, addr: BlockAddress, size: BlockSize, skip_write: bool) {
        // Find out which linked list we have to insert this hole to based on its size.
        let index = get_log2_index(size);

        // Perform the linked list insertion.
        let maybe_next = self.roots[index].clone();

        let hole = Hole {
            size,
            address: addr,
            prev: None,
            next: maybe_next,
        };

        let header = hole.to_header();
        let hole = NonNull::<Hole>::from(Box::leak(Box::new(hole)));

        if !skip_write {
            write_struct::<M, HoleHeader>(addr, &header);
        }

        #[cfg(test)]
        ACTIVE_HOLE.with(|c| {
            *c.borrow_mut() += 1;
        });

        if let Some(mut next) = maybe_next {
            unsafe {
                next.as_mut().prev = Some(hole);
            }
        }

        self.map.insert(addr, hole.clone());
        self.roots[index] = Some(hole);

        if index >= self.roots_right_boundary {
            self.roots_right_boundary = index + 1;
        }

        if index < self.roots_left_boundary {
            self.roots_left_boundary = index;
        }
    }

    /// Insert a new hole to the list at the given address and size. This method merges the hole
    /// with the possible empty hole before and after this hole to resolve fragmentation and form
    /// a larger hole.
    pub fn insert(&mut self, addr: BlockAddress, size: BlockSize) {
        let next_block = self.get_next_block(addr, size);
        let previous_block = self.get_previous_block(addr);

        let mut new_hole_addr = addr;
        let mut new_hole_size = size;

        if let Some((next_addr, next_hole)) = next_block {
            unsafe {
                new_hole_size += next_hole.as_ref().size;
                self.remove_hole(next_addr);
            }
        }

        if let Some((prev_addr, prev_hole)) = previous_block {
            unsafe {
                new_hole_size += prev_hole.as_ref().size;
                new_hole_addr = prev_addr;
                self.remove_hole(prev_addr);
            }
        }

        self.raw_insert(new_hole_addr, new_hole_size, false);
    }

    /// Remove the hole at the given block address.
    ///
    /// # Panics
    ///
    /// If there is no hole starting at the provided offset.
    unsafe fn remove_hole(&mut self, addr: BlockAddress) {
        let mut hole = self.map.remove(&addr).unwrap();
        let hole_mut = hole.as_mut();

        if hole_mut.is_root() {
            let index = get_log2_index(hole_mut.size);
            self.roots[index] = hole_mut.next.clone();

            if self.roots_right_boundary == index + 1 {
                while self.roots_right_boundary > 0
                    && self.roots[self.roots_right_boundary - 1].is_none()
                {
                    self.roots_right_boundary -= 1;
                }
            }

            if self.roots_left_boundary == index {
                while self.roots_left_boundary < 36
                    && self.roots[self.roots_left_boundary].is_none()
                {
                    self.roots_left_boundary += 1;
                }
            }
        }

        hole_mut.remove_from_linked_list();

        // Drop the hole.
        let _ = Box::from_raw(hole.as_ptr());
    }

    /// Return the immediate hole right before the provided address, this method only returns the
    /// previous hole if there is no gap between it and the provided address.
    fn get_previous_block(&self, addr: BlockAddress) -> Option<(BlockAddress, NonNull<Hole>)> {
        let (b_addr, hole) = self.map.range(..addr).last()?;
        if b_addr + unsafe { hole.as_ref().size } == addr {
            Some((*b_addr, hole.clone()))
        } else {
            None
        }
    }

    /// Just like `get_previous_block` but returns the immediate block right after the provided
    /// address and size.
    fn get_next_block(
        &self,
        addr: BlockAddress,
        size: BlockSize,
    ) -> Option<(BlockAddress, NonNull<Hole>)> {
        let (b_addr, hole) = self.map.range(addr..).next()?;
        if *b_addr == addr + size {
            Some((*b_addr, hole.clone()))
        } else {
            None
        }
    }

    /// Return an iterator over the holes at the given level.
    #[inline]
    fn iter(&self, level: usize) -> HoleIterator {
        HoleIterator::new(self.roots[level].clone())
    }
}

impl Hole {
    /// Returns true if the hole is the index hole in a linked list.
    pub fn is_root(&self) -> bool {
        self.prev.is_none()
    }

    /// Remove this hole from the linked list.
    pub fn remove_from_linked_list(&mut self) {
        let next = self.next.clone();
        let prev = self.prev.clone();

        if let Some(mut prev) = self.prev {
            unsafe { prev.as_mut().next = next };
        }

        if let Some(mut next) = self.next {
            unsafe { next.as_mut().prev = prev };
        }

        self.next = None;
        self.prev = None;
    }

    /// Return a stable storage header for this hole.
    pub fn to_header(&self) -> HoleHeader {
        HoleHeader {
            size: self.size,
            next: match self.next {
                Some(x) => unsafe { x.as_ref().address },
                None => 0,
            },
        }
    }
}

impl<M: Memory> Drop for HoleList<M> {
    fn drop(&mut self) {
        for (_, hole) in self.map.iter() {
            unsafe {
                let _ = Box::from_raw(hole.as_ptr());
            }
        }
    }
}

#[cfg(test)]
impl Drop for Hole {
    fn drop(&mut self) {
        ACTIVE_HOLE.with(|c| {
            *c.borrow_mut() -= 1;
        })
    }
}

struct HoleIterator {
    head: Option<NonNull<Hole>>,
}

impl HoleIterator {
    /// Create a new hole iterator with the given head.
    pub fn new(head: Option<NonNull<Hole>>) -> Self {
        Self { head }
    }

    /// Tries to find a hole with size larger than or equal to the provided size, address of the
    /// block along side the value of delta is returned.
    fn find(self, size: BlockSize) -> Option<(BlockAddress, Delta)> {
        if self.head.is_none() {
            return None;
        }

        // Do a worst-fit and best-fit search in parallel, and then consider:
        // if the best-fit is a prefect match (best_fit_delta == 0) use it (no fragmentation)
        // if the worst_fit_delta < MIN_ALLOCATION_SIZE: use best fit to minimize wasted gap.
        // otherwise use worst_fit.

        let mut worst_fit_delta = 0;
        let mut worst_fit_addr: Option<BlockAddress> = None;

        let mut best_fit_delta = BlockSize::MAX;
        let mut best_fit_addr: Option<BlockAddress> = None;

        for (addr, b_size) in self {
            if b_size < size {
                continue;
            }

            let delta = b_size - size;

            if delta > worst_fit_delta {
                worst_fit_delta = delta;
                worst_fit_addr = Some(addr);
            }

            if delta < best_fit_delta {
                best_fit_delta = delta;
                best_fit_addr = Some(addr);
            }
        }

        if best_fit_delta == 0 {
            return Some((best_fit_addr.unwrap(), 0));
        }

        if worst_fit_delta < MIN_ALLOCATION_SIZE {
            return Some((best_fit_addr?, best_fit_delta));
        }

        Some((worst_fit_addr?, worst_fit_delta))
    }
}

impl Iterator for HoleIterator {
    type Item = (BlockAddress, BlockSize);

    fn next(&mut self) -> Option<Self::Item> {
        let head = unsafe { self.head?.as_ref() };
        self.head = head.next.clone();
        Some((head.address, head.size))
    }
}

const fn ceiling_log2(mut x: u64) -> usize {
    let t = [
        0xFFFFFFFF00000000,
        0x00000000FFFF0000,
        0x000000000000FF00,
        0x00000000000000F0,
        0x000000000000000C,
        0x0000000000000002,
    ];

    let mut y = if x & (x - 1) == 0 { 0 } else { 1 };
    let mut j = 32;
    let mut i = 0;

    while i < 6 {
        let k = if (x & t[i]) == 0 { 0 } else { j };
        y += k;
        x >>= k;
        j >>= 1;
        i += 1;
    }

    y
}

fn get_log2_index(size: BlockSize) -> usize {
    const OFFSET: usize = ceiling_log2(MIN_ALLOCATION_SIZE);
    ceiling_log2(size) - OFFSET
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::mock::MockMemory;

    /// return the number of active holes in the current thread.
    fn holes() -> usize {
        ACTIVE_HOLE.with(|c| c.borrow().clone())
    }

    #[test]
    fn test_ceiling_log2() {
        assert_eq!(ceiling_log2(1), 0);
        assert_eq!(ceiling_log2(2), 1);
        assert_eq!(ceiling_log2(3), 2);
        assert_eq!(ceiling_log2(4), 2);
        assert_eq!(ceiling_log2(7), 3);
        assert_eq!(ceiling_log2(8), 3);
        assert_eq!(ceiling_log2(15), 4);
        assert_eq!(ceiling_log2(32), 5);
        assert_eq!(ceiling_log2(33), 6);
        assert_eq!(ceiling_log2(63), 6);
        assert_eq!(ceiling_log2(64), 6);
        assert_eq!(ceiling_log2(65), 7);
        assert_eq!(ceiling_log2(u32::MAX as u64), 32);
    }

    #[test]
    fn test_get_log2_index() {
        assert_eq!(get_log2_index(MIN_ALLOCATION_SIZE), 0);
        assert_eq!(get_log2_index(MIN_ALLOCATION_SIZE << 1), 1);
        assert_eq!(get_log2_index(MIN_ALLOCATION_SIZE << 2), 2);
        assert_eq!(get_log2_index(MIN_ALLOCATION_SIZE << 3), 3);
        assert_eq!(get_log2_index(MIN_ALLOCATION_SIZE << 4), 4);
    }

    #[test]
    fn hole_list_find_simple() {
        MockMemory::stable_grow(1);

        let mut list = HoleList::<MockMemory>::new();
        list.insert(0, 116);
        assert_eq!(list.find(20), Some((0, 20)));
        assert_eq!(list.find(20), Some((20, 20)));
        assert_eq!(list.find(20), Some((40, 20)));
        assert_eq!(list.find(20), Some((60, 20)));
        assert_eq!(list.find(20), Some((80, 20)));
        assert_eq!(list.find(20), None);
        assert_eq!(list.find(16), Some((100, 16)));
    }

    #[test]
    fn hole_list_find_small_size() {
        MockMemory::stable_grow(1);

        let mut list = HoleList::<MockMemory>::new();
        list.insert(0, 116);
        assert_eq!(holes(), 1);

        assert_eq!(list.find(100), Some((0, 100)));
        assert_eq!(holes(), 1);

        assert_eq!(list.find(20), None);
        assert_eq!(list.find(16), Some((100, 16)));
        assert_eq!(holes(), 0);

        let mut list = HoleList::<MockMemory>::new();
        list.insert(0, 117);
        assert_eq!(holes(), 1);
        assert_eq!(list.find(100), Some((0, 100)));
        assert_eq!(holes(), 1);
        assert_eq!(list.find(20), None);
        assert_eq!(holes(), 1);
        assert_eq!(list.find(16), Some((100, 17)));
        assert_eq!(holes(), 0);
    }

    #[test]
    fn hole_list_merge_prev() {
        MockMemory::stable_grow(1);

        {
            let mut list = HoleList::<MockMemory>::new();
            list.insert(0, 100);
            assert_eq!(holes(), 1);
            list.insert(100, 70);
            assert_eq!(holes(), 1);
            assert_eq!(list.find(150), Some((0, 150)));
            assert_eq!(holes(), 1);
        }

        assert_eq!(holes(), 0);
    }

    #[test]
    fn hole_list_merge_next() {
        MockMemory::stable_grow(1);

        let mut list = HoleList::<MockMemory>::new();
        list.insert(100, 70);
        list.insert(0, 100);
        assert_eq!(list.find(150), Some((0, 150)));
        assert_eq!(holes(), 1);
    }

    #[test]
    fn hole_list_merge_mid() {
        MockMemory::stable_grow(1);

        let mut list = HoleList::<MockMemory>::new();
        list.insert(0, 70);
        list.insert(100, 70);
        assert_eq!(list.find(150), None);
        list.insert(70, 30);
        assert_eq!(list.find(150), Some((0, 150)));
    }

    #[test]
    fn hole_root_boundary() {
        MockMemory::stable_grow(1);

        let mut list = HoleList::<MockMemory>::new();
        assert_eq!(list.roots_right_boundary, 0);
        assert_eq!(list.roots_left_boundary, 36);

        list.insert(0, 16);
        assert_eq!(list.roots_right_boundary, 1);
        assert_eq!(list.roots_left_boundary, 0);

        list.insert(100, 32);
        assert_eq!(list.roots_right_boundary, 2);
        assert_eq!(list.roots_left_boundary, 0);

        list.insert(1000, 128);
        assert_eq!(list.roots_right_boundary, 4);
        assert_eq!(list.roots_left_boundary, 0);

        assert_eq!(holes(), 3);

        assert_eq!(list.find(128), Some((1000, 128)));
        assert_eq!(list.roots_right_boundary, 2);
        assert_eq!(list.roots_left_boundary, 0);

        assert_eq!(list.find(16), Some((0, 16)));
        assert_eq!(list.find(10), Some((100, 16))); // 10 byte should return 16 bytes.
        assert_eq!(list.find(16), Some((116, 16)));
        assert_eq!(list.find(16), None);
        assert_eq!(list.roots_left_boundary, 36);

        assert_eq!(holes(), 0);
    }

    #[test]
    fn hole_list_right_boundary() {
        MockMemory::stable_grow(1);

        let mut list = HoleList::<MockMemory>::new();
        assert_eq!(list.roots_left_boundary, 36);

        list.insert(0, 128);
        assert_eq!(list.roots_left_boundary, 3);

        list.insert(200, 32);
        assert_eq!(list.roots_left_boundary, 1);

        list.insert(300, 16);
        assert_eq!(list.roots_left_boundary, 0);

        assert_eq!(list.find(16), Some((300, 16)));
        assert_eq!(list.roots_left_boundary, 1);

        assert_eq!(list.find(16), Some((200, 16)));
        assert_eq!(list.roots_left_boundary, 0);

        assert_eq!(list.find(16), Some((216, 16)));
        assert_eq!(list.roots_left_boundary, 3);

        assert_eq!(list.find(64), Some((0, 64)));
        assert_eq!(list.roots_left_boundary, 2);

        assert_eq!(list.find(32), Some((64, 32)));
        assert_eq!(list.roots_left_boundary, 1);

        assert_eq!(list.find(32), Some((96, 32)));
        assert_eq!(list.roots_left_boundary, 36);

        assert_eq!(holes(), 0);
    }
}
