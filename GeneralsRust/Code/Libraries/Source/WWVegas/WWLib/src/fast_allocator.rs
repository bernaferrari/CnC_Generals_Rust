//! Fast memory allocators for high-performance applications.
//!
//! This module implements fast memory allocators that prevent memory fragmentation
//! and provide better performance than generic malloc/free operations. They use
//! slightly more space on average but offer significant speed improvements.
//!
//! The module includes:
//! - `StackAllocator`: Stack-based allocation for temporary blocks
//! - `FastFixedAllocator`: Fixed-size block allocator
//! - `FastAllocatorGeneral`: General-purpose fast allocator
//! - `FastSTLAllocator`: STL-compatible allocator wrapper

use std::alloc::{self, Layout};
use std::mem;
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// A stack-based allocator that uses stack space for small allocations
/// and falls back to heap allocation for larger requests.
///
/// This allocator is designed for temporary memory allocations where you know
/// the memory will be freed in LIFO order. It provides significant performance
/// benefits over malloc for small, temporary allocations.
///
/// # Type Parameters
///
/// * `T` - The type of object being allocated
/// * `STACK_COUNT` - Maximum number of objects to allocate on stack before using heap
/// * `CONSTRUCT` - Whether to call constructors/destructors (for non-trivial types)
///
/// # Examples
///
/// ```
/// use wwlib_rust::fast_allocator::StackAllocator;
///
/// let mut allocator: StackAllocator<i32, 512, true> = StackAllocator::new();
/// let memory = allocator.alloc(100).unwrap();
/// // Use the memory...
/// allocator.dealloc(memory);
/// ```
pub struct StackAllocator<T, const STACK_COUNT: usize, const CONSTRUCT: bool> {
    /// Count of objects allocated. None means nothing is allocated.
    alloc_count: Option<usize>,
    /// Heap allocation pointer, used when request exceeds stack capacity
    heap_ptr: Option<NonNull<T>>,
    /// Stack-based storage
    stack_array: [mem::MaybeUninit<T>; STACK_COUNT],
}

impl<T, const STACK_COUNT: usize, const CONSTRUCT: bool> StackAllocator<T, STACK_COUNT, CONSTRUCT> {
    /// Creates a new stack allocator.
    pub fn new() -> Self {
        Self {
            alloc_count: None,
            heap_ptr: None,
            stack_array: unsafe { mem::MaybeUninit::uninit().assume_init() },
        }
    }

    /// Allocates memory for `count` objects of type T.
    ///
    /// Returns a pointer to the allocated memory, or None if allocation fails.
    /// The memory will either come from the stack buffer or heap depending on size.
    /// Note: For CONSTRUCT=true, T must implement Default.
    pub fn alloc(&mut self, count: usize) -> Option<NonNull<T>>
    where
        T: Default,
    {
        if self.alloc_count.is_none() {
            self.alloc_count = Some(count);

            if count <= STACK_COUNT {
                // Use stack allocation
                let ptr = self.stack_array.as_mut_ptr() as *mut T;

                if CONSTRUCT {
                    // Initialize objects by calling default constructor
                    unsafe {
                        for i in 0..count {
                            ptr.add(i).write(T::default());
                        }
                    }
                }

                NonNull::new(ptr)
            } else {
                // Use heap allocation
                let layout = Layout::array::<T>(count).ok()?;
                let ptr = unsafe { alloc::alloc(layout) as *mut T };

                if ptr.is_null() {
                    return None;
                }

                if CONSTRUCT {
                    // Initialize objects
                    unsafe {
                        for i in 0..count {
                            ptr.add(i).write(T::default());
                        }
                    }
                }

                let non_null_ptr = NonNull::new(ptr)?;
                self.heap_ptr = Some(non_null_ptr);
                Some(non_null_ptr)
            }
        } else {
            // Already in use, allocate from heap
            let layout = Layout::array::<T>(count).ok()?;
            let ptr = unsafe { alloc::alloc(layout) as *mut T };

            if ptr.is_null() {
                return None;
            }

            if CONSTRUCT {
                unsafe {
                    for i in 0..count {
                        ptr.add(i).write(T::default());
                    }
                }
            }

            NonNull::new(ptr)
        }
    }

