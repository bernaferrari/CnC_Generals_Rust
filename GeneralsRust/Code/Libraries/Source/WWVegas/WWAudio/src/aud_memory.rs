//! Audio Memory Pool Implementation
//! 
//! This module provides a memory pool for efficient allocation and deallocation
//! of fixed-size audio memory blocks. The pool pre-allocates a fixed number of
//! items to avoid frequent malloc/free operations during audio processing.
//! 
//! Original C++ implementation converted to Rust with proper memory safety,
//! ownership semantics, and error handling.

use std::ptr::NonNull;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

/// Error types for audio memory pool operations
#[derive(Debug, Clone, PartialEq)]
pub enum AudioMemoryError {
    /// Pool is empty, no items available
    PoolEmpty,
    /// Invalid item returned to pool
    InvalidItem,
    /// Pool underflow (returning more items than were taken)
    PoolUnderflow,
    /// Pool overflow (too many items requested)
    PoolOverflow,
    /// Memory allocation failed
    AllocationFailed,
    /// Invalid pool parameters
    InvalidParameters,
}

impl std::fmt::Display for AudioMemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioMemoryError::PoolEmpty => write!(f, "Pool is empty"),
            AudioMemoryError::InvalidItem => write!(f, "Invalid item returned to pool"),
            AudioMemoryError::PoolUnderflow => write!(f, "Pool underflow"),
            AudioMemoryError::PoolOverflow => write!(f, "Pool overflow"),
            AudioMemoryError::AllocationFailed => write!(f, "Memory allocation failed"),
            AudioMemoryError::InvalidParameters => write!(f, "Invalid pool parameters"),
        }
    }
}

impl std::error::Error for AudioMemoryError {}

/// Result type for audio memory pool operations
pub type AudioMemoryResult<T> = Result<T, AudioMemoryError>;

/// A memory item in the pool
/// 
/// Represents a single allocatable unit in the memory pool.
/// In the original C++ implementation, this was a linked list node
/// with a next pointer. In Rust, we use Vec indices for safety.
#[derive(Debug)]
struct MemoryItem {
    /// The actual data storage
    data: Vec<u8>,
    /// Whether this item is currently in use
    in_use: bool,
}

impl MemoryItem {
    /// Create a new memory item with the specified size
    fn new(size: usize) -> Self {
        Self {
            data: vec![0u8; size],
            in_use: false,
        }
    }
    
    /// Get a mutable pointer to the data
    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.data.as_mut_ptr()
    }
    
    /// Get the size of this item's data
    fn len(&self) -> usize {
        self.data.len()
    }
}

/// Audio Memory Pool
/// 
/// A pre-allocated pool of fixed-size memory blocks for efficient allocation
/// and deallocation. This is particularly useful for audio processing where
/// frequent memory allocation/deallocation can cause audio dropouts.
/// 
/// Thread-safe implementation using Arc<Mutex<_>>.
#[derive(Debug)]
pub struct AudioMemoryPool {
    /// Storage for all memory items
    items: Vec<MemoryItem>,
    /// Queue of available item indices
    available_indices: VecDeque<usize>,
    /// Number of items currently checked out
    items_out: usize,
    /// Total number of items in the pool
    num_items: usize,
    /// Size of each item in bytes
    item_size: usize,
}

impl AudioMemoryPool {
    /// Create a new audio memory pool
    /// 
    /// # Arguments
    /// * `items` - Number of items to allocate in the pool
    /// * `size` - Size of each item in bytes
    /// 
    /// # Returns
    /// * `Ok(AudioMemoryPool)` on success
    /// * `Err(AudioMemoryError)` if parameters are invalid or allocation fails
    /// 
    /// # Example
    /// ```rust
    /// let pool = AudioMemoryPool::new(100, 4096)?;
    /// ```
    pub fn new(items: u32, size: u32) -> AudioMemoryResult<Self> {
        if items == 0 || size == 0 {
            return Err(AudioMemoryError::InvalidParameters);
        }
        
        let items = items as usize;
        let size = size as usize;
        
        // Pre-allocate all memory items
        let mut memory_items = Vec::with_capacity(items);
        for _ in 0..items {
            memory_items.push(MemoryItem::new(size));
        }
        
        // Initialize available indices queue
        let available_indices: VecDeque<usize> = (0..items).collect();
        
        Ok(Self {
            items: memory_items,
            available_indices,
            items_out: 0,
            num_items: items,
            item_size: size,
        })
    }
    
