use candid::utils::{ArgumentDecoder, ArgumentEncoder};
use ic_cdk::api::stable::StableMemoryError;

/// Store the given data to the stable storage.
#[inline(always)]
pub fn stable_store<T>(data: T) -> Result<(), candid::Error>
where
    T: ArgumentEncoder,
{
    todo!()
}

/// Restore the data from the stable storage. If the data is not already stored the None value
/// is returned.
#[inline(always)]
pub fn stable_restore<T>() -> Result<T, String>
where
    T: for<'de> ArgumentDecoder<'de>,
{
    todo!()
}

/// Returns the current size of the stable memory in WebAssembly pages.
/// (One WebAssembly page is 64KiB)
#[inline(always)]
pub fn stable_size() -> u32 {
    todo!()
}

/// Tries to grow the memory by new_pages many pages containing zeroes.
/// This system call traps if the previous size of the memory exceeds 2^32 bytes.
/// Errors if the new size of the memory exceeds 2^32 bytes or growing is unsuccessful.
/// Otherwise, it grows the memory and returns the previous size of the memory in pages.
#[inline(always)]
pub fn stable_grow(new_pages: u32) -> Result<u32, StableMemoryError> {
    todo!()
}

/// Writes data to the stable memory location specified by an offset.
#[inline(always)]
pub fn stable_write(offset: u32, buf: &[u8]) {
    todo!()
}

/// Reads data from the stable memory location specified by an offset.
#[inline(always)]
pub fn stable_read(offset: u32, buf: &mut [u8]) {
    todo!()
}

/// Returns a copy of the stable memory.
///
/// This will map the whole memory (even if not all of it has been written to), we don't recommend
/// using such a method as this is an expensive read of the entire stable storage to the heap.
///
/// Only use it with caution.
pub fn stable_bytes() -> Vec<u8> {
    todo!()
}
