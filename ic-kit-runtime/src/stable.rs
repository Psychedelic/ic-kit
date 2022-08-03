use memmap::MmapMut;

/// A dynamic backend that can be used to handle stable storage. An implementation can decide
/// where to store the data as long as it provides the given functionalities.
pub trait StableMemoryBackend {
    fn stable_read(&mut self, offset: u64, buf: &mut [u8]);
}

/// An stable storage backend that uses a mapped file under the hood to provide the storage space.
struct FileSystemStableMemory {
    file: MmapMut,
}

/// An stable storage backend that stores everything in the heap.
struct HeapStableMemory {
    data: Vec<u8>,
}