    /// Get an item from the pool
    /// 
    /// # Returns
    /// * `Ok(*mut u8)` - Pointer to the allocated memory block
    /// * `Err(AudioMemoryError::PoolEmpty)` if no items are available
    /// * `Err(AudioMemoryError::PoolOverflow)` if too many items are checked out
    /// 
    /// # Safety
    /// The returned pointer is valid until the corresponding item is returned
    /// to the pool via `return_item()`. The caller must not access the memory
    /// after returning it to the pool.
    /// 
    /// # Example
    /// ```rust
    /// let ptr = pool.get_item()?;
    /// // Use the memory...
    /// pool.return_item(ptr)?;
    /// ```
    pub fn get_item(&mut self) -> AudioMemoryResult<*mut u8> {
        // Check if any items are available
        let item_index = self.available_indices.pop_front()
            .ok_or(AudioMemoryError::PoolEmpty)?;
        
        // Check for overflow (should not happen with correct usage)
        if self.items_out >= self.num_items {
            // Return the index back to available queue
            self.available_indices.push_front(item_index);
            return Err(AudioMemoryError::PoolOverflow);
        }
        
        // Mark item as in use and increment counter
        self.items[item_index].in_use = true;
        self.items_out += 1;
        
        // Return pointer to the item's data
        Ok(self.items[item_index].as_mut_ptr())
    }
    
    /// Return an item to the pool
    /// 
    /// # Arguments
    /// * `data` - Pointer to the memory block to return (must have been obtained from `get_item()`)
    /// 
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(AudioMemoryError::InvalidItem)` if the pointer is not from this pool
    /// * `Err(AudioMemoryError::PoolUnderflow)` if returning more items than were taken
    /// 
    /// # Safety
    /// The caller must ensure that:
    /// - The pointer was obtained from this pool via `get_item()`
    /// - The pointer is not used after calling this function
    /// - The pointer has not already been returned to the pool
    /// 
    /// # Example
    /// ```rust
    /// let ptr = pool.get_item()?;
    /// // Use the memory...
    /// pool.return_item(ptr)?;
    /// ```
    pub fn return_item(&mut self, data: *mut u8) -> AudioMemoryResult<()> {
        // Check for underflow
        if self.items_out == 0 {
            return Err(AudioMemoryError::PoolUnderflow);
        }
        
        // Find the item that corresponds to this pointer
        let mut found_index = None;
        for (index, item) in self.items.iter_mut().enumerate() {
            if item.as_mut_ptr() == data && item.in_use {
                found_index = Some(index);
                break;
            }
        }
        
        let item_index = found_index.ok_or(AudioMemoryError::InvalidItem)?;
        
        // Mark item as available
        self.items[item_index].in_use = false;
        self.available_indices.push_back(item_index);
        self.items_out -= 1;
        
        Ok(())
    }
    
    /// Get the count of available items in the pool
    /// 
    /// # Returns
    /// The number of items currently available for allocation
    /// 
    /// # Example
    /// ```rust
    /// let available = pool.count();
    /// println!("Available items: {}", available);
    /// ```
    pub fn count(&self) -> usize {
        self.available_indices.len()
    }
    
    /// Get the number of items currently checked out
    /// 
    /// # Returns
    /// The number of items currently in use
    pub fn items_out(&self) -> usize {
        self.items_out
    }
    
    /// Get the total number of items in the pool
    /// 
    /// # Returns
    /// The total capacity of the pool
    pub fn total_items(&self) -> usize {
        self.num_items
    }
    
    /// Get the size of each item in bytes
    /// 
    /// # Returns
    /// The size of each memory block in the pool
    pub fn item_size(&self) -> usize {
        self.item_size
    }
}

