//! Immutable targeting receipts owned by the receiving simulation.
//! Target/state writes are synchronous; presentation only consumes their order.
use super::ObjectId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostAttackEvent {
    pub attacker: ObjectId,
    pub target: ObjectId,
}

#[derive(Debug, Default)]
pub struct AttackEvents {
    pending: Vec<HostAttackEvent>,
    completed: Vec<HostAttackEvent>,
}
impl AttackEvents {
    pub fn record(&mut self, attacker: ObjectId, target: ObjectId) {
        self.pending.push(HostAttackEvent { attacker, target });
    }
    pub(crate) fn complete_boundary(&mut self) {
        self.completed.append(&mut self.pending);
    }
    pub(crate) fn take_completed(&mut self) -> Vec<HostAttackEvent> {
        std::mem::take(&mut self.completed)
    }
    pub(crate) fn clear(&mut self) {
        self.pending.clear();
        self.completed.clear();
    }
}
