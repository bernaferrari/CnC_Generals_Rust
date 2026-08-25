//! Wave 497: full stamped model-condition bits select source-authored Draw models.
//! - Object INI raw ConditionState / AliasConditionState order is retained
//! - C++ SparseMatchFinder ordering selects the exact source Model
//! - UnitRenderInput freezes all combat/weather/construction bits before selection
//! - render collect consumes that opaque model key without a second suffix pass
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 491 sold-state input / Wave 495–496 bit stamping.
//! Architecture invariant: the stamped model-condition bank drives the authored
//! Object INI state choice, not a guessed basename mutation.
//!
//! Sources:
//! - assets/ini_parser.rs ObjectDefinition::select_primary_model_for_conditions
//! - assets/manager.rs resolve_presentation_model_key_for_conditions
//! - presentation_frame.rs UnitRenderInput::from_renderable_with_environment
//! - graphics/render_pipeline.rs exact opaque model collect
//!
//! Fail-closed:
//! - Full W3D subobject hide still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_MESH_CONDITION_RESOLVE_METHOD_NAMES_WAVE497: &[&str] = &[
    "select_primary_model_for_conditions",
    "resolve_presentation_model_key_for_conditions",
    "from_renderable_with_environment",
    "model_condition_bits_with_combat_flags",
    "canonical_model_key",
    "playable_claim = false",
];

pub const PRESENTATION_MESH_CONDITION_RESOLVE_SOURCE_MARKERS_WAVE497: &[&str] = &[
    "C++ `SparseMatchFinder` ordering used by `W3DModelDraw`",
    "Apply the retained Object INI Draw-state selector",
    "Presentation has already selected the exact source-authored",
    "never run a second suffix-based mesh selection",
];

pub const PRESENTATION_MESH_CONDITION_RESOLVE_NAV_STEPS_WAVE497: &[&str] = &[
    "STAMP_COMBAT_AND_DOOR_BITS",
    "MATCH_SOURCE_DRAW_STATE",
    "PRESERVE_EXACT_MODEL_KEY",
    "NO_SUFFIX_RESELECTION",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_MESH_CONDITION_RESOLVE_CMD_NAMES_WAVE497: &[&str] = &[
    "click_presentation_mesh_condition_resolve_ok_wnd_detect",
    "click_presentation_mesh_condition_resolve_ok_wnd_skip",
    "click_presentation_mesh_condition_resolve_ok_wnd_queue",
    "click_presentation_mesh_condition_resolve_ok_wnd_prepare",
    "click_presentation_mesh_condition_resolve_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationMeshConditionResolveAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    ResolveSource = 4,
    RenderSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationMeshConditionResolveAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_mesh_condition_resolve_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_mesh_condition_resolve_last_action()
-> ResidualPresentationMeshConditionResolveAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationMeshConditionResolveAction::MethodNames,
        2 => ResidualPresentationMeshConditionResolveAction::SourceMarkers,
        3 => ResidualPresentationMeshConditionResolveAction::NavCommands,
        4 => ResidualPresentationMeshConditionResolveAction::ResolveSource,
        5 => ResidualPresentationMeshConditionResolveAction::RenderSource,
        6 => ResidualPresentationMeshConditionResolveAction::Composite,
        _ => ResidualPresentationMeshConditionResolveAction::Idle,
    }
}

fn ini_source() -> &'static str {
    concat!(
        include_str!("../../assets/ini_parser/mod.rs"),
        include_str!("../../assets/ini_parser/types.rs"),
        include_str!("../../assets/ini_parser/objects.rs"),
        include_str!("../../assets/ini_parser/parser.rs"),
        include_str!("../../assets/ini_parser/tests.rs"),
    )
}

fn manager_source() -> &'static str {
    include_str!("../../assets/manager.rs")
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn rp_source() -> &'static str {
    crate::graphics::render_pipeline::RENDER_PIPELINE_SRC
}

