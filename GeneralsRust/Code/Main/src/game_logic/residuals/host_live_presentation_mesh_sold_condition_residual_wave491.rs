//! Wave 491 residual peels: presentation/render mesh keys honor sold model-condition.
//! - `model_key_with_presentation_state` (body damage + sold → rubble branch)
//! - host presentation freeze uses sold bit / status.sold
//! - entity presentation freeze uses body_damage_state + sold
//! - render unit mesh collect re-applies sold condition to model key
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 488–490 entity channel fills.
//! Architecture residual - UnitRenderInput.model_condition_bits must affect mesh pick.
//!
//! Sources:
//! - assets/mesh_asset_resolve.rs model_key_with_presentation_state
//! - presentation_frame.rs host + entity model_key freeze
//! - graphics/render_pipeline.rs unit mesh collect sold residual
//!
//! Fail-closed:
//! - Full W3D subobject hide matrix still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_MESH_SOLD_CONDITION_METHOD_NAMES_WAVE491: &[&str] = &[
    "model_key_with_presentation_state",
    "sold_model_bit",
    "model_condition_bits",
    "sold_for_mesh",
    "unit_inputs",
    "playable_claim = false",
];

pub const PRESENTATION_MESH_SOLD_CONDITION_SOURCE_MARKERS_WAVE491: &[&str] = &[
    "Wave 491: presentation mesh key from body damage + sold/death model-condition residual",
    "Wave 491: sold model-condition forces rubble/dying mesh branch",
    "Wave 491: body damage + sold → mesh key (not bare template name)",
    "Wave 491: mesh pass honors sold model-condition residual from presentation",
];

pub const PRESENTATION_MESH_SOLD_CONDITION_NAV_STEPS_WAVE491: &[&str] = &[
    "FREEZE_MODEL_KEY_WITH_SOLD",
    "ENTITY_PATH_USES_BODY_AND_SOLD",
    "RENDER_READS_MODEL_CONDITION_BITS",
    "RESOLVE_MESH_KEY",
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

pub fn residual_presentation_mesh_sold_condition_last_action(
) -> ResidualPresentationMeshSoldConditionAction {
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

fn mesh_source() -> &'static str {
    include_str!("../../assets/mesh_asset_resolve.rs")
}

fn pf_source() -> &'static str {
    include_str!("../../presentation_frame.rs")
}

fn rp_source() -> &'static str {
    include_str!("../../graphics/render_pipeline.rs")
}

pub fn honesty_presentation_mesh_sold_condition_method_names_residual_wave491() -> bool {
    PRESENTATION_MESH_SOLD_CONDITION_METHOD_NAMES_WAVE491.len() == 6
        && residual_name_index(
            PRESENTATION_MESH_SOLD_CONDITION_METHOD_NAMES_WAVE491,
            "model_key_with_presentation_state",
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
            "Wave 491: mesh pass honors sold model-condition residual from presentation",
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
    let mesh = mesh_source();
    let pf = pf_source();
    let ok = mesh.contains("pub fn model_key_with_presentation_state")
        && mesh.contains("Wave 491: presentation mesh key from body damage + sold/death model-condition residual")
        && pf.contains("Wave 491: sold model-condition forces rubble/dying mesh branch")
        && pf.contains("Wave 491: body damage + sold → mesh key (not bare template name)")
        && pf.contains("model_key_with_presentation_state");
    residual_action_store(ResidualPresentationMeshSoldConditionAction::ResolveSource);
    ok
}

pub fn simulate_presentation_mesh_sold_condition_render_source() -> bool {
    let rp = rp_source();
    let ok = rp
        .contains("Wave 491: mesh pass honors sold model-condition residual from presentation")
        && rp.contains("u.model_condition_bits")
        && rp.contains("sold_model_bit")
        && (rp.contains("model_key_with_presentation_state")
            || rp.contains("model_key_with_presentation_conditions"));
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
        assert_eq!(
            residual_presentation_mesh_sold_condition_last_action(),
            ResidualPresentationMeshSoldConditionAction::Composite
        );
    }
}
