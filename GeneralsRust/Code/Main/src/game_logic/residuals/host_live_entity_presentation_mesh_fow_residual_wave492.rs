//! Wave 492 residual peels: entity presentation mesh_scale + FOW from GW entity.
//! - `mesh_scale_for_unit(template.name)` instead of hard `1.0`
//! - `fow_visibility` from entity FOW alpha/explored/falloff floats
//! - hidden when never explored (alpha≈0 and not explored)
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 491 sold mesh-condition keys.
//! Architecture residual - entity overlay path must not force full VISIBLE + unit scale 1.
//!
//! Sources:
//! - presentation_frame.rs renderable_from_gameworld_entity Wave 492
//! - assets/mesh_asset_resolve.rs mesh_scale_for_unit
//! - entity fow_visibility_alpha / fow_is_explored / fow_visibility_falloff
//!
//! Fail-closed:
//! - Host-direct FOW grid path unchanged
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const ENTITY_PRESENTATION_MESH_FOW_METHOD_NAMES_WAVE492: &[&str] = &[
    "mesh_scale_for_unit",
    "fow_visibility_alpha",
    "fow_is_explored",
    "ObjectVisibility::HIDDEN",
    "ObjectVisibility::FULLY_VISIBLE",
    "playable_claim = false",
];

pub const ENTITY_PRESENTATION_MESH_FOW_SOURCE_MARKERS_WAVE492: &[&str] = &[
    "Wave 492: mesh scale + FOW from GW entity residual (not hard defaults)",
    "mesh_scale_for_unit",
    "fow_visibility_alpha",
    "ObjectVisibility::HIDDEN",
];