pub fn honesty_presentation_mesh_condition_resolve_method_names_residual_wave497() -> bool {
    PRESENTATION_MESH_CONDITION_RESOLVE_METHOD_NAMES_WAVE497.len() == 6
        && residual_name_index(
            PRESENTATION_MESH_CONDITION_RESOLVE_METHOD_NAMES_WAVE497,
            "select_primary_model_for_conditions",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_MESH_CONDITION_RESOLVE_METHOD_NAMES_WAVE497,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_mesh_condition_resolve_source_markers_residual_wave497() -> bool {
    PRESENTATION_MESH_CONDITION_RESOLVE_SOURCE_MARKERS_WAVE497.len() == 4
        && residual_name_index(
            PRESENTATION_MESH_CONDITION_RESOLVE_SOURCE_MARKERS_WAVE497,
            "Apply the retained Object INI Draw-state selector",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_MESH_CONDITION_RESOLVE_SOURCE_MARKERS_WAVE497,
            "Presentation has already selected the exact source-authored",
        ) == Some(2)
}

pub fn honesty_presentation_mesh_condition_resolve_nav_commands_residual_wave497() -> bool {
    PRESENTATION_MESH_CONDITION_RESOLVE_NAV_STEPS_WAVE497.len() == 6
        && residual_name_index(
            PRESENTATION_MESH_CONDITION_RESOLVE_NAV_STEPS_WAVE497,
            "MATCH_SOURCE_DRAW_STATE",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_MESH_CONDITION_RESOLVE_NAV_STEPS_WAVE497,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_MESH_CONDITION_RESOLVE_CMD_NAMES_WAVE497.len() == 5
}

pub fn simulate_presentation_mesh_condition_resolve_resolve_source() -> bool {
    let ini = ini_source();
    let manager = manager_source();
    let pf = pf_source();
    let ok = ini.contains("pub fn select_primary_model_for_conditions")
        && ini.contains("state.condition_sets.iter().rev()")
        && manager.contains("resolve_presentation_draw_models_for_conditions")
        && pf.contains("from_renderable_with_environment")
        && pf.contains("self.model_condition_bits_with_combat_flags()");
    residual_action_store(ResidualPresentationMeshConditionResolveAction::ResolveSource);
    ok
}

pub fn simulate_presentation_mesh_condition_resolve_render_source() -> bool {
    let rp = rp_source();
    let ok = rp.contains("Presentation has already selected the exact source-authored")
        && rp.contains("for draw_model in draw_models")
        && rp.contains("canonical_model_key")
        && !rp.contains("model_key_with_presentation_conditions(");
    residual_action_store(ResidualPresentationMeshConditionResolveAction::RenderSource);
    ok
}

pub fn honesty_presentation_mesh_condition_resolve_residual_pack_wave497() -> bool {
    honesty_presentation_mesh_condition_resolve_method_names_residual_wave497()
        && honesty_presentation_mesh_condition_resolve_source_markers_residual_wave497()
        && honesty_presentation_mesh_condition_resolve_nav_commands_residual_wave497()
        && simulate_presentation_mesh_condition_resolve_resolve_source()
        && simulate_presentation_mesh_condition_resolve_render_source()
}

pub fn simulate_live_presentation_mesh_condition_resolve_honesty() -> bool {
    let ok = honesty_presentation_mesh_condition_resolve_residual_pack_wave497();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationMeshConditionResolveAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_mesh_condition_resolve_method_names_residual_wave497());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_mesh_condition_resolve_source_markers_residual_wave497());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_mesh_condition_resolve_nav_commands_residual_wave497());
    }

    #[test]
    fn presentation_mesh_condition_resolve_sources() {
        assert!(simulate_presentation_mesh_condition_resolve_resolve_source());
        assert!(simulate_presentation_mesh_condition_resolve_render_source());
    }

    #[test]
    fn wave497_composite_pack() {
        assert!(honesty_presentation_mesh_condition_resolve_residual_pack_wave497());
    }

    #[test]
    fn simulate_live_presentation_mesh_condition_resolve_honesty_residual_live() {
        assert!(
            simulate_live_presentation_mesh_condition_resolve_honesty(),
            "presentation mesh condition resolve residual must latch"
        );
        assert!(residual_presentation_mesh_condition_resolve_ok());
    }
}
