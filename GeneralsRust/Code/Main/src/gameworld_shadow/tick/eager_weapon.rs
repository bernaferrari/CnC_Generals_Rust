//! Post-logic weapon residuals: bonus/slot/set/stats, entity power, continuous fire.

use super::*;
use crate::game_logic::GameLogic;
use crate::gameworld_shadow::GameWorldShadow;

// Wave 690: post-logic weapon-bonus / weapon-slot batch handoff (avoid double-apply).
thread_local! {
    static EARLY_WEAPON_BONUS_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_weapon_bonus_log::HostWeaponBonusEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 690: post-logic weapon-bonus / weapon-slot batch handoff (avoid double-apply).
thread_local! {
    static EARLY_WEAPON_SLOT_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_weapon_slot_log::HostWeaponSlotEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_weapon_bonus_batch() -> Option<(
    Vec<crate::game_logic::host_weapon_bonus_log::HostWeaponBonusEvent>,
    bool,
)> {
    EARLY_WEAPON_BONUS_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_weapon_slot_batch() -> Option<(
    Vec<crate::game_logic::host_weapon_slot_log::HostWeaponSlotEvent>,
    bool,
)> {
    EARLY_WEAPON_SLOT_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 690: post-logic drain `host_weapon_bonus_log` into GameWorld SetWeaponBonus.
pub fn eager_apply_host_weapon_bonus_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 690: post-logic weapon-bonus materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_weapon_bonus_log::drain();
    if events.is_empty() {
        EARLY_WEAPON_BONUS_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_weapon_bonus_events(&events);
    EARLY_WEAPON_BONUS_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 690: post-logic drain `host_weapon_slot_log` into GameWorld SetActiveWeaponSlot.
pub fn eager_apply_host_weapon_slot_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 690: post-logic weapon-slot materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_weapon_slot_log::drain();
    if events.is_empty() {
        EARLY_WEAPON_SLOT_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_weapon_slot_events(&events);
    EARLY_WEAPON_SLOT_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 691: post-logic weapon-set / entity-power batch handoff (avoid double-apply).
thread_local! {
    static EARLY_WEAPON_SET_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_weapon_set_log::HostWeaponSetEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 691: post-logic weapon-set / entity-power batch handoff (avoid double-apply).
thread_local! {
    static EARLY_ENTITY_POWER_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_entity_power_log::HostEntityPowerEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_weapon_set_batch() -> Option<(
    Vec<crate::game_logic::host_weapon_set_log::HostWeaponSetEvent>,
    bool,
)> {
    EARLY_WEAPON_SET_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_entity_power_batch() -> Option<(
    Vec<crate::game_logic::host_entity_power_log::HostEntityPowerEvent>,
    bool,
)> {
    EARLY_ENTITY_POWER_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 691: post-logic drain `host_weapon_set_log` into GameWorld SetWeaponSetFlags.
pub fn eager_apply_host_weapon_set_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 691: post-logic weapon-set materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_weapon_set_log::drain();
    if events.is_empty() {
        EARLY_WEAPON_SET_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_weapon_set_events(&events);
    EARLY_WEAPON_SET_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 691: post-logic drain `host_entity_power_log` into GameWorld SetEntityPower.
pub fn eager_apply_host_entity_power_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 691: post-logic entity-power materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_entity_power_log::drain();
    if events.is_empty() {
        EARLY_ENTITY_POWER_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_entity_power_events(&events);
    EARLY_ENTITY_POWER_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 693: post-logic target-location / detector / continuous-fire batch handoff.
thread_local! {
    static EARLY_CONTINUOUS_FIRE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_continuous_fire_log::HostContinuousFireEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_continuous_fire_batch() -> Option<(
    Vec<crate::game_logic::host_continuous_fire_log::HostContinuousFireEvent>,
    bool,
)> {
    EARLY_CONTINUOUS_FIRE_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 693: post-logic drain `host_continuous_fire_log` into GameWorld SetContinuousFire.
pub fn eager_apply_host_continuous_fire_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 693: post-logic continuous-fire materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_continuous_fire_log::drain();
    if events.is_empty() {
        EARLY_CONTINUOUS_FIRE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_continuous_fire_events(&events);
    EARLY_CONTINUOUS_FIRE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 697: post-logic weapon-stats / selection-radius / model-condition batch handoff.
thread_local! {
    static EARLY_WEAPON_STATS_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_weapon_stats_log::HostWeaponStatsEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_weapon_stats_batch() -> Option<(
    Vec<crate::game_logic::host_weapon_stats_log::HostWeaponStatsEvent>,
    bool,
)> {
    EARLY_WEAPON_STATS_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 697: post-logic drain `host_weapon_stats_log` into GameWorld SetWeaponStats.
pub fn eager_apply_host_weapon_stats_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 697: post-logic weapon-stats materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_weapon_stats_log::drain();
    if events.is_empty() {
        EARLY_WEAPON_STATS_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_weapon_stats_events(&events);
    EARLY_WEAPON_STATS_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Drop unused post-logic handoff batches when the outermost couple ends.
pub(super) fn clear_early_weapon_batches() {
    EARLY_WEAPON_BONUS_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_WEAPON_SLOT_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_WEAPON_SET_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_ENTITY_POWER_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_CONTINUOUS_FIRE_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_WEAPON_STATS_BATCH.with(|c| *c.borrow_mut() = None);
}
