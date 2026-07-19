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
    pub disabled_emp: Option<bool>,
    pub weapons_jammed: Option<bool>,
}

thread_local! {
    static LOG: RefCell<Vec<HostStatusEvent>> = RefCell::new(Vec::new());
}

fn push(ev: HostStatusEvent) {
    LOG.with(|log| log.borrow_mut().push(ev));
}

fn empty(object: ObjectId) -> HostStatusEvent {
    HostStatusEvent {
        object,
        selected: None,
        attacking: None,
        is_firing_weapon: None,
        is_aiming_weapon: None,
        stealthed: None,
        detected: None,
        disabled_emp: None,
        weapons_jammed: None,
    }
}

pub fn record_selected(object: ObjectId, selected: bool) {
    let mut ev = empty(object);
    ev.selected = Some(selected);
    push(ev);
}

pub fn record_attacking(object: ObjectId, attacking: bool) {
    let mut ev = empty(object);
    ev.attacking = Some(attacking);
    push(ev);
}

pub fn record_firing(object: ObjectId, is_firing_weapon: bool) {
    let mut ev = empty(object);
    ev.is_firing_weapon = Some(is_firing_weapon);
    push(ev);
}

pub fn record_aiming(object: ObjectId, is_aiming_weapon: bool) {
    let mut ev = empty(object);
    ev.is_aiming_weapon = Some(is_aiming_weapon);
    push(ev);
}

pub fn record_stealthed(object: ObjectId, stealthed: bool) {
    let mut ev = empty(object);
    ev.stealthed = Some(stealthed);
    push(ev);
}

pub fn record_detected(object: ObjectId, detected: bool) {
    let mut ev = empty(object);
    ev.detected = Some(detected);
    push(ev);
}

pub fn record_disabled_emp(object: ObjectId, disabled_emp: bool) {
    let mut ev = empty(object);
    ev.disabled_emp = Some(disabled_emp);
    push(ev);
}

pub fn record_weapons_jammed(object: ObjectId, weapons_jammed: bool) {
    let mut ev = empty(object);
    ev.weapons_jammed = Some(weapons_jammed);
    push(ev);
}

pub fn drain() -> Vec<HostStatusEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
