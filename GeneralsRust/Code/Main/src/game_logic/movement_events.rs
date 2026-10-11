//! Movement observations owned by the driving game instance.
//!
//! Canonical object/path writes remain immediate. This transport preserves their
//! order through completed fixed steps and one consuming presentation boundary.
//! Its runtime identities and receipts are never part of game save/Xfer state.

use super::ObjectId;
use generals_presentation::events::MovementSessionId;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MovementEvent {
    pub(crate) session: MovementSessionId,
    pub(crate) logic_frame: u32,
    pub(crate) sequence: u64,
    pub(crate) unit: ObjectId,
    /// None clears a diagnostic move target; ordinary stop uses its own channel.
    pub(crate) destination: Option<[f32; 3]>,
}

/// One ordered optional-shadow handoff; only the admitted prefix is complete.
#[derive(Debug, Default)]
pub(crate) struct MovementBatch {
    events: Vec<MovementEvent>,
    shadow_applied_prefix: usize,
}

impl MovementBatch {
    pub(crate) fn events(&self) -> &[MovementEvent] {
        &self.events
    }

    pub(crate) fn pending_shadow_events(&self) -> &[MovementEvent] {
        &self.events[self.shadow_applied_prefix..]
    }

    pub(crate) fn mark_shadow_applied(&mut self) {
        self.shadow_applied_prefix = self.events.len();
    }

    /// A removed canonical subject cannot receive an optional shadow order.
    /// Completed presentation receipts remain untouched; only the unadmitted
    /// suffix is pruned after the owner's lifecycle phase has completed.
    pub(crate) fn retain_pending_subjects(&mut self, mut alive: impl FnMut(ObjectId) -> bool) {
        let mut index = 0;
        self.events.retain(|event| {
            let keep = index < self.shadow_applied_prefix || alive(event.unit);
            index += 1;
            keep
        });
    }
}

/// Ephemeral match output; no shared mutable ownership or thread-local state.
#[derive(Debug, Default)]
pub(crate) struct MovementEvents {
    session: MovementSessionId,
    next_sequence: u64,
    pending: Vec<MovementEvent>,
    completed: Vec<MovementEvent>,
    early_batch: Option<MovementBatch>,
}

impl MovementEvents {
    pub(crate) fn session(&self) -> &MovementSessionId {
        &self.session
    }

    pub(crate) fn writer(&mut self, logic_frame: u32) -> MovementEventWriter<'_> {
        MovementEventWriter {
            events: self,
            logic_frame,
        }
    }

    pub(crate) fn record(
        &mut self,
        logic_frame: u32,
        unit: ObjectId,
        destination: Option<[f32; 3]>,
    ) {
        let sequence = self
            .next_sequence
            .checked_add(1)
            .expect("movement receipt exhausted");
        self.next_sequence = sequence;
        self.pending.push(MovementEvent {
            session: self.session.clone(),
            logic_frame,
            sequence,
            unit,
            destination,
        });
    }

    /// Finish an ordinary completed logic step without losing previous steps.
    pub(crate) fn drain_for_host_boundary(&mut self) -> Vec<MovementEvent> {
        self.drain_for_shadow_boundary().events
    }

    /// Keep an admitted eager prefix, append later records, and freeze only the
    /// fresh suffix. Repeated eager/session handoffs never duplicate receipts.
    pub(crate) fn drain_for_shadow_boundary(&mut self) -> MovementBatch {
        let mut batch = self.early_batch.take().unwrap_or_default();
        let fresh = std::mem::take(&mut self.pending);
        self.completed.extend(fresh.iter().cloned());
        batch.events.extend(fresh);
        batch
    }

    pub(crate) fn set_early_batch(&mut self, batch: MovementBatch) {
        assert!(self.early_batch.is_none(), "unconsumed movement handoff");
        assert!(
            batch
                .events
                .iter()
                .all(|event| event.session == self.session),
            "movement handoff belongs to another session"
        );
        if !batch.events.is_empty() {
            self.early_batch = Some(batch);
        }
    }

    /// Protect newer owner destinations from experimental reverse writes.
    pub(crate) fn has_pending(&self, object: ObjectId) -> bool {
        self.pending.iter().any(|event| event.unit == object)
            || self.early_batch.as_ref().is_some_and(|batch| {
                batch
                    .pending_shadow_events()
                    .iter()
                    .any(|event| event.unit == object)
            })
    }

    pub(crate) fn len(&self) -> usize {
        self.pending.len()
    }

    pub(crate) fn snapshot_pending(&self) -> &[MovementEvent] {
        &self.pending
    }

    pub(crate) fn snapshot_completed(&self) -> &[MovementEvent] {
        &self.completed
    }

    /// Publish completed observations once. Unfinished operations stay pending.
    pub(crate) fn take_completed(&mut self) -> Vec<MovementEvent> {
        std::mem::take(&mut self.completed)
    }

    /// Drop transient cues after saved-world admission, retaining runtime owner.
    pub(crate) fn clear(&mut self) {
        self.pending.clear();
        self.completed.clear();
        self.early_batch = None;
    }

    /// A new match cannot reuse the previous match's receipt identity.
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Borrowed for one synchronous operation together with its admitted object.
pub(crate) struct MovementEventWriter<'a> {
    events: &'a mut MovementEvents,
    logic_frame: u32,
}

impl MovementEventWriter<'_> {
    pub(crate) fn record(&mut self, unit: ObjectId, destination: Option<[f32; 3]>) {
        self.events.record(self.logic_frame, unit, destination);
    }
}

#[cfg(test)]
#[path = "movement_events_tests.rs"]
mod tests;
