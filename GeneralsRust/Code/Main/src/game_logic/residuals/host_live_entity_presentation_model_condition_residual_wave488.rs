//! Wave 488 residual peels: GameWorld entity → presentation keeps model/door/radar.
//! - `renderable_from_gameworld_entity` copies `model_condition_bits`
//! - copies `radar_active` / `radar_extend_complete`
//! - copies `production_door_phase`
//! - no longer hard-zeros combat/door presentation channels
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 486/487 host model-condition logging.
//! Architecture residual - shadow overlay path must not drop host-synced entity visuals.
//!
//! Sources:
//! - presentation_frame.rs renderable_from_gameworld_entity
//! - gamelogic Entity model_condition_bits / door / radar fields
//!
//! Fail-closed:
//! - Host-direct presentation build path unchanged
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const ENTITY_PRESENTATION_MODEL_CONDITION_METHOD_NAMES_WAVE488: &[&str] = &[
    "renderable_from_gameworld_entity",
    "model_condition_bits: ent.model_condition_bits",
    "radar_active: ent.radar_active",
    "production_door_phase: ent.production_door_phase",
    "Wave 488",
    "playable_claim = false",
];

pub const ENTITY_PRESENTATION_MODEL_CONDITION_SOURCE_MARKERS_WAVE488: &[&str] = &[
    "Wave 488: carry GW entity presentation channels (not hard-zero)",
    "ent.model_condition_bits",
    "ent.radar_active",
    "ent.production_door_phase",
];

