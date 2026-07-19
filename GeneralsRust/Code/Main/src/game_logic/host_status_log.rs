//! Frame-local host status-flag log for GameWorld SetCombatStatus parity.

use super::ObjectId;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostStatusEvent {
    pub object: ObjectId,
    pub selected: Option<bool>,
    pub attacking: Option<bool>,
    pub moving: Option<bool>,
    pub is_firing_weapon: Option<bool>,
    pub is_aiming_weapon: Option<bool>,
    pub stealthed: Option<bool>,
    pub detected: Option<bool>,
    pub disabled_emp: Option<bool>,
    pub weapons_jammed: Option<bool>,
    pub disabled_hacked: Option<bool>,
    pub disabled_unmanned: Option<bool>,
    pub disabled_paralyzed: Option<bool>,
    pub disabled_subdued: Option<bool>,
    pub masked: Option<bool>,
    pub disguised: Option<bool>,
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
        moving: None,
        is_firing_weapon: None,
        is_aiming_weapon: None,
        stealthed: None,
        detected: None,
        disabled_emp: None,
        weapons_jammed: None,
        disabled_hacked: None,
        disabled_unmanned: None,
        disabled_paralyzed: None,
        disabled_subdued: None,
        masked: None,
        disguised: None,
    }
}

macro_rules! record_flag {
    ($name:ident, $field:ident) => {
        pub fn $name(object: ObjectId, value: bool) {
            let mut ev = empty(object);
            ev.$field = Some(value);
            push(ev);
        }
    };
}

record_flag!(record_selected, selected);
record_flag!(record_attacking, attacking);
record_flag!(record_moving, moving);
record_flag!(record_firing, is_firing_weapon);
record_flag!(record_aiming, is_aiming_weapon);
record_flag!(record_stealthed, stealthed);
record_flag!(record_detected, detected);
record_flag!(record_disabled_emp, disabled_emp);
record_flag!(record_weapons_jammed, weapons_jammed);
record_flag!(record_disabled_hacked, disabled_hacked);
record_flag!(record_disabled_unmanned, disabled_unmanned);
record_flag!(record_disabled_paralyzed, disabled_paralyzed);
record_flag!(record_disabled_subdued, disabled_subdued);
record_flag!(record_masked, masked);
record_flag!(record_disguised, disguised);

pub fn drain() -> Vec<HostStatusEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
