//! Variable-size bucket allocator (`FastAllocatorGeneral` / `FastSTLAllocator`).

use super::fixed::{AllocHandle, FastFixedAllocator};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Handle to a general-allocator block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GeneralAlloc {
    id: u32,
}

enum Loc {
    Bucket { index: usize, slot: AllocHandle },
    Large,
}

/// Bucketed allocator. Large sizes use owned `Vec<u8>`.
pub struct FastAllocatorGeneral {
    max_alloc_size: usize,
    alloc_step: usize,
    buckets: Vec<Mutex<FastFixedAllocator>>,
    large: Mutex<HashMap<u32, Vec<u8>>>,
    locs: Mutex<HashMap<u32, Loc>>,
    next_id: AtomicU32,
    malloc_bytes: AtomicUsize,
    malloc_count: AtomicUsize,
    actual_bytes: AtomicUsize,
}

impl FastAllocatorGeneral {
    const MAX_ALLOC_SIZE: usize = 2048;
    const ALLOC_STEP: usize = 16;

    pub fn new() -> Self {
        let n = Self::MAX_ALLOC_SIZE / Self::ALLOC_STEP;
        let mut buckets = Vec::with_capacity(n);
        for i in 0..n {
            buckets.push(Mutex::new(FastFixedAllocator::new(
                (i + 1) * Self::ALLOC_STEP,
            )));
        }
        Self {
            max_alloc_size: Self::MAX_ALLOC_SIZE,
            alloc_step: Self::ALLOC_STEP,
            buckets,
            large: Mutex::new(HashMap::new()),
            locs: Mutex::new(HashMap::new()),
            next_id: AtomicU32::new(1),
            malloc_bytes: AtomicUsize::new(0),
            malloc_count: AtomicUsize::new(0),
            actual_bytes: AtomicUsize::new(0),
        }
    }

    pub fn alloc(&self, size: usize) -> Option<GeneralAlloc> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.actual_bytes.fetch_add(size, Ordering::Relaxed);
        if size < self.max_alloc_size && size > 0 {
            let index = (size - 1) / self.alloc_step;
            let mut bucket = self.buckets.get(index)?.lock().ok()?;
            let slot = bucket.alloc()?;
            drop(bucket);
            self.locs
                .lock()
                .ok()?
                .insert(id, Loc::Bucket { index, slot });
        } else {
            self.malloc_bytes.fetch_add(size, Ordering::Relaxed);
            self.malloc_count.fetch_add(1, Ordering::Relaxed);
            self.large.lock().ok()?.insert(id, vec![0u8; size]);
            self.locs.lock().ok()?.insert(id, Loc::Large);
        }
        Some(GeneralAlloc { id })
    }

    pub fn get<'a>(&'a self, handle: GeneralAlloc) -> Option<Vec<u8>> {
        let locs = self.locs.lock().ok()?;
        match locs.get(&handle.id)? {
            Loc::Bucket { index, slot } => {
                let bucket = self.buckets.get(*index)?.lock().ok()?;
                bucket.get(*slot).map(|s| s.to_vec())
            }
            Loc::Large => self.large.lock().ok()?.get(&handle.id).cloned(),
        }
    }

    pub fn write(&self, handle: GeneralAlloc, data: &[u8]) -> bool {
        let locs = match self.locs.lock() {
            Ok(g) => g,
            Err(_) => return false,
        };
        match locs.get(&handle.id) {
            Some(Loc::Bucket { index, slot }) => {
                let Some(bucket) = self.buckets.get(*index) else {
                    return false;
                };
                let Ok(mut guard) = bucket.lock() else {
                    return false;
                };
                let Some(slot) = guard.get_mut(*slot) else {
                    return false;
                };
                let n = data.len().min(slot.len());
                slot[..n].copy_from_slice(&data[..n]);
                true
            }
            Some(Loc::Large) => {
                let Ok(mut large) = self.large.lock() else {
                    return false;
                };
                let Some(buf) = large.get_mut(&handle.id) else {
                    return false;
                };
                let n = data.len().min(buf.len());
                buf[..n].copy_from_slice(&data[..n]);
                true
            }
            None => false,
        }
    }

    pub fn free(&self, handle: GeneralAlloc) {
        let Ok(mut locs) = self.locs.lock() else {
            return;
        };
        let Some(loc) = locs.remove(&handle.id) else {
            return;
        };
        match loc {
            Loc::Bucket { index, slot } => {
                if let Some(bucket) = self.buckets.get(index) {
                    if let Ok(mut guard) = bucket.lock() {
                        let size = guard.allocated_size() / guard.allocation_count().max(1);
                        self.actual_bytes.fetch_sub(
                            size.min(self.actual_bytes.load(Ordering::Relaxed)),
                            Ordering::Relaxed,
                        );
                        guard.free(slot);
                    }
                }
            }
            Loc::Large => {
                if let Ok(mut large) = self.large.lock() {
                    if let Some(buf) = large.remove(&handle.id) {
                        let n = buf.len();
                        self.actual_bytes.fetch_sub(
                            n.min(self.actual_bytes.load(Ordering::Relaxed)),
                            Ordering::Relaxed,
                        );
                        self.malloc_bytes.fetch_sub(
                            n.min(self.malloc_bytes.load(Ordering::Relaxed)),
                            Ordering::Relaxed,
                        );
                        self.malloc_count.fetch_sub(1, Ordering::Relaxed);
                    }
                }
            }
        }
    }

    pub fn realloc(&self, handle: Option<GeneralAlloc>, new_size: usize) -> Option<GeneralAlloc> {
        if new_size == 0 {
            if let Some(h) = handle {
                self.free(h);
            }
            return None;
        }
        let new_h = self.alloc(new_size)?;
        if let Some(old) = handle {
            if let Some(bytes) = self.get(old) {
                self.write(new_h, &bytes);
            }
            self.free(old);
        }
        Some(new_h)
    }

    pub fn total_heap_size(&self) -> usize {
        let malloc = self.malloc_bytes.load(Ordering::Relaxed);
        let fixed: usize = self
            .buckets
            .iter()
            .map(|b| b.lock().map(|g| g.heap_size()).unwrap_or(0))
            .sum();
        malloc + fixed
    }

    pub fn total_allocated_size(&self) -> usize {
        let malloc = self.malloc_bytes.load(Ordering::Relaxed);
        let fixed: usize = self
            .buckets
            .iter()
            .map(|b| b.lock().map(|g| g.allocated_size()).unwrap_or(0))
            .sum();
        malloc + fixed
    }

    pub fn total_allocation_count(&self) -> usize {
        let malloc = self.malloc_count.load(Ordering::Relaxed);
        let fixed: usize = self
            .buckets
            .iter()
            .map(|b| b.lock().map(|g| g.allocation_count()).unwrap_or(0))
            .sum();
        malloc + fixed
    }

    pub fn actual_memory_usage(&self) -> usize {
        self.actual_bytes.load(Ordering::Relaxed)
    }
}

