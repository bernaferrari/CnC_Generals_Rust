//! Low-Level Pool Allocator
//!
//! This module implements the core slab allocator that manages
//! memory blocks for typed objects. It handles allocation,
//! deallocation, and memory layout.

use super::config::PoolConfig;
use super::generation::{FreeListEntry, Generation};
use std::alloc::{Layout, alloc, dealloc};
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
    /// Byte stride between slots. At least `size_of::<SlotEntry<T>>()`,
    /// padded up to the slab alignment so every slot is well-aligned.
    slot_stride: usize,
    /// Marker for type safety.
    _marker: PhantomData<T>,
}

impl<T> Slab<T> {
    /// Alignment and padded stride for `SlotEntry<T>` at `requested` align.
    ///
    /// Low-alignment `T` (e.g. `u8`) must still host `FreeListEntry` and
    /// honour a caller-requested cache-line align. `ptr.add(i)` is not used
    /// for indexing: it would step by `size_of` and skip the pad.
    pub(crate) fn slot_layout(requested: usize) -> Result<(usize, usize), String> {
        let align = requested.max(mem::align_of::<SlotEntry<T>>()).max(1);
        if !align.is_power_of_two() {
            return Err(format!("Slab alignment {align} is not a power of 2"));
        }
        let raw_size = mem::size_of::<SlotEntry<T>>().max(1);
        let stride = raw_size
            .checked_add(align - 1)
            .map(|rounded| rounded & !(align - 1))
            .ok_or_else(|| format!("Slab stride overflows for alignment {align}"))?;
        Ok((align, stride))
    }

    /// Validate the complete allocation layout without allocating.
    fn checked_layout(capacity: usize, alignment: usize) -> Result<(usize, usize, Layout), String> {
        if capacity == 0 {
            return Err("Slab capacity cannot be zero".to_string());
        }
        let (align, slot_stride) = Self::slot_layout(alignment)?;
        let size = slot_stride
            .checked_mul(capacity)
            .ok_or_else(|| format!("Slab size overflows for capacity {capacity}"))?;
        let layout = Layout::from_size_align(size, align)
            .map_err(|e| format!("Invalid slab layout: {e}"))?;
        Ok((align, slot_stride, layout))
    }

    /// Allocate a new slab with the given capacity and alignment.
    pub fn new(capacity: usize, alignment: usize) -> Result<Self, String> {
        let (align, slot_stride, layout) = Self::checked_layout(capacity, alignment)?;

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
            slot_stride,
            _marker: PhantomData,
        })
    }

    /// Get a pointer to a specific slot.
    #[inline]
    // SAFETY: caller contract below — index < capacity, so offset
    // index*slot_stride stays inside this slab allocation.
    pub unsafe fn slot_ptr(&self, index: usize) -> *mut SlotEntry<T> {
        debug_assert!(index < self.capacity);
        (self.ptr.as_ptr() as *mut u8)
            .add(index.saturating_mul(self.slot_stride))
            .cast()
    }

    /// Initialize a slot with a free list entry.
    #[inline]
    // SAFETY: caller contract below — in bounds and slot currently
    // free, so the `free` union arm is the initialized one.
    pub unsafe fn init_free_slot(&mut self, index: usize, entry: FreeListEntry) {
        let slot = self.slot_ptr(index);
        ptr::write(&mut (*slot).free, entry);
    }

    /// Read the free list entry from a slot.
    #[inline]
    // SAFETY: caller contract below — in bounds and slot currently
    // free, so reading the `free` arm is initialized.
    pub unsafe fn read_free_entry(&self, index: usize) -> FreeListEntry {
        let slot = self.slot_ptr(index);
        ptr::read(&(*slot).free)
    }

    /// Write a value into a slot.
    #[inline]
    // SAFETY: caller contract below — in bounds and slot vacant;
    // writing over a live T would leak its drop, which alloc avoids
    pub unsafe fn write_value(&mut self, index: usize, value: T) {
        let slot = self.slot_ptr(index);
        ptr::write(
            &mut (*slot).value,
            ManuallyDrop::new(MaybeUninit::new(value)),
        );
    }

    /// Read a value from a slot.
    #[inline]
    // SAFETY: caller contract below — in bounds and slot holds a
    // live, fully-initialized T.
    pub unsafe fn read_value(&self, index: usize) -> T {
        let slot = self.slot_ptr(index);
        ManuallyDrop::into_inner(ptr::read(&(*slot).value)).assume_init()
    }

    /// Get a reference to a value in a slot.
    #[inline]
    // SAFETY: caller contract below — in bounds and occupied;
    // &self proves no outstanding exclusive borrow of this slot.
    pub unsafe fn get_ref(&self, index: usize) -> &T {
        let slot = self.slot_ptr(index);
        (&*(*slot).value).assume_init_ref()
    }

    /// Get a mutable reference to a value in a slot.
    #[inline]
    // SAFETY: caller contract below — in bounds and occupied; the
    // unique &mut self makes the returned borrow exclusive.
    pub unsafe fn get_mut(&mut self, index: usize) -> &mut T {
        let slot = self.slot_ptr(index);
        (&mut *(*slot).value).assume_init_mut()
    }

    /// Zero the memory of a slot.
    #[inline]
    // SAFETY: caller contract below — in bounds; zeroing raw union
    // bytes is valid while the slot is unoccupied.
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
// SAFETY: Slab owns its raw allocation and exposes memory only through
// &self/&mut self accessors, so it moves exactly like its T values —
// Send iff T: Send.
unsafe impl<T: Send> Send for Slab<T> {}
// SAFETY: shared access reads through &self only, which is sound iff
// T: Sync for the same reason as the Send impl above.
unsafe impl<T: Sync> Sync for Slab<T> {}

