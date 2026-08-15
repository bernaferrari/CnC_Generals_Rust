//! Fixed-size block allocator (`FastFixedAllocator` in C++).

/// Handle to a slot in [`FastFixedAllocator`]. Invalid after `free`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AllocHandle {
    index: usize,
}

/// Fixed-size blocks stored in a `Vec`; free list is indices, not pointers.
pub struct FastFixedAllocator {
    element_size: usize,
    slots: Vec<Vec<u8>>,
    free: Vec<usize>,
    live: usize,
    heap_bytes: usize,
}

impl FastFixedAllocator {
    const CHUNK_SLOTS: usize = 64;

    pub fn new(element_size: usize) -> Self {
        Self {
            element_size: element_size.max(1),
            slots: Vec::new(),
            free: Vec::new(),
            live: 0,
            heap_bytes: 0,
        }
    }

    pub fn init(&mut self, element_size: usize) {
        self.element_size = element_size.max(1);
    }

    pub fn alloc(&mut self) -> Option<AllocHandle> {
        if self.free.is_empty() {
            self.grow();
        }
        let index = self.free.pop()?;
        self.live += 1;
        Some(AllocHandle { index })
    }

    pub fn get(&self, handle: AllocHandle) -> Option<&[u8]> {
        self.slots.get(handle.index).map(Vec::as_slice)
    }

    pub fn get_mut(&mut self, handle: AllocHandle) -> Option<&mut [u8]> {
        self.slots.get_mut(handle.index).map(Vec::as_mut_slice)
    }

    pub fn free(&mut self, handle: AllocHandle) {
        if handle.index >= self.slots.len() {
            return;
        }
        if self.live > 0 {
            self.live -= 1;
        }
        self.free.push(handle.index);
    }

    pub fn heap_size(&self) -> usize {
        self.heap_bytes
    }

    pub fn allocated_size(&self) -> usize {
        self.live * self.element_size
    }

    pub fn allocation_count(&self) -> usize {
        self.live
    }

    fn grow(&mut self) {
        let start = self.slots.len();
        for i in 0..Self::CHUNK_SLOTS {
            self.slots.push(vec![0u8; self.element_size]);
            self.free.push(start + i);
        }
        self.heap_bytes += Self::CHUNK_SLOTS * self.element_size;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_free_counts() {
        let mut allocator = FastFixedAllocator::new(64);
        let a = allocator.alloc().expect("a");
        let b = allocator.alloc().expect("b");
        assert_eq!(allocator.allocation_count(), 2);
        assert_eq!(allocator.allocated_size(), 128);
        assert_eq!(allocator.get(a).map(|s| s.len()), Some(64));
        allocator.free(a);
        allocator.free(b);
        assert_eq!(allocator.allocation_count(), 0);
    }
}
