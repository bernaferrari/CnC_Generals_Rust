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
    pub no_collisions: Option<bool>,
    pub private_captured: Option<bool>,
    pub disguise_transitioning_to: Option<bool>,
    pub disguise_halfpoint_reached: Option<bool>,
    pub faerie_fire: Option<bool>,
    pub booby_trapped: Option<bool>,
    pub eject_invulnerable: Option<bool>,
    pub pilot_did_move_to_base: Option<bool>,
    pub parachuting: Option<bool>,
    pub parachute_open: Option<bool>,
    pub parachute_landing_override_set: Option<bool>,
    pub using_ability: Option<bool>,
    pub deployed: Option<bool>,
    pub under_construction: Option<bool>,
    pub sold: Option<bool>,
    pub reconstructing: Option<bool>,
    pub unselectable: Option<bool>,
    pub ignoring_stealth: Option<bool>,
    pub repulsor: Option<bool>,
    pub disabled_underpowered: Option<bool>,
    pub disabled_freefall: Option<bool>,
    pub is_carbomb: Option<bool>,
    pub hijacked: Option<bool>,
    pub force_attack: Option<bool>,
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
        no_collisions: None,
        private_captured: None,
        disguise_transitioning_to: None,
        disguise_halfpoint_reached: None,
        faerie_fire: None,
        booby_trapped: None,
        eject_invulnerable: None,
        pilot_did_move_to_base: None,
        parachuting: None,
        parachute_open: None,
        parachute_landing_override_set: None,
        using_ability: None,
        deployed: None,
        under_construction: None,
        sold: None,
        reconstructing: None,
        unselectable: None,
        ignoring_stealth: None,
        repulsor: None,
        disabled_underpowered: None,
        disabled_freefall: None,
        is_carbomb: None,
        hijacked: None,
        force_attack: None,
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
record_flag!(record_no_collisions, no_collisions);
record_flag!(record_private_captured, private_captured);
record_flag!(record_disguise_transitioning_to, disguise_transitioning_to);
record_flag!(
    record_disguise_halfpoint_reached,
    disguise_halfpoint_reached
);
record_flag!(record_faerie_fire, faerie_fire);
record_flag!(record_booby_trapped, booby_trapped);
record_flag!(record_eject_invulnerable, eject_invulnerable);
record_flag!(record_pilot_did_move_to_base, pilot_did_move_to_base);
record_flag!(record_parachuting, parachuting);
record_flag!(record_parachute_open, parachute_open);
record_flag!(record_using_ability, using_ability);
record_flag!(record_deployed, deployed);
record_flag!(record_under_construction, under_construction);
record_flag!(record_sold, sold);
record_flag!(record_reconstructing, reconstructing);
record_flag!(record_unselectable, unselectable);
record_flag!(record_ignoring_stealth, ignoring_stealth);
record_flag!(record_repulsor, repulsor);
record_flag!(record_disabled_underpowered, disabled_underpowered);
record_flag!(record_disabled_freefall, disabled_freefall);
record_flag!(record_is_carbomb, is_carbomb);
record_flag!(record_hijacked, hijacked);
record_flag!(record_force_attack, force_attack);
record_flag!(
    record_parachute_landing_override_set,
    parachute_landing_override_set
);

pub fn drain() -> Vec<HostStatusEvent> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

pub fn clear() {
    LOG.with(|log| log.borrow_mut().clear());
}