/// The pool allocator manages one or more slabs.
pub struct PoolAllocator<T> {
    /// All slabs in this pool.
    slabs: Vec<Slab<T>>,
    /// Generation history by slot index, retained past shrink so regrowing a
    /// slot cannot revive handles issued before that slot was freed.
    generations: Vec<Generation>,
    /// Explicit initialization state for each slot. The free-list union arm
    /// cannot prove that a `T` is initialized.
    occupied: Vec<bool>,
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
            occupied: Vec::new(),
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
        if additional == 0 {
            return Err("Slab capacity cannot be zero".to_string());
        }
        let alignment = self.config.alignment();
        // Do all fallible arithmetic before allocating the slab or changing
        // pool state. The capacity is an exclusive bound, so it may contain
        // u32::MAX as the final valid slot index.
        let new_capacity = self.total_capacity.checked_add(additional).ok_or_else(|| {
            format!(
                "Pool capacity overflows (current: {}, requested: {additional})",
                self.total_capacity
            )
        })?;
        let max_indexable_capacity = (u32::MAX as usize).checked_add(1).unwrap_or(usize::MAX);
        if new_capacity > max_indexable_capacity {
            return Err(format!(
                "Pool capacity {new_capacity} exceeds u32 slot index range"
            ));
        }
        let base_index = u32::try_from(self.total_capacity)
            .map_err(|_| format!("Pool base index {} exceeds u32", self.total_capacity))?;
        let last_index = u32::try_from(new_capacity - 1)
            .map_err(|_| format!("Last pool index {} exceeds u32", new_capacity - 1))?;
        if base_index > last_index {
            return Err("Pool growth produced an empty or reversed slot range".to_string());
        }
        if let Some(max_cap) = self.config.max_capacity {
            if new_capacity > max_cap {
                return Err(format!(
                    "Cannot grow pool beyond max capacity {} (current: {}, requested: {})",
                    max_cap, self.total_capacity, additional
                ));
            }
        }
        // Validate stride × capacity even before any allocation. This is
        // intentionally separate from Slab::new so hostile capacities fail
        // without invoking the allocator.
        Slab::<T>::checked_layout(additional, alignment)?;
        let generation_reserve = new_capacity.saturating_sub(self.generations.len());
        let occupied_reserve = new_capacity.saturating_sub(self.occupied.len());
        self.slabs
            .try_reserve(1)
            .map_err(|e| format!("Cannot reserve slab metadata: {e}"))?;
        self.generations
            .try_reserve(generation_reserve)
            .map_err(|e| format!("Cannot reserve generation metadata: {e}"))?;
        self.occupied
            .try_reserve(occupied_reserve)
            .map_err(|e| format!("Cannot reserve occupancy metadata: {e}"))?;

        let mut slab = Slab::new(additional, alignment)?;

