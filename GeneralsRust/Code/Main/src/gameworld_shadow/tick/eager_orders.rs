//! Post-logic order residuals: owner, movement, status, veterancy, turret, guard, rally, target.

use super::*;
use crate::game_logic::GameLogic;
use crate::gameworld_shadow::GameWorldShadow;

// Wave 688: post-logic owner / movement batch handoff (avoid double-apply).
thread_local! {
    static EARLY_OWNER_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_owner_log::HostOwnerEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 688: post-logic owner / movement batch handoff (avoid double-apply).
thread_local! {
    static EARLY_MOVEMENT_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_movement_log::HostMovementEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_owner_batch()
-> Option<(Vec<crate::game_logic::host_owner_log::HostOwnerEvent>, bool)> {
    EARLY_OWNER_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_movement_batch() -> Option<(
    Vec<crate::game_logic::host_movement_log::HostMovementEvent>,
    bool,
)> {
    EARLY_MOVEMENT_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 688: post-logic drain `host_owner_log` into GameWorld TransferOwner.
pub fn eager_apply_host_owner_after_logic(
    shadow: &mut GameWorldShadow,
    logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 688: post-logic owner materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_owner_log::drain();
    if events.is_empty() {
        EARLY_OWNER_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_owner_events(logic, &events);
    EARLY_OWNER_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 688: post-logic drain `host_movement_log` into GameWorld SetMovement.
pub fn eager_apply_host_movement_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 688: post-logic movement residual materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_movement_log::drain();
    if events.is_empty() {
        EARLY_MOVEMENT_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_movement_events(&events);
    EARLY_MOVEMENT_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 689: post-logic combat-status / veterancy batch handoff (avoid double-apply).
thread_local! {
    static EARLY_STATUS_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_status_log::HostStatusEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 689: post-logic combat-status / veterancy batch handoff (avoid double-apply).
thread_local! {
    static EARLY_VETERANCY_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_veterancy_log::HostVeterancyEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_status_batch() -> Option<(
    Vec<crate::game_logic::host_status_log::HostStatusEvent>,
    bool,
)> {
    EARLY_STATUS_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_veterancy_batch() -> Option<(
    Vec<crate::game_logic::host_veterancy_log::HostVeterancyEvent>,
    bool,
)> {
    EARLY_VETERANCY_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 689: post-logic drain `host_status_log` into GameWorld SetCombatStatus.
pub fn eager_apply_host_status_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 689: post-logic combat-status materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_status_log::drain();
    if events.is_empty() {
        EARLY_STATUS_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let mut n = 0usize;
    for ev in &events {
        if shadow.queue_set_combat_status_for_host(*ev) {
            n = n.saturating_add(1);
        }
    }
    if n > 0 {
        let _ = shadow.apply_pending();
    }
    EARLY_STATUS_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 689: post-logic drain `host_veterancy_log` into GameWorld SetVeterancy.
pub fn eager_apply_host_veterancy_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 689: post-logic veterancy materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_veterancy_log::drain();
    if events.is_empty() {
        EARLY_VETERANCY_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let mut n = 0usize;
    for ev in &events {
        if shadow.queue_set_veterancy_for_host(ev.object, ev.ordinal) {
            n = n.saturating_add(1);
        }
    }
    if n > 0 {
        let _ = shadow.apply_pending();
    }
    EARLY_VETERANCY_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 692: post-logic turret / guard / rally batch handoff (avoid double-apply).
thread_local! {
    static EARLY_TURRET_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_turret_log::HostTurretEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 692: post-logic turret / guard / rally batch handoff (avoid double-apply).
thread_local! {
    static EARLY_GUARD_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_guard_log::HostGuardEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 692: post-logic turret / guard / rally batch handoff (avoid double-apply).
thread_local! {
    static EARLY_RALLY_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_rally_log::HostRallyEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_turret_batch() -> Option<(
    Vec<crate::game_logic::host_turret_log::HostTurretEvent>,
    bool,
)> {
    EARLY_TURRET_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_guard_batch()
-> Option<(Vec<crate::game_logic::host_guard_log::HostGuardEvent>, bool)> {
    EARLY_GUARD_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_rally_batch()
-> Option<(Vec<crate::game_logic::host_rally_log::HostRallyEvent>, bool)> {
    EARLY_RALLY_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 692: post-logic drain `host_turret_log` into GameWorld SetTurret.
pub fn eager_apply_host_turret_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 692: post-logic turret materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_turret_log::drain();
    if events.is_empty() {
        EARLY_TURRET_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_turret_events(&events);
    EARLY_TURRET_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 692: post-logic drain `host_guard_log` into GameWorld SetGuard.
pub fn eager_apply_host_guard_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 692: post-logic guard materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_guard_log::drain();
    if events.is_empty() {
        EARLY_GUARD_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_guard_events(&events);
    EARLY_GUARD_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 692: post-logic drain `host_rally_log` into GameWorld SetRallyPoint.
pub fn eager_apply_host_rally_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 692: post-logic rally materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_rally_log::drain();
    if events.is_empty() {
        EARLY_RALLY_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_rally_events(&events);
    EARLY_RALLY_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 693: post-logic target-location / detector / continuous-fire batch handoff.
thread_local! {
    static EARLY_TARGET_LOCATION_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_target_location_log::HostTargetLocationEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_target_location_batch() -> Option<(
    Vec<crate::game_logic::host_target_location_log::HostTargetLocationEvent>,
    bool,
)> {
    EARLY_TARGET_LOCATION_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 693: post-logic drain `host_target_location_log` into GameWorld SetTargetLocation.
pub fn eager_apply_host_target_location_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 693: post-logic target-location materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_target_location_log::drain();
    if events.is_empty() {
        EARLY_TARGET_LOCATION_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_target_location_events(&events);
    EARLY_TARGET_LOCATION_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Drop unused post-logic handoff batches when the outermost couple ends.
pub(super) fn clear_early_orders_batches() {
    EARLY_OWNER_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_MOVEMENT_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_STATUS_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_VETERANCY_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_TURRET_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_GUARD_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_RALLY_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_TARGET_LOCATION_BATCH.with(|c| *c.borrow_mut() = None);
}
