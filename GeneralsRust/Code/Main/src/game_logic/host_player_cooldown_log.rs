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
        log.borrow_mut()
            .push(HostPlayerCooldownEvent { player_id, cooldowns });
    });
}

pub fn drain() -> Vec<HostPlayerCooldownEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
