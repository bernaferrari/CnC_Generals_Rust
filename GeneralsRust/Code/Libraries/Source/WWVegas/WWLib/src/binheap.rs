// Auto-generated C++ compatibility shim for binary heap
use std::collections::BinaryHeap;

pub struct BinHeap<T: Ord> {
    heap: BinaryHeap<T>,
}

impl<T: Ord> BinHeap<T> {
    pub fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
        }
    }

    pub fn push(&mut self, value: T) {
        self.heap.push(value);
    }

    pub fn pop(&mut self) -> Option<T> {
        self.heap.pop()
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }
}
