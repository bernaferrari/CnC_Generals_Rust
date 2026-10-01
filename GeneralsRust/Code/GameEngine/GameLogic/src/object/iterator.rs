//! Simple object iterator used by the partition manager and other systems.
//!
//! This closely mirrors the original C++ `SimpleObjectIterator` implementation.

use std::cmp::Ordering;

use crate::common::{INVALID_ID, Int, ObjectID, Real};
use crate::object::registry::OBJECT_REGISTRY;

use super::Object;

/// Iterator ordering options (matching the C++ `IterOrderType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IterOrderType {
    /// Iterate in the order objects were inserted (fastest).
    Fastest,
    /// Sort by numeric value from nearest to farthest.
    SortedNearToFar,
    /// Sort by numeric value from farthest to nearest.
    SortedFarToNear,
    /// Sort by build cost from cheapest to most expensive.
    SortedCheapToExpensive,
    /// Sort by build cost from most expensive to cheapest.
    SortedExpensiveToCheap,
}

/// Lightweight entry storing object id and associated numeric key.
#[derive(Debug, Clone)]
struct Clump {
    object_id: ObjectID,
    numeric: Real,
}

impl Clump {
    fn new(object_id: ObjectID, numeric: Real) -> Self {
        Self { object_id, numeric }
    }

    fn is_live(&self) -> bool {
        if self.object_id == INVALID_ID {
            return false;
        }
        OBJECT_REGISTRY
            .with_object(self.object_id, |_| ())
            .is_some()
    }

    fn build_cost(&self) -> Option<Int> {
        if self.object_id == INVALID_ID {
            return None;
        }
        OBJECT_REGISTRY.with_object(self.object_id, |o| o.get_build_cost())
    }

    fn is_live(&self) -> bool {
        self.upgrade().is_some()
    }

    fn build_cost(&self) -> Option<Int> {
        self.upgrade()
            .and_then(|obj| obj.read().ok().map(|o| o.get_build_cost()))
    }
}

/// Simple object iterator implementation.
#[derive(Debug, Default)]
pub struct SimpleObjectIterator {
    clumps: Vec<Clump>,
    cursor: usize,
}

impl SimpleObjectIterator {
    /// Create a new empty iterator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Remove all contents of the iterator.
    pub fn make_empty(&mut self) {
        self.clumps.clear();
        self.cursor = 0;
    }

    /// Insert an object ID at the head of the iterator with an optional numeric sort key.
    pub fn insert_id(&mut self, object_id: ObjectID, numeric: Real) {
        self.clumps.insert(0, Clump::new(object_id, numeric));
        self.cursor = 0;
    }

    /// Prefer [`Self::insert_id`].
    pub fn insert(&mut self, object_id: ObjectID, numeric: Real) {
        self.insert_id(object_id, numeric);
        self.cursor = 0;
    }

    /// Reset internal cursor to the beginning.
    pub fn reset(&mut self) {
        self.cursor = 0;
    }

    /// Return number of live clumps held by the iterator.
    pub fn get_count(&self) -> usize {
        self.clumps.len()
    }

    /// Convenience helper: reset and return the first live object ID (if any).
    pub fn first_id(&mut self) -> Option<ObjectID> {
        self.first_id_with_numeric().map(|(id, _)| id)
    }

    /// Convenience helper: return next live object ID without numeric value.
    pub fn next_id(&mut self) -> Option<ObjectID> {
        self.next_id_with_numeric().map(|(id, _)| id)
    }

    /// Convenience helper: reset and return the first object (if any).
    pub fn first(&mut self) -> Option<ObjectID> {
        self.first_id()
    }

    /// Convenience helper: return next object id without numeric value.
    pub fn next(&mut self) -> Option<ObjectID> {
        self.next_id()
    }

    /// Reset and return the first live object ID alongside its numeric value.
    pub fn first_id_with_numeric(&mut self) -> Option<(ObjectID, Real)> {
        self.reset();
        self.next_id_with_numeric()
    }

    /// Return next live object ID together with its numeric value.
    pub fn next_id_with_numeric(&mut self) -> Option<(ObjectID, Real)> {
        while self.cursor < self.clumps.len() {
            let idx = self.cursor;
            self.cursor += 1;
            let clump = &self.clumps[idx];
            if clump.object_id != INVALID_ID && clump.is_live() {
                return Some((clump.object_id, clump.numeric));
            }
        }
        None
    }

    /// Reset and return the first object alongside its numeric value.
    pub fn first_with_numeric(&mut self) -> Option<(ObjectID, Real)> {
        self.first_id_with_numeric()
    }

    /// Return next object id together with its numeric value.
    pub fn next_with_numeric(&mut self) -> Option<(ObjectID, Real)> {
        self.next_id_with_numeric()
    }

    /// Sort according to the requested order.
    pub fn sort(&mut self, order: IterOrderType) {
        self.prune_dead();

        match order {
            IterOrderType::Fastest => {}
            IterOrderType::SortedNearToFar => {
                stable_sort_by(&mut self.clumps, |a, b| cmp_real(a.numeric, b.numeric));
            }
            IterOrderType::SortedFarToNear => {
                stable_sort_by(&mut self.clumps, |a, b| cmp_real(b.numeric, a.numeric));
            }
            IterOrderType::SortedCheapToExpensive => {
                stable_sort_by(&mut self.clumps, |a, b| {
                    let ca = a.build_cost().unwrap_or(i32::MAX);
                    let cb = b.build_cost().unwrap_or(i32::MAX);
                    ca.cmp(&cb)
                });
            }
            IterOrderType::SortedExpensiveToCheap => {
                stable_sort_by(&mut self.clumps, |a, b| {
                    let ca = a.build_cost().unwrap_or(i32::MIN);
                    let cb = b.build_cost().unwrap_or(i32::MIN);
                    cb.cmp(&ca)
                });
            }
        }

        self.reset();
    }

    fn prune_dead(&mut self) {
        self.clumps.retain(|clump| clump.is_live());
        if self.cursor > self.clumps.len() {
            self.cursor = self.clumps.len();
        }
    }
}

fn cmp_real(a: Real, b: Real) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}

fn stable_sort_by<T: Clone, F: Fn(&T, &T) -> Ordering>(items: &mut [T], cmp: F) {
    let len = items.len();
    if len <= 1 {
        return;
    }

    let mut buffer = items.to_vec();
    let mut width = 1;
    while width < len {
        let mut left = 0;
        while left < len {
            let mid = (left + width).min(len);
            let right = (left + (2 * width)).min(len);
            merge_by(items, &mut buffer, left, mid, right, &cmp);
            left += 2 * width;
        }
        items.clone_from_slice(&buffer);
        width *= 2;
    }
}

fn merge_by<T: Clone, F: Fn(&T, &T) -> Ordering>(
    items: &[T],
    buffer: &mut [T],
    left: usize,
    mid: usize,
    right: usize,
    cmp: &F,
) {
    let mut i = left;
    let mut j = mid;
    let mut k = left;

    while i < mid && j < right {
        if cmp(&items[i], &items[j]) != Ordering::Greater {
            buffer[k] = items[i].clone();
            i += 1;
        } else {
            buffer[k] = items[j].clone();
            j += 1;
        }
        k += 1;
    }

    while i < mid {
        buffer[k] = items[i].clone();
        i += 1;
        k += 1;
    }

    while j < right {
        buffer[k] = items[j].clone();
        j += 1;
        k += 1;
    }
}
