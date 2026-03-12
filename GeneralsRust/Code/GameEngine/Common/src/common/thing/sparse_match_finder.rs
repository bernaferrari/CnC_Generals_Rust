////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! Sparse match finder – generic helper mirroring `SparseMatchFinder.h`.
//!
//! The original C++ helper caches the best match for a given bit-mask and
//! uses that cache to accelerate repeated lookups. We reproduce the search
//! heuristics exactly:
//!   * Prefer the candidate with the greatest number of "yes" matches.
//!   * Break ties by picking the candidate with the fewest extraneous "yes"
//!     bits (bits set in the candidate but not in the query).

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::RwLock;

/// Bit-set behaviour required by the sparse match helper.
pub trait SparseBitSet: Sized {
    /// Number of addressable bits in the set.
    fn bit_len(&self) -> usize;
    /// Test whether a bit is set.
    fn bit_test(&self, index: usize) -> bool;
    /// Count "yes" matches between `self` and `other`.
    fn yes_match_count(&self, other: &Self) -> usize;
    /// Count "extraneous" yes bits (bits set in `other` but not in `self`).
    fn extraneous_yes_count(&self, other: &Self) -> usize;

    /// Build a compact byte key suitable for hash-map storage.
    fn key_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut byte: u8 = 0;
        let mut bit_idx = 0u8;

        for bit in 0..self.bit_len() {
            if self.bit_test(bit) {
                byte |= 1 << bit_idx;
            }
            bit_idx += 1;
            if bit_idx == 8 {
                bytes.push(byte);
                byte = 0;
                bit_idx = 0;
            }
        }

        if bit_idx != 0 {
            bytes.push(byte);
        }

        bytes
    }
}

/// Trait implemented by match candidates.
pub trait SparseMatchCandidate<B: SparseBitSet> {
    /// Number of "yes" condition sets carried by this candidate.
    fn conditions_yes_count(&self) -> usize;
    /// Retrieve the `index`th "yes" condition set.
    fn nth_conditions_yes(&self, index: usize) -> &B;
}

/// Cache of best matches for sparse bit-set lookups.
#[derive(Debug, Default)]
pub struct SparseMatchFinder<M, B>
where
    B: SparseBitSet,
{
    cache: RwLock<HashMap<Vec<u8>, usize>>,
    _marker: PhantomData<fn() -> (M, B)>,
}

impl<M, B> Clone for SparseMatchFinder<M, B>
where
    B: SparseBitSet,
{
    fn clone(&self) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            _marker: PhantomData,
        }
    }
}

impl<M, B> SparseMatchFinder<M, B>
where
    M: SparseMatchCandidate<B>,
    B: SparseBitSet,
{
    /// Create an empty finder.
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            _marker: PhantomData,
        }
    }

    /// Drop any cached matches. Call this whenever the backing vector
    /// is mutated (items added/removed).
    pub fn clear(&self) {
        if let Ok(mut guard) = self.cache.write() {
            guard.clear();
        }
    }

    /// Find the best-matching candidate for `bits` within `candidates`.
    ///
    /// Returns `None` if the candidate set is empty.
    pub fn find_best<'a>(&self, candidates: &'a [M], bits: &B) -> Option<&'a M> {
        let key = bits.key_bytes();
        if let Ok(cache) = self.cache.read() {
            if let Some(&cached_index) = cache.get(&key) {
                if let Some(candidate) = candidates.get(cached_index) {
                    return Some(candidate);
                }
            }
        }

        let mut best_index: Option<usize> = None;
        let mut best_yes_match = 0usize;
        let mut best_extraneous = usize::MAX;

        for (candidate_index, candidate) in candidates.iter().enumerate() {
            for condition_index in 0..candidate.conditions_yes_count() {
                let yes_flags = candidate.nth_conditions_yes(condition_index);

                let yes_match = bits.yes_match_count(yes_flags);
                let extraneous = bits.extraneous_yes_count(yes_flags);

                if yes_match > best_yes_match
                    || (yes_match == best_yes_match && extraneous < best_extraneous)
                {
                    best_yes_match = yes_match;
                    best_extraneous = extraneous;
                    best_index = Some(candidate_index);
                }
            }
        }

        if let Some(index) = best_index {
            if let Ok(mut cache) = self.cache.write() {
                cache.insert(key, index);
            }
            candidates.get(index)
        } else {
            None
        }
    }
}
