use ic_kit_sys::ic0;
use std::error;
use std::fmt;

/// A type which represents either a page count or an offset in the stable storage, it's a u64
/// when the `experimental-stable64` feature is enabled, otherwise a u32.
#[cfg(feature = "experimental-stable64")]
pub type StableSize = u64;

/// A type which represents either a page count or an offset in the stable storage, it's a u64
/// when the `experimental-stable64` feature is enabled, otherwise a u32.
#[cfg(not(feature = "experimental-stable64"))]
pub type StableSize = u32;

/// A possible error value when dealing with stable memory.
#[derive(Debug)]
pub enum StableMemoryError {
    /// No more stable memory could be allocated.
    OutOfMemory,
    /// Attempted to read more stable memory than had been allocated.
    OutOfBounds,
}

impl fmt::Display for StableMemoryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::OutOfMemory => f.write_str("Out of memory"),
            Self::OutOfBounds => f.write_str("Read exceeds allocated memory"),
        }
    }
}

impl error::Error for StableMemoryError {}

/// Returns the current size of the stable memory in WebAssembly pages.
/// Note: One WebAssembly page is 64KiB
#[inline(always)]
pub fn stable_size() -> StableSize {
    #[cfg(not(feature = "experimental-stable64"))]
    unsafe {
        ic0::stable_size() as u32
    }

    #[cfg(feature = "experimental-stable64")]
    unsafe {
        ic0::stable64_size() as u64
    }
}

/// Attempts to grow the stable memory by `new_pages` (added pages).
///
/// Returns an error if it wasn't possible. Otherwise, returns the previous
/// size that was reserved.
///
/// Note: One WebAssembly page is 64KiB
#[inline(always)]
pub fn stable_grow(new_pages: StableSize) -> Result<StableSize, StableMemoryError> {
    #[cfg(not(feature = "experimental-stable64"))]
    unsafe {
        match ic0::stable_grow(new_pages as i32) {
            -1 => Err(StableMemoryError::OutOfMemory),
            x => Ok(x as u32),
        }
    }

    #[cfg(feature = "experimental-stable64")]
    unsafe {
        match if new_pages < (u32::MAX as u64 - 1) {
            ic0::stable_grow(new_pages as i32) as i64
        } else {
            ic0::stable64_grow(new_pages as i64) as i64
        } {
            -1 => Err(StableMemoryError::OutOfMemory),
            x => Ok(x as u64),
        }
    }
}

/// Writes data to the stable memory location specified by an offset.
#[inline(always)]
pub fn stable_write(offset: StableSize, buf: &[u8]) {
    #[cfg(not(feature = "experimental-stable64"))]
    unsafe {
        ic0::stable_write(offset as i32, buf.as_ptr() as isize, buf.len() as isize)
    }

    #[cfg(feature = "experimental-stable64")]
    unsafe {
        if offset < (u32::MAX as u64 - 1) {
            ic0::stable_write(offset as i32, buf.as_ptr() as isize, buf.len() as isize)
        } else {
            ic0::stable64_write(offset as i64, buf.as_ptr() as i64, buf.len() as i64)
        }
    }
}

/// Reads data from the stable memory location specified by an offset.
#[inline(always)]
pub fn stable_read(offset: StableSize, buf: &mut [u8]) {
    #[cfg(not(feature = "experimental-stable64"))]
    unsafe {
        ic0::stable_read(buf.as_ptr() as isize, offset as i32, buf.len() as isize);
    };

    #[cfg(feature = "experimental-stable64")]
    unsafe {
        if offset < (u32::MAX as u64 - 1) {
            ic0::stable_read(buf.as_ptr() as isize, offset as i32, buf.len() as isize);
        } else {
            ic0::stable64_read(buf.as_ptr() as i64, offset as i64, buf.len() as i64);
        }
    }
}

pub(crate) fn stable_bytes() -> Vec<u8> {
    let size = (stable_size() as usize) << 16;
    let mut vec = Vec::with_capacity(size);
    unsafe {
        ic0::stable_read(vec.as_ptr() as isize, 0, size as isize);
        vec.set_len(size);
    }
    vec
}
