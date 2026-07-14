//! Frame-local host heal / absolute-HP log for GameWorld shadow parity.
//!
//! Complements `host_damage_log` for HP increases and absolute health writes
//! (battle-drone repair, construction finish, composite armor, etc.).

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostHealEvent {
    pub target: ObjectId,
    /// Absolute health after the host write.
    pub health: f32,
}

thread_local! {
    static LOG: RefCell<Vec<HostHealEvent>> = RefCell::new(Vec::new());
}

pub fn record(target: ObjectId, health: f32) {
    if !health.is_finite() || health < 0.0 {
        return;
    }
    LOG.with(|log| {
        log.borrow_mut().push(HostHealEvent { target, health });
    });
}

pub fn drain() -> Vec<HostHealEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_drain() {
        clear();
        record(ObjectId(1), 50.0);
        assert_eq!(drain().len(), 1);
        assert!(drain().is_empty());
    }
}
