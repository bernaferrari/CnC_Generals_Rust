//! Wave 614: GameWorld production-ready residual log.
//!
//! Under PRODUCTION_AUTHORITY sole-tick, `writeback_production_to_host` records
//! producers whose queue head is finished (progress complete + exit delay clear).
//! Host `host_collect_production_completions` drains this log so GameWorld is the
//! sole authority for *which* producers are ready; host still try_complete + spawn.
//!
//! Fail-closed: empty drain is valid (no completions this frame).

use crate::game_logic::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct HostProductionReadyEvent {
    pub producer: ObjectId,
    pub template_name: String,
    pub is_upgrade: bool,
}

thread_local! {
    static LOG: RefCell<Vec<HostProductionReadyEvent>> = RefCell::new(Vec::new());
    static LAST_DRAIN: RefCell<Vec<HostProductionReadyEvent>> = RefCell::new(Vec::new());
}

pub fn record(producer: ObjectId, template_name: impl Into<String>, is_upgrade: bool) {
    LOG.with(|log| {
        log.borrow_mut().push(HostProductionReadyEvent {
            producer,
            template_name: template_name.into(),
            is_upgrade,
        });
    });
}

pub fn drain() -> Vec<HostProductionReadyEvent> {
    LOG.with(|log| {
        let events = std::mem::take(&mut *log.borrow_mut());
        LAST_DRAIN.with(|last| *last.borrow_mut() = events.clone());
        events
    })
}

pub fn snapshot() -> Vec<HostProductionReadyEvent> {
    LOG.with(|log| log.borrow().clone())
}

pub fn take_last_drain() -> Vec<HostProductionReadyEvent> {
    LAST_DRAIN.with(|last| std::mem::take(&mut *last.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
    LAST_DRAIN.with(|last| last.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_drain_roundtrip() {
        clear();
        record(ObjectId(7), "USA_Ranger", false);
        record(ObjectId(8), "Upgrade_Flashbang", true);
        let d = drain();
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].producer.0, 7);
        assert!(d[1].is_upgrade);
        assert!(drain().is_empty());
        clear();
    }
}
