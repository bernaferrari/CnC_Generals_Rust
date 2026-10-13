//! Real factory completion receipts owned by the receiving Main session.
//! CPP ProductionUpdate completes creation/notifications before presentation;
//! these receipts cannot modify a queue or spawn a second object.
use super::ObjectId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostProductionEvent {
    Complete {
        producer: ObjectId,
        template_name: String,
        spawned: ObjectId,
    },
}

#[derive(Debug, Default)]
pub struct ProductionEvents {
    pending: Vec<HostProductionEvent>,
    completed: Vec<HostProductionEvent>,
}
impl ProductionEvents {
    pub(crate) fn record_complete(
        &mut self,
        producer: ObjectId,
        template_name: impl Into<String>,
        spawned: ObjectId,
    ) {
        self.pending.push(HostProductionEvent::Complete {
            producer,
            template_name: template_name.into(),
            spawned,
        });
    }
    /// Preserve FIFO across multiple logic frames before a presentation freeze.
    pub(crate) fn complete_boundary(&mut self) {
        self.completed.append(&mut self.pending);
    }
    pub(crate) fn take_completed(&mut self) -> Vec<HostProductionEvent> {
        std::mem::take(&mut self.completed)
    }
    pub(crate) fn clear(&mut self) {
        self.pending.clear();
        self.completed.clear();
    }
    #[cfg(test)]
    pub(crate) fn snapshot_completed(&self) -> &[HostProductionEvent] {
        &self.completed
    }
}
