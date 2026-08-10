//! Post-logic stealth residuals: detector, flags, disguise, vision/camo, delay, faerie, hijacker.

use crate::game_logic::GameLogic;
use crate::gameworld_shadow::GameWorldShadow;
use super::*;

// Wave 693: post-logic target-location / detector / continuous-fire batch handoff.
thread_local! {
    static EARLY_DETECTOR_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_detector_log::HostDetectorEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_detector_batch() -> Option<(
    Vec<crate::game_logic::host_detector_log::HostDetectorEvent>,
    bool,
)> {
    EARLY_DETECTOR_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 693: post-logic drain `host_detector_log` into GameWorld SetDetector.
pub fn eager_apply_host_detector_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 693: post-logic detector materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_detector_log::drain();
    if events.is_empty() {
        EARLY_DETECTOR_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_detector_events(&events);
    EARLY_DETECTOR_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 694: post-logic ai-attitude / overcharge / stealth-flags batch handoff.
thread_local! {
    static EARLY_STEALTH_FLAGS_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_stealth_flags_log::HostStealthFlagsEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_stealth_flags_batch() -> Option<(
    Vec<crate::game_logic::host_stealth_flags_log::HostStealthFlagsEvent>,
    bool,
)> {
    EARLY_STEALTH_FLAGS_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 694: post-logic drain `host_stealth_flags_log` into GameWorld SetStealthFlags.
pub fn eager_apply_host_stealth_flags_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 694: post-logic stealth-flags materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_stealth_flags_log::drain();
    if events.is_empty() {
        EARLY_STEALTH_FLAGS_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_stealth_flags_events(&events);
    EARLY_STEALTH_FLAGS_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 696: post-logic command-set / disguise / vision-camo batch handoff.
thread_local! {
    static EARLY_DISGUISE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_disguise_log::HostDisguiseEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 696: post-logic command-set / disguise / vision-camo batch handoff.
thread_local! {
    static EARLY_VISION_CAMO_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_vision_camo_log::HostVisionCamoEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_disguise_batch() -> Option<(
    Vec<crate::game_logic::host_disguise_log::HostDisguiseEvent>,
    bool,
)> {
    EARLY_DISGUISE_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_vision_camo_batch() -> Option<(
    Vec<crate::game_logic::host_vision_camo_log::HostVisionCamoEvent>,
    bool,
)> {
    EARLY_VISION_CAMO_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 696: post-logic drain `host_disguise_log` into GameWorld SetDisguise.
pub fn eager_apply_host_disguise_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 696: post-logic disguise materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_disguise_log::drain();
    if events.is_empty() {
        EARLY_DISGUISE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_disguise_events(&events);
    EARLY_DISGUISE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 696: post-logic drain `host_vision_camo_log` into GameWorld SetVisionCamo.
pub fn eager_apply_host_vision_camo_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 696: post-logic vision-camo materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_vision_camo_log::drain();
    if events.is_empty() {
        EARLY_VISION_CAMO_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_vision_camo_events(&events);
    EARLY_VISION_CAMO_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 701: post-logic faerie-fire / repulsor / disable-timers batch handoff.
thread_local! {
    static EARLY_FAERIE_FIRE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_faerie_fire_log::HostFaerieFireEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_faerie_fire_batch() -> Option<(
    Vec<crate::game_logic::host_faerie_fire_log::HostFaerieFireEvent>,
    bool,
)> {
    EARLY_FAERIE_FIRE_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 701: post-logic drain `host_faerie_fire_log` into GameWorld SetFaerieFire.
pub fn eager_apply_host_faerie_fire_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 701: post-logic faerie-fire materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_faerie_fire_log::drain();
    if events.is_empty() {
        EARLY_FAERIE_FIRE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_faerie_fire_events(&events);
    EARLY_FAERIE_FIRE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 705: post-logic stealth-delay / sole-healing / radar-extend batch handoff.
thread_local! {
    static EARLY_STEALTH_DELAY_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_stealth_delay_log::HostStealthDelayEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_stealth_delay_batch() -> Option<(
    Vec<crate::game_logic::host_stealth_delay_log::HostStealthDelayEvent>,
    bool,
)> {
    EARLY_STEALTH_DELAY_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 705: post-logic drain `host_stealth_delay_log` into GameWorld SetStealthDelay.
pub fn eager_apply_host_stealth_delay_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 705: post-logic stealth-delay materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_stealth_delay_log::drain();
    if events.is_empty() {
        EARLY_STEALTH_DELAY_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_stealth_delay_events(&events);
    EARLY_STEALTH_DELAY_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 706: post-logic hijacker / rebuild-producer / stored-supplies batch handoff.
thread_local! {
    static EARLY_HIJACKER_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_hijacker_log::HostHijackerEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_hijacker_batch() -> Option<(
    Vec<crate::game_logic::host_hijacker_log::HostHijackerEvent>,
    bool,
)> {
    EARLY_HIJACKER_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 706: post-logic drain `host_hijacker_log` into GameWorld SetHijacker.
pub fn eager_apply_host_hijacker_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 706: post-logic hijacker materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_hijacker_log::drain();
    if events.is_empty() {
        EARLY_HIJACKER_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_hijacker_events(&events);
    EARLY_HIJACKER_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Drop unused post-logic handoff batches when the outermost couple ends.
pub(super) fn clear_early_stealth_batches() {
    EARLY_DETECTOR_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_STEALTH_FLAGS_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_DISGUISE_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_VISION_CAMO_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_FAERIE_FIRE_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_STEALTH_DELAY_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_HIJACKER_BATCH.with(|c| *c.borrow_mut() = None);
}
