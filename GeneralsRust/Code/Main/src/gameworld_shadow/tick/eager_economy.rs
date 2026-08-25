//! Post-logic economy residuals: special power, radar, player, production, construction, spawn.

use super::*;
use crate::game_logic::GameLogic;
use crate::gameworld_shadow::GameWorldShadow;

// Wave 694: post-logic ai-attitude / overcharge / stealth-flags batch handoff.
thread_local! {
    static EARLY_OVERCHARGE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_overcharge_log::HostOverchargeEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_overcharge_batch() -> Option<(
    Vec<crate::game_logic::host_overcharge_log::HostOverchargeEvent>,
    bool,
)> {
    EARLY_OVERCHARGE_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 694: post-logic drain `host_overcharge_log` into GameWorld SetOvercharge.
pub fn eager_apply_host_overcharge_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 694: post-logic overcharge materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_overcharge_log::drain();
    if events.is_empty() {
        EARLY_OVERCHARGE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_overcharge_events(&events);
    EARLY_OVERCHARGE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 707: post-logic special-power / radar / player-progress batch handoff.
thread_local! {
    static EARLY_SPECIAL_POWER_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_special_power_log::HostSpecialPowerEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 707: post-logic special-power / radar / player-progress batch handoff.
thread_local! {
    static EARLY_RADAR_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_radar_log::HostRadarEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 707: post-logic special-power / radar / player-progress batch handoff.
thread_local! {
    static EARLY_PLAYER_PROGRESS_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_player_progress_log::HostPlayerProgressEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_special_power_batch() -> Option<(
    Vec<crate::game_logic::host_special_power_log::HostSpecialPowerEvent>,
    bool,
)> {
    EARLY_SPECIAL_POWER_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_radar_batch()
-> Option<(Vec<crate::game_logic::host_radar_log::HostRadarEvent>, bool)> {
    EARLY_RADAR_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_player_progress_batch() -> Option<(
    Vec<crate::game_logic::host_player_progress_log::HostPlayerProgressEvent>,
    bool,
)> {
    EARLY_PLAYER_PROGRESS_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 707: post-logic drain `host_special_power_log` into GameWorld SetSpecialPower.
pub fn eager_apply_host_special_power_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 707: post-logic special-power materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_special_power_log::drain();
    if events.is_empty() {
        EARLY_SPECIAL_POWER_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_special_power_events(&events);
    EARLY_SPECIAL_POWER_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 707: post-logic drain `host_radar_log` into GameWorld SetRadar.
pub fn eager_apply_host_radar_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 707: post-logic radar materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_radar_log::drain();
    if events.is_empty() {
        EARLY_RADAR_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_radar_events(&events);
    EARLY_RADAR_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 707: post-logic drain `host_player_progress_log` into GameWorld SetPlayerProgress.
pub fn eager_apply_host_player_progress_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 707: post-logic player-progress materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_player_progress_log::drain();
    if events.is_empty() {
        EARLY_PLAYER_PROGRESS_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_player_progress_events(&events);
    EARLY_PLAYER_PROGRESS_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 708: post-logic player-meta / player-cooldown / production-door batch handoff.
thread_local! {
    static EARLY_PLAYER_META_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_player_meta_log::HostPlayerMetaEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 708: post-logic player-meta / player-cooldown / production-door batch handoff.
thread_local! {
    static EARLY_PLAYER_COOLDOWN_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_player_cooldown_log::HostPlayerCooldownEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 708: post-logic player-meta / player-cooldown / production-door batch handoff.
thread_local! {
    static EARLY_PRODUCTION_DOOR_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_production_door_log::HostProductionDoorEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_player_meta_batch() -> Option<(
    Vec<crate::game_logic::host_player_meta_log::HostPlayerMetaEvent>,
    bool,
)> {
    EARLY_PLAYER_META_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_player_cooldown_batch() -> Option<(
    Vec<crate::game_logic::host_player_cooldown_log::HostPlayerCooldownEvent>,
    bool,
)> {
    EARLY_PLAYER_COOLDOWN_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_production_door_batch() -> Option<(
    Vec<crate::game_logic::host_production_door_log::HostProductionDoorEvent>,
    bool,
)> {
    EARLY_PRODUCTION_DOOR_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 708: post-logic drain `host_player_meta_log` into GameWorld player meta.
pub fn eager_apply_host_player_meta_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 708: post-logic player-meta materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_player_meta_log::drain();
    if events.is_empty() {
        EARLY_PLAYER_META_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_player_meta_events(&events);
    EARLY_PLAYER_META_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 708: post-logic drain `host_player_cooldown_log` into GameWorld player cooldowns.
pub fn eager_apply_host_player_cooldown_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 708: post-logic player-cooldown materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_player_cooldown_log::drain();
    if events.is_empty() {
        EARLY_PLAYER_COOLDOWN_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_player_cooldown_events(&events);
    EARLY_PLAYER_COOLDOWN_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 708: post-logic drain `host_production_door_log` into GameWorld SetProductionDoor.
pub fn eager_apply_host_production_door_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 708: post-logic production-door materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_production_door_log::drain();
    if events.is_empty() {
        EARLY_PRODUCTION_DOOR_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_production_door_events(&events);
    EARLY_PRODUCTION_DOOR_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 709: post-logic production / production-progress / construction batch handoff.
thread_local! {
    static EARLY_PRODUCTION_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_production_log::HostProductionEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 709: post-logic production / production-progress / construction batch handoff.
thread_local! {
    static EARLY_PRODUCTION_PROGRESS_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_production_progress_log::HostProductionProgressEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 709: post-logic production / production-progress / construction batch handoff.
thread_local! {
    static EARLY_CONSTRUCTION_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_construction_log::HostConstructionEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 709: post-logic production / production-progress / construction batch handoff.
thread_local! {
    static EARLY_CONSTRUCTION_PROGRESS_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_construction_progress_log::HostConstructionProgressEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_production_batch() -> Option<(
    Vec<crate::game_logic::host_production_log::HostProductionEvent>,
    bool,
)> {
    EARLY_PRODUCTION_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_production_progress_batch() -> Option<(
    Vec<crate::game_logic::host_production_progress_log::HostProductionProgressEvent>,
    bool,
)> {
    EARLY_PRODUCTION_PROGRESS_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_construction_batch() -> Option<(
    Vec<crate::game_logic::host_construction_log::HostConstructionEvent>,
    bool,
)> {
    EARLY_CONSTRUCTION_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_construction_progress_batch() -> Option<(
    Vec<crate::game_logic::host_construction_progress_log::HostConstructionProgressEvent>,
    bool,
)> {
    EARLY_CONSTRUCTION_PROGRESS_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 709: post-logic drain `host_production_log` into GameWorld production mutations.
pub fn eager_apply_host_production_after_logic(
    shadow: &mut GameWorldShadow,
    logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 709: post-logic production materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_production_log::drain();
    if events.is_empty() {
        EARLY_PRODUCTION_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_production_events(&events, logic);
    EARLY_PRODUCTION_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 709: post-logic drain `host_production_progress_log` into GameWorld production progress.
pub fn eager_apply_host_production_progress_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 709: post-logic production-progress materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_production_progress_log::drain();
    if events.is_empty() {
        EARLY_PRODUCTION_PROGRESS_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_production_progress_events(&events);
    EARLY_PRODUCTION_PROGRESS_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 709: post-logic drain `host_construction_log` into GameWorld construction.
pub fn eager_apply_host_construction_after_logic(
    shadow: &mut GameWorldShadow,
    logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 709: post-logic construction materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_construction_log::drain();
    if events.is_empty() {
        EARLY_CONSTRUCTION_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_construction_events(&events, logic);
    EARLY_CONSTRUCTION_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 709: post-logic drain `host_construction_progress_log` into GameWorld construction progress.
pub fn eager_apply_host_construction_progress_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 709: post-logic construction-progress materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_construction_progress_log::drain();
    if events.is_empty() {
        EARLY_CONSTRUCTION_PROGRESS_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_construction_progress_events(&events);
    EARLY_CONSTRUCTION_PROGRESS_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 712: post-logic spawn batch handoff (complements mid-frame eager_map).
thread_local! {
    static EARLY_SPAWN_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_spawn_log::HostSpawnEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_spawn_batch()
-> Option<(Vec<crate::game_logic::host_spawn_log::HostSpawnEvent>, bool)> {
    EARLY_SPAWN_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 712: post-logic drain remaining `host_spawn_log` into GameWorld (idempotent with mid-frame map).
pub fn eager_apply_host_spawn_after_logic(
    shadow: &mut GameWorldShadow,
    logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 712: post-logic spawn materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_spawn_log::drain();
    if events.is_empty() {
        EARLY_SPAWN_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_spawn_events(&events, logic);
    EARLY_SPAWN_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Drop unused post-logic handoff batches when the outermost couple ends.
pub(super) fn clear_early_economy_batches() {
    EARLY_OVERCHARGE_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_SPECIAL_POWER_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_RADAR_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_PLAYER_PROGRESS_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_PLAYER_META_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_PLAYER_COOLDOWN_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_PRODUCTION_DOOR_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_PRODUCTION_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_PRODUCTION_PROGRESS_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_CONSTRUCTION_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_CONSTRUCTION_PROGRESS_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_SPAWN_BATCH.with(|c| *c.borrow_mut() = None);
}
