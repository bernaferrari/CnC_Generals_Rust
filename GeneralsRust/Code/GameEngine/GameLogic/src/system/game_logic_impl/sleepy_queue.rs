//! Match-owned indexed heap. C++ GameLogic.cpp:2737–2958 is the ordering contract.
//! Equal priorities never swap; downward balancing chooses the left child on ties.

use super::{SleepyUpdateEntry, SleepyUpdatePhase, UnsignedInt, UpdateModulePtr};
use std::collections::HashMap;
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
    entries: Vec<SleepyUpdateEntry>,
    indices: HashMap<ModuleIdentity, usize>,
}

impl SleepyUpdateQueue {
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn peek(&self) -> Option<&SleepyUpdateEntry> {
        self.entries.first()
    }

    pub(super) fn iter(&self) -> std::slice::Iter<'_, SleepyUpdateEntry> {
        self.entries.iter()
    }

    pub(super) fn clear(&mut self) {
        self.indices.clear();
        self.entries.clear();
    }

    pub(super) fn index_of(&self, module: &UpdateModulePtr) -> Option<usize> {
        self.indices.get(&ModuleIdentity::of(module)).copied()
    }

    pub(super) fn entry_for(&self, module: &UpdateModulePtr) -> Option<&SleepyUpdateEntry> {
        self.index_of(module).map(|index| &self.entries[index])
    }

    pub(super) fn push(&mut self, entry: SleepyUpdateEntry) {
        // A registration has exactly one heap position. Replacement follows the
        // original erase+push operations, including their effects on equal ties.
        self.erase(&entry.module);
        let index = self.entries.len();
        self.indices
            .insert(ModuleIdentity::of(&entry.module), index);
        self.entries.push(entry);
        self.rebalance_parent(index);
    }

    pub(super) fn pop(&mut self) -> Option<SleepyUpdateEntry> {
        if self.entries.is_empty() {
            return None;
        }
        // C++ popSleepyUpdate replaces the root with the final entry, then sifts.
        Some(self.erase_index(0))
    }

    pub(super) fn erase(&mut self, module: &UpdateModulePtr) -> Option<SleepyUpdateEntry> {
        self.erase_identity(ModuleIdentity::of(module))
    }

    pub(super) fn erase_identity(&mut self, identity: ModuleIdentity) -> Option<SleepyUpdateEntry> {
        self.indices
            .get(&identity)
            .copied()
            .map(|index| self.erase_index(index))
    }

    fn erase_index(&mut self, index: usize) -> SleepyUpdateEntry {
        let removed = self.entries.swap_remove(index);
        self.indices.remove(&ModuleIdentity::of(&removed.module));
        if index < self.entries.len() {
            self.indices
                .insert(ModuleIdentity::of(&self.entries[index].module), index);
            self.rebalance(index);
        }
        removed
    }

    pub(super) fn reschedule(
        &mut self,
        module: &UpdateModulePtr,
        wake_frame: UnsignedInt,
        phase: Option<SleepyUpdatePhase>,
    ) {
        if let Some(index) = self.index_of(module) {
            let entry = &mut self.entries[index];
            entry.wake_frame = wake_frame;
            if let Some(phase) = phase {
                entry.phase = phase;
            }
            self.rebalance(index);
        }
    }

    fn lower_priority(&self, a: usize, b: usize) -> bool {
        let a = &self.entries[a];
        let b = &self.entries[b];
        (a.wake_frame, a.phase) > (b.wake_frame, b.phase)
    }

    fn swap(&mut self, a: usize, b: usize) {
        self.entries.swap(a, b);
        self.indices
            .insert(ModuleIdentity::of(&self.entries[a].module), a);
        self.indices
            .insert(ModuleIdentity::of(&self.entries[b].module), b);
    }

    pub(super) fn rebalance_parent(&mut self, mut index: usize) -> usize {
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

    pub(super) fn rebalance_child(&mut self, mut index: usize) -> usize {
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

    pub(super) fn rebalance(&mut self, index: usize) {
        if index < self.entries.len() {
            let index = self.rebalance_parent(index);
            self.rebalance_child(index);
        }
    }

    pub(super) fn remake(&mut self) {
        if !self.entries.is_empty() {
            for index in (0..=self.entries.len() / 2).rev() {
                self.rebalance_child(index);
            }
        }
        self.validate();
    }

    pub(super) fn validate(&self) {
        debug_assert_eq!(self.entries.len(), self.indices.len());
        for (index, entry) in self.entries.iter().enumerate() {
            debug_assert_eq!(self.index_of(&entry.module), Some(index));
            if index > 0 {
                debug_assert!(!self.lower_priority((index - 1) / 2, index));
            }
        }
    }
}

impl<'a> IntoIterator for &'a SleepyUpdateQueue {
    type Item = &'a SleepyUpdateEntry;
    type IntoIter = std::slice::Iter<'a, SleepyUpdateEntry>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
