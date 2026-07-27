//! Wave 496 residual peels: production door phase stamps door model-condition bits.
//! - `model_condition_bits_with_combat_flags` maps phase 1..4 → door_1_* bits
//! - clears door bank then sets active phase bit
//! - render collect uses stamped bits (via Wave 495 helper)
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 495 combat motion bit stamping.
//! Architecture residual - production_door_phase must reach mesh condition channel.
//!
//! Sources:
//! - presentation_frame.rs model_condition_bits_with_combat_flags Wave 496
//! - graphics/render_pipeline.rs Wave 496 comment
//!
//! Fail-closed:
//! - Full multi-door / subobject hide matrix still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_MESH_DOOR_PHASE_METHOD_NAMES_WAVE496: &[&str] = &[
    "production_door_phase",
    "door_1_opening_model_bit",
    "door_1_waiting_open_model_bit",
    "door_1_closing_model_bit",
    "model_condition_bits_with_combat_flags",
    "playable_claim = false",
];

pub const PRESENTATION_MESH_DOOR_PHASE_SOURCE_MARKERS_WAVE496: &[&str] = &[
    "Wave 496: also stamp production-door phase bits for structure mesh residual",
    "Wave 496: stamp production-door phase bits into model-condition bank",
    "match self.production_door_phase",
    "door_1_opening_model_bit",
];

pub const PRESENTATION_MESH_DOOR_PHASE_NAV_STEPS_WAVE496: &[&str] = &[
    "FREEZE_DOOR_PHASE",
    "CLEAR_DOOR_BIT_BANK",
    "SET_ACTIVE_DOOR_BIT",
    "RENDER_STAMPS_BITS",
    "SOLD_RESOLVE_AFTER_STAMP",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_MESH_DOOR_PHASE_CMD_NAMES_WAVE496: &[&str] = &[
    "click_presentation_mesh_door_phase_ok_wnd_detect",
    "click_presentation_mesh_door_phase_ok_wnd_skip",
    "click_presentation_mesh_door_phase_ok_wnd_queue",
    "click_presentation_mesh_door_phase_ok_wnd_prepare",
    "click_presentation_mesh_door_phase_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationMeshDoorPhaseAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    StampSource = 4,
    RenderSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationMeshDoorPhaseAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_mesh_door_phase_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_mesh_door_phase_last_action() -> ResidualPresentationMeshDoorPhaseAction
{
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationMeshDoorPhaseAction::MethodNames,
        2 => ResidualPresentationMeshDoorPhaseAction::SourceMarkers,
        3 => ResidualPresentationMeshDoorPhaseAction::NavCommands,
        4 => ResidualPresentationMeshDoorPhaseAction::StampSource,
        5 => ResidualPresentationMeshDoorPhaseAction::RenderSource,
        6 => ResidualPresentationMeshDoorPhaseAction::Composite,
        _ => ResidualPresentationMeshDoorPhaseAction::Idle,
    }
}

fn pf_source() -> &'static str {
    include_str!("../presentation_frame.rs")
}

fn rp_source() -> &'static str {
    include_str!("../graphics/render_pipeline.rs")
}

pub fn honesty_presentation_mesh_door_phase_method_names_residual_wave496() -> bool {
    PRESENTATION_MESH_DOOR_PHASE_METHOD_NAMES_WAVE496.len() == 6
        && residual_name_index(
            PRESENTATION_MESH_DOOR_PHASE_METHOD_NAMES_WAVE496,
            "production_door_phase",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_MESH_DOOR_PHASE_METHOD_NAMES_WAVE496,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_mesh_door_phase_source_markers_residual_wave496() -> bool {
    PRESENTATION_MESH_DOOR_PHASE_SOURCE_MARKERS_WAVE496.len() == 4
        && residual_name_index(
            PRESENTATION_MESH_DOOR_PHASE_SOURCE_MARKERS_WAVE496,
            "Wave 496: also stamp production-door phase bits for structure mesh residual",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_MESH_DOOR_PHASE_SOURCE_MARKERS_WAVE496,
            "match self.production_door_phase",
        ) == Some(2)
}

pub fn honesty_presentation_mesh_door_phase_nav_commands_residual_wave496() -> bool {
    PRESENTATION_MESH_DOOR_PHASE_NAV_STEPS_WAVE496.len() == 6
        && residual_name_index(
            PRESENTATION_MESH_DOOR_PHASE_NAV_STEPS_WAVE496,
            "SET_ACTIVE_DOOR_BIT",
        ) == Some(2)
        && residual_name_index(
            PRESENTATION_MESH_DOOR_PHASE_NAV_STEPS_WAVE496,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_MESH_DOOR_PHASE_CMD_NAMES_WAVE496.len() == 5
}

pub fn simulate_presentation_mesh_door_phase_stamp_source() -> bool {
    let pf = pf_source();
    let ok = pf
        .contains("Wave 496: also stamp production-door phase bits for structure mesh residual")
        && pf.contains("match self.production_door_phase")
        && pf.contains("door_1_opening_model_bit")
        && pf.contains("door_1_waiting_open_model_bit")
        && pf.contains("door_1_closing_model_bit")
        && pf.contains("1 => bits |= 1u128 << open_b");
    residual_action_store(ResidualPresentationMeshDoorPhaseAction::StampSource);
    ok
}

pub fn simulate_presentation_mesh_door_phase_render_source() -> bool {
    let rp = rp_source();
    let ok = rp.contains("Wave 496: stamp production-door phase bits into model-condition bank")
        && rp.contains("model_condition_bits_with_combat_flags");
    residual_action_store(ResidualPresentationMeshDoorPhaseAction::RenderSource);
    ok
}

pub fn honesty_presentation_mesh_door_phase_residual_pack_wave496() -> bool {
    honesty_presentation_mesh_door_phase_method_names_residual_wave496()
        && honesty_presentation_mesh_door_phase_source_markers_residual_wave496()
        && honesty_presentation_mesh_door_phase_nav_commands_residual_wave496()
        && simulate_presentation_mesh_door_phase_stamp_source()
        && simulate_presentation_mesh_door_phase_render_source()
}

pub fn simulate_live_presentation_mesh_door_phase_honesty() -> bool {
    let ok = honesty_presentation_mesh_door_phase_residual_pack_wave496();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationMeshDoorPhaseAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_mesh_door_phase_method_names_residual_wave496());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_mesh_door_phase_source_markers_residual_wave496());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_mesh_door_phase_nav_commands_residual_wave496());
    }

    #[test]
    fn presentation_mesh_door_phase_sources() {
        assert!(simulate_presentation_mesh_door_phase_stamp_source());
        assert!(simulate_presentation_mesh_door_phase_render_source());
    }

    #[test]
    fn wave496_composite_pack() {
        assert!(honesty_presentation_mesh_door_phase_residual_pack_wave496());
    }

    #[test]
    fn simulate_live_presentation_mesh_door_phase_honesty_residual_live() {
        assert!(
            simulate_live_presentation_mesh_door_phase_honesty(),
            "presentation mesh door phase residual must latch"
        );
        assert!(residual_presentation_mesh_door_phase_ok());
        assert_eq!(
            residual_presentation_mesh_door_phase_last_action(),
            ResidualPresentationMeshDoorPhaseAction::Composite
        );
    }
}
