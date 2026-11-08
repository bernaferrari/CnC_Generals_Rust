//! Per-session profiling memory helpers.
//!
//! Blocks own initialized bytes and borrow their session's statistics; growing
//! zero-fills new bytes. Arrays own initialized `T` values and destroy removed
//! elements on shrink, clear, or drop. Untracked arrays and strings can be
//! created without a session. These are Rust ownership APIs, not raw C++
//! allocation interfaces.
//!
//! Statistics count requested byte-block sizes and array capacity in bytes,
//! rather than allocator overhead or spare `Vec` capacity. A successful size
//! change records a free of the old nonzero size and an allocation of the new
//! nonzero size, even when existing storage is reused. Empty and zero-sized
//! element allocations contribute no events. Failed growth leaves the value
//! and its statistics unchanged.

use crate::{ProfileError, ProfileResult};
use std::alloc::Layout;
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

/// Factory and explicit operations for owned profiling memory blocks.
pub struct ProfileMemory;

impl ProfileMemory {
    pub fn alloc(session: &crate::WwDebugSession, size: usize) -> ProfileResult<ProfileMemoryBlock<'_>> {
        ProfileMemoryBlock::new(session, size)
    }

    pub fn realloc(block: &mut ProfileMemoryBlock<'_>, new_size: usize) -> ProfileResult<()> {
        block.resize(new_size)
    }

    pub fn free(block: ProfileMemoryBlock<'_>) {
        drop(block);
    }

    pub fn get_stats(session: &crate::WwDebugSession) -> &MemoryStats {
        &session.memory_stats
    }
}

/// Owned, zero-initialized bytes accounted to the borrowed session.
pub struct ProfileMemoryBlock<'a> {
    bytes: Vec<u8>,
    stats: &'a MemoryStats,
}

impl<'a> ProfileMemoryBlock<'a> {
    pub fn new(session: &'a crate::WwDebugSession, size: usize) -> ProfileResult<Self> {
        let mut block = Self { bytes: Vec::new(), stats: &session.memory_stats };
        block.resize(size)?;
        Ok(block)
    }

    pub fn size(&self) -> usize {
        self.bytes.len()
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    /// Resize, preserving the common prefix and zero-initializing any growth.
    pub fn resize(&mut self, new_size: usize) -> ProfileResult<()> {
        let old_size = self.bytes.len();
        if old_size == new_size {
            return Ok(());
        }
        if new_size > old_size {
            self.bytes.try_reserve_exact(new_size - old_size)
                .map_err(|_| ProfileError::MemoryAllocation)?;
            self.bytes.resize(new_size, 0);
        } else if new_size == 0 {
            self.bytes = Vec::new();
        } else {
            self.bytes.truncate(new_size);
        }
        if old_size != 0 {
            self.stats.record_deallocation(old_size);
        }
        if new_size != 0 {
            self.stats.record_allocation(new_size);
        }
        Ok(())
    }
}

impl Drop for ProfileMemoryBlock<'_> {
    fn drop(&mut self) {
        if !self.bytes.is_empty() {
            self.stats.record_deallocation(self.bytes.len());
        }
    }
}

/// Owned initialized elements with an optional borrowed statistics source.
pub struct ProfileArray<'a, T> {
    values: Vec<T>,
    capacity: usize,
    stats: Option<&'a MemoryStats>,
}

impl<'a, T> ProfileArray<'a, T> {
    /// Create an untracked empty array.
    pub fn new() -> Self {
        Self { values: Vec::new(), capacity: 0, stats: None }
    }

