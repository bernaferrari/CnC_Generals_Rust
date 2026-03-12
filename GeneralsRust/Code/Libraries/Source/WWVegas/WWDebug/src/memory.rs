//! Memory Management Module
//!
//! Provides memory allocation functions equivalent to the C++ ProfileAllocMemory,
//! ProfileReAllocMemory, and ProfileFreeMemory functions, but using safe Rust
//! memory management patterns.

use crate::{ProfileError, ProfileResult};
use std::alloc::{self, Layout};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Memory allocation statistics
pub struct MemoryStats {
    total_allocated: AtomicUsize,
    total_freed: AtomicUsize,
    current_allocated: AtomicUsize,
    allocation_count: AtomicUsize,
}

impl MemoryStats {
    pub const fn new() -> Self {
        Self {
            total_allocated: AtomicUsize::new(0),
            total_freed: AtomicUsize::new(0),
            current_allocated: AtomicUsize::new(0),
            allocation_count: AtomicUsize::new(0),
        }
    }

    pub fn get_total_allocated(&self) -> usize {
        self.total_allocated.load(Ordering::Relaxed)
    }

    pub fn get_total_freed(&self) -> usize {
        self.total_freed.load(Ordering::Relaxed)
    }

    pub fn get_current_allocated(&self) -> usize {
        self.current_allocated.load(Ordering::Relaxed)
    }

    pub fn get_allocation_count(&self) -> usize {
        self.allocation_count.load(Ordering::Relaxed)
    }

    fn record_allocation(&self, size: usize) {
        self.total_allocated.fetch_add(size, Ordering::Relaxed);
        self.current_allocated.fetch_add(size, Ordering::Relaxed);
        self.allocation_count.fetch_add(1, Ordering::Relaxed);
    }

    fn record_deallocation(&self, size: usize) {
        self.total_freed.fetch_add(size, Ordering::Relaxed);
        self.current_allocated.fetch_sub(size, Ordering::Relaxed);
    }
}

/// Global memory statistics
static MEMORY_STATS: MemoryStats = MemoryStats::new();

/// Profile memory allocator - equivalent to ProfileAllocMemory
pub struct ProfileMemory;

impl ProfileMemory {
    /// Allocate memory (equivalent to ProfileAllocMemory)
    pub fn alloc(size: usize) -> ProfileResult<*mut u8> {
        if size == 0 {
            return Ok(std::ptr::null_mut());
        }

        let layout =
            Layout::from_size_align(size, 1).map_err(|_| ProfileError::MemoryAllocation)?;

        let ptr = unsafe { alloc::alloc(layout) };

        if ptr.is_null() {
            return Err(ProfileError::MemoryAllocation);
        }

        MEMORY_STATS.record_allocation(size);
        Ok(ptr)
    }

    /// Reallocate memory (equivalent to ProfileReAllocMemory)
    pub fn realloc(old_ptr: *mut u8, old_size: usize, new_size: usize) -> ProfileResult<*mut u8> {
        // Handle special cases like the C++ version
        if old_ptr.is_null() {
            return if new_size > 0 {
                Self::alloc(new_size)
            } else {
                Ok(std::ptr::null_mut())
            };
        }

        if new_size == 0 {
            Self::free(old_ptr, old_size);
            return Ok(std::ptr::null_mut());
        }

        let old_layout =
            Layout::from_size_align(old_size, 1).map_err(|_| ProfileError::MemoryAllocation)?;
        let _new_layout =
            Layout::from_size_align(new_size, 1).map_err(|_| ProfileError::MemoryAllocation)?;

        let new_ptr = unsafe { alloc::realloc(old_ptr, old_layout, new_size) };

        if new_ptr.is_null() {
            return Err(ProfileError::MemoryAllocation);
        }

        // Update statistics
        MEMORY_STATS.record_deallocation(old_size);
        MEMORY_STATS.record_allocation(new_size);

        Ok(new_ptr)
    }

    /// Free memory (equivalent to ProfileFreeMemory)
    pub fn free(ptr: *mut u8, size: usize) {
        if ptr.is_null() || size == 0 {
            return;
        }

        let layout = Layout::from_size_align(size, 1).expect("Invalid layout for free");
        unsafe {
            alloc::dealloc(ptr, layout);
        }

        MEMORY_STATS.record_deallocation(size);
    }

