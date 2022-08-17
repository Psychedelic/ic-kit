use memmap::MmapMut;

/// A dynamic backend that can be used to handle stable storage. An implementation can decide
/// where to store the data as long as it provides the given functionalities.
pub trait StableMemoryBackend {
    fn stable_size(&mut self) -> u64;
    fn stable_grow(&mut self, new_pages: u64) -> i64;
    fn stable_read(&mut self, offset: u64, buf: &mut [u8]);
    fn stable_write(&mut self, offset: u64, buf: &[u8]);
}

/// An stable storage backend that uses a mapped file under the hood to provide the storage space.
pub struct FileSystemStableMemory {
    _file: MmapMut,
}

/// An stable storage backend that stores everything in the heap. By default it has a 128MB limit.
pub struct HeapStableMemory {
    pages: Vec<[u8; 1 << 16]>,
    max_pages: u64,
}

impl Default for HeapStableMemory {
    fn default() -> Self {
        Self {
            pages: Vec::new(),
            max_pages: 128 << 20 >> 16,
        }
    }
}

impl HeapStableMemory {
    /// Create a stable storage backend with the provided max page.
    pub fn new(max_pages: u64) -> Self {
        Self {
            pages: Vec::new(),
            max_pages,
        }
    }
}

impl StableMemoryBackend for HeapStableMemory {
    fn stable_size(&mut self) -> u64 {
        self.pages.len() as u64
    }

    fn stable_grow(&mut self, new_pages: u64) -> i64 {
        let size = self.pages.len() as u64;
        if new_pages + size > self.max_pages {
            -1
        } else {
            for _ in 0..new_pages {
                self.pages.push([0; 1 << 16]);
            }
            size as i64
        }
    }

    fn stable_read(&mut self, offset: u64, buf: &mut [u8]) {
        // TODO(qti3e) This can be optimized.
        for i in 0..buf.len() {
            let offset = offset + i as u64;
            let page = offset >> 16;
            let byte = offset - (page << 16);
            buf[i] = self.pages[page as usize][byte as usize];
        }
    }

    fn stable_write(&mut self, offset: u64, buf: &[u8]) {
        // TODO(qti3e) This can be optimized.
        for i in 0..buf.len() {
            let offset = offset + i as u64;
            let page = offset >> 16;
            let byte = offset - (page << 16);
            self.pages[page as usize][byte as usize] = buf[i];
        }
    }
}
