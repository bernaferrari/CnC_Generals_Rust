//! Frame-local host move-destination log for GameWorld shadow parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostMoveEvent {
    pub unit: ObjectId,
    /// None clears the move target (stop).
    pub destination: Option<[f32; 3]>,
}

thread_local! {
    static LOG: RefCell<Vec<HostMoveEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostMoveEvent>> = RefCell::new(Vec::new());
}

/// Both move queues belong to one host world.  `LAST_DRAIN` is presentation
/// state, so staging must preserve it as well as the pending mutation queue.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HostMoveLogWorldStageState {
    pub(crate) pending: Vec<HostMoveEvent>,
    pub(crate) last_drain: Vec<HostMoveEvent>,
}

pub fn record(unit: ObjectId, destination: Option<[f32; 3]>) {
    LOG.with(|log| {
        log.borrow_mut().push(HostMoveEvent { unit, destination });
    });
}

pub fn has_pending(object: ObjectId) -> bool {
    LOG.with(|log| log.borrow().iter().any(|e| e.unit == object))
}

pub fn drain() -> Vec<HostMoveEvent> {
    let v = LOG.with(|log| std::mem::take(&mut *log.borrow_mut()));
    // Keep last non-empty batch for PresentationFrame after shadow session.
    if !v.is_empty() {
        LAST_DRAIN.with(|last| *last.borrow_mut() = v.clone());
    }
    v
}

/// Move both queues out for a whole-world staging boundary.  Unlike `clear`,
/// this is rollback-safe for a playable active match.
pub(crate) fn take_for_world_stage() -> HostMoveLogWorldStageState {
    HostMoveLogWorldStageState {
        pending: LOG.with(|log| std::mem::take(&mut *log.borrow_mut())),
        last_drain: LAST_DRAIN.with(|last| std::mem::take(&mut *last.borrow_mut())),
    }
}

/// Restore or install both queues and return the contents they displaced.
pub(crate) fn replace_for_world_stage(
    next: HostMoveLogWorldStageState,
) -> HostMoveLogWorldStageState {
    let pending = LOG.with(|log| std::mem::replace(&mut *log.borrow_mut(), next.pending));
    let last_drain =
        LAST_DRAIN.with(|last| std::mem::replace(&mut *last.borrow_mut(), next.last_drain));
    HostMoveLogWorldStageState {
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
pub fn take_last_drain() -> Vec<HostMoveEvent> {
    LAST_DRAIN.with(|last| std::mem::take(&mut *last.borrow_mut()))
}

/// Non-destructive peek (tests).
pub fn last_drain_snapshot() -> Vec<HostMoveEvent> {
    LAST_DRAIN.with(|last| last.borrow().clone())
}