    /// Create an array whose capacity changes are tracked by `session`.
    pub fn with_capacity(session: &'a crate::WwDebugSession, capacity: usize) -> ProfileResult<Self> {
        let mut array = Self { values: Vec::new(), capacity: 0, stats: Some(&session.memory_stats) };
        array.resize(capacity)?;
        Ok(array)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Requested capacity; the underlying vector may reserve more.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Change capacity, dropping elements beyond the new capacity.
    pub fn resize(&mut self, new_capacity: usize) -> ProfileResult<()> {
        if new_capacity == self.capacity {
            return Ok(());
        }
        let new_size = Layout::array::<T>(new_capacity)
            .map_err(|_| ProfileError::MemoryAllocation)?.size();
        let old_size = self.capacity * std::mem::size_of::<T>();
        if new_capacity > self.values.capacity() {
            self.values.try_reserve_exact(new_capacity - self.values.len())
                .map_err(|_| ProfileError::MemoryAllocation)?;
        } else if new_capacity < self.capacity {
            self.values.truncate(new_capacity);
            if new_capacity == 0 {
                self.values = Vec::new();
            } else {
                self.values.shrink_to(new_capacity);
            }
        }
        if let Some(stats) = self.stats {
            if old_size != 0 {
                stats.record_deallocation(old_size);
            }
            if new_size != 0 {
                stats.record_allocation(new_size);
            }
        }
        self.capacity = new_capacity;
        Ok(())
    }

    pub fn push(&mut self, value: T) -> ProfileResult<()> {
        if self.values.len() == self.capacity {
            let capacity = if self.capacity == 0 {
                1
            } else {
                self.capacity.checked_mul(2).ok_or(ProfileError::MemoryAllocation)?
            };
            self.resize(capacity)?;
        }
        self.values.push(value);
        Ok(())
    }

    pub fn pop(&mut self) -> Option<T> {
        self.values.pop()
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.values.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.values.get_mut(index)
    }

    /// Drop all elements and release the array's capacity.
    pub fn clear(&mut self) {
        self.values.clear();
        self.values = Vec::new();
        let old_size = self.capacity * std::mem::size_of::<T>();
        if let Some(stats) = self.stats {
            if old_size != 0 {
                stats.record_deallocation(old_size);
            }
        }
        self.capacity = 0;
    }

    pub fn as_slice(&self) -> &[T] {
        &self.values
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.values
    }
}

impl<T> Drop for ProfileArray<'_, T> {
    fn drop(&mut self) {
        self.clear();
    }
}

impl<T> Default for ProfileArray<'_, T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Owned UTF-8 string with a trailing zero byte.
pub struct ProfileString {
    bytes: Vec<u8>,
}

impl ProfileString {
    pub fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    pub fn from_str(s: &str) -> ProfileResult<Self> {
        let mut string = Self::new();
        string.set_str(s)?;
        Ok(string)
    }

    /// Replace the contents, retaining the old string on allocation failure.
    pub fn set_str(&mut self, s: &str) -> ProfileResult<()> {
        let required = s.len().checked_add(1).ok_or(ProfileError::MemoryAllocation)?;
        self.bytes.try_reserve_exact(required.saturating_sub(self.bytes.len()))
            .map_err(|_| ProfileError::MemoryAllocation)?;
        self.bytes.clear();
        self.bytes.extend_from_slice(s.as_bytes());
        self.bytes.push(0);
        Ok(())
    }

    pub fn as_bytes_with_nul(&self) -> &[u8] {
        if self.bytes.is_empty() {
            &[0]
        } else {
            &self.bytes
        }
    }

    pub fn as_str(&self) -> &str {
        let bytes = &self.bytes[..self.bytes.len().saturating_sub(1)];
        std::str::from_utf8(bytes).expect("ProfileString is constructed from UTF-8")
    }