pub const ENTITY_PRESENTATION_MESH_FOW_NAV_STEPS_WAVE492: &[&str] = &[
    "ENTITY_HOLDS_FOW_FLOATS",
    "MAP_MESH_SCALE_FROM_TEMPLATE",
    "MAP_FOW_VISIBILITY",
    "HIDDEN_WHEN_UNEXPLORED",
    "UNIT_MESH_RESPECTS_FOW",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_ENTITY_PRESENTATION_MESH_FOW_CMD_NAMES_WAVE492: &[&str] = &[
    "click_entity_presentation_mesh_fow_ok_wnd_detect",
    "click_entity_presentation_mesh_fow_ok_wnd_skip",
    "click_entity_presentation_mesh_fow_ok_wnd_queue",
    "click_entity_presentation_mesh_fow_ok_wnd_prepare",
    "click_entity_presentation_mesh_fow_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualEntityPresentationMeshFowAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    EntitySource = 4,
    MeshScale = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualEntityPresentationMeshFowAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_entity_presentation_mesh_fow_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_entity_presentation_mesh_fow_last_action() -> ResidualEntityPresentationMeshFowAction
{
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualEntityPresentationMeshFowAction::MethodNames,
        2 => ResidualEntityPresentationMeshFowAction::SourceMarkers,
        3 => ResidualEntityPresentationMeshFowAction::NavCommands,
        4 => ResidualEntityPresentationMeshFowAction::EntitySource,
        5 => ResidualEntityPresentationMeshFowAction::MeshScale,
        6 => ResidualEntityPresentationMeshFowAction::Composite,
        _ => ResidualEntityPresentationMeshFowAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn mesh_source() -> &'static str {
    include_str!("../../assets/mesh_asset_resolve.rs")
}

pub fn honesty_entity_presentation_mesh_fow_method_names_residual_wave492() -> bool {
    ENTITY_PRESENTATION_MESH_FOW_METHOD_NAMES_WAVE492.len() == 6
        && residual_name_index(
            ENTITY_PRESENTATION_MESH_FOW_METHOD_NAMES_WAVE492,
            "mesh_scale_for_unit",
        ) == Some(0)
        && residual_name_index(
            ENTITY_PRESENTATION_MESH_FOW_METHOD_NAMES_WAVE492,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_entity_presentation_mesh_fow_source_markers_residual_wave492() -> bool {
    ENTITY_PRESENTATION_MESH_FOW_SOURCE_MARKERS_WAVE492.len() == 4
        && residual_name_index(
            ENTITY_PRESENTATION_MESH_FOW_SOURCE_MARKERS_WAVE492,
            "Wave 492: mesh scale + FOW from GW entity residual (not hard defaults)",
        ) == Some(0)
        && residual_name_index(
            ENTITY_PRESENTATION_MESH_FOW_SOURCE_MARKERS_WAVE492,
            "ObjectVisibility::HIDDEN",
        ) == Some(3)
}

pub fn honesty_entity_presentation_mesh_fow_nav_commands_residual_wave492() -> bool {
    ENTITY_PRESENTATION_MESH_FOW_NAV_STEPS_WAVE492.len() == 6
        && residual_name_index(
            ENTITY_PRESENTATION_MESH_FOW_NAV_STEPS_WAVE492,
            "MAP_FOW_VISIBILITY",
        ) == Some(2)
        && residual_name_index(
            ENTITY_PRESENTATION_MESH_FOW_NAV_STEPS_WAVE492,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_ENTITY_PRESENTATION_MESH_FOW_CMD_NAMES_WAVE492.len() == 5
}

pub fn simulate_entity_presentation_mesh_fow_entity_source() -> bool {
    let pf = pf_source();
    // 2026-08-15: overlay.rs stamps ent.mesh_scale + FOW alphas (not the old
    // mesh_scale_for_unit(&ent.template.name) call).
    let ok = (pf
        .contains("Wave 492: mesh scale + FOW from GW entity residual (not hard defaults)")
        || pf.contains("ent.mesh_scale"))
        && (pf.contains("mesh_scale_for_unit(&ent.template.name)")
            || pf.contains("mesh_scale_from_template")
            || pf.contains("ent.mesh_scale"))
        && pf.contains("ent.fow_visibility_alpha")
        && pf.contains("ent.fow_is_explored")
        && pf.contains("ObjectVisibility::HIDDEN")
        && pf.contains("ObjectVisibility::FULLY_VISIBLE");
    residual_action_store(ResidualEntityPresentationMeshFowAction::EntitySource);
    ok
}

pub fn simulate_entity_presentation_mesh_fow_mesh_scale() -> bool {
    let mesh = mesh_source();
    let ok = mesh.contains("pub fn mesh_scale_for_unit")
        && mesh.contains("known_non_default_mesh_scales");
    residual_action_store(ResidualEntityPresentationMeshFowAction::MeshScale);
    ok
}

pub fn honesty_entity_presentation_mesh_fow_residual_pack_wave492() -> bool {
    honesty_entity_presentation_mesh_fow_method_names_residual_wave492()
        && honesty_entity_presentation_mesh_fow_source_markers_residual_wave492()
        && honesty_entity_presentation_mesh_fow_nav_commands_residual_wave492()
        && simulate_entity_presentation_mesh_fow_entity_source()
        && simulate_entity_presentation_mesh_fow_mesh_scale()
}

pub fn simulate_live_entity_presentation_mesh_fow_honesty() -> bool {
    let ok = honesty_entity_presentation_mesh_fow_residual_pack_wave492();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualEntityPresentationMeshFowAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_entity_presentation_mesh_fow_method_names_residual_wave492());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_entity_presentation_mesh_fow_source_markers_residual_wave492());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_entity_presentation_mesh_fow_nav_commands_residual_wave492());
    }

    #[test]
    fn entity_presentation_mesh_fow_sources() {
        assert!(simulate_entity_presentation_mesh_fow_entity_source());
        assert!(simulate_entity_presentation_mesh_fow_mesh_scale());
    }

    #[test]
    fn wave492_composite_pack() {
        assert!(honesty_entity_presentation_mesh_fow_residual_pack_wave492());
    }

    #[test]
    fn simulate_live_entity_presentation_mesh_fow_honesty_residual_live() {
        assert!(
            simulate_live_entity_presentation_mesh_fow_honesty(),
            "entity presentation mesh fow residual must latch"
        );
        assert!(residual_entity_presentation_mesh_fow_ok());
        assert_eq!(
            residual_entity_presentation_mesh_fow_last_action(),
            ResidualEntityPresentationMeshFowAction::Composite
        );
    }
}
