//! Frame-local host attack-target log for GameWorld shadow parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostAttackEvent {
    pub attacker: ObjectId,
    pub target: Option<ObjectId>,
}

thread_local! {
    static LOG: RefCell<Vec<HostAttackEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostAttackEvent>> = RefCell::new(Vec::new());
}

/// Both attack queues belong to one host world. `LAST_DRAIN` is presentation
/// state, so staging must preserve it as well as the pending mutation queue.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HostAttackLogWorldStageState {
    pub(crate) pending: Vec<HostAttackEvent>,
    pub(crate) last_drain: Vec<HostAttackEvent>,
}

pub fn record(attacker: ObjectId, target: Option<ObjectId>) {
    LOG.with(|log| {
        log.borrow_mut().push(HostAttackEvent { attacker, target });
    });
}

pub fn has_pending(object: ObjectId) -> bool {
    LOG.with(|log| {
        log.borrow()
            .iter()
            .any(|e| e.attacker == object || e.target == Some(object))
    })
}

pub fn drain() -> Vec<HostAttackEvent> {
    let v = LOG.with(|log| std::mem::take(&mut *log.borrow_mut()));
    // Keep last non-empty batch for PresentationFrame after shadow session.
    if !v.is_empty() {
        LAST_DRAIN.with(|last| *last.borrow_mut() = v.clone());
    }
    v
}

/// Move both queues out for a whole-world staging boundary. Unlike `clear`,
/// this is rollback-safe for a playable active match.
pub(crate) fn take_for_world_stage() -> HostAttackLogWorldStageState {
    HostAttackLogWorldStageState {
        pending: LOG.with(|log| std::mem::take(&mut *log.borrow_mut())),
        last_drain: LAST_DRAIN.with(|last| std::mem::take(&mut *last.borrow_mut())),
    }
}

/// Restore or install both queues and return the contents they displaced.
pub(crate) fn replace_for_world_stage(
    next: HostAttackLogWorldStageState,
) -> HostAttackLogWorldStageState {
    let pending = LOG.with(|log| std::mem::replace(&mut *log.borrow_mut(), next.pending));
    let last_drain =
        LAST_DRAIN.with(|last| std::mem::replace(&mut *last.borrow_mut(), next.last_drain));
    HostAttackLogWorldStageState {
        pending,
        last_drain,
    }
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
    LAST_DRAIN.with(|last| last.borrow_mut().clear());
}

/// Take events from the most recent non-empty `drain()` (PresentationFrame sole consumer).
pub fn take_last_drain() -> Vec<HostAttackEvent> {
    LAST_DRAIN.with(|last| std::mem::take(&mut *last.borrow_mut()))
}

/// Non-destructive peek (tests).
pub fn last_drain_snapshot() -> Vec<HostAttackEvent> {
    LAST_DRAIN.with(|last| last.borrow().clone())
}