    /// Allocates uninitialized memory for `count` objects of type T.
    ///
    /// This version doesn't call constructors and works with any type T.
    pub fn alloc_uninit(&mut self, count: usize) -> Option<NonNull<T>> {
        if self.alloc_count.is_none() {
            self.alloc_count = Some(count);

            if count <= STACK_COUNT {
                // Use stack allocation
                let ptr = self.stack_array.as_mut_ptr() as *mut T;
                NonNull::new(ptr)
            } else {
                // Use heap allocation
                let layout = Layout::array::<T>(count).ok()?;
                let ptr = unsafe { alloc::alloc(layout) as *mut T };

                if ptr.is_null() {
                    return None;
                }

                let non_null_ptr = NonNull::new(ptr)?;
                self.heap_ptr = Some(non_null_ptr);
                Some(non_null_ptr)
            }
        } else {
            // Already in use, allocate from heap
            let layout = Layout::array::<T>(count).ok()?;
            let ptr = unsafe { alloc::alloc(layout) as *mut T };

            if ptr.is_null() {
                return None;
            }

            NonNull::new(ptr)
        }
    }

    /// Deallocates memory previously allocated with `alloc`.
    pub fn dealloc(&mut self, ptr: NonNull<T>) {
        let stack_ptr = self.stack_array.as_mut_ptr() as *mut T;

        if ptr.as_ptr() == stack_ptr {
            // Deallocation from stack
            if let Some(count) = self.alloc_count {
                if CONSTRUCT {
                    unsafe {
                        for i in 0..count {
                            ptr.as_ptr().add(i).drop_in_place();
                        }
                    }
                }
                self.alloc_count = None;
            }
        } else if Some(ptr) == self.heap_ptr {
            // Deallocation from our heap allocation
            if let Some(count) = self.alloc_count {
                if CONSTRUCT {
                    unsafe {
                        for i in 0..count {
                            ptr.as_ptr().add(i).drop_in_place();
                        }
                    }
                }

                let layout = Layout::array::<T>(count).unwrap();
                unsafe { alloc::dealloc(ptr.as_ptr() as *mut u8, layout) };
                self.heap_ptr = None;
                self.alloc_count = None;
            }
        } else {
            // External heap allocation
            // Note: We can't properly deallocate this without knowing the size
            // In the original C++, this would use delete[]
            // For safety, we'll assume it was allocated with the system allocator
            unsafe {
                // This is unsafe because we don't know the original size
                // In a real implementation, you'd need to track allocation sizes
                let layout = Layout::new::<T>();
                alloc::dealloc(ptr.as_ptr() as *mut u8, layout);
            }
        }
    }
}

impl<T, const STACK_COUNT: usize, const CONSTRUCT: bool> Default
    for StackAllocator<T, STACK_COUNT, CONSTRUCT>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const STACK_COUNT: usize, const CONSTRUCT: bool> Drop
    for StackAllocator<T, STACK_COUNT, CONSTRUCT>
{
    fn drop(&mut self) {
        if let Some(count) = self.alloc_count {
            if let Some(heap_ptr) = self.heap_ptr {
                if CONSTRUCT {
                    unsafe {
                        for i in 0..count {
                            heap_ptr.as_ptr().add(i).drop_in_place();
                        }
                    }
                }
                let layout = Layout::array::<T>(count).unwrap();
                unsafe { alloc::dealloc(heap_ptr.as_ptr() as *mut u8, layout) };
            } else {
                // Stack allocation cleanup
                if CONSTRUCT {
                    let ptr = self.stack_array.as_mut_ptr() as *mut T;
                    unsafe {
                        for i in 0..count {
                            ptr.add(i).drop_in_place();
                        }
                    }
                }
            }
        }
    }
}

