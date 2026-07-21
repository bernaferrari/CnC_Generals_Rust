//! Frame-local host AICommand decision buffer for GameWorld PushAiDecision parity.

use super::ObjectId;
use glam::Vec3;
use std::cell::RefCell;

/// 0 AttackTarget, 1 StopAttack, 2 MoveTo, 3 SetAIState.
pub const AI_DECISION_ATTACK: u8 = 0;
pub const AI_DECISION_STOP_ATTACK: u8 = 1;
pub const AI_DECISION_MOVE_TO: u8 = 2;
pub const AI_DECISION_SET_STATE: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostAiDecisionEvent {
    pub host_object: ObjectId,
    pub kind: u8,
    pub target_host: u32,
    pub destination: Option<[f32; 3]>,
    pub ai_state_ordinal: u8,
}

thread_local! {
    static LOG: RefCell<Vec<HostAiDecisionEvent>> = RefCell::new(Vec::new());
}

pub fn record(
    host_object: ObjectId,
    kind: u8,
    target_host: u32,
    destination: Option<[f32; 3]>,
    ai_state_ordinal: u8,
) {
    LOG.with(|log| {
        log.borrow_mut().push(HostAiDecisionEvent {
            host_object,
            kind,
            target_host,
            destination,
            ai_state_ordinal,
        });
    });
}

pub fn record_attack(object: ObjectId, target: ObjectId) {
    record(object, AI_DECISION_ATTACK, target.0, None, 0);
}

pub fn record_stop_attack(object: ObjectId) {
    record(object, AI_DECISION_STOP_ATTACK, 0, None, 0);
}

pub fn record_move_to(object: ObjectId, position: Vec3) {
    record(
        object,
        AI_DECISION_MOVE_TO,
        0,
        Some([position.x, position.y, position.z]),
        0,
    );
}

pub fn record_set_state(object: ObjectId, ordinal: u8) {
    record(object, AI_DECISION_SET_STATE, 0, None, ordinal);
}

pub fn drain() -> Vec<HostAiDecisionEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

/// Non-destructive copy for tests / honesty probes.
pub fn snapshot() -> Vec<HostAiDecisionEvent> {
    LOG.with(|log| log.borrow().clone())
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