    pub fn len(&self) -> usize {
        self.bytes.len().saturating_sub(1)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
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
    use std::cell::Cell;

    #[test]
    fn block_preserves_bytes_and_accounts_for_resizes_and_drop() {
        let session = crate::WwDebugSession::new();
        let stats = ProfileMemory::get_stats(&session);
        {
            let mut block = ProfileMemory::alloc(&session, 4).unwrap();
            assert_eq!(block.as_slice(), &[0, 0, 0, 0]);
            block.as_mut_slice().copy_from_slice(&[1, 2, 3, 4]);
            ProfileMemory::realloc(&mut block, 7).unwrap();
            assert_eq!(block.as_slice(), &[1, 2, 3, 4, 0, 0, 0]);
            block.resize(2).unwrap();
            assert_eq!(block.as_slice(), &[1, 2]);
            assert_eq!(stats.get_total_allocated(), 13);
            assert_eq!(stats.get_total_freed(), 11);
            assert_eq!(stats.get_current_allocated(), 2);
            assert_eq!(stats.get_allocation_count(), 3);
        }
        assert_eq!(stats.get_total_freed(), 13);
        assert_eq!(stats.get_current_allocated(), 0);
    }

    #[test]
    fn regrowth_zeroes_discarded_bytes_and_sessions_are_independent() {
        let first = crate::WwDebugSession::new();
        let second = crate::WwDebugSession::new();
        let mut block = ProfileMemory::alloc(&first, 4).unwrap();
        block.as_mut_slice().copy_from_slice(&[1, 2, 3, 4]);
        block.resize(1).unwrap();
        block.resize(4).unwrap();
        assert_eq!(block.as_slice(), &[1, 0, 0, 0]);
        block.resize(4).unwrap();
        assert_eq!(ProfileMemory::get_stats(&first).get_allocation_count(), 3);
        assert_eq!(ProfileMemory::get_stats(&second).get_allocation_count(), 0);
    }

    #[test]
    fn empty_block_free_and_failed_growth_preserve_state() {
        let session = crate::WwDebugSession::new();
        let stats = ProfileMemory::get_stats(&session);
        let mut block = ProfileMemoryBlock::new(&session, 0).unwrap();
        assert_eq!(block.size(), 0);
        assert_eq!(stats.get_allocation_count(), 0);
        block.resize(3).unwrap();
        block.as_mut_slice().copy_from_slice(&[8, 9, 10]);
        assert!(matches!(block.resize(usize::MAX), Err(ProfileError::MemoryAllocation)));
        assert_eq!(block.as_slice(), &[8, 9, 10]);
        assert_eq!(stats.get_current_allocated(), 3);
        block.resize(0).unwrap();
        assert_eq!(stats.get_current_allocated(), 0);
        ProfileMemory::free(block);
        assert_eq!(stats.get_total_allocated(), 3);
        assert_eq!(stats.get_total_freed(), 3);
    }

    #[test]
    fn arrays_retain_values_drop_removed_elements_and_account_for_capacity() {
        struct Item<'a>(&'a Cell<usize>, i32);
        impl Drop for Item<'_> {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }

        let session = crate::WwDebugSession::new();
        let stats = ProfileMemory::get_stats(&session);
        let drops = Cell::new(0);
        let size = std::mem::size_of::<Item<'_>>();
        {
            let mut array = ProfileArray::with_capacity(&session, 3).unwrap();
            for value in 1..=3 {
                array.push(Item(&drops, value)).unwrap();
            }
            array.resize(5).unwrap();
            assert_eq!(array.as_slice().iter().map(|item| item.1).collect::<Vec<_>>(), [1, 2, 3]);
            array.get_mut(0).unwrap().1 = 9;
            array.resize(1).unwrap();
            assert_eq!(drops.get(), 2);
            assert_eq!(array.get(0).unwrap().1, 9);
            assert_eq!(stats.get_total_allocated(), 9 * size);
            assert_eq!(stats.get_total_freed(), 8 * size);
            assert_eq!(stats.get_current_allocated(), size);
            assert_eq!(stats.get_allocation_count(), 3);
            assert!(matches!(array.resize(usize::MAX), Err(ProfileError::MemoryAllocation)));
            assert_eq!(array.get(0).unwrap().1, 9);
        }
        assert_eq!(drops.get(), 3);
        assert_eq!(stats.get_total_freed(), 9 * size);
        assert_eq!(stats.get_current_allocated(), 0);
    }

    #[test]
    fn array_pop_clear_zero_capacity_and_zero_sized_elements() {
        let session = crate::WwDebugSession::new();
        let stats = ProfileMemory::get_stats(&session);
        let mut array = ProfileArray::with_capacity(&session, 0).unwrap();
        array.push(42_u32).unwrap();
        array.push(24).unwrap();
        assert_eq!(array.pop(), Some(24));
        assert_eq!(array.as_mut_slice(), &mut [42]);
        array.clear();
        assert!(array.is_empty());
        assert_eq!(array.capacity(), 0);
        assert_eq!(stats.get_current_allocated(), 0);

        let mut zst = ProfileArray::with_capacity(&session, 2).unwrap();
        zst.push(()).unwrap();
        zst.push(()).unwrap();
        zst.push(()).unwrap();
        assert_eq!(zst.len(), 3);
        zst.resize(1).unwrap();
        assert_eq!(zst.as_slice(), &[()]);
        assert_eq!(stats.get_current_allocated(), 0);
    }

    #[test]
    fn string_replacement_preserves_utf8_and_terminator() {
        let mut string = ProfileString::from_str("Hello").unwrap();
        string.set_str("雪").unwrap();
        assert_eq!(string.as_str(), "雪");
        assert_eq!(string.len(), 3);
        assert_eq!(string.as_bytes_with_nul(), "雪\0".as_bytes());
        string.set_str("").unwrap();
        assert!(string.is_empty());
        assert_eq!(string.as_bytes_with_nul(), &[0]);
    }
}
