//! Frame-local host status-flag log for GameWorld SetCombatStatus parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostStatusEvent {
    pub object: ObjectId,
    pub selected: Option<bool>,
    pub attacking: Option<bool>,
    pub is_firing_weapon: Option<bool>,
    pub is_aiming_weapon: Option<bool>,
    pub stealthed: Option<bool>,
    pub detected: Option<bool>,
}

thread_local! {
    static LOG: RefCell<Vec<HostStatusEvent>> = RefCell::new(Vec::new());
}

fn push(ev: HostStatusEvent) {
    LOG.with(|log| log.borrow_mut().push(ev));
}

pub fn record_selected(object: ObjectId, selected: bool) {
    push(HostStatusEvent {
        object,
        selected: Some(selected),
        attacking: None,
        is_firing_weapon: None,
        is_aiming_weapon: None,
        stealthed: None,
        detected: None,
    });
}

pub fn record_attacking(object: ObjectId, attacking: bool) {
    push(HostStatusEvent {
        object,
        selected: None,
        attacking: Some(attacking),
        is_firing_weapon: None,
        is_aiming_weapon: None,
        stealthed: None,
        detected: None,
    });
}

pub fn record_firing(object: ObjectId, is_firing_weapon: bool) {
    push(HostStatusEvent {
        object,
        selected: None,
        attacking: None,
        is_firing_weapon: Some(is_firing_weapon),
        is_aiming_weapon: None,
        stealthed: None,
        detected: None,
    });
}

pub fn record_aiming(object: ObjectId, is_aiming_weapon: bool) {
    push(HostStatusEvent {
        object,
        selected: None,
        attacking: None,
        is_firing_weapon: None,
        is_aiming_weapon: Some(is_aiming_weapon),
        stealthed: None,
        detected: None,
    });
}

pub fn drain() -> Vec<HostStatusEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