impl Drop for AudioMemoryPool {
    fn drop(&mut self) {
        // In debug builds, warn if there are still items checked out
        #[cfg(debug_assertions)]
        if self.items_out > 0 {
            eprintln!("Warning: Destroying memory pool with {} items still checked out", self.items_out);
        }
    }
}

/// Thread-safe wrapper for AudioMemoryPool
/// 
/// This provides the same interface as AudioMemoryPool but with thread safety
/// using Arc<Mutex<_>>. Use this when the pool needs to be accessed from
/// multiple threads.
#[derive(Debug, Clone)]
pub struct ThreadSafeAudioMemoryPool {
    inner: Arc<Mutex<AudioMemoryPool>>,
}

impl ThreadSafeAudioMemoryPool {
    /// Create a new thread-safe audio memory pool
    /// 
    /// # Arguments
    /// * `items` - Number of items to allocate in the pool
    /// * `size` - Size of each item in bytes
    /// 
    /// # Returns
    /// * `Ok(ThreadSafeAudioMemoryPool)` on success
    /// * `Err(AudioMemoryError)` if parameters are invalid or allocation fails
    pub fn new(items: u32, size: u32) -> AudioMemoryResult<Self> {
        let pool = AudioMemoryPool::new(items, size)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(pool)),
        })
    }
    
    /// Get an item from the pool (thread-safe)
    /// 
    /// # Returns
    /// * `Ok(*mut u8)` - Pointer to the allocated memory block
    /// * `Err(AudioMemoryError)` on failure
    pub fn get_item(&self) -> AudioMemoryResult<*mut u8> {
        let mut pool = self.inner.lock().unwrap();
        pool.get_item()
    }
    
    /// Return an item to the pool (thread-safe)
    /// 
    /// # Arguments
    /// * `data` - Pointer to the memory block to return
    /// 
    /// # Returns
    /// * `Ok(())` on success
    /// * `Err(AudioMemoryError)` on failure
    pub fn return_item(&self, data: *mut u8) -> AudioMemoryResult<()> {
        let mut pool = self.inner.lock().unwrap();
        pool.return_item(data)
    }
    
    /// Get the count of available items in the pool (thread-safe)
    /// 
    /// # Returns
    /// The number of items currently available for allocation
    pub fn count(&self) -> usize {
        let pool = self.inner.lock().unwrap();
        pool.count()
    }
    
    /// Get the number of items currently checked out (thread-safe)
    /// 
    /// # Returns
    /// The number of items currently in use
    pub fn items_out(&self) -> usize {
        let pool = self.inner.lock().unwrap();
        pool.items_out()
    }
    
    /// Get the total number of items in the pool (thread-safe)
    /// 
    /// # Returns
    /// The total capacity of the pool
    pub fn total_items(&self) -> usize {
        let pool = self.inner.lock().unwrap();
        pool.total_items()
    }
    
    /// Get the size of each item in bytes (thread-safe)
    /// 
    /// # Returns
    /// The size of each memory block in the pool
    pub fn item_size(&self) -> usize {
        let pool = self.inner.lock().unwrap();
        pool.item_size()
    }
}

// C-compatible interface functions
// These maintain the same function signatures as the original C++ code
// for compatibility with existing code that might call these functions

