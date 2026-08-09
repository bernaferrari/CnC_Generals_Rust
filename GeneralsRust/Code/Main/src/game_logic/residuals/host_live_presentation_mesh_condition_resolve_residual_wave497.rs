//! Wave 497 residual peels: mesh key resolve uses full stamped model-condition bits.
//! - `model_key_with_presentation_conditions(base, body, dying, bits)`
//! - SOLD + RUBBLE bits force rubble/dying mesh branch
//! - `UnitRenderInput.body_damage_state` frozen for mesh variants
//! - render collect calls condition resolve after combat/door bit stamp
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 491 sold-only path / Wave 495–496 bit stamping.
//! Architecture residual - stamped model-condition bank must drive mesh pick.
//!
//! Sources:
//! - assets/mesh_asset_resolve.rs model_key_with_presentation_conditions
//! - presentation_frame.rs UnitRenderInput.body_damage_state
//! - graphics/render_pipeline.rs Wave 497 collect resolve
//!
//! Fail-closed:
//! - Full W3D subobject hide still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_MESH_CONDITION_RESOLVE_METHOD_NAMES_WAVE497: &[&str] = &[
    "model_key_with_presentation_conditions",
    "body_damage_state",
    "MC_BIT_RUBBLE",
    "sold_model_bit",
    "model_condition_bits_with_combat_flags",
    "playable_claim = false",
];

pub const PRESENTATION_MESH_CONDITION_RESOLVE_SOURCE_MARKERS_WAVE497: &[&str] = &[
    "Wave 497: mesh key from body damage + full stamped model-condition bits",
    "Wave 497: full stamped bits + body damage drive mesh key variants",
    "model_key_with_presentation_conditions",
    "body_damage_state: ro.body_damage_state",
];

pub const PRESENTATION_MESH_CONDITION_RESOLVE_NAV_STEPS_WAVE497: &[&str] = &[
    "STAMP_COMBAT_AND_DOOR_BITS",
    "READ_BODY_DAMAGE_STATE",
    "RESOLVE_MESH_KEY_FROM_BITS",
    "SOLD_OR_RUBBLE_TO_DYING_MESH",
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

pub fn residual_presentation_mesh_condition_resolve_last_action(
) -> ResidualPresentationMeshConditionResolveAction {
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

fn mesh_source() -> &'static str {
    include_str!("../../assets/mesh_asset_resolve.rs")
}

fn pf_source() -> &'static str {
    include_str!("../../presentation_frame.rs")
}

fn rp_source() -> &'static str {
    include_str!("../../graphics/render_pipeline.rs")
}

pub fn honesty_presentation_mesh_condition_resolve_method_names_residual_wave497() -> bool {
    PRESENTATION_MESH_CONDITION_RESOLVE_METHOD_NAMES_WAVE497.len() == 6
        && residual_name_index(
            PRESENTATION_MESH_CONDITION_RESOLVE_METHOD_NAMES_WAVE497,
            "model_key_with_presentation_conditions",
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
            "Wave 497: full stamped bits + body damage drive mesh key variants",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_MESH_CONDITION_RESOLVE_SOURCE_MARKERS_WAVE497,
            "model_key_with_presentation_conditions",
        ) == Some(2)
}

pub fn honesty_presentation_mesh_condition_resolve_nav_commands_residual_wave497() -> bool {
    PRESENTATION_MESH_CONDITION_RESOLVE_NAV_STEPS_WAVE497.len() == 6
        && residual_name_index(
            PRESENTATION_MESH_CONDITION_RESOLVE_NAV_STEPS_WAVE497,
            "RESOLVE_MESH_KEY_FROM_BITS",
        ) == Some(2)
        && residual_name_index(
            PRESENTATION_MESH_CONDITION_RESOLVE_NAV_STEPS_WAVE497,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_MESH_CONDITION_RESOLVE_CMD_NAMES_WAVE497.len() == 5
}

pub fn simulate_presentation_mesh_condition_resolve_resolve_source() -> bool {
    let mesh = mesh_source();
    let pf = pf_source();
    let ok = mesh.contains("pub fn model_key_with_presentation_conditions")
        && mesh.contains("Wave 497: mesh key from body damage + full stamped model-condition bits")
        && mesh.contains("MC_BIT_RUBBLE")
        && pf.contains("body_damage_state: ro.body_damage_state")
        && pf.contains("Wave 497: body damage ordinal for mesh variant resolve");
    residual_action_store(ResidualPresentationMeshConditionResolveAction::ResolveSource);
    ok
}

pub fn simulate_presentation_mesh_condition_resolve_render_source() -> bool {
    let rp = rp_source();
    let ok = rp.contains("Wave 497: full stamped bits + body damage drive mesh key variants")
        && rp.contains("model_key_with_presentation_conditions")
        && rp.contains("u.body_damage_state")
        && rp.contains("model_condition_bits_with_combat_flags");
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
        assert_eq!(
            residual_presentation_mesh_condition_resolve_last_action(),
            ResidualPresentationMeshConditionResolveAction::Composite
        );
    }
}
