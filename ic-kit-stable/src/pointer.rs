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
    pub fn as_ref(&self) -> Option<StableRef<T>> {
        with_lru(|lru| {});

        todo!()
    }

    /// Returns a mutable reference to the data.
    pub fn as_ref_mut(&self) -> Option<StableRefMut<T>> {
        todo!()
    }
}

/// An immutable reference to
pub struct StableRef<'b, T> {
    data: *mut T,
    ptr: &'b StablePtr<T>,
}

pub struct StableRefMut<'b, T> {
    data: *mut T,
    ptr: &'b StablePtr<T>,
}

impl<T> Deref for StableRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        todo!()
    }
}

impl<T> Deref for StableRefMut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        todo!()
    }
}

impl<T> DerefMut for StableRefMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        todo!()
    }
}

impl<T> Drop for StableRef<T> {
    fn drop(&mut self) {
        todo!()
    }
}

impl<T> Drop for StableRefMut<T> {
    fn drop(&mut self) {
        todo!()
    }
}
