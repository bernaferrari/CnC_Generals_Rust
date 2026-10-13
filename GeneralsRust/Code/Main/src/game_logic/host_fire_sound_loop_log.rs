//! FiringTracker loop audio receipts owned by the accepting CombatSystem.
//! CPP FiringTracker.cpp138–185 starts a loop, extends its deadline, then stops
//! it at the original logic frame. Delivery never changes that gameplay state.
use super::ObjectId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostFireSoundLoopEvent {
    pub unit: ObjectId,
    pub sound: String,
    pub start: bool,
}

#[derive(Debug, Default)]
pub struct FireSoundLoopEvents {
    pending: Vec<HostFireSoundLoopEvent>,
    completed: Vec<HostFireSoundLoopEvent>,
}

impl FireSoundLoopEvents {
    pub(crate) fn record(&mut self, unit: ObjectId, sound: String, start: bool) {
        self.pending
            .push(HostFireSoundLoopEvent { unit, sound, start });
    }

    /// Admit this owner's current frame in FIFO order. Multiple fixed frames
    /// before presentation retain every transition rather than overwriting it.
    pub(crate) fn complete_boundary(&mut self) {
        self.completed.append(&mut self.pending);
    }

    pub(crate) fn take_completed(&mut self) -> Vec<HostFireSoundLoopEvent> {
        std::mem::take(&mut self.completed)
    }

    pub(crate) fn clear(&mut self) {
        self.pending.clear();
        self.completed.clear();
    }

    #[cfg(test)]
    pub(crate) fn pending(&self) -> &[HostFireSoundLoopEvent] {
        &self.pending
    }
    #[cfg(test)]
    pub(crate) fn completed(&self) -> &[HostFireSoundLoopEvent] {
        &self.completed
    }
}