        // Initialize free list for new slab. Preflight above proved every
        // `base_index + i` and the final successor fit in u32.
        for i in 0..additional {
            let index = base_index + i as u32;
            let next_free = if i < additional - 1 {
                Some(index + 1)
            } else {
                self.free_list_head
            };

            let generation = self
                .generations
                .get(index as usize)
                .copied()
                .unwrap_or_else(Generation::first);
            if self.generations.len() <= index as usize {
                self.generations.push(generation);
            }
            // SAFETY: slot i of the brand-new slab is vacant; writing the
            // free-list entry initializes its `free` arm in bounds.
            unsafe {
                slab.init_free_slot(i, FreeListEntry::new(next_free, generation));
            }
            self.occupied.push(false);
        }

        // Update free list head to point to first slot in new slab
        self.free_list_head = Some(base_index);
        self.total_capacity = new_capacity;
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
        // SAFETY: index came off free_list_head, so the slot is free and
        // its `free` arm is initialized; locate_slot mapped it in bounds.
        let free_entry = unsafe { self.slabs[slab_idx].read_free_entry(local_idx) };

        // Update free list head
        self.free_list_head = free_entry.next_free;

        // Zero memory if configured
        if self.config.zero_on_alloc {
            // SAFETY: local_idx in bounds; slot still vacant pre-write.
            unsafe {
                self.slabs[slab_idx].zero_slot(local_idx);
            }
        }

        // Write the value
        // SAFETY: the slot was just dequeued from the free list, so
        // writing the value arm cannot leak a prior live object.
        unsafe {
            self.slabs[slab_idx].write_value(local_idx, value);
        }

