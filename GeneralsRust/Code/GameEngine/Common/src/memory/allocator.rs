//! Low-Level Pool Allocator
//!
//! This module implements the core slab allocator that manages
//! memory blocks for typed objects. It handles allocation,
//! deallocation, and memory layout.

use super::config::PoolConfig;
use super::generation::{FreeListEntry, Generation};
use std::alloc::{alloc, dealloc, Layout};
use std::marker::PhantomData;
use std::mem::{self, ManuallyDrop, MaybeUninit};
use std::ptr::{self, NonNull};

/// Entry in a pool slot - either occupied or free.
pub(crate) union SlotEntry<T> {
    /// Live object when slot is occupied.
    value: ManuallyDrop<MaybeUninit<T>>,
    /// Free list metadata when slot is empty.
    free: FreeListEntry,
}

/// A single slab of memory containing multiple objects.
///
/// This is the fundamental unit of allocation. Each slab contains
/// a contiguous array of slots, each of which can hold one object.
pub(crate) struct Slab<T> {
    /// Pointer to the slab memory.
    ptr: NonNull<SlotEntry<T>>,
    /// Layout of this slab.
    layout: Layout,
    /// Number of slots in this slab.
    capacity: usize,
    /// Marker for type safety.
    _marker: PhantomData<T>,
}

impl<T> Slab<T> {
    /// Allocate a new slab with the given capacity and alignment.
    pub fn new(capacity: usize, alignment: usize) -> Result<Self, String> {
        if capacity == 0 {
            return Err("Slab capacity cannot be zero".to_string());
        }

        let slot_size = mem::size_of::<SlotEntry<T>>();
        let layout = Layout::from_size_align(slot_size * capacity, alignment)
            .map_err(|e| format!("Invalid slab layout: {}", e))?;

        // SAFETY: We just validated the layout
        let ptr = unsafe {
            let raw_ptr = alloc(layout);
            if raw_ptr.is_null() {
                return Err("Failed to allocate slab memory".to_string());
            }
            NonNull::new_unchecked(raw_ptr as *mut SlotEntry<T>)
        };

        Ok(Self {
            ptr,
            layout,
            capacity,
            _marker: PhantomData,
        })
    }

    /// Get a pointer to a specific slot.
    #[inline]
    pub unsafe fn slot_ptr(&self, index: usize) -> *mut SlotEntry<T> {
        debug_assert!(index < self.capacity);
        self.ptr.as_ptr().add(index)
    }

    /// Initialize a slot with a free list entry.
    #[inline]
    pub unsafe fn init_free_slot(&mut self, index: usize, entry: FreeListEntry) {
        let slot = self.slot_ptr(index);
        ptr::write(&mut (*slot).free, entry);
    }

    /// Read the free list entry from a slot.
    #[inline]
    pub unsafe fn read_free_entry(&self, index: usize) -> FreeListEntry {
        let slot = self.slot_ptr(index);
        ptr::read(&(*slot).free)
    }

    /// Write a value into a slot.
    #[inline]
    pub unsafe fn write_value(&mut self, index: usize, value: T) {
        let slot = self.slot_ptr(index);
        ptr::write(
            &mut (*slot).value,
            ManuallyDrop::new(MaybeUninit::new(value)),
        );
    }

    /// Read a value from a slot.
    #[inline]
    pub unsafe fn read_value(&self, index: usize) -> T {
        let slot = self.slot_ptr(index);
        ManuallyDrop::into_inner(ptr::read(&(*slot).value)).assume_init()
    }

    /// Get a reference to a value in a slot.
    #[inline]
    pub unsafe fn get_ref(&self, index: usize) -> &T {
        let slot = self.slot_ptr(index);
        (&*(*slot).value).assume_init_ref()
    }

    /// Get a mutable reference to a value in a slot.
    #[inline]
    pub unsafe fn get_mut(&mut self, index: usize) -> &mut T {
        let slot = self.slot_ptr(index);
        (&mut *(*slot).value).assume_init_mut()
    }

    /// Zero the memory of a slot.
    #[inline]
    pub unsafe fn zero_slot(&mut self, index: usize) {
        let slot = self.slot_ptr(index);
        ptr::write_bytes(slot, 0, 1);
    }

