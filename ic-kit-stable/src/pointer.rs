use crate::allocator::{BlockAddress, BlockSize};
use crate::lru::{BlockEntry, LruCache};
use crate::{allocate, with_lru};
use ic_kit::stable::StableMemoryError;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

/// A smart pointer for data that lives on the stable storage, this uses the LRU layer to GC the
/// data from heap and prevent multiple reads of the same block by keeping the most recently used
/// addresses in the heap.
#[derive(Copy, Clone)]
#[repr(packed)]
pub struct StablePtr<T>(BlockAddress, PhantomData<T>);

impl<T> StablePtr<T>
where
    T: Copy,
{
    /// Allocate space for the given data on the stable storage and return a stable pointer.
    pub fn new(data: T) -> Result<Self, StableMemoryError> {
        let addr = allocate(std::mem::size_of::<T>() as BlockSize)?;
        todo!()
    }

    /// Create a new pointer at the given address.
    pub fn from_address(address: BlockAddress) -> Self {
        StablePtr(address, PhantomData::default())
    }

    /// Creates an returns a null reference.
    pub fn null() -> Self {
        Self::from_address(BlockAddress::MAX)
    }

    /// Returns true if the pointer is not null.
    pub fn is_null(&self) -> bool {
        self.0 == BlockAddress::MAX
    }

    /// Returns an immutable reference to the data.
    pub unsafe fn as_ref(&self) -> Option<StableRef<T>> {
        if self.is_null() {
            None
        } else {
            let data = with_lru(|lru| {
                lru.pin(self.0);
                lru.get(self.0)
            });

            Some(StableRef {
                data: unsafe { data as *mut T },
                ptr: &self,
            })
        }
    }

    /// Returns a mutable reference to the data. Calling this method marks the block as modified.
    pub unsafe fn as_mut(&self) -> Option<StableRefMut<T>> {
        if self.is_null() {
            None
        } else {
            let data = with_lru(|lru| {
                lru.pin(self.0);
                let data = lru.get(self.0);
                lru.mark_modified(self.0);
                data
            });

            Some(StableRefMut {
                data: unsafe { data as *mut T },
                ptr: &self,
            })
        }
    }
}

/// An immutable reference to the data at the given block.
pub struct StableRef<'b, T> {
    data: *mut T,
    ptr: &'b StablePtr<T>,
}

/// A mutable reference to the data at the given block.
pub struct StableRefMut<'b, T> {
    data: *mut T,
    ptr: &'b StablePtr<T>,
}

impl<T> Deref for StableRef<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.data.as_ref().unwrap() }
    }
}

impl<T> Deref for StableRefMut<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.data.as_ref().unwrap() }
    }
}

impl<T> DerefMut for StableRefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.data.as_mut().unwrap() }
    }
}

impl<T> Drop for StableRef<'_, T> {
    fn drop(&mut self) {
        with_lru(|lru| {
            lru.unpin(self.ptr.0);
        });
    }
}

impl<T> Drop for StableRefMut<'_, T> {
    fn drop(&mut self) {
        with_lru(|lru| {
            lru.unpin(self.ptr.0);
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::allocator::BlockSize;
    use crate::pointer::StablePtr;
    use crate::utils::write_struct;
    use crate::{allocate, set_global_allocator, DefaultMemory, StableAllocator};

    #[derive(Copy, Clone)]
    struct Counter {
        count: u128,
    }

    #[test]
    fn test_ref() {
        let counter = Counter {
            count: 0xaabbccddeeff,
        };

        // Setup the env.
        set_global_allocator(StableAllocator::new());

        // Allocate storage and write the initial version of counter to the stable storage.
        let addr = allocate(std::mem::size_of::<Counter>() as BlockSize).unwrap();
        write_struct::<DefaultMemory, Counter>(addr, &counter);

        // Create a pointer from the address.
        let ptr = StablePtr::<Counter>::from_address(addr);

        let counter_ref = unsafe { ptr.as_ref().unwrap() };
        println!("Count = 0x{:x}", counter_ref.count);

        let mut mut_ref = unsafe { ptr.as_mut().unwrap() };
        mut_ref.count += 1;

        println!("Count = 0x{:x}", mut_ref.count);
    }
}
