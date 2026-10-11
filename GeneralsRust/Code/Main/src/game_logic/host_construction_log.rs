//! Ordered construction completion output owned by the driving match.
//! Gameplay completion happens immediately; this FIFO only freezes its
//! presentation receipts. Save/load never replays already-presented events.
use super::ObjectId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostConstructionEvent {
    pub id: ObjectId,
    pub template_name: String,
}

#[derive(Debug, Default)]
pub(crate) struct ConstructionEvents {
    pending: Vec<HostConstructionEvent>,
}
impl ConstructionEvents {
    pub(crate) fn record(&mut self, id: ObjectId, template_name: impl Into<String>) {
        self.pending.push(HostConstructionEvent {
            id,
            template_name: template_name.into(),
        });
    }
    pub(crate) fn pending(&self) -> &[HostConstructionEvent] {
        &self.pending
    }
    pub(crate) fn has_pending(&self, object: ObjectId) -> bool {
        self.pending.iter().any(|event| event.id == object)
    }
    pub(crate) fn drain(&mut self) -> Vec<HostConstructionEvent> {
        std::mem::take(&mut self.pending)
    }
    pub(crate) fn reset(&mut self) {
        self.pending.clear();
    }
}