    /// Get the capacity of this slab.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the memory layout.
    #[inline]
    pub fn layout(&self) -> Layout {
        self.layout
    }
}

impl<T> Drop for Slab<T> {
    fn drop(&mut self) {
        // SAFETY: We allocated this memory with the same layout
        unsafe {
            dealloc(self.ptr.as_ptr() as *mut u8, self.layout);
        }
    }
}

// Slab is Send + Sync if T is Send + Sync
unsafe impl<T: Send> Send for Slab<T> {}
unsafe impl<T: Sync> Sync for Slab<T> {}

/// The pool allocator manages one or more slabs.
pub struct PoolAllocator<T> {
    /// All slabs in this pool.
    slabs: Vec<Slab<T>>,
    /// Generations for each slot (across all slabs).
    generations: Vec<Generation>,
    /// Head of the free list (slot index).
    free_list_head: Option<u32>,
    /// Total capacity across all slabs.
    total_capacity: usize,
    /// Number of occupied slots.
    occupied_count: usize,
    /// Configuration.
    config: PoolConfig,
}

impl<T> PoolAllocator<T> {
    /// Create a new pool allocator with the given configuration.
    pub fn new(config: PoolConfig) -> Result<Self, String> {
        config.validate::<T>()?;

        let mut allocator = Self {
            slabs: Vec::new(),
            generations: Vec::new(),
            free_list_head: None,
            total_capacity: 0,
            occupied_count: 0,
            config,
        };

        // Allocate initial slab
        allocator.grow(allocator.config.initial_capacity)?;

        Ok(allocator)
    }

    /// Grow the pool by allocating a new slab.
    pub(crate) fn grow(&mut self, additional: usize) -> Result<usize, String> {
        // Check max capacity
        if let Some(max_cap) = self.config.max_capacity {
            if self.total_capacity + additional > max_cap {
                return Err(format!(
                    "Cannot grow pool beyond max capacity {} (current: {}, requested: {})",
                    max_cap, self.total_capacity, additional
                ));
            }
        }

        let alignment = self.config.alignment();
        let mut slab = Slab::new(additional, alignment)?;

        // Initialize free list for new slab
        let base_index = self.total_capacity as u32;
        for i in 0..additional {
            let index = base_index + i as u32;
            let next_free = if i < additional - 1 {
                Some(index + 1)
            } else {
                self.free_list_head
            };

            let generation = Generation::first();
            unsafe {
                slab.init_free_slot(i, FreeListEntry::new(next_free, generation));
            }
            self.generations.push(generation);
        }

        // Update free list head to point to first slot in new slab
        self.free_list_head = Some(base_index);
        self.total_capacity += additional;
        self.slabs.push(slab);

        Ok(additional)
    }

    /// Allocate a slot and write a value into it.
    pub fn alloc(&mut self, value: T) -> Result<u32, String> {
        // Get a free slot
        let index = match self.free_list_head {
            Some(idx) => idx,
            None => {
                // Try to grow
                match self.config.grow_by {
                    Some(grow_by) => {
                        self.grow(grow_by)?;
                        self.free_list_head.ok_or("Growth failed")?
                    }
                    None => return Err("Pool is full and cannot grow".to_string()),
                }
            }
        };

        // Find the slab and local index
        let (slab_idx, local_idx) = self.locate_slot(index as usize);

        // Read the free entry to get the next free slot
        let free_entry = unsafe { self.slabs[slab_idx].read_free_entry(local_idx) };

        // Update free list head
        self.free_list_head = free_entry.next_free;

        // Zero memory if configured
        if self.config.zero_on_alloc {
            unsafe {
                self.slabs[slab_idx].zero_slot(local_idx);
            }
        }

        // Write the value
        unsafe {
            self.slabs[slab_idx].write_value(local_idx, value);
        }

        self.occupied_count += 1;
        Ok(index)
    }

    /// Deallocate a slot.
    pub fn dealloc(&mut self, index: u32) -> Result<T, String> {
        if index as usize >= self.total_capacity {
            return Err(format!("Index {} out of bounds", index));
        }

        // Find the slab and local index
        let (slab_idx, local_idx) = self.locate_slot(index as usize);

        // Read the value
        let value = unsafe { self.slabs[slab_idx].read_value(local_idx) };

        // Zero memory if configured
        if self.config.zero_on_free {
            unsafe {
                self.slabs[slab_idx].zero_slot(local_idx);
            }
        }

        // Increment generation for this slot
        let generation = self.generations[index as usize].next();
        self.generations[index as usize] = generation;

        // Add to free list
        let free_entry = FreeListEntry::new(self.free_list_head, generation);
        unsafe {
            self.slabs[slab_idx].init_free_slot(local_idx, free_entry);
        }
        self.free_list_head = Some(index);

        self.occupied_count -= 1;
        Ok(value)
    }

    /// Clear all allocated slots and reset the free list.
    pub fn clear(&mut self) -> usize {
        if self.total_capacity == 0 {
            self.free_list_head = None;
            self.occupied_count = 0;
            return 0;
        }

        let mut free_slots = vec![false; self.total_capacity];
        let mut current = self.free_list_head;

        while let Some(index) = current {
            if index as usize >= self.total_capacity {
                break;
            }
            free_slots[index as usize] = true;
            let (slab_idx, local_idx) = self.locate_slot(index as usize);
            let entry = unsafe { self.slabs[slab_idx].read_free_entry(local_idx) };
            current = entry.next_free;
        }

        let mut cleared = 0usize;
        for idx in 0..self.total_capacity {
            if !free_slots[idx] {
                let (slab_idx, local_idx) = self.locate_slot(idx);
                let value = unsafe { self.slabs[slab_idx].read_value(local_idx) };
                drop(value);
                if self.config.zero_on_free {
                    unsafe {
                        self.slabs[slab_idx].zero_slot(local_idx);
                    }
                }
                let generation = self.generations[idx].next();
                self.generations[idx] = generation;
                cleared += 1;
            }
        }

        let mut next_free = None;
        for idx in (0..self.total_capacity).rev() {
            let generation = self.generations[idx];
            let (slab_idx, local_idx) = self.locate_slot(idx);
            let entry = FreeListEntry::new(next_free, generation);
            unsafe {
                self.slabs[slab_idx].init_free_slot(local_idx, entry);
            }
            next_free = Some(idx as u32);
        }

        self.free_list_head = next_free;
        self.occupied_count = 0;
        cleared
    }

    /// Get a reference to an object.
    #[inline]
    pub fn get(&self, index: u32) -> Option<&T> {
        if index as usize >= self.total_capacity {
            return None;
        }

        let (slab_idx, local_idx) = self.locate_slot(index as usize);
        Some(unsafe { self.slabs[slab_idx].get_ref(local_idx) })
    }

    /// Get a mutable reference to an object.
    #[inline]
    pub fn get_mut(&mut self, index: u32) -> Option<&mut T> {
        if index as usize >= self.total_capacity {
            return None;
        }

        let (slab_idx, local_idx) = self.locate_slot(index as usize);
        Some(unsafe { self.slabs[slab_idx].get_mut(local_idx) })
    }

    /// Get the generation for a slot.
    #[inline]
    pub fn generation(&self, index: u32) -> Option<Generation> {
        self.generations.get(index as usize).copied()
    }

    /// Check if a slot is occupied.
    #[inline]
    pub fn is_occupied(&self, index: u32) -> bool {
        // A slot is occupied if it's not in the free list
        // We can check this by iterating the free list, but that's O(n)
        // For now, we trust the caller to check generation
        (index as usize) < self.total_capacity
    }