/// Create a memory pool (C-compatible interface)
/// 
/// # Arguments
/// * `items` - Number of items to allocate
/// * `size` - Size of each item in bytes
/// 
/// # Returns
/// * Non-null pointer to AudioMemoryPool on success
/// * Null pointer on failure
/// 
/// # Safety
/// This function is unsafe as it returns a raw pointer.
/// The caller must ensure proper cleanup using `memory_pool_destroy`.
#[no_mangle]
pub unsafe extern "C" fn memory_pool_create(items: u32, size: u32) -> *mut AudioMemoryPool {
    match AudioMemoryPool::new(items, size) {
        Ok(pool) => Box::into_raw(Box::new(pool)),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Destroy a memory pool (C-compatible interface)
/// 
/// # Arguments
/// * `pool` - Pointer to the AudioMemoryPool to destroy
/// 
/// # Safety
/// This function is unsafe as it operates on raw pointers.
/// The caller must ensure the pool pointer is valid and not used after calling this function.
#[no_mangle]
pub unsafe extern "C" fn memory_pool_destroy(pool: *mut AudioMemoryPool) {
    if !pool.is_null() {
        let _ = Box::from_raw(pool);
    }
}

/// Get an item from the pool (C-compatible interface)
/// 
/// # Arguments
/// * `pool` - Pointer to the AudioMemoryPool
/// 
/// # Returns
/// * Non-null pointer to allocated memory on success
/// * Null pointer on failure
/// 
/// # Safety
/// This function is unsafe as it operates on raw pointers.
/// The caller must ensure the pool pointer is valid.
#[no_mangle]
pub unsafe extern "C" fn memory_pool_get_item(pool: *mut AudioMemoryPool) -> *mut u8 {
    if pool.is_null() {
        return std::ptr::null_mut();
    }
    
    match (*pool).get_item() {
        Ok(ptr) => ptr,
        Err(_) => std::ptr::null_mut(),
    }
}

/// Return an item to the pool (C-compatible interface)
/// 
/// # Arguments
/// * `pool` - Pointer to the AudioMemoryPool
/// * `data` - Pointer to the memory block to return
/// 
/// # Safety
/// This function is unsafe as it operates on raw pointers.
/// The caller must ensure both pointers are valid and the data pointer
/// was obtained from the specified pool.
#[no_mangle]
pub unsafe extern "C" fn memory_pool_return_item(pool: *mut AudioMemoryPool, data: *mut u8) {
    if !pool.is_null() && !data.is_null() {
        let _ = (*pool).return_item(data);
    }
}

/// Count available items in the pool (C-compatible interface)
/// 
/// # Arguments
/// * `pool` - Pointer to the AudioMemoryPool
/// 
/// # Returns
/// * Number of available items, or 0 if pool is null
/// 
/// # Safety
/// This function is unsafe as it operates on raw pointers.
/// The caller must ensure the pool pointer is valid.
#[no_mangle]
pub unsafe extern "C" fn memory_pool_count(pool: *const AudioMemoryPool) -> i32 {
    if pool.is_null() {
        return 0;
    }
    
    (*pool).count() as i32
}

// Utility functions from the original C++ file

/// Add a trailing backslash to a string if it doesn't already have one
/// 
/// # Arguments
/// * `string` - The string to modify
/// 
/// # Example
/// ```rust
/// let mut path = String::from("C:\\temp");
/// audio_add_slash(&mut path);
/// assert_eq!(path, "C:\\temp");
/// ```
pub fn audio_add_slash(string: &mut String) {
    if !string.is_empty() && !string.ends_with('\\') {
        string.push('\\');
    }
}

/// Check if a string contains path information
/// 
/// # Arguments
/// * `string` - The string to check
/// 
/// # Returns
/// `true` if the string contains path separators or drive letters, `false` otherwise
/// 
/// # Example
/// ```rust
/// assert!(audio_has_path("C:\\temp\\file.txt"));
/// assert!(audio_has_path("./relative/path"));
/// assert!(!audio_has_path("filename"));
/// ```
pub fn audio_has_path(string: &str) -> bool {
    string.contains(':') || string.contains('\\') || string.contains('/') || string.contains('.')
}

// C-compatible versions of utility functions

/// Add a trailing backslash to a C string (C-compatible interface)
/// 
/// # Arguments
/// * `string` - Null-terminated C string to modify (must have space for one additional character)
/// 
/// # Safety
/// This function is unsafe as it operates on raw C strings.
/// The caller must ensure the string is null-terminated and has space for the additional backslash.
#[no_mangle]
pub unsafe extern "C" fn audio_add_slash_c(string: *mut std::os::raw::c_char) {
    use std::ffi::{CStr, CString};
    
    if string.is_null() {
        return;
    }
    
    let c_str = CStr::from_ptr(string);
    if let Ok(rust_str) = c_str.to_str() {
        if !rust_str.is_empty() && !rust_str.ends_with('\\') {
            let len = rust_str.len();
            // Add backslash and null terminator
            *string.add(len) = b'\\' as std::os::raw::c_char;
            *string.add(len + 1) = 0;
        }
    }
}

/// Check if a C string contains path information (C-compatible interface)
/// 
/// # Arguments
/// * `string` - Null-terminated C string to check
/// 
/// # Returns
/// * 1 if the string contains path information, 0 otherwise
/// 
/// # Safety
/// This function is unsafe as it operates on raw C strings.
/// The caller must ensure the string is null-terminated.
#[no_mangle]
pub unsafe extern "C" fn audio_has_path_c(string: *const std::os::raw::c_char) -> i32 {
    use std::ffi::CStr;
    
    if string.is_null() {
        return 0;
    }
    
    let c_str = CStr::from_ptr(string);
    if let Ok(rust_str) = c_str.to_str() {
        if audio_has_path(rust_str) { 1 } else { 0 }
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_creation() {
        let pool = AudioMemoryPool::new(10, 1024).unwrap();
        assert_eq!(pool.total_items(), 10);
        assert_eq!(pool.item_size(), 1024);
        assert_eq!(pool.count(), 10);
        assert_eq!(pool.items_out(), 0);
    }

    #[test]
    fn test_pool_invalid_params() {
        assert!(AudioMemoryPool::new(0, 1024).is_err());
        assert!(AudioMemoryPool::new(10, 0).is_err());
    }

    #[test]
    fn test_get_and_return_item() {
        let mut pool = AudioMemoryPool::new(5, 256).unwrap();
        
        // Get an item
        let ptr1 = pool.get_item().unwrap();
        assert_eq!(pool.count(), 4);
        assert_eq!(pool.items_out(), 1);
        
        // Get another item
        let ptr2 = pool.get_item().unwrap();
        assert_eq!(pool.count(), 3);
        assert_eq!(pool.items_out(), 2);
        
        // Return first item
        pool.return_item(ptr1).unwrap();
        assert_eq!(pool.count(), 4);
        assert_eq!(pool.items_out(), 1);
        
        // Return second item
        pool.return_item(ptr2).unwrap();
        assert_eq!(pool.count(), 5);
        assert_eq!(pool.items_out(), 0);
    }

    #[test]
    fn test_pool_exhaustion() {
        let mut pool = AudioMemoryPool::new(2, 128).unwrap();
        
        let _ptr1 = pool.get_item().unwrap();
        let _ptr2 = pool.get_item().unwrap();
        
        // Pool should be empty now
        assert!(matches!(pool.get_item(), Err(AudioMemoryError::PoolEmpty)));
    }

    #[test]
    fn test_double_return_error() {
        let mut pool = AudioMemoryPool::new(1, 128).unwrap();
        
        let ptr = pool.get_item().unwrap();
        pool.return_item(ptr).unwrap();
        
        // Returning the same pointer again should fail
        assert!(matches!(pool.return_item(ptr), Err(AudioMemoryError::InvalidItem)));
    }

    #[test]
    fn test_thread_safe_pool() {
        let pool = ThreadSafeAudioMemoryPool::new(5, 256).unwrap();
        
        let ptr = pool.get_item().unwrap();
        assert_eq!(pool.count(), 4);
        
        pool.return_item(ptr).unwrap();
        assert_eq!(pool.count(), 5);
    }

    #[test]
    fn test_utility_functions() {
        let mut path = String::from("C:\\temp");
        audio_add_slash(&mut path);
        assert_eq!(path, "C:\\temp");
        
        // Should not add another slash
        audio_add_slash(&mut path);
        assert_eq!(path, "C:\\temp");
        
        assert!(audio_has_path("C:\\temp\\file.txt"));
        assert!(audio_has_path("./relative"));
        assert!(audio_has_path("/unix/path"));
        assert!(audio_has_path("file.txt"));
        assert!(!audio_has_path("filename"));
    }
}