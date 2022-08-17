use super::{BlockAddress, BlockSize, MIN_ALLOCATION_SIZE};
use std::collections::BTreeMap;
use std::ptr::NonNull;

pub type Delta = BlockSize;

pub struct HoleList {
    // assert(map[A].address = A)
    map: BTreeMap<BlockAddress, NonNull<Hole>>,
    // assert(ceil(log2(roots[i].size) == i)
    roots: [Option<NonNull<Hole>>; 33],
    // minimum S such that:
    //      for all `i >= S` -> roots[i] == Null
    max_root: usize,
}

#[derive(Debug)]
struct Hole {
    size: BlockSize,
    address: BlockAddress,
    prev: Option<NonNull<Hole>>,
    next: Option<NonNull<Hole>>,
}

impl HoleList {
    pub fn find(&mut self, size: BlockSize) -> Option<(BlockAddress, BlockSize)> {
        let mut i = get_log2_index(size);

        let (addr, delta) = loop {
            if i >= self.max_root {
                break None;
            }

            if let Some((addr, delta)) = self.iter(i).find(size) {
                break Some((addr, delta));
            }
        }?;

        unsafe {
            self.remove_hole(addr);
        }

        if delta >= MIN_ALLOCATION_SIZE {}

        None
    }

    /// Insert the given hole to this list without attempting to merge with neighbouring nodes.
    pub fn raw_insert(&mut self, addr: BlockAddress, size: BlockSize) {
        // Find out which linked list we have to insert this hole to based on its size.
        let level = get_log2_index(size);

        // Perform the linked list insertion.
        let maybe_next = self.roots[level].clone();

        let hole = NonNull::<Hole>::from(Box::leak(Box::new(Hole {
            size,
            address: addr,
            prev: None,
            next: maybe_next,
        })));

        if let Some(mut next) = maybe_next {
            unsafe {
                next.as_mut().prev = Some(hole);
            }
        }

        self.roots[level] = Some(hole);
        if level > self.max_root {
            self.max_root = level;
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

        self.raw_insert(new_hole_addr, new_hole_size);
    }

    /// Remove the hole at the given block address.
    ///
    /// # Panics
    ///
    /// If there is no hole starting at the provided offset.
    unsafe fn remove_hole(&mut self, addr: BlockAddress) {
        let mut hole = self.map.remove(&addr).unwrap();
        let hole_mut = hole.as_mut();

        if dbg!(hole_mut.is_root()) {
            let root = get_log2_index(hole_mut.size);
            self.roots[root] = hole_mut.next.clone();
        }

        hole_mut.remove_from_linked_list();

        // Drop the hole.
        let _ = dbg!(Box::from_raw(hole.as_ptr()));
    }

    /// Return the immediate hole right before the provided address, this method only returns the
    /// previous hole if there is no gap between it and the provided address.
    fn get_previous_block(&self, addr: BlockAddress) -> Option<(BlockAddress, NonNull<Hole>)> {
        let (b_addr, hole) = self.map.range(..addr).last()?;
        if b_addr + (unsafe { hole.as_ref().size as u64 }) == addr {
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
        if *b_addr == addr + (size as u64) {
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
    /// Returns true if the hole is the root hole in a linked list.
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
    // we could have used a compact implementation using loops but rust const doesn't support loops.
    let mut y = if x & (x - 1) == 0 { 0 } else { 1 };

    if (x & 0xFFFFFFFF00000000) > 0 {
        y += 32;
        x >>= 32;
    }

    if (x & 0x00000000FFFF0000) > 0 {
        y += 16;
        x >>= 16;
    }

    if (x & 0x000000000000FF00) > 0 {
        y += 8;
        x >>= 8;
    }

    if (x & 0x00000000000000F0) > 0 {
        y += 4;
        x >>= 4;
    }

    if (x & 0x000000000000000C) > 0 {
        y += 2;
        x >>= 2;
    }

    if (x & 0x0000000000000002) > 0 {
        y += 1;
        x >>= 1;
    }

    y
}

fn get_log2_index(size: BlockSize) -> usize {
    const OFFSET: usize = ceiling_log2(MIN_ALLOCATION_SIZE as u64);
    ceiling_log2(size as u64) - OFFSET
}

mod tests {
    use super::*;

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
}