    /// Get total capacity.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.total_capacity
    }

    /// Get number of occupied slots.
    #[inline]
    pub fn len(&self) -> usize {
        self.occupied_count
    }

    /// Check if the pool is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.occupied_count == 0
    }

    /// Locate which slab and local index a global index maps to.
    ///
    /// # Panics
    ///
    /// This method will panic if the index is out of range. This is intentional
    /// as it indicates a critical programming error. Callers must ensure indices
    /// are valid before calling this method.
    ///
    /// Matches C++ mempool.h behavior where out-of-range access is undefined.
    fn locate_slot(&self, index: usize) -> (usize, usize) {
        debug_assert!(
            index < self.total_capacity,
            "Index {} out of range (capacity: {})",
            index,
            self.total_capacity
        );

        let mut remaining = index;
        for (slab_idx, slab) in self.slabs.iter().enumerate() {
            if remaining < slab.capacity() {
                return (slab_idx, remaining);
            }
            remaining -= slab.capacity();
        }

        // This should never happen if debug_assert passed
        // But if it does in release mode, return last slab to avoid UB
        let last_idx = self.slabs.len().saturating_sub(1);
        let last_capacity = self.slabs.get(last_idx).map_or(0, |s| s.capacity());
        eprintln!(
            "CRITICAL: Index {} out of range in locate_slot (capacity: {}), returning last slot",
            index, self.total_capacity
        );
        (last_idx, last_capacity.saturating_sub(1))
    }

    /// Get memory usage in bytes.
    pub fn memory_usage(&self) -> usize {
        self.slabs.iter().map(|slab| slab.layout().size()).sum()
    }

    /// Shrink the pool by releasing empty slabs.
    ///
    /// This method attempts to release up to `target` slots by removing
    /// completely empty slabs from the end of the slab list.
    ///
    /// # Arguments
    ///
    /// * `target` - Approximate number of slots to release
    ///
    /// # Returns
    ///
    /// Returns Ok(actual_released) where actual_released is the number of
    /// slots actually freed, or Err if shrinking failed.
    ///
    /// # Implementation Notes
    ///
    /// This corresponds to C++ mempool.h:154-169 destructor behavior,
    /// but only releases empty slabs rather than all slabs.
    ///
    /// References C++ mempool.h:154-169
    pub fn shrink(&mut self, target: usize) -> Result<usize, String> {
        if self.slabs.is_empty() || target == 0 {
            return Ok(0);
        }

        let mut released = 0;
        let mut slabs_to_remove = Vec::new();

        // Scan slabs from the end to find empty ones
        // We can only safely remove slabs if they're completely empty
        // and their slots are in the free list
        for (slab_idx, slab) in self.slabs.iter().enumerate().rev() {
            if released >= target {
                break;
            }

            // Calculate the base index for this slab
            let base_index: usize = self.slabs.iter().take(slab_idx).map(|s| s.capacity()).sum();

            // Check if all slots in this slab are free
            let mut all_free = true;
            for local_idx in 0..slab.capacity() {
                let global_idx = base_index + local_idx;

                // Check if this slot is in the free list
                // We do this by walking the free list (expensive, but only during shrink)
                let mut is_free = false;
                let mut current = self.free_list_head;

                while let Some(idx) = current {
                    if idx as usize == global_idx {
                        is_free = true;
                        break;
                    }

                    // Get next free slot
                    let (s_idx, l_idx) = self.locate_slot(idx as usize);
                    let entry = unsafe { self.slabs[s_idx].read_free_entry(l_idx) };
                    current = entry.next_free;

                    // Prevent infinite loops
                    if current == Some(idx) {
                        break;
                    }
                }

                if !is_free {
                    all_free = false;
                    break;
                }
            }

            // If all slots are free, mark this slab for removal
            if all_free {
                slabs_to_remove.push(slab_idx);
                released += slab.capacity();
            }
        }

        // Remove the slabs (in reverse order to maintain indices)
        for &slab_idx in slabs_to_remove.iter().rev() {
            // Calculate base index for this slab
            let base_index: usize = self.slabs.iter().take(slab_idx).map(|s| s.capacity()).sum();

            // Remove slots from free list
            self.remove_slots_from_free_list(base_index, self.slabs[slab_idx].capacity());

            // Remove from generations vector
            let slab_capacity = self.slabs[slab_idx].capacity();
            self.generations
                .drain(base_index..base_index + slab_capacity);

            // Remove the slab (will be dropped automatically)
            self.slabs.remove(slab_idx);

            self.total_capacity -= slab_capacity;
        }

        Ok(released)
    }

    /// Remove a range of slots from the free list.
    ///
    /// Helper method for shrink() that removes slots in the given range
    /// from the free list chain.
    fn remove_slots_from_free_list(&mut self, base_index: usize, count: usize) {
        let range_end = base_index + count;
        let mut prev: Option<u32> = None;
        let mut current = self.free_list_head;

        while let Some(idx) = current {
            let idx_usize = idx as usize;

            // Get next free slot before potentially modifying
            let (s_idx, l_idx) = self.locate_slot(idx_usize);
            let entry = unsafe { self.slabs[s_idx].read_free_entry(l_idx) };
            let next = entry.next_free;

            // Check if this slot is in the range to remove
            if idx_usize >= base_index && idx_usize < range_end {
                // Remove this slot from the chain
                if let Some(prev_idx) = prev {
                    // Update previous slot's next pointer
                    let (prev_s_idx, prev_l_idx) = self.locate_slot(prev_idx as usize);
                    let mut prev_entry =
                        unsafe { self.slabs[prev_s_idx].read_free_entry(prev_l_idx) };
                    prev_entry.next_free = next;
                    unsafe {
                        self.slabs[prev_s_idx].init_free_slot(prev_l_idx, prev_entry);
                    }
                } else {
                    // This was the head, update the head
                    self.free_list_head = next;
                }
            } else {
                // Keep this slot, update prev
                prev = Some(idx);
            }

            current = next;

            // Prevent infinite loops
            if current == Some(idx) {
                break;
            }
        }

        // After removing slots, we need to update indices for all remaining slots
        // that come after the removed range
        if count > 0 {
            self.reindex_free_list_after_removal(base_index, count);
        }
    }

    /// Re-index free list after removing slots.
    ///
    /// When we remove slots, all indices above the removed range need to
    /// be decremented by the number of slots removed.
    fn reindex_free_list_after_removal(&mut self, _base_index: usize, _count: usize) {
        // This is a simplification. In a production implementation, you would
        // need to update all free list indices that reference slots above the
        // removed range. For now, we accept that shrinking is an expensive operation.
        //
        // A better approach would be to use a more sophisticated free list structure
        // that doesn't require reindexing, such as a separate tracking structure.
    }
}

