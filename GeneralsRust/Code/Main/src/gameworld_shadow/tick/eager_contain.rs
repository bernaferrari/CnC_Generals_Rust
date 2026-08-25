//! Post-logic contain residuals: capacity, hive, overlord, garrison/contain.

use super::*;
use crate::game_logic::GameLogic;
use crate::gameworld_shadow::GameWorldShadow;

// Wave 695: post-logic contain-capacity / hive / overlord batch handoff.
thread_local! {
    static EARLY_CONTAIN_CAPACITY_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_contain_capacity_log::HostContainCapacityEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 695: post-logic contain-capacity / hive / overlord batch handoff.
thread_local! {
    static EARLY_HIVE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_hive_log::HostHiveEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 695: post-logic contain-capacity / hive / overlord batch handoff.
thread_local! {
    static EARLY_OVERLORD_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_overlord_log::HostOverlordEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_contain_capacity_batch() -> Option<(
    Vec<crate::game_logic::host_contain_capacity_log::HostContainCapacityEvent>,
    bool,
)> {
    EARLY_CONTAIN_CAPACITY_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_hive_batch()
-> Option<(Vec<crate::game_logic::host_hive_log::HostHiveEvent>, bool)> {
    EARLY_HIVE_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_overlord_batch() -> Option<(
    Vec<crate::game_logic::host_overlord_log::HostOverlordEvent>,
    bool,
)> {
    EARLY_OVERLORD_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 695: post-logic drain `host_contain_capacity_log` into GameWorld SetContainCapacity.
pub fn eager_apply_host_contain_capacity_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 695: post-logic contain-capacity materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_contain_capacity_log::drain();
    if events.is_empty() {
        EARLY_CONTAIN_CAPACITY_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_contain_capacity_events(&events);
    EARLY_CONTAIN_CAPACITY_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 695: post-logic drain `host_hive_log` into GameWorld SetHiveSlaves.
pub fn eager_apply_host_hive_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 695: post-logic hive materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_hive_log::drain();
    if events.is_empty() {
        EARLY_HIVE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_hive_events(&events);
    EARLY_HIVE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 695: post-logic drain `host_overlord_log` into GameWorld SetOverlordAddon.
pub fn eager_apply_host_overlord_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 695: post-logic overlord materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_overlord_log::drain();
    if events.is_empty() {
        EARLY_OVERLORD_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_overlord_events(&events);
    EARLY_OVERLORD_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 711: post-logic destroy / contain / AI-decision batch handoff.
thread_local! {
    static EARLY_CONTAIN_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_contain_log::HostContainEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_contain_batch() -> Option<(
    Vec<crate::game_logic::host_contain_log::HostContainEvent>,
    bool,
)> {
    EARLY_CONTAIN_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 711: post-logic drain `host_contain_log` into GameWorld contain/garrison.
pub fn eager_apply_host_contain_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 711: post-logic contain materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_contain_log::drain();
    if events.is_empty() {
        EARLY_CONTAIN_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_contain_events(&events);
    EARLY_CONTAIN_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Drop unused post-logic handoff batches when the outermost couple ends.
pub(super) fn clear_early_contain_batches() {
    EARLY_CONTAIN_CAPACITY_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_HIVE_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_OVERLORD_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_CONTAIN_BATCH.with(|c| *c.borrow_mut() = None);
}
