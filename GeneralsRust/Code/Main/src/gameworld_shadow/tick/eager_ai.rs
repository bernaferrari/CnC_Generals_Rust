//! Post-logic AI residuals: state, fire-intent, attitude, mood, request, shock/stun, decision.

use super::*;
use crate::game_logic::GameLogic;
use crate::gameworld_shadow::GameWorldShadow;

// Wave 687: post-logic AI-state / fire-intent batch handoff (avoid double-apply).
thread_local! {
    static EARLY_AI_STATE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_ai_state_log::HostAiStateEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 687: post-logic AI-state / fire-intent batch handoff (avoid double-apply).
thread_local! {
    static EARLY_FIRE_INTENT_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_fire_intent_log::HostFireIntentEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_ai_state_batch() -> Option<(
    Vec<crate::game_logic::host_ai_state_log::HostAiStateEvent>,
    bool,
)> {
    EARLY_AI_STATE_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_fire_intent_batch() -> Option<(
    Vec<crate::game_logic::host_fire_intent_log::HostFireIntentEvent>,
    bool,
)> {
    EARLY_FIRE_INTENT_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 687: post-logic drain `host_ai_state_log` into GameWorld SetAiState.
pub fn eager_apply_host_ai_state_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 687: post-logic AI-state materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_ai_state_log::drain();
    if events.is_empty() {
        EARLY_AI_STATE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_ai_state_events(&events);
    EARLY_AI_STATE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 687: post-logic drain `host_fire_intent_log` into GameWorld SetFireIntent.
pub fn eager_apply_host_fire_intent_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 687: post-logic fire-intent materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_fire_intent_log::drain();
    if events.is_empty() {
        EARLY_FIRE_INTENT_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_fire_intent_events(&events);
    EARLY_FIRE_INTENT_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 694: post-logic ai-attitude / overcharge / stealth-flags batch handoff.
thread_local! {
    static EARLY_AI_ATTITUDE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_ai_attitude_log::HostAiAttitudeEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_ai_attitude_batch() -> Option<(
    Vec<crate::game_logic::host_ai_attitude_log::HostAiAttitudeEvent>,
    bool,
)> {
    EARLY_AI_ATTITUDE_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 694: post-logic drain `host_ai_attitude_log` into GameWorld SetAiAttitude.
pub fn eager_apply_host_ai_attitude_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 694: post-logic AI-attitude materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_ai_attitude_log::drain();
    if events.is_empty() {
        EARLY_AI_ATTITUDE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_ai_attitude_events(&events);
    EARLY_AI_ATTITUDE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 704: post-logic AI-mood / AI-request / shock-stun batch handoff.
thread_local! {
    static EARLY_AI_MOOD_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_ai_mood_log::HostAiMoodEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 704: post-logic AI-mood / AI-request / shock-stun batch handoff.
thread_local! {
    static EARLY_AI_REQUEST_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_ai_request_log::HostAiRequestEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 704: post-logic AI-mood / AI-request / shock-stun batch handoff.
thread_local! {
    static EARLY_SHOCK_STUN_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_shock_stun_log::HostShockStunEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_ai_mood_batch() -> Option<(
    Vec<crate::game_logic::host_ai_mood_log::HostAiMoodEvent>,
    bool,
)> {
    EARLY_AI_MOOD_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_ai_request_batch() -> Option<(
    Vec<crate::game_logic::host_ai_request_log::HostAiRequestEvent>,
    bool,
)> {
    EARLY_AI_REQUEST_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_shock_stun_batch() -> Option<(
    Vec<crate::game_logic::host_shock_stun_log::HostShockStunEvent>,
    bool,
)> {
    EARLY_SHOCK_STUN_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 704: post-logic drain `host_ai_mood_log` into GameWorld SetAiMood.
pub fn eager_apply_host_ai_mood_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 704: post-logic AI-mood materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_ai_mood_log::drain();
    if events.is_empty() {
        EARLY_AI_MOOD_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_ai_mood_events(&events);
    EARLY_AI_MOOD_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 704: post-logic drain `host_ai_request_log` into GameWorld SetAiRequest.
pub fn eager_apply_host_ai_request_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 704: post-logic AI-request materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_ai_request_log::drain();
    if events.is_empty() {
        EARLY_AI_REQUEST_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_ai_request_events(&events);
    EARLY_AI_REQUEST_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 704: post-logic drain `host_shock_stun_log` into GameWorld SetShockStun.
pub fn eager_apply_host_shock_stun_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 704: post-logic shock-stun materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_shock_stun_log::drain();
    if events.is_empty() {
        EARLY_SHOCK_STUN_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_shock_stun_events(&events);
    EARLY_SHOCK_STUN_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 711: post-logic destroy / contain / AI-decision batch handoff.
thread_local! {
    static EARLY_AI_DECISION_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_ai_decision_log::HostAiDecisionEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_ai_decision_batch() -> Option<(
    Vec<crate::game_logic::host_ai_decision_log::HostAiDecisionEvent>,
    bool,
)> {
    EARLY_AI_DECISION_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 711: post-logic drain `host_ai_decision_log` into GameWorld AI decisions.
pub fn eager_apply_host_ai_decision_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active()
        || !gameworld_shadow_enabled()
        || !gameworld_ai_decision_authority_enabled()
    {
        return 0;
    }
    // Wave 711: post-logic AI-decision materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_ai_decision_log::drain();
    if events.is_empty() {
        EARLY_AI_DECISION_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_ai_decisions_as_world_mutations(&events);
    EARLY_AI_DECISION_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Drop unused post-logic handoff batches when the outermost couple ends.
pub(super) fn clear_early_ai_batches() {
    EARLY_AI_STATE_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_FIRE_INTENT_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_AI_ATTITUDE_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_AI_MOOD_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_AI_REQUEST_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_SHOCK_STUN_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_AI_DECISION_BATCH.with(|c| *c.borrow_mut() = None);
}
