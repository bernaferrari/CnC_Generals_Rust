//! Post-logic misc residuals: repulsor, timers, body/death, physics, locomotor, supplies.

use super::*;
use crate::game_logic::GameLogic;
use crate::gameworld_shadow::GameWorldShadow;

// Wave 701: post-logic faerie-fire / repulsor / disable-timers batch handoff.
thread_local! {
    static EARLY_REPULSOR_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_repulsor_log::HostRepulsorEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 701: post-logic faerie-fire / repulsor / disable-timers batch handoff.
thread_local! {
    static EARLY_DISABLE_TIMERS_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_disable_timers_log::HostDisableTimersEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_repulsor_batch() -> Option<(
    Vec<crate::game_logic::host_repulsor_log::HostRepulsorEvent>,
    bool,
)> {
    EARLY_REPULSOR_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_disable_timers_batch() -> Option<(
    Vec<crate::game_logic::host_disable_timers_log::HostDisableTimersEvent>,
    bool,
)> {
    EARLY_DISABLE_TIMERS_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 701: post-logic drain `host_repulsor_log` into GameWorld SetRepulsor.
pub fn eager_apply_host_repulsor_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 701: post-logic repulsor materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_repulsor_log::drain();
    if events.is_empty() {
        EARLY_REPULSOR_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_repulsor_events(&events);
    EARLY_REPULSOR_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 701: post-logic drain `host_disable_timers_log` into GameWorld SetDisableTimers.
pub fn eager_apply_host_disable_timers_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 701: post-logic disable-timers materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_disable_timers_log::drain();
    if events.is_empty() {
        EARLY_DISABLE_TIMERS_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_disable_timers_events(&events);
    EARLY_DISABLE_TIMERS_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 702: post-logic body-damage / death-type / physics-motive batch handoff.
thread_local! {
    static EARLY_BODY_DAMAGE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_body_damage_log::HostBodyDamageEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 702: post-logic body-damage / death-type / physics-motive batch handoff.
thread_local! {
    static EARLY_DEATH_TYPE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_death_type_log::HostDeathTypeEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 702: post-logic body-damage / death-type / physics-motive batch handoff.
thread_local! {
    static EARLY_PHYSICS_MOTIVE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_physics_motive_log::HostPhysicsMotiveEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_body_damage_batch() -> Option<(
    Vec<crate::game_logic::host_body_damage_log::HostBodyDamageEvent>,
    bool,
)> {
    EARLY_BODY_DAMAGE_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_death_type_batch() -> Option<(
    Vec<crate::game_logic::host_death_type_log::HostDeathTypeEvent>,
    bool,
)> {
    EARLY_DEATH_TYPE_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_physics_motive_batch() -> Option<(
    Vec<crate::game_logic::host_physics_motive_log::HostPhysicsMotiveEvent>,
    bool,
)> {
    EARLY_PHYSICS_MOTIVE_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 702: post-logic drain `host_body_damage_log` into GameWorld SetBodyDamage.
pub fn eager_apply_host_body_damage_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 702: post-logic body-damage materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_body_damage_log::drain();
    if events.is_empty() {
        EARLY_BODY_DAMAGE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_body_damage_events(&events);
    EARLY_BODY_DAMAGE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 702: post-logic drain `host_death_type_log` into GameWorld SetDeathType.
pub fn eager_apply_host_death_type_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 702: post-logic death-type materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_death_type_log::drain();
    if events.is_empty() {
        EARLY_DEATH_TYPE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_death_type_events(&events);
    EARLY_DEATH_TYPE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 702: post-logic drain `host_physics_motive_log` into GameWorld SetPhysicsMotive.
pub fn eager_apply_host_physics_motive_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 702: post-logic physics-motive materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_physics_motive_log::drain();
    if events.is_empty() {
        EARLY_PHYSICS_MOTIVE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_physics_motive_events(&events);
    EARLY_PHYSICS_MOTIVE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 703: post-logic locomotor / bounce-land batch handoff.
thread_local! {
    static EARLY_LOCOMOTOR_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_locomotor_log::HostLocomotorEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 703: post-logic locomotor / bounce-land batch handoff.
thread_local! {
    static EARLY_BOUNCE_LAND_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_bounce_land_log::HostBounceLandEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_locomotor_batch() -> Option<(
    Vec<crate::game_logic::host_locomotor_log::HostLocomotorEvent>,
    bool,
)> {
    EARLY_LOCOMOTOR_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_bounce_land_batch() -> Option<(
    Vec<crate::game_logic::host_bounce_land_log::HostBounceLandEvent>,
    bool,
)> {
    EARLY_BOUNCE_LAND_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 703: post-logic drain `host_locomotor_log` into GameWorld SetLocomotor.
pub fn eager_apply_host_locomotor_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 703: post-logic locomotor materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_locomotor_log::drain();
    if events.is_empty() {
        EARLY_LOCOMOTOR_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_locomotor_events(&events);
    EARLY_LOCOMOTOR_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 703: post-logic drain `host_bounce_land_log` into GameWorld SetBounceLand.
pub fn eager_apply_host_bounce_land_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 703: post-logic bounce-land materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_bounce_land_log::drain();
    if events.is_empty() {
        EARLY_BOUNCE_LAND_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_bounce_land_events(&events);
    EARLY_BOUNCE_LAND_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 705: post-logic stealth-delay / sole-healing / radar-extend batch handoff.
thread_local! {
    static EARLY_SOLE_HEALING_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_sole_healing_log::HostSoleHealingEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 705: post-logic stealth-delay / sole-healing / radar-extend batch handoff.
thread_local! {
    static EARLY_RADAR_EXTEND_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_radar_extend_log::HostRadarExtendEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_sole_healing_batch() -> Option<(
    Vec<crate::game_logic::host_sole_healing_log::HostSoleHealingEvent>,
    bool,
)> {
    EARLY_SOLE_HEALING_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_radar_extend_batch() -> Option<(
    Vec<crate::game_logic::host_radar_extend_log::HostRadarExtendEvent>,
    bool,
)> {
    EARLY_RADAR_EXTEND_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 705: post-logic drain `host_sole_healing_log` into GameWorld SetSoleHealing.
pub fn eager_apply_host_sole_healing_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 705: post-logic sole-healing materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_sole_healing_log::drain();
    if events.is_empty() {
        EARLY_SOLE_HEALING_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_sole_healing_events(&events);
    EARLY_SOLE_HEALING_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 705: post-logic drain `host_radar_extend_log` into GameWorld SetRadarExtend.
pub fn eager_apply_host_radar_extend_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 705: post-logic radar-extend materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_radar_extend_log::drain();
    if events.is_empty() {
        EARLY_RADAR_EXTEND_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_radar_extend_events(&events);
    EARLY_RADAR_EXTEND_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 706: post-logic hijacker / rebuild-producer / stored-supplies batch handoff.
thread_local! {
    static EARLY_REBUILD_PRODUCER_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_rebuild_producer_log::HostRebuildProducerEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 706: post-logic hijacker / rebuild-producer / stored-supplies batch handoff.
thread_local! {
    static EARLY_STORED_SUPPLIES_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_stored_supplies_log::HostStoredSuppliesEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_rebuild_producer_batch() -> Option<(
    Vec<crate::game_logic::host_rebuild_producer_log::HostRebuildProducerEvent>,
    bool,
)> {
    EARLY_REBUILD_PRODUCER_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_stored_supplies_batch() -> Option<(
    Vec<crate::game_logic::host_stored_supplies_log::HostStoredSuppliesEvent>,
    bool,
)> {
    EARLY_STORED_SUPPLIES_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 706: post-logic drain `host_rebuild_producer_log` into GameWorld SetRebuildProducer.
pub fn eager_apply_host_rebuild_producer_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 706: post-logic rebuild-producer materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_rebuild_producer_log::drain();
    if events.is_empty() {
        EARLY_REBUILD_PRODUCER_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_rebuild_producer_events(&events);
    EARLY_REBUILD_PRODUCER_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 706: post-logic drain `host_stored_supplies_log` into GameWorld SetStoredSupplies.
pub fn eager_apply_host_stored_supplies_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 706: post-logic stored-supplies materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_stored_supplies_log::drain();
    if events.is_empty() {
        EARLY_STORED_SUPPLIES_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_stored_supplies_events(&events);
    EARLY_STORED_SUPPLIES_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Drop unused post-logic handoff batches when the outermost couple ends.
pub(super) fn clear_early_misc_batches() {
    EARLY_REPULSOR_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_DISABLE_TIMERS_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_BODY_DAMAGE_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_DEATH_TYPE_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_PHYSICS_MOTIVE_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_LOCOMOTOR_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_BOUNCE_LAND_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_SOLE_HEALING_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_RADAR_EXTEND_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_REBUILD_PRODUCER_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_STORED_SUPPLIES_BATCH.with(|c| *c.borrow_mut() = None);
}