/// A fast allocator for fixed-size memory blocks.
///
/// This allocator maintains a free list of fixed-size blocks and can allocate/deallocate
/// very quickly. It's ideal when you need to allocate many objects of the same size.
pub struct FastFixedAllocator {
    /// Size of each allocation block
    element_size: usize,
    /// Total heap size allocated
    total_heap_size: AtomicUsize,
    /// Total size currently allocated to users
    total_allocated_size: AtomicUsize,
    /// Total number of active allocations
    total_allocation_count: AtomicUsize,
    /// Head of the free list
    free_list: Option<NonNull<Link>>,
    /// List of memory chunks
    chunks: Vec<Chunk>,
}

/// Internal structure for the free list
#[repr(C)]
struct Link {
    next: Option<NonNull<Link>>,
}

// SAFETY: Link only contains a pointer which can be safely sent between threads
// when properly synchronized (which we do through Mutex)
unsafe impl Send for Link {}
unsafe impl Sync for Link {}

/// Memory chunk used by FastFixedAllocator
struct Chunk {
    /// Size of usable memory in the chunk
    memory: Vec<u8>,
}

impl Chunk {
    const SIZE: usize = 8 * 1024 - 16; // 8KB - 16 bytes overhead

    fn new() -> Self {
        Self {
            memory: vec![0u8; Self::SIZE],
        }
    }

    fn as_ptr(&mut self) -> *mut u8 {
        self.memory.as_mut_ptr()
    }
}

impl FastFixedAllocator {
    /// Creates a new fixed allocator for blocks of size `element_size`.
    pub fn new(element_size: usize) -> Self {
        let actual_size = element_size.max(mem::size_of::<Link>());

        Self {
            element_size: actual_size,
            total_heap_size: AtomicUsize::new(0),
            total_allocated_size: AtomicUsize::new(0),
            total_allocation_count: AtomicUsize::new(0),
            free_list: None,
            chunks: Vec::new(),
        }
    }

    /// Initializes or changes the element size (must be called before first allocation).
    pub fn init(&mut self, element_size: usize) {
        self.element_size = element_size.max(mem::size_of::<Link>());
    }

    /// Allocates a block of memory.
    pub fn alloc(&mut self) -> Option<NonNull<u8>> {
        self.total_allocation_count.fetch_add(1, Ordering::Relaxed);
        self.total_allocated_size
            .fetch_add(self.element_size, Ordering::Relaxed);

        if self.free_list.is_none() {
            self.grow();
        }

        if let Some(link) = self.free_list {
            unsafe {
                self.free_list = (*link.as_ptr()).next;
                Some(NonNull::new_unchecked(link.as_ptr() as *mut u8))
            }
        } else {
            None
        }
    }

    /// Frees a previously allocated block.
    pub fn free(&mut self, ptr: NonNull<u8>) {
        self.total_allocation_count.fetch_sub(1, Ordering::Relaxed);
        self.total_allocated_size
            .fetch_sub(self.element_size, Ordering::Relaxed);

        let link = ptr.as_ptr() as *mut Link;
        unsafe {
            (*link).next = self.free_list;
            self.free_list = Some(NonNull::new_unchecked(link));
        }
    }

    /// Returns the total heap size allocated by this allocator.
    pub fn heap_size(&self) -> usize {
        self.total_heap_size.load(Ordering::Relaxed)
    }

    /// Returns the total size currently allocated to users.
    pub fn allocated_size(&self) -> usize {
        self.total_allocated_size.load(Ordering::Relaxed)
    }

    /// Returns the number of active allocations.
    pub fn allocation_count(&self) -> usize {
        self.total_allocation_count.load(Ordering::Relaxed)
    }

