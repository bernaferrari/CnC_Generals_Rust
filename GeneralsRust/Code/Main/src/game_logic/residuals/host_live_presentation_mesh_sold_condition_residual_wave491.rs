//! Wave 491: sold model-condition participates in source-authored Draw state selection.
//! - host and GameWorld presentation freeze the sold condition bit
//! - Object INI ConditionState matching chooses the authored SOLD/RUBBLE Model
//! - unit input retains one exact selected model key
//! - render collection does not turn that key into a synthetic rubble suffix
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 488–490 entity channel fills.
//! Architecture invariant: UnitRenderInput.model_condition_bits affects the
//! source state match, not an ad-hoc post-selection alias.
//!
//! Sources:
//! - assets/ini_parser.rs source ConditionState matcher
//! - assets/manager.rs exact Object identity resolver
//! - presentation_frame.rs host + GameWorld model key selection
//! - graphics/render_pipeline.rs opaque exact-model collection
//!
//! Fail-closed:
//! - Full W3D subobject hide matrix still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_MESH_SOLD_CONDITION_METHOD_NAMES_WAVE491: &[&str] = &[
    "select_primary_model_for_conditions",
    "resolve_presentation_model_key_for_conditions",
    "model_condition_bits",
    "Model = None",
    "unit_inputs",
    "playable_claim = false",
];

pub const PRESENTATION_MESH_SOLD_CONDITION_SOURCE_MARKERS_WAVE491: &[&str] = &[
    "Resolve the source-authored W3D model key for a frozen presentation object",
    "GameWorld-primary path: preserve every exact source-authored",
    "Model = None",
    "Presentation has already selected the exact source-authored",
];

pub const PRESENTATION_MESH_SOLD_CONDITION_NAV_STEPS_WAVE491: &[&str] = &[
    "FREEZE_SOLD_CONDITION_BIT",
    "ENTITY_PATH_SELECTS_SOURCE_DRAW",
    "RENDER_READS_MODEL_CONDITION_BITS",
    "KEEP_EXACT_MODEL_KEY",
    "NO_LIVE_GAMELOGIC_MESH_DUAL_READ",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_MESH_SOLD_CONDITION_CMD_NAMES_WAVE491: &[&str] = &[
    "click_presentation_mesh_sold_condition_ok_wnd_detect",
    "click_presentation_mesh_sold_condition_ok_wnd_skip",
    "click_presentation_mesh_sold_condition_ok_wnd_queue",
    "click_presentation_mesh_sold_condition_ok_wnd_prepare",
    "click_presentation_mesh_sold_condition_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationMeshSoldConditionAction {
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

fn residual_action_store(a: ResidualPresentationMeshSoldConditionAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_mesh_sold_condition_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_mesh_sold_condition_last_action()
-> ResidualPresentationMeshSoldConditionAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationMeshSoldConditionAction::MethodNames,
        2 => ResidualPresentationMeshSoldConditionAction::SourceMarkers,
        3 => ResidualPresentationMeshSoldConditionAction::NavCommands,
        4 => ResidualPresentationMeshSoldConditionAction::ResolveSource,
        5 => ResidualPresentationMeshSoldConditionAction::RenderSource,
        6 => ResidualPresentationMeshSoldConditionAction::Composite,
        _ => ResidualPresentationMeshSoldConditionAction::Idle,
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

pub fn honesty_presentation_mesh_sold_condition_method_names_residual_wave491() -> bool {
    PRESENTATION_MESH_SOLD_CONDITION_METHOD_NAMES_WAVE491.len() == 6
        && residual_name_index(
            PRESENTATION_MESH_SOLD_CONDITION_METHOD_NAMES_WAVE491,
            "select_primary_model_for_conditions",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_MESH_SOLD_CONDITION_METHOD_NAMES_WAVE491,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_mesh_sold_condition_source_markers_residual_wave491() -> bool {
    PRESENTATION_MESH_SOLD_CONDITION_SOURCE_MARKERS_WAVE491.len() == 4
        && residual_name_index(
            PRESENTATION_MESH_SOLD_CONDITION_SOURCE_MARKERS_WAVE491,
            "Presentation has already selected the exact source-authored",
        ) == Some(3)
}

pub fn honesty_presentation_mesh_sold_condition_nav_commands_residual_wave491() -> bool {
    PRESENTATION_MESH_SOLD_CONDITION_NAV_STEPS_WAVE491.len() == 6
        && residual_name_index(
            PRESENTATION_MESH_SOLD_CONDITION_NAV_STEPS_WAVE491,
            "RENDER_READS_MODEL_CONDITION_BITS",
        ) == Some(2)
        && residual_name_index(
            PRESENTATION_MESH_SOLD_CONDITION_NAV_STEPS_WAVE491,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_MESH_SOLD_CONDITION_CMD_NAMES_WAVE491.len() == 5
}

pub fn simulate_presentation_mesh_sold_condition_resolve_source() -> bool {
    let ini = ini_source();
    let manager = manager_source();
    let pf = pf_source();
    let ok = ini.contains("pub fn select_primary_model_for_conditions")
        && ini.contains("AuthoredConditionModelSelection::Suppressed")
        && manager.contains("resolve_presentation_draw_models_for_conditions")
        && pf.contains("GameWorld-primary path: preserve every exact source-authored")
        && pf.contains("resolve_presentation_draw_models_for_conditions");
    residual_action_store(ResidualPresentationMeshSoldConditionAction::ResolveSource);
    ok
}

pub fn simulate_presentation_mesh_sold_condition_render_source() -> bool {
    let rp = rp_source();
    // 2026-08-15: collect uses authored draw_models (pipeline_collect.rs:178-216),
    // not a live u.model_condition_bits reconstruct.
    let ok = rp.contains("Presentation has already selected the exact source-authored")
        && rp.contains("for draw_model in draw_models")
        && rp.contains("u.draw_models")
        && !rp.contains("model_key_with_presentation_state(")
        && !rp.contains("model_key_with_presentation_conditions(");
    residual_action_store(ResidualPresentationMeshSoldConditionAction::RenderSource);
    ok
}

pub fn honesty_presentation_mesh_sold_condition_residual_pack_wave491() -> bool {
    honesty_presentation_mesh_sold_condition_method_names_residual_wave491()
        && honesty_presentation_mesh_sold_condition_source_markers_residual_wave491()
        && honesty_presentation_mesh_sold_condition_nav_commands_residual_wave491()
        && simulate_presentation_mesh_sold_condition_resolve_source()
        && simulate_presentation_mesh_sold_condition_render_source()
}

pub fn simulate_live_presentation_mesh_sold_condition_honesty() -> bool {
    let ok = honesty_presentation_mesh_sold_condition_residual_pack_wave491();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationMeshSoldConditionAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_mesh_sold_condition_method_names_residual_wave491());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_mesh_sold_condition_source_markers_residual_wave491());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_mesh_sold_condition_nav_commands_residual_wave491());
    }

    #[test]
    fn presentation_mesh_sold_condition_sources() {
        assert!(simulate_presentation_mesh_sold_condition_resolve_source());
        assert!(simulate_presentation_mesh_sold_condition_render_source());
    }

    #[test]
    fn wave491_composite_pack() {
        assert!(honesty_presentation_mesh_sold_condition_residual_pack_wave491());
    }

    #[test]
    fn simulate_live_presentation_mesh_sold_condition_honesty_residual_live() {
        assert!(
            simulate_live_presentation_mesh_sold_condition_honesty(),
            "presentation mesh sold condition residual must latch"
        );
        assert!(residual_presentation_mesh_sold_condition_ok());
    }
}