    /// Get memory statistics
    pub fn get_stats() -> &'static MemoryStats {
        &MEMORY_STATS
    }
}

/// RAII wrapper for profile memory allocation
pub struct ProfileMemoryBlock {
    ptr: *mut u8,
    size: usize,
}

impl ProfileMemoryBlock {
    /// Allocate a new memory block
    pub fn new(size: usize) -> ProfileResult<Self> {
        let ptr = ProfileMemory::alloc(size)?;
        Ok(Self { ptr, size })
    }

    /// Get a raw pointer to the memory
    pub fn as_ptr(&self) -> *mut u8 {
        self.ptr
    }

    /// Get the size of the allocation
    pub fn size(&self) -> usize {
        self.size
    }

    /// Convert to a slice (unsafe)
    pub unsafe fn as_slice(&self) -> &[u8] {
        if self.ptr.is_null() || self.size == 0 {
            &[]
        } else {
            std::slice::from_raw_parts(self.ptr, self.size)
        }
    }

    /// Convert to a mutable slice (unsafe)
    pub unsafe fn as_mut_slice(&mut self) -> &mut [u8] {
        if self.ptr.is_null() || self.size == 0 {
            &mut []
        } else {
            std::slice::from_raw_parts_mut(self.ptr, self.size)
        }
    }

    /// Resize the memory block
    pub fn resize(&mut self, new_size: usize) -> ProfileResult<()> {
        let new_ptr = ProfileMemory::realloc(self.ptr, self.size, new_size)?;
        self.ptr = new_ptr;
        self.size = new_size;
        Ok(())
    }

    /// Release the memory without dropping (for manual management)
    pub fn into_raw(self) -> (*mut u8, usize) {
        let ptr = self.ptr;
        let size = self.size;
        std::mem::forget(self);
        (ptr, size)
    }
}

impl Drop for ProfileMemoryBlock {
    fn drop(&mut self) {
        ProfileMemory::free(self.ptr, self.size);
    }
}

/// Safe wrapper for dynamic arrays (like the C++ dynamic arrays)
pub struct ProfileArray<T> {
    ptr: *mut T,
    len: usize,
    capacity: usize,
}

impl<T> ProfileArray<T> {
    /// Create a new empty array
    pub fn new() -> Self {
        Self {
            ptr: std::ptr::null_mut(),
            len: 0,
            capacity: 0,
        }
    }

    /// Create an array with initial capacity
    pub fn with_capacity(capacity: usize) -> ProfileResult<Self> {
        if capacity == 0 {
            return Ok(Self::new());
        }

        let layout = Layout::array::<T>(capacity).map_err(|_| ProfileError::MemoryAllocation)?;

        let ptr = unsafe { alloc::alloc(layout) as *mut T };

        if ptr.is_null() {
            return Err(ProfileError::MemoryAllocation);
        }

        MEMORY_STATS.record_allocation(layout.size());

        Ok(Self {
            ptr,
            len: 0,
            capacity,
        })
    }

    /// Get the length
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Resize the array
    pub fn resize(&mut self, new_capacity: usize) -> ProfileResult<()> {
        if new_capacity == self.capacity {
            return Ok(());
        }

        if new_capacity == 0 {
            self.clear();
            return Ok(());
        }

        let new_layout =
            Layout::array::<T>(new_capacity).map_err(|_| ProfileError::MemoryAllocation)?;

        let new_ptr = if self.ptr.is_null() {
            unsafe { alloc::alloc(new_layout) as *mut T }
        } else {
            let old_layout =
                Layout::array::<T>(self.capacity).map_err(|_| ProfileError::MemoryAllocation)?;
            unsafe { alloc::realloc(self.ptr as *mut u8, old_layout, new_layout.size()) as *mut T }
        };

        if new_ptr.is_null() {
            return Err(ProfileError::MemoryAllocation);
        }

        // Update statistics
        if !self.ptr.is_null() {
            let old_size = Layout::array::<T>(self.capacity)
                .map(|l| l.size())
                .unwrap_or(0);
            MEMORY_STATS.record_deallocation(old_size);
        }
        MEMORY_STATS.record_allocation(new_layout.size());

        self.ptr = new_ptr;
        self.capacity = new_capacity;

        // Adjust length if it exceeds new capacity
        if self.len > new_capacity {
            // Drop excess elements
            for i in new_capacity..self.len {
                unsafe {
                    std::ptr::drop_in_place(self.ptr.add(i));
                }
            }
            self.len = new_capacity;
        }

        Ok(())
    }

