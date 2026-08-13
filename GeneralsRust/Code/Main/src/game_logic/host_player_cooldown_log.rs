//! Frame-local host shared special-power cooldown log for GameWorld parity.

use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq)]
pub struct HostPlayerCooldownEvent {
    pub player_id: u32,
    /// Debug-name keys with seconds remaining.
    pub cooldowns: Vec<(String, f32)>,
}

thread_local! {
    static LOG: RefCell<Vec<HostPlayerCooldownEvent>> = RefCell::new(Vec::new());
}

pub fn record(player_id: u32, cooldowns: Vec<(String, f32)>) {
    LOG.with(|log| {
        log.borrow_mut().push(HostPlayerCooldownEvent {
            player_id,
            cooldowns,
        });
    });
}

pub fn has_pending(player_id: u32) -> bool {
    LOG.with(|log| log.borrow().iter().any(|e| e.player_id == player_id))
}

pub fn drain() -> Vec<HostPlayerCooldownEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

/// Move the queue out for a whole-world staging boundary without dropping the
/// active world's pending presentation mutations.
pub(crate) fn take_for_world_stage() -> Vec<HostPlayerCooldownEvent> {
    drain()
}

/// Restore or install the queue owned by a whole-world staging boundary.
pub(crate) fn replace_for_world_stage(
    next: Vec<HostPlayerCooldownEvent>,
) -> Vec<HostPlayerCooldownEvent> {
    LOG.with(|log| std::mem::replace(&mut *log.borrow_mut(), next))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}

pub fn len() -> usize {
    LOG.with(|log| log.borrow().len())
}
