//! Match-owned indexed heap. C++ GameLogic.cpp:2737–2958 is the ordering contract.
//! Equal priorities never swap; downward balancing chooses the left child on ties.

use super::{SleepyUpdateEntry, SleepyUpdatePhase, UnsignedInt, UpdateModulePtr};
use std::sync::Arc;

// Private lookup token, never dereferenced or exported. Each heap entry retains
// its Arc, so its allocation cannot be recycled while this index contains it.
// This identifies modules, not objects: equal ObjectIDs in other matches are fine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct ModuleIdentity(usize);

impl ModuleIdentity {
    pub(super) fn of(module: &UpdateModulePtr) -> Self {
        Self(Arc::as_ptr(module).cast::<()>() as usize)
    }
}

#[derive(Default)]
pub(super) struct SleepyUpdateQueue {
    heap: game_engine::indexed_heap::IndexedHeap<ModuleIdentity, SleepyUpdateEntry>,
}
fn priority(frame: u32, phase: SleepyUpdatePhase) -> u32 {
    frame.wrapping_shl(2) | phase as u32
}
impl SleepyUpdateQueue {
    pub(super) fn len(&self) -> usize {
        self.heap.len()
    }
    pub(super) fn peek(&self) -> Option<&SleepyUpdateEntry> {
        self.heap.peek().map(|e| &e.value)
    }
    pub(super) fn iter(&self) -> impl Iterator<Item = &SleepyUpdateEntry> {
        self.heap.iter().map(|e| &e.value)
    }
    pub(super) fn clear(&mut self) {
        self.heap.clear();
    }
    pub(super) fn index_of(&self, module: &UpdateModulePtr) -> Option<usize> {
        self.heap.index_of(ModuleIdentity::of(module))
    }
    pub(super) fn entry_for(&self, module: &UpdateModulePtr) -> Option<&SleepyUpdateEntry> {
        self.heap.get(ModuleIdentity::of(module)).map(|e| &e.value)
    }
    pub(super) fn push(&mut self, entry: SleepyUpdateEntry) {
        self.heap.push(game_engine::indexed_heap::IndexedHeapEntry {
            key: ModuleIdentity::of(&entry.module),
            priority: priority(entry.wake_frame, entry.phase),
            value: entry,
        });
    }
    pub(super) fn pop(&mut self) -> Option<SleepyUpdateEntry> {
        self.heap.pop().map(|e| e.value)
    }
    pub(super) fn erase(&mut self, module: &UpdateModulePtr) -> Option<SleepyUpdateEntry> {
        self.erase_identity(ModuleIdentity::of(module))
    }
    pub(super) fn erase_identity(&mut self, key: ModuleIdentity) -> Option<SleepyUpdateEntry> {
        self.heap.erase(key).map(|e| e.value)
    }
    pub(super) fn reschedule(
        &mut self,
        module: &UpdateModulePtr,
        wake_frame: UnsignedInt,
        phase: Option<SleepyUpdatePhase>,
    ) {
        let key = ModuleIdentity::of(module);
        if let Some(entry) = self.heap.value_mut(key) {
            entry.wake_frame = wake_frame;
            if let Some(phase) = phase {
                entry.phase = phase;
            }
            let packed = priority(wake_frame, entry.phase);
            self.heap.reschedule(key, packed);
        }
    }
    pub(super) fn rebalance_parent(&mut self, index: usize) -> usize {
        self.heap.rebalance_parent(index)
    }
    pub(super) fn rebalance_child(&mut self, index: usize) -> usize {
        self.heap.rebalance_child(index)
    }
    pub(super) fn rebalance(&mut self, index: usize) {
        self.heap.rebalance(index);
    }
    pub(super) fn remake(&mut self) {
        self.heap.remake();
    }
    pub(super) fn validate(&self) {
        self.heap.validate();
    }
}
impl<'a> IntoIterator for &'a SleepyUpdateQueue {
    type Item = &'a SleepyUpdateEntry;
    type IntoIter = std::iter::Map<
        std::slice::Iter<
            'a,
            game_engine::indexed_heap::IndexedHeapEntry<ModuleIdentity, SleepyUpdateEntry>,
        >,
        fn(
            &'a game_engine::indexed_heap::IndexedHeapEntry<ModuleIdentity, SleepyUpdateEntry>,
        ) -> &'a SleepyUpdateEntry,
    >;
    fn into_iter(self) -> Self::IntoIter {
        self.heap.iter().map(|entry| &entry.value)
    }
}