    /// Push an element
    pub fn push(&mut self, value: T) -> ProfileResult<()> {
        if self.len >= self.capacity {
            let new_capacity = if self.capacity == 0 {
                1
            } else {
                self.capacity * 2
            };
            self.resize(new_capacity)?;
        }

        unsafe {
            self.ptr.add(self.len).write(value);
        }
        self.len += 1;

        Ok(())
    }

    /// Pop an element
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }

        self.len -= 1;
        unsafe { Some(self.ptr.add(self.len).read()) }
    }

    /// Get element at index
    pub fn get(&self, index: usize) -> Option<&T> {
        if index < self.len {
            unsafe { Some(&*self.ptr.add(index)) }
        } else {
            None
        }
    }

    /// Get mutable element at index
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index < self.len {
            unsafe { Some(&mut *self.ptr.add(index)) }
        } else {
            None
        }
    }

    /// Clear all elements
    pub fn clear(&mut self) {
        // Drop all elements
        for i in 0..self.len {
            unsafe {
                std::ptr::drop_in_place(self.ptr.add(i));
            }
        }
        self.len = 0;

        // Free memory
        if !self.ptr.is_null() && self.capacity > 0 {
            let layout = Layout::array::<T>(self.capacity).unwrap();
            unsafe {
                alloc::dealloc(self.ptr as *mut u8, layout);
            }
            MEMORY_STATS.record_deallocation(layout.size());
        }

        self.ptr = std::ptr::null_mut();
        self.capacity = 0;
    }

    /// Get as slice
    pub fn as_slice(&self) -> &[T] {
        if self.ptr.is_null() || self.len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
        }
    }

    /// Get as mutable slice
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        if self.ptr.is_null() || self.len == 0 {
            &mut []
        } else {
            unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
        }
    }
}

impl<T> Drop for ProfileArray<T> {
    fn drop(&mut self) {
        self.clear();
    }
}

impl<T> Default for ProfileArray<T> {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl<T: Send> Send for ProfileArray<T> {}
unsafe impl<T: Sync> Sync for ProfileArray<T> {}

/// String type that uses profile memory allocation (equivalent to dynamically allocated C strings)
pub struct ProfileString {
    array: ProfileArray<u8>,
}

impl ProfileString {
    /// Create a new empty string
    pub fn new() -> Self {
        Self {
            array: ProfileArray::new(),
        }
    }

    /// Create from a Rust string
    pub fn from_str(s: &str) -> ProfileResult<Self> {
        let mut string = Self::new();
        string.set_str(s)?;
        Ok(string)
    }

    /// Set the string content
    pub fn set_str(&mut self, s: &str) -> ProfileResult<()> {
        let bytes = s.as_bytes();
        self.array.resize(bytes.len() + 1)?; // +1 for null terminator

        // Copy string data
        for (i, &byte) in bytes.iter().enumerate() {
            unsafe {
                *self.array.ptr.add(i) = byte;
            }
        }

        // Add null terminator
        unsafe {
            *self.array.ptr.add(bytes.len()) = 0;
        }

        self.array.len = bytes.len() + 1;
        Ok(())
    }

    /// Get as C string pointer
    pub fn as_ptr(&self) -> *const i8 {
        if self.array.is_empty() {
            static EMPTY: i8 = 0;
            &EMPTY
        } else {
            self.array.ptr as *const i8
        }
    }

    /// Get as Rust string (without null terminator)
    pub fn as_str(&self) -> &str {
        if self.array.is_empty() {
            ""
        } else {
            let slice = unsafe {
                std::slice::from_raw_parts(self.array.ptr, self.array.len.saturating_sub(1))
            };
            std::str::from_utf8(slice).unwrap_or("")
        }
    }

    /// Get length (excluding null terminator)
    pub fn len(&self) -> usize {
        self.array.len.saturating_sub(1)
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.array.len <= 1
    }
}

impl Default for ProfileString {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ProfileString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::fmt::Debug for ProfileString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ProfileString({:?})", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_allocation() {
        let _lock = crate::test_lock();
        let stats_before = ProfileMemory::get_stats();
        let initial_allocated = stats_before.get_current_allocated();