    /// Grows the allocator by adding a new chunk.
    fn grow(&mut self) {
        let mut chunk = Chunk::new();
        self.total_heap_size
            .fetch_add(mem::size_of::<Chunk>(), Ordering::Relaxed);

        let num_elements = Chunk::SIZE / self.element_size;
        let start = chunk.as_ptr();

        // Build the free list
        unsafe {
            for i in 0..(num_elements - 1) {
                let current = start.add(i * self.element_size) as *mut Link;
                let next = start.add((i + 1) * self.element_size) as *mut Link;
                (*current).next = Some(NonNull::new_unchecked(next));
            }

            // Last element points to null
            let last = start.add((num_elements - 1) * self.element_size) as *mut Link;
            (*last).next = None;

            // Set the head of the free list
            self.free_list = Some(NonNull::new_unchecked(start as *mut Link));
        }

        self.chunks.push(chunk);
    }
}

// SAFETY: FastFixedAllocator is safe to send between threads when properly synchronized
// The raw pointers are only used within the context of the allocator's internal logic
unsafe impl Send for FastFixedAllocator {}
unsafe impl Sync for FastFixedAllocator {}

/// A general-purpose fast allocator that handles variable-sized allocations.
///
/// This allocator uses a bucket system where different size ranges are handled
/// by separate FastFixedAllocators. Sizes above the maximum bucket size fall
/// back to the system allocator.
pub struct FastAllocatorGeneral {
    /// Maximum size handled by fixed allocators
    max_alloc_size: usize,
    /// Allocation step size for buckets
    alloc_step: usize,
    /// Fixed allocators for different size buckets
    allocators: Vec<Mutex<FastFixedAllocator>>,
    /// Amount allocated directly via malloc
    allocated_with_malloc: AtomicUsize,
    /// Count of allocations via malloc
    allocated_with_malloc_count: AtomicUsize,
    /// Total actual memory usage
    actual_memory_usage: AtomicUsize,
}

impl FastAllocatorGeneral {
    const MAX_ALLOC_SIZE: usize = 2048;
    const ALLOC_STEP: usize = 16;

    /// Creates a new general allocator.
    pub fn new() -> Self {
        let num_buckets = Self::MAX_ALLOC_SIZE / Self::ALLOC_STEP;
        let mut allocators = Vec::with_capacity(num_buckets);

        for i in 0..num_buckets {
            let alloc_size = (i + 1) * Self::ALLOC_STEP;
            allocators.push(Mutex::new(FastFixedAllocator::new(alloc_size)));
        }

        Self {
            max_alloc_size: Self::MAX_ALLOC_SIZE,
            alloc_step: Self::ALLOC_STEP,
            allocators,
            allocated_with_malloc: AtomicUsize::new(0),
            allocated_with_malloc_count: AtomicUsize::new(0),
            actual_memory_usage: AtomicUsize::new(0),
        }
    }

    /// Allocates memory of the specified size.
    ///
    /// The first 4 bytes store the allocation size, and the returned pointer
    /// points to the memory after this size header.
    pub fn alloc(&self, size: usize) -> Option<NonNull<u8>> {
        // Add space for size header
        let total_size = size + mem::size_of::<u32>();
        self.actual_memory_usage
            .fetch_add(total_size, Ordering::Relaxed);

        let memory = if total_size < self.max_alloc_size {
            let index = (total_size - 1) / self.alloc_step;
            let allocator = self.allocators.get(index)?;
            let mut guard = allocator.lock().ok()?;
            guard.alloc()
        } else {
            // Use system allocator for large allocations
            self.allocated_with_malloc
                .fetch_add(total_size, Ordering::Relaxed);
            self.allocated_with_malloc_count
                .fetch_add(1, Ordering::Relaxed);

            let layout = Layout::from_size_align(total_size, mem::align_of::<u32>()).ok()?;
            let ptr = unsafe { alloc::alloc(layout) };
            NonNull::new(ptr)
        }?;

        // Store the size in the first 4 bytes
        unsafe {
            *(memory.as_ptr() as *mut u32) = total_size as u32;
            let user_ptr = memory.as_ptr().add(mem::size_of::<u32>());
            NonNull::new(user_ptr)
        }
    }