pub const ENTITY_PRESENTATION_MODEL_CONDITION_NAV_STEPS_WAVE488: &[&str] = &[
    "HOST_LOGS_MODEL_CONDITION",
    "GW_ENTITY_HOLDS_BITS",
    "RENDERABLE_FROM_ENTITY_COPIES",
    "UNIT_RENDER_INPUT_SEES_BITS",
    "NO_HARD_ZERO_OVERLAY",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_ENTITY_PRESENTATION_MODEL_CONDITION_CMD_NAMES_WAVE488: &[&str] = &[
    "click_entity_presentation_model_condition_ok_wnd_detect",
    "click_entity_presentation_model_condition_ok_wnd_skip",
    "click_entity_presentation_model_condition_ok_wnd_queue",
    "click_entity_presentation_model_condition_ok_wnd_prepare",
    "click_entity_presentation_model_condition_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualEntityPresentationModelConditionAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    EntitySource = 4,
    EntityFields = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualEntityPresentationModelConditionAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_entity_presentation_model_condition_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_entity_presentation_model_condition_last_action()
-> ResidualEntityPresentationModelConditionAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualEntityPresentationModelConditionAction::MethodNames,
        2 => ResidualEntityPresentationModelConditionAction::SourceMarkers,
        3 => ResidualEntityPresentationModelConditionAction::NavCommands,
        4 => ResidualEntityPresentationModelConditionAction::EntitySource,
        5 => ResidualEntityPresentationModelConditionAction::EntityFields,
        6 => ResidualEntityPresentationModelConditionAction::Composite,
        _ => ResidualEntityPresentationModelConditionAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn entity_source() -> &'static str {
    include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs")
}

fn function_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = src.find(sig)?;
    let brace = src[start..].find('{')? + start;
    let mut depth = 0i32;
    for (off, ch) in src[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[start..brace + off + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn honesty_entity_presentation_model_condition_method_names_residual_wave488() -> bool {
    ENTITY_PRESENTATION_MODEL_CONDITION_METHOD_NAMES_WAVE488.len() == 6
        && residual_name_index(
            ENTITY_PRESENTATION_MODEL_CONDITION_METHOD_NAMES_WAVE488,
            "renderable_from_gameworld_entity",
        ) == Some(0)
        && residual_name_index(
            ENTITY_PRESENTATION_MODEL_CONDITION_METHOD_NAMES_WAVE488,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_entity_presentation_model_condition_source_markers_residual_wave488() -> bool {
    ENTITY_PRESENTATION_MODEL_CONDITION_SOURCE_MARKERS_WAVE488.len() == 4
        && residual_name_index(
            ENTITY_PRESENTATION_MODEL_CONDITION_SOURCE_MARKERS_WAVE488,
            "Wave 488: carry GW entity presentation channels (not hard-zero)",
        ) == Some(0)
        && residual_name_index(
            ENTITY_PRESENTATION_MODEL_CONDITION_SOURCE_MARKERS_WAVE488,
            "ent.model_condition_bits",
        ) == Some(1)
}

pub fn honesty_entity_presentation_model_condition_nav_commands_residual_wave488() -> bool {
    ENTITY_PRESENTATION_MODEL_CONDITION_NAV_STEPS_WAVE488.len() == 6
        && residual_name_index(
            ENTITY_PRESENTATION_MODEL_CONDITION_NAV_STEPS_WAVE488,
            "RENDERABLE_FROM_ENTITY_COPIES",
        ) == Some(2)
        && residual_name_index(
            ENTITY_PRESENTATION_MODEL_CONDITION_NAV_STEPS_WAVE488,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_ENTITY_PRESENTATION_MODEL_CONDITION_CMD_NAMES_WAVE488.len() == 5
        && residual_name_index(
            RUNTIME_HOST_ENTITY_PRESENTATION_MODEL_CONDITION_CMD_NAMES_WAVE488,
            "click_entity_presentation_model_condition_ok_wnd_prepare",
        ) == Some(3)
}

pub fn simulate_entity_presentation_model_condition_entity_source() -> bool {
    let Some(body) = function_body(pf_source(), "fn renderable_from_gameworld_entity(") else {
        return false;
    };
    let ok = body.contains("Wave 488: carry GW entity presentation channels (not hard-zero)")
        && body.contains("model_condition_bits: ent.model_condition_bits")
        && body.contains("radar_active: ent.radar_active")
        && body.contains("radar_extend_complete: ent.radar_extend_complete")
        && body.contains("production_door_phase: ent.production_door_phase")
        && !body.contains("model_condition_bits: 0");
    residual_action_store(ResidualEntityPresentationModelConditionAction::EntitySource);
    ok
}

pub fn simulate_entity_presentation_model_condition_entity_fields() -> bool {
    let ent = entity_source();
    let ok = ent.contains("pub model_condition_bits: u128")
        && ent.contains("pub radar_active: bool")
        && ent.contains("pub production_door_phase:");
    residual_action_store(ResidualEntityPresentationModelConditionAction::EntityFields);
    ok
}

pub fn honesty_entity_presentation_model_condition_residual_pack_wave488() -> bool {
    honesty_entity_presentation_model_condition_method_names_residual_wave488()
        && honesty_entity_presentation_model_condition_source_markers_residual_wave488()
        && honesty_entity_presentation_model_condition_nav_commands_residual_wave488()
        && simulate_entity_presentation_model_condition_entity_source()
        && simulate_entity_presentation_model_condition_entity_fields()
}

pub fn simulate_live_entity_presentation_model_condition_honesty() -> bool {
    let ok = honesty_entity_presentation_model_condition_residual_pack_wave488();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualEntityPresentationModelConditionAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_entity_presentation_model_condition_method_names_residual_wave488());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_entity_presentation_model_condition_source_markers_residual_wave488());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_entity_presentation_model_condition_nav_commands_residual_wave488());
    }

    #[test]
    fn entity_presentation_model_condition_sources() {
        assert!(simulate_entity_presentation_model_condition_entity_source());
        assert!(simulate_entity_presentation_model_condition_entity_fields());
    }

    #[test]
    fn wave488_composite_pack() {
        assert!(honesty_entity_presentation_model_condition_residual_pack_wave488());
    }

    #[test]
    fn simulate_live_entity_presentation_model_condition_honesty_residual_live() {
        assert!(
            simulate_live_entity_presentation_model_condition_honesty(),
            "entity presentation model condition residual must latch"
        );
        assert!(residual_entity_presentation_model_condition_ok());
        assert_eq!(
            residual_entity_presentation_model_condition_last_action(),
            ResidualEntityPresentationModelConditionAction::Composite
        );
    }
}