impl Default for FastAllocatorGeneral {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL_ALLOCATOR: std::sync::OnceLock<Arc<FastAllocatorGeneral>> =
    std::sync::OnceLock::new();

pub fn get_global_allocator() -> Arc<FastAllocatorGeneral> {
    GLOBAL_ALLOCATOR
        .get_or_init(|| Arc::new(FastAllocatorGeneral::new()))
        .clone()
}

/// STL-shaped wrapper over the global general allocator.
pub struct FastSTLAllocator<T> {
    _ty: std::marker::PhantomData<T>,
}

impl<T> FastSTLAllocator<T> {
    pub fn new() -> Self {
        Self {
            _ty: std::marker::PhantomData,
        }
    }

    pub fn allocate(&self, count: usize) -> Option<GeneralAlloc> {
        if count == 0 {
            return None;
        }
        get_global_allocator().alloc(count * std::mem::size_of::<T>())
    }

    pub fn deallocate(&self, handle: GeneralAlloc) {
        get_global_allocator().free(handle);
    }

    pub fn max_size(&self) -> usize {
        usize::MAX / std::mem::size_of::<T>().max(1)
    }
}

impl<T> Default for FastSTLAllocator<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for FastSTLAllocator<T> {
    fn clone(&self) -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_and_large() {
        let allocator = FastAllocatorGeneral::new();
        let a = allocator.alloc(64).expect("small");
        assert!(allocator.actual_memory_usage() > 0);
        let b = allocator.alloc(4096).expect("large");
        allocator.free(a);
        allocator.free(b);
    }

    #[test]
    fn realloc_copy() {
        let allocator = FastAllocatorGeneral::new();
        let a = allocator.alloc(64).expect("a");
        allocator.write(a, &[1, 2, 3, 4]);
        let b = allocator.realloc(Some(a), 128).expect("grow");
        let bytes = allocator.get(b).expect("read");
        assert_eq!(&bytes[..4], &[1, 2, 3, 4]);
        assert!(allocator.realloc(Some(b), 0).is_none());
    }
}
