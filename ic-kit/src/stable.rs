/// Provides utility methods to deal with stable storage on your canister.
// This file is copied from ic_cdk, but changed so that it works with IC-Kit.
use crate::ic::{
    stable_bytes, stable_grow, stable_read, stable_size, stable_write, StableMemoryError,
    StableSize,
};
use candid::utils::{ArgumentDecoder, ArgumentEncoder};
use std::io;

/// A writer to the stable memory.
///
/// Will attempt to grow the memory as it writes,
/// and keep offsets and total capacity.
pub struct StableWriter {
    /// The offset of the next write.
    offset: StableSize,
    /// The capacity, in pages.
    capacity: StableSize,
}

impl Default for StableWriter {
    fn default() -> Self {
        let capacity = stable_size();

        Self {
            offset: 0,
            capacity,
        }
    }
}

impl StableWriter {
    /// Create a new stable writer that writes from the given offset forward.
    pub fn new(offset: StableSize) -> Self {
        StableWriter {
            offset,
            capacity: stable_size(),
        }
    }

    /// Returns the current offset of the writer.
    pub fn offset(&self) -> StableSize {
        self.offset
    }

    /// Attempts to grow the memory by adding new pages.
    pub fn grow(&mut self, added_pages: StableSize) -> Result<(), StableMemoryError> {
        let old_page_count = stable_grow(added_pages)?;
        self.capacity = old_page_count + added_pages;
        Ok(())
    }

    /// Writes a byte slice to the buffer.
    ///
    /// The only condition where this will error out is if it cannot grow the memory.
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, StableMemoryError> {
        if self.offset + (buf.len() as StableSize) > (self.capacity << 16) {
            self.grow((buf.len() >> 16) as StableSize + 1)?;
        }

        stable_write(self.offset, buf);
        self.offset += buf.len() as StableSize;
        Ok(buf.len())
    }
}

impl io::Write for StableWriter {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        self.write(buf)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Out Of Memory"))
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        // Noop.
        Ok(())
    }
}

/// A reader to the stable memory.
///
/// Keeps an offset and reads off stable memory consecutively.
pub struct StableReader {
    /// The offset of the next write.
    offset: StableSize,
}

impl Default for StableReader {
    fn default() -> Self {
        Self { offset: 0 }
    }
}

impl StableReader {
    /// Create a new stable reader that reads from the given offset forward.
    pub fn new(offset: StableSize) -> Self {
        StableReader { offset }
    }

    /// Reads data from the stable memory location specified by an offset.
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, StableMemoryError> {
        stable_read(self.offset, buf);
        self.offset += buf.len() as StableSize;
        Ok(buf.len())
    }
}

impl io::Read for StableReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
        self.read(buf)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Unexpected error."))
    }
}

/// Store the given data to the stable storage.
#[deprecated(
    since = "0.5.0",
    note = "This is a non-performant legacy from IC-CDK for us to deal with."
)]
pub fn stable_store<T>(data: T) -> Result<(), candid::Error>
where
    T: ArgumentEncoder,
{
    candid::write_args(&mut StableWriter::default(), data)
}

/// Restore the data from the stable storage. If the data is not already stored the None value
/// is returned.
#[deprecated(
    since = "0.5.0",
    note = "This is a non-performant legacy from IC-CDK for us to deal with."
)]
pub fn stable_restore<T>() -> Result<T, String>
where
    T: for<'de> ArgumentDecoder<'de>,
{
    let bytes = stable_bytes();
    let mut de =
        candid::de::IDLDeserialize::new(bytes.as_slice()).map_err(|e| format!("{:?}", e))?;
    let res = ArgumentDecoder::decode(&mut de).map_err(|e| format!("{:?}", e))?;
    Ok(res)
}