    /// Frees memory previously allocated with `alloc`.
    pub fn free(&self, ptr: NonNull<u8>) {
        // Get the size from the header
        let header_ptr = unsafe { ptr.as_ptr().sub(mem::size_of::<u32>()) };
        let size = unsafe { *(header_ptr as *const u32) } as usize;

        self.actual_memory_usage.fetch_sub(size, Ordering::Relaxed);

        if size < self.max_alloc_size {
            let index = (size - 1) / self.alloc_step;
            if let Some(allocator) = self.allocators.get(index) {
                if let Ok(mut guard) = allocator.lock() {
                    guard.free(unsafe { NonNull::new_unchecked(header_ptr) });
                }
            }
        } else {
            // Free using system allocator
            self.allocated_with_malloc_count
                .fetch_sub(1, Ordering::Relaxed);
            self.allocated_with_malloc
                .fetch_sub(size, Ordering::Relaxed);

            let layout = Layout::from_size_align(size, mem::align_of::<u32>()).unwrap();
            unsafe { alloc::dealloc(header_ptr, layout) };
        }
    }

    /// Reallocates memory to a new size.
    ///
    /// This follows ANSI C realloc semantics:
    /// - realloc(null, size) is equivalent to malloc(size)
    /// - realloc(ptr, 0) is equivalent to free(ptr) and returns null
    /// - If realloc fails, the original memory is unchanged
    pub fn realloc(&self, ptr: Option<NonNull<u8>>, new_size: usize) -> Option<NonNull<u8>> {
        if new_size == 0 {
            if let Some(p) = ptr {
                self.free(p);
            }
            return None;
        }

        let new_ptr = self.alloc(new_size)?;

        if let Some(old_ptr) = ptr {
            // Copy old data to new location
            let old_header_ptr = unsafe { old_ptr.as_ptr().sub(mem::size_of::<u32>()) };
            let old_size = unsafe { *(old_header_ptr as *const u32) } as usize;
            let old_user_size = old_size - mem::size_of::<u32>();

            let copy_size = old_user_size.min(new_size);
            unsafe {
                ptr::copy_nonoverlapping(old_ptr.as_ptr(), new_ptr.as_ptr(), copy_size);
            }

            self.free(old_ptr);
        }

        Some(new_ptr)
    }

    /// Returns the total heap size used by this allocator.
    pub fn total_heap_size(&self) -> usize {
        let malloc_size = self.allocated_with_malloc.load(Ordering::Relaxed);
        let fixed_size: usize = self
            .allocators
            .iter()
            .map(|alloc| alloc.lock().map(|guard| guard.heap_size()).unwrap_or(0))
            .sum();
        malloc_size + fixed_size
    }

    /// Returns the total allocated size.
    pub fn total_allocated_size(&self) -> usize {
        let malloc_size = self.allocated_with_malloc.load(Ordering::Relaxed);
        let fixed_size: usize = self
            .allocators
            .iter()
            .map(|alloc| {
                alloc
                    .lock()
                    .map(|guard| guard.allocated_size())
                    .unwrap_or(0)
            })
            .sum();
        malloc_size + fixed_size
    }

    /// Returns the total number of allocations.
    pub fn total_allocation_count(&self) -> usize {
        let malloc_count = self.allocated_with_malloc_count.load(Ordering::Relaxed);
        let fixed_count: usize = self
            .allocators
            .iter()
            .map(|alloc| {
                alloc
                    .lock()
                    .map(|guard| guard.allocation_count())
                    .unwrap_or(0)
            })
            .sum();
        malloc_count + fixed_count
    }

    /// Returns the actual memory usage.
    pub fn actual_memory_usage(&self) -> usize {
        self.actual_memory_usage.load(Ordering::Relaxed)
    }
}

impl Default for FastAllocatorGeneral {
    fn default() -> Self {
        Self::new()
    }
}

/// Global instance of the general allocator.
static GLOBAL_ALLOCATOR: std::sync::OnceLock<Arc<FastAllocatorGeneral>> =
    std::sync::OnceLock::new();