        let ptr = ProfileMemory::alloc(1024).unwrap();
        assert!(!ptr.is_null());

        let stats_after = ProfileMemory::get_stats();
        assert_eq!(
            stats_after.get_current_allocated(),
            initial_allocated + 1024
        );

        ProfileMemory::free(ptr, 1024);

        let stats_final = ProfileMemory::get_stats();
        assert_eq!(stats_final.get_current_allocated(), initial_allocated);
    }

    #[test]
    fn test_memory_reallocation() {
        let _lock = crate::test_lock();
        let ptr = ProfileMemory::alloc(512).unwrap();

        let new_ptr = ProfileMemory::realloc(ptr, 512, 1024).unwrap();
        assert!(!new_ptr.is_null());

        ProfileMemory::free(new_ptr, 1024);

        // Test realloc with null pointer
        let ptr2 = ProfileMemory::realloc(std::ptr::null_mut(), 0, 256).unwrap();
        assert!(!ptr2.is_null());

        // Test realloc to zero size
        let null_ptr = ProfileMemory::realloc(ptr2, 256, 0).unwrap();
        assert!(null_ptr.is_null());
    }

    #[test]
    fn test_memory_block() {
        let _lock = crate::test_lock();
        let mut block = ProfileMemoryBlock::new(1024).unwrap();
        assert_eq!(block.size(), 1024);
        assert!(!block.as_ptr().is_null());

        block.resize(2048).unwrap();
        assert_eq!(block.size(), 2048);

        let (ptr, size) = block.into_raw();
        assert_eq!(size, 2048);
        ProfileMemory::free(ptr, size);
    }

    #[test]
    fn test_profile_array() {
        let mut array = ProfileArray::<i32>::new();
        assert!(array.is_empty());
        assert_eq!(array.capacity(), 0);

        array.push(42).unwrap();
        array.push(24).unwrap();

        assert_eq!(array.len(), 2);
        assert_eq!(array.get(0), Some(&42));
        assert_eq!(array.get(1), Some(&24));

        let popped = array.pop();
        assert_eq!(popped, Some(24));
        assert_eq!(array.len(), 1);

        array.resize(10).unwrap();
        assert_eq!(array.capacity(), 10);
        assert_eq!(array.len(), 1);

        array.clear();
        assert!(array.is_empty());
        assert_eq!(array.capacity(), 0);
    }

    #[test]
    fn test_profile_string() {
        let mut string = ProfileString::new();
        assert!(string.is_empty());
        assert_eq!(string.as_str(), "");

        string.set_str("Hello, World!").unwrap();
        assert_eq!(string.len(), 13);
        assert_eq!(string.as_str(), "Hello, World!");
        assert!(!string.as_ptr().is_null());

        let string2 = ProfileString::from_str("Test").unwrap();
        assert_eq!(string2.as_str(), "Test");
        assert_eq!(string2.len(), 4);
    }

    #[test]
    fn test_memory_stats() {
        let stats = ProfileMemory::get_stats();
        let initial_count = stats.get_allocation_count();
        let initial_allocated = stats.get_total_allocated();

        let _block = ProfileMemoryBlock::new(512).unwrap();

        assert_eq!(stats.get_allocation_count(), initial_count + 1);
        assert_eq!(stats.get_total_allocated(), initial_allocated + 512);
    }

    #[test]
    fn test_array_with_complex_type() {
        #[derive(Debug, PartialEq)]
        struct TestStruct {
            value: i32,
            name: String,
        }

        let mut array = ProfileArray::<TestStruct>::new();

        array
            .push(TestStruct {
                value: 1,
                name: "one".to_string(),
            })
            .unwrap();
        array
            .push(TestStruct {
                value: 2,
                name: "two".to_string(),
            })
            .unwrap();

        assert_eq!(array.len(), 2);
        assert_eq!(array.get(0).unwrap().value, 1);
        assert_eq!(array.get(1).unwrap().name, "two");

        let slice = array.as_slice();
        assert_eq!(slice.len(), 2);
        assert_eq!(slice[0].value, 1);
    }
}
