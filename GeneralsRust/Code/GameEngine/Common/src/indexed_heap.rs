//! C++ GameLogic.cpp:2737–2958 indexed sleepy-heap mechanics.
//! Strict comparisons preserve topology: ties do not swap, left child wins.
use std::{collections::HashMap, hash::Hash};

#[derive(Debug, Clone)]
pub struct IndexedHeapEntry<K, V> {
    pub key: K,
    pub priority: u32,
    pub value: V,
}

pub struct IndexedHeap<K, V> {
    entries: Vec<IndexedHeapEntry<K, V>>,
    indices: HashMap<K, usize>,
}
impl<K, V> Default for IndexedHeap<K, V> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            indices: HashMap::new(),
        }
    }
}
impl<K: Copy + Eq + Hash, V> IndexedHeap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn peek(&self) -> Option<&IndexedHeapEntry<K, V>> {
        self.entries.first()
    }
    pub fn iter(&self) -> std::slice::Iter<'_, IndexedHeapEntry<K, V>> {
        self.entries.iter()
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.indices.clear();
    }
    pub fn index_of(&self, key: K) -> Option<usize> {
        self.indices.get(&key).copied()
    }
    pub fn get(&self, key: K) -> Option<&IndexedHeapEntry<K, V>> {
        self.index_of(key).map(|i| &self.entries[i])
    }
    pub fn value_mut(&mut self, key: K) -> Option<&mut V> {
        self.index_of(key).map(|i| &mut self.entries[i].value)
    }
    pub fn push(&mut self, entry: IndexedHeapEntry<K, V>) {
        let index = self.append_unbalanced(entry);
        self.rebalance_parent(index);
    }
    pub fn append_unbalanced(&mut self, entry: IndexedHeapEntry<K, V>) -> usize {
        self.erase(entry.key);
        let index = self.entries.len();
        self.indices.insert(entry.key, index);
        self.entries.push(entry);
        index
    }
    pub fn pop(&mut self) -> Option<IndexedHeapEntry<K, V>> {
        if self.entries.is_empty() {
            None
        } else {
            Some(self.erase_index(0))
        }
    }
    pub fn erase(&mut self, key: K) -> Option<IndexedHeapEntry<K, V>> {
        self.index_of(key).map(|i| self.erase_index(i))
    }
    fn erase_index(&mut self, index: usize) -> IndexedHeapEntry<K, V> {
        let removed = self.entries.swap_remove(index);
        self.indices.remove(&removed.key);
        if index < self.entries.len() {
            self.indices.insert(self.entries[index].key, index);
            self.rebalance(index);
        }
        removed
    }

    pub fn reschedule(&mut self, key: K, priority: u32) -> bool {
        if let Some(index) = self.index_of(key) {
            self.entries[index].priority = priority;
            self.rebalance(index);
            true
        } else {
            false
        }
    }

    /// C++ GameLogic update completion writes the original module's packed
    /// time, then rebalances slot zero, even if a callback inserted a new root.
    /// This is deliberately distinct from normal keyed rescheduling. It does
    /// not promise to repair a non-root entry dirtied by that original sequence.
    pub fn complete_root_update(&mut self, key: K, priority: u32) -> bool {
        let Some(index) = self.index_of(key) else {
            return false;
        };
        self.entries[index].priority = priority;
        self.rebalance(0);
        true
    }

    fn lower_priority(&self, a: usize, b: usize) -> bool {
        let a = &self.entries[a];
        let b = &self.entries[b];
        a.priority > b.priority
    }

    fn swap(&mut self, a: usize, b: usize) {
        self.entries.swap(a, b);
        self.indices.insert(self.entries[a].key, a);
        self.indices.insert(self.entries[b].key, b);
    }

    pub fn rebalance_parent(&mut self, mut index: usize) -> usize {
        while index > 0 {
            let parent = (index - 1) / 2;
            if !self.lower_priority(parent, index) {
                break;
            }
            self.swap(parent, index);
            index = parent;
        }
        index
    }

    pub fn rebalance_child(&mut self, mut index: usize) -> usize {
        let size = self.entries.len();
        let mut child = index * 2 + 1;
        while child < size {
            if child + 1 < size && self.lower_priority(child, child + 1) {
                child += 1;
            }
            if !self.lower_priority(index, child) {
                break;
            }
            self.swap(index, child);
            index = child;
            child = index * 2 + 1;
        }
        index
    }

    pub fn rebalance(&mut self, index: usize) {
        if index < self.entries.len() {
            let index = self.rebalance_parent(index);
            self.rebalance_child(index);
        }
    }

    pub fn remake(&mut self) {
        if !self.entries.is_empty() {
            for index in (0..=self.entries.len() / 2).rev() {
                self.rebalance_child(index);
            }
        }
        self.validate();
    }

    pub fn validate(&self) {
        debug_assert_eq!(self.entries.len(), self.indices.len());
        for (index, entry) in self.entries.iter().enumerate() {
            debug_assert_eq!(self.index_of(entry.key), Some(index));
            if index > 0 {
                debug_assert!(!self.lower_priority((index - 1) / 2, index));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(key: u32, priority: u32) -> IndexedHeapEntry<u32, u32> {
        IndexedHeapEntry {
            key,
            priority,
            value: key,
        }
    }
    #[test]
    fn strict_ties_and_root_reschedule_keep_heap_topology() {
        let mut heap = IndexedHeap::new();
        for key in 1..=4 {
            heap.push(entry(key, 10));
        }
        assert_eq!(heap.pop().unwrap().key, 1);
        // Last entry replaces root; no FIFO or ID sorting occurs on ties.
        assert_eq!(heap.peek().unwrap().key, 4);
        heap.reschedule(4, 20);
        assert_eq!(heap.peek().unwrap().key, 2); // left child wins equal priority
        heap.validate();
    }
    #[test]
    fn erase_replacement_and_remake_retain_exact_indices() {
        let mut heap = IndexedHeap::new();
        for (key, priority) in [(1, 30), (2, 10), (3, 20), (4, 5)] {
            heap.append_unbalanced(entry(key, priority));
        }
        heap.remake();
        assert_eq!(heap.peek().unwrap().key, 4);
        assert_eq!(heap.erase(2).unwrap().value, 2);
        heap.reschedule(1, 0);
        assert_eq!(heap.index_of(1), Some(0));
        heap.push(entry(1, 40)); // replacement is erase+push, one owner/token
        assert_eq!(heap.len(), 3);
        assert_eq!(heap.pop().unwrap().key, 4);
        assert_eq!(heap.pop().unwrap().key, 3);
        assert_eq!(heap.pop().unwrap().key, 1);
        assert!(heap.pop().is_none());
    }
    #[test]
    fn callback_registration_completion_rebalances_literal_root() {
        let mut heap = IndexedHeap::new();
        heap.push(entry(1, 10)); // original executing root
        heap.push(entry(2, 20));
        heap.push(entry(3, 5)); // callback registers earlier wake, becomes root
        assert_eq!(heap.peek().unwrap().key, 3);
        // CPP writes original 1, then rebalances zero, not index_of(1).
        assert!(heap.complete_root_update(1, 1));
        assert_eq!(heap.peek().unwrap().key, 1);
        assert_eq!(
            heap.iter().map(|e| e.key).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert!(!heap.complete_root_update(99, 0));
        let mut heap = IndexedHeap::new();
        heap.push(entry(1, 10));
        heap.push(entry(2, 20));
        heap.push(entry(3, 30));
        heap.push(entry(4, 5));
        assert!(heap.complete_root_update(1, 40));
        // Literal CPP completion leaves original non-root 1 above child 2;
        // keyed reschedule would instead repair that branch. Do not validate
        // or silently substitute a different operation here.
        assert_eq!(
            heap.iter().map(|e| e.key).collect::<Vec<_>>(),
            vec![4, 1, 3, 2]
        );
    }
}