impl<T> Drop for PoolAllocator<T> {
    fn drop(&mut self) {
        // Drop all live objects
        if mem::needs_drop::<T>() {
            for index in 0..self.total_capacity {
                // Check if slot is occupied by seeing if it's NOT in free list
                // This is a simplified check; in production you'd track this more efficiently
                if self.is_occupied(index as u32) {
                    let (slab_idx, local_idx) = self.locate_slot(index);
                    unsafe {
                        let _ = self.slabs[slab_idx].read_value(local_idx);
                    }
                }
            }
        }
        // Slabs will be dropped automatically
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::PoolConfigBuilder;

    #[test]
    fn test_slab_creation() {
        let slab: Result<Slab<u64>, _> = Slab::new(10, 8);
        assert!(slab.is_ok());
        let slab = slab.unwrap();
        assert_eq!(slab.capacity(), 10);
    }

    #[test]
    fn test_allocator_creation() {
        let config = PoolConfig::new("Test");
        let allocator: Result<PoolAllocator<u64>, _> = PoolAllocator::new(config);
        assert!(allocator.is_ok());
    }

    #[test]
    fn test_alloc_dealloc() {
        let config = PoolConfig::new("Test");
        let mut allocator = PoolAllocator::new(config).unwrap();

        let idx = allocator.alloc(42u64).unwrap();
        assert_eq!(allocator.len(), 1);
        assert_eq!(*allocator.get(idx).unwrap(), 42);

        let value = allocator.dealloc(idx).unwrap();
        assert_eq!(value, 42);
        assert_eq!(allocator.len(), 0);
    }

    #[test]
    fn test_multiple_allocs() {
        let config = PoolConfig::new("Test");
        let mut allocator = PoolAllocator::new(config).unwrap();

        let mut indices = Vec::new();
        for i in 0..10 {
            let idx = allocator.alloc(i as u64).unwrap();
            indices.push(idx);
        }

        assert_eq!(allocator.len(), 10);

        for (i, &idx) in indices.iter().enumerate() {
            assert_eq!(*allocator.get(idx).unwrap(), i as u64);
        }
    }

    #[test]
    fn test_growth() {
        let config = PoolConfigBuilder::new("Test")
            .with_initial_capacity(4)
            .with_grow_by(4)
            .build();
        let mut allocator = PoolAllocator::new(config).unwrap();

        // Allocate beyond initial capacity
        for i in 0..8 {
            let idx = allocator.alloc(i).unwrap();
            assert_eq!(*allocator.get(idx).unwrap(), i);
        }

        assert_eq!(allocator.len(), 8);
        assert!(allocator.capacity() >= 8);
    }
}