/// Gets the global fast allocator instance.
pub fn get_global_allocator() -> Arc<FastAllocatorGeneral> {
    GLOBAL_ALLOCATOR
        .get_or_init(|| Arc::new(FastAllocatorGeneral::new()))
        .clone()
}

/// STL-compatible allocator that uses the FastAllocatorGeneral.
///
/// This can be used as a drop-in replacement for std::allocator in STL containers
/// to improve allocation performance.
pub struct FastSTLAllocator<T> {
    _phantom: std::marker::PhantomData<T>,
}

impl<T> FastSTLAllocator<T> {
    /// Creates a new STL allocator.
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }

    /// Allocates memory for `count` objects of type T.
    pub fn allocate(&self, count: usize) -> Option<NonNull<T>> {
        if count == 0 {
            return None;
        }

        let size = count * mem::size_of::<T>();
        let allocator = get_global_allocator();
        let ptr = allocator.alloc(size)?;

        // Cast to the appropriate type
        Some(unsafe { NonNull::new_unchecked(ptr.as_ptr() as *mut T) })
    }

    /// Deallocates memory previously allocated with `allocate`.
    pub fn deallocate(&self, ptr: NonNull<T>) {
        let allocator = get_global_allocator();
        let byte_ptr = unsafe { NonNull::new_unchecked(ptr.as_ptr() as *mut u8) };
        allocator.free(byte_ptr);
    }

    /// Constructs an object at the given location.
    pub fn construct(&self, ptr: NonNull<T>, value: T) {
        unsafe { ptr.as_ptr().write(value) };
    }

    /// Destroys an object at the given location.
    pub fn destroy(&self, ptr: NonNull<T>) {
        unsafe { ptr.as_ptr().drop_in_place() };
    }

    /// Returns the maximum number of objects that can be allocated.
    pub fn max_size(&self) -> usize {
        usize::MAX / mem::size_of::<T>()
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

impl<T, U> PartialEq<FastSTLAllocator<U>> for FastSTLAllocator<T> {
    fn eq(&self, _other: &FastSTLAllocator<U>) -> bool {
        true // All instances are equivalent
    }
}

impl<T> Eq for FastSTLAllocator<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_allocator() {
        let mut allocator: StackAllocator<i32, 100, false> = StackAllocator::new();

        // Test small allocation (should use stack)
        let ptr = allocator.alloc(50).unwrap();
        allocator.dealloc(ptr);

        // Test large allocation (should use heap)
        let ptr = allocator.alloc(200).unwrap();
        allocator.dealloc(ptr);
    }

    #[test]
    fn test_fixed_allocator() {
        let mut allocator = FastFixedAllocator::new(64);

        let ptr1 = allocator.alloc().unwrap();
        let ptr2 = allocator.alloc().unwrap();

        assert_eq!(allocator.allocation_count(), 2);
        assert_eq!(allocator.allocated_size(), 128);

        allocator.free(ptr1);
        allocator.free(ptr2);

        assert_eq!(allocator.allocation_count(), 0);
        assert_eq!(allocator.allocated_size(), 0);
    }

    #[test]
    fn test_general_allocator() {
        let allocator = FastAllocatorGeneral::new();

        // Test small allocation
        let ptr1 = allocator.alloc(64).unwrap();
        assert!(allocator.actual_memory_usage() > 0);

        // Test large allocation
        let ptr2 = allocator.alloc(4096).unwrap();

        allocator.free(ptr1);
        allocator.free(ptr2);
    }

    #[test]
    fn test_realloc() {
        let allocator = FastAllocatorGeneral::new();

        // Test basic realloc
        let ptr = allocator.alloc(64).unwrap();
        let new_ptr = allocator.realloc(Some(ptr), 128).unwrap();
        allocator.free(new_ptr);

        // Test realloc with null pointer (should work like malloc)
        let ptr = allocator.realloc(None, 64).unwrap();
        allocator.free(ptr);

        // Test realloc with size 0 (should work like free)
        let ptr = allocator.alloc(64).unwrap();
        let result = allocator.realloc(Some(ptr), 0);
        assert!(result.is_none());
    }
}
