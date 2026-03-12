//! Generational Indices for Stable Object IDs
//!
//! This module implements generational indices, which solve the ABA problem
//! and prevent use-after-free errors. Each handle contains both an index
//! (slot in the pool) and a generation counter. When an object is freed,
//! its generation is incremented, invalidating all old handles.

use std::fmt;
use std::num::NonZeroU32;

/// Generation counter for generational indices.
///
/// Using NonZeroU32 allows for niche optimization, making Option<GenerationalIndex>
/// the same size as GenerationalIndex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Generation(NonZeroU32);

impl Generation {
    /// Create the first generation (1).
    #[inline]
    pub fn first() -> Self {
        // SAFETY: 1 is non-zero
        Generation(unsafe { NonZeroU32::new_unchecked(1) })
    }

    /// Increment to the next generation, wrapping at u32::MAX.
    #[inline]
    pub fn next(self) -> Self {
        let next_val = self.0.get().wrapping_add(1);
        // Avoid zero - wrap to 1 instead
        let next_val = if next_val == 0 { 1 } else { next_val };
        // SAFETY: We just ensured it's non-zero
        Generation(unsafe { NonZeroU32::new_unchecked(next_val) })
    }

    /// Get the raw generation value.
    #[inline]
    pub fn get(self) -> u32 {
        self.0.get()
    }
}

impl Default for Generation {
    fn default() -> Self {
        Self::first()
    }
}

impl fmt::Display for Generation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Gen({})", self.get())
    }
}

/// A generational index that uniquely identifies an object in a pool.
///
/// This combines an index (slot number) with a generation counter.
/// The generation prevents the ABA problem where an old handle
/// accidentally refers to a new object in the same slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GenerationalIndex {
    /// Index into the pool's storage.
    pub(crate) index: u32,
    /// Generation counter for this slot.
    pub(crate) generation: Generation,
}

impl GenerationalIndex {
    /// Create a new generational index.
    #[inline]
    pub fn new(index: u32, generation: Generation) -> Self {
        Self { index, generation }
    }

    /// Get the index component.
    #[inline]
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Get the generation component.
    #[inline]
    pub fn generation(&self) -> Generation {
        self.generation
    }

    /// Create an index with the first generation.
    #[inline]
    pub fn with_first_generation(index: u32) -> Self {
        Self::new(index, Generation::first())
    }

    /// Check if this index is valid for the given slot generation.
    #[inline]
    pub fn is_valid_for(&self, slot_generation: Generation) -> bool {
        self.generation == slot_generation
    }
}

impl fmt::Display for GenerationalIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Index({}, {})", self.index, self.generation)
    }
}

/// Entry in the pool's free list.
///
/// This is stored in the slot when it's not in use. We use a union-like
/// approach where the slot either contains a live object or a free list entry.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FreeListEntry {
    /// Index of the next free slot, or None if this is the last.
    pub next_free: Option<u32>,
    /// Current generation of this slot.
    pub generation: Generation,
}

impl FreeListEntry {
    #[inline]
    pub fn new(next_free: Option<u32>, generation: Generation) -> Self {
        Self {
            next_free,
            generation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generation_increment() {
        let gen1 = Generation::first();
        assert_eq!(gen1.get(), 1);

        let gen2 = gen1.next();
        assert_eq!(gen2.get(), 2);

        let gen3 = gen2.next();
        assert_eq!(gen3.get(), 3);
    }

    #[test]
    fn test_generation_wrap() {
        // Test wrapping from u32::MAX to 1 (not 0)
        let gen_max = Generation(unsafe { NonZeroU32::new_unchecked(u32::MAX) });
        let gen_wrapped = gen_max.next();
        assert_eq!(gen_wrapped.get(), 1);
    }

    #[test]
    fn test_generational_index() {
        let idx = GenerationalIndex::new(42, Generation::first());
        assert_eq!(idx.index(), 42);
        assert_eq!(idx.generation().get(), 1);
    }

    #[test]
    fn test_validity_check() {
        let idx = GenerationalIndex::new(10, Generation::first());
        assert!(idx.is_valid_for(Generation::first()));
        assert!(!idx.is_valid_for(Generation::first().next()));
    }

    #[test]
    fn test_size_optimization() {
        // Option<GenerationalIndex> should be the same size as GenerationalIndex
        // due to NonZeroU32 niche optimization
        use std::mem::size_of;
        assert_eq!(
            size_of::<Option<GenerationalIndex>>(),
            size_of::<GenerationalIndex>()
        );
    }
}