        self.occupied[index as usize] = true;
        self.occupied_count += 1;
        Ok(index)
    }

    /// Deallocate a slot.
    pub fn dealloc(&mut self, index: u32) -> Result<T, String> {
        if index as usize >= self.total_capacity {
            return Err(format!("Index {} out of bounds", index));
        }
        if !self.occupied[index as usize] {
            return Err(format!("Index {} is not occupied", index));
        }

        // Find the slab and local index
        let (slab_idx, local_idx) = self.locate_slot(index as usize);

        // Read the value
        // SAFETY: bounds checked above; callers only hand out indices
        // returned by alloc, so the value arm holds an initialized T.
        let value = unsafe { self.slabs[slab_idx].read_value(local_idx) };
        self.occupied[index as usize] = false;

        // Zero memory if configured
        if self.config.zero_on_free {
            // SAFETY: in-bounds raw byte scrub while the slot transitions
            // to vacant.
            unsafe {
                self.slabs[slab_idx].zero_slot(local_idx);
            }
        }

        // Increment generation for this slot
        let generation = self.generations[index as usize].next();
        self.generations[index as usize] = generation;

        // Add to free list
        let free_entry = FreeListEntry::new(self.free_list_head, generation);
        // SAFETY: the slot just became vacant; rewriting its free arm
        // re-links it into the list at head position, in bounds.
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

        let mut cleared = 0usize;
        for idx in 0..self.total_capacity {
            if self.occupied[idx] {
                // `dealloc` commits the slot's free-list entry, generation,
                // and occupancy before running user drop code. If `T::drop`
                // unwinds, the pool therefore still contains valid slots.
                let value = self
                    .dealloc(idx as u32)
                    .expect("occupied slot must deallocate");
                cleared += 1;
                drop(value);
            }
        }

        let mut next_free = None;
        for idx in (0..self.total_capacity).rev() {
            let generation = self.generations[idx];
            let (slab_idx, local_idx) = self.locate_slot(idx);
            let entry = FreeListEntry::new(next_free, generation);
            // SAFETY: every slot is vacant after the drop loop; rebuilding
            // each free arm in bounds reconstructs the whole-list chain.
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
        if !self.occupied[index as usize] {
            return None;
        }

        let (slab_idx, local_idx) = self.locate_slot(index as usize);
        // SAFETY: bounds-checked above; &self proves aliasing is shared-only.
        Some(unsafe { self.slabs[slab_idx].get_ref(local_idx) })
    }

    /// Get a mutable reference to an object.
    #[inline]
    pub fn get_mut(&mut self, index: u32) -> Option<&mut T> {
        if index as usize >= self.total_capacity {
            return None;
        }
        if !self.occupied[index as usize] {
            return None;
        }

        let (slab_idx, local_idx) = self.locate_slot(index as usize);
        // SAFETY: bounds-checked above; unique &mut self makes the result
        // exclusive.
        Some(unsafe { self.slabs[slab_idx].get_mut(local_idx) })
    }

    /// Get the generation for a slot.
    #[inline]
    pub fn generation(&self, index: u32) -> Option<Generation> {
        ((index as usize) < self.total_capacity)
            .then(|| self.generations.get(index as usize).copied())
            .flatten()
    }

    /// Check if a slot is occupied.
    #[inline]
    pub fn is_occupied(&self, index: u32) -> bool {
        self.occupied.get(index as usize).copied().unwrap_or(false)
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
        while released < target {
            let Some(slab) = self.slabs.last() else {
                break;
            };
            let slab_capacity = slab.capacity();
            let base_index = self.total_capacity - slab_capacity;

            // Only remove trailing slabs. Removing an interior slab would
            // renumber live slots and invalidate indices already handed out.
            if self.occupied[base_index..].iter().any(|occupied| *occupied) {
                break;
            }

            self.remove_slots_from_free_list(base_index, slab_capacity);
            self.occupied.drain(base_index..self.total_capacity);
            self.slabs.pop();
            self.total_capacity -= slab_capacity;
            released += slab_capacity;
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
            // SAFETY: traversal visits only current free-list members whose
            // `free` arm is initialized.
            let entry = unsafe { self.slabs[s_idx].read_free_entry(l_idx) };
            let next = entry.next_free;

            // Check if this slot is in the range to remove
            if idx_usize >= base_index && idx_usize < range_end {
                // Remove this slot from the chain
                if let Some(prev_idx) = prev {
                    // Update previous slot's next pointer
                    let (prev_s_idx, prev_l_idx) = self.locate_slot(prev_idx as usize);
                    let mut prev_entry =
                    // SAFETY: prev slot is a free-list member; its `free` arm
                    // is initialized and in bounds.
                        unsafe { self.slabs[prev_s_idx].read_free_entry(prev_l_idx) };
                    prev_entry.next_free = next;
                    // SAFETY: prev slot is vacant, so overwriting its free arm
                    // is initialized and in bounds.
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
                // Explicit metadata distinguishes live values from free-list
                // union payloads without reading either arm speculatively.
                if self.occupied[index] {
                    let (slab_idx, local_idx) = self.locate_slot(index);
                    // SAFETY: the occupancy bit is set only after writing a
                    // fully initialized T and cleared on every removal path.
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
    fn slab_layout_rejects_stride_and_total_size_overflow_without_allocating() {
        let huge_alignment = 1usize << (usize::BITS - 1);
        assert!(Slab::<u8>::checked_layout(1, huge_alignment).is_err());

        assert!(Slab::<u8>::checked_layout(usize::MAX, 1).is_err());
    }

    #[test]
    fn grow_rejects_index_and_capacity_overflow_without_losing_existing_slots() {
        let config = PoolConfigBuilder::new("Test")
            .with_initial_capacity(1)
            .with_grow_by(1)
            .build();
        let mut allocator = PoolAllocator::new(config).unwrap();
        let live = allocator.alloc(42_u64).unwrap();

        // With one existing slot, adding 2^32 slots would require a slot
        // index beyond u32::MAX. This is representable as usize on 64-bit
        // targets and checked out as `None` on 32-bit targets.
        if let Some(additional) = (u32::MAX as usize).checked_add(1) {
            assert!(allocator.grow(additional).is_err());
        }
        assert!(allocator.grow(usize::MAX).is_err());

        assert_eq!(allocator.capacity(), 1);
        assert_eq!(allocator.len(), 1);
        assert_eq!(allocator.get(live), Some(&42));
        assert_eq!(allocator.dealloc(live).unwrap(), 42);
        assert_eq!(allocator.alloc(7).unwrap(), live);
    }

    #[test]
    fn max_capacity_failure_keeps_pool_usable() {
        let config = PoolConfigBuilder::new("Test")
            .with_initial_capacity(1)
            .with_max_capacity(1)
            .fixed_size()
            .build();
        let mut allocator = PoolAllocator::new(config).unwrap();
        let live = allocator.alloc(11_u64).unwrap();

        assert!(allocator.grow(1).is_err());
        assert_eq!(allocator.capacity(), 1);
        assert_eq!(allocator.get(live), Some(&11));
        assert_eq!(
            allocator.alloc(12),
            Err("Pool is full and cannot grow".to_string())
        );
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
    fn vacant_and_freed_slots_reject_reads_and_double_free() {
        let config = PoolConfigBuilder::new("Test")
            .with_initial_capacity(2)
            .fixed_size()
            .build();
        let mut allocator = PoolAllocator::<String>::new(config).unwrap();

        assert!(!allocator.is_occupied(0));
        assert!(allocator.get(0).is_none());
        assert!(allocator.get_mut(0).is_none());
        assert!(allocator.dealloc(0).is_err());

        let index = allocator.alloc("live".to_string()).unwrap();
        assert!(allocator.is_occupied(index));
        assert_eq!(allocator.dealloc(index).unwrap(), "live");
        assert!(!allocator.is_occupied(index));
        assert!(allocator.get(index).is_none());
        assert!(allocator.get_mut(index).is_none());
        assert!(allocator.dealloc(index).is_err());

        let reused = allocator.alloc("reused".to_string()).unwrap();
        assert_eq!(reused, index);
        assert_eq!(allocator.get(reused).map(String::as_str), Some("reused"));
    }

    #[test]
    fn shrink_only_releases_trailing_free_slabs_without_renumbering_live_slots() {
        let config = PoolConfigBuilder::new("Test")
            .with_initial_capacity(2)
            .with_grow_by(2)
            .build();
        let mut allocator = PoolAllocator::new(config).unwrap();
        let first = allocator.alloc(10_u64).unwrap();
        let second = allocator.alloc(20_u64).unwrap();
        let third = allocator.alloc(30_u64).unwrap();
        assert_eq!((first, second, third), (0, 1, 2));
        let stale_generation = allocator.generation(third).unwrap();
        assert_eq!(allocator.dealloc(third).unwrap(), 30);
        let free_generation = allocator.generation(third).unwrap();
        assert_ne!(stale_generation, free_generation);

        assert_eq!(allocator.shrink(1).unwrap(), 2);
        assert_eq!(allocator.capacity(), 2);
        assert_eq!(allocator.generation(third), None);
        assert_eq!(allocator.get(first), Some(&10));
        assert_eq!(allocator.get(second), Some(&20));
        assert_eq!(allocator.alloc(30).unwrap(), 2);
        assert_eq!(allocator.generation(third), Some(free_generation));
        assert_ne!(allocator.generation(third), Some(stale_generation));
    }

    #[test]
    fn clear_and_pool_drop_destroy_each_live_value_once() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        struct DropCounter(Arc<AtomicUsize>);
        impl Drop for DropCounter {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let drops = Arc::new(AtomicUsize::new(0));
        let config = PoolConfigBuilder::new("Test")
            .with_initial_capacity(2)
            .fixed_size()
            .build();
        let mut allocator = PoolAllocator::new(config).unwrap();
        let deallocated = allocator.alloc(DropCounter(drops.clone())).unwrap();
        let live = allocator.alloc(DropCounter(drops.clone())).unwrap();

        drop(allocator.dealloc(deallocated).unwrap());
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(allocator.clear(), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 2);
        assert!(!allocator.is_occupied(live));
        drop(allocator);
        assert_eq!(drops.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn clear_leaves_pool_valid_when_a_value_drop_unwinds() {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        };

        struct PanicOnceDrop {
            panic_next: Arc<AtomicBool>,
            drops: Arc<AtomicUsize>,
        }
        impl Drop for PanicOnceDrop {
            fn drop(&mut self) {
                self.drops.fetch_add(1, Ordering::SeqCst);
                if self.panic_next.swap(false, Ordering::SeqCst) {
                    panic!("intentional drop panic");
                }
            }
        }

        let panic_next = Arc::new(AtomicBool::new(true));
        let drops = Arc::new(AtomicUsize::new(0));
        let config = PoolConfigBuilder::new("Test")
            .with_initial_capacity(2)
            .fixed_size()
            .build();
        let mut allocator = PoolAllocator::new(config).unwrap();
        let first = allocator
            .alloc(PanicOnceDrop {
                panic_next: panic_next.clone(),
                drops: drops.clone(),
            })
            .unwrap();
        let second = allocator
            .alloc(PanicOnceDrop {
                panic_next: panic_next.clone(),
                drops: drops.clone(),
            })
            .unwrap();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            allocator.clear();
        }));
        assert!(result.is_err());
        assert!(!allocator.is_occupied(first));
        assert!(allocator.is_occupied(second));
        assert_eq!(allocator.len(), 1);
        assert!(allocator.get(first).is_none());
        assert!(allocator.get(second).is_some());
        assert_eq!(allocator.clear(), 1);
        assert_eq!(allocator.len(), 0);
        assert_eq!(drops.load(Ordering::SeqCst), 2);
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
