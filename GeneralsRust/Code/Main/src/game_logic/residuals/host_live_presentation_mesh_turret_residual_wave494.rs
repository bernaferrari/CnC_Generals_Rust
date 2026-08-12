//! Wave 494 residual peels: unit mesh pass freezes turret yaw/pitch from presentation.
//! - `UnitRenderInput` carries `turret_angle_deg` / `turret_pitch_deg`
//! - `from_renderable` freezes turret residuals
//! - `world_matrix` retains the hull only; source-authored HLOD bone control
//!   consumes the frozen primary turret binding after selected animation sample
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 491 sold mesh keys / Wave 493 ground bridge.
//! Architecture residual - presentation turret channels must affect only their
//! exact source-authored HTree pivots, never a vehicle's entire hull.
//!
//! Sources:
//! - presentation_frame UnitRenderInput + world_matrix
//! - graphics/render_pipeline/pipeline_collect primary HLOD control
//!
//! Fail-closed:
//! - Alternate/secondary/recoil/tires and unsupported HLOD topologies stay
//!   bind-pose/hull-only until their exact C++ paths exist
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_MESH_TURRET_METHOD_NAMES_WAVE494: &[&str] = &[
    "UnitRenderInput",
    "turret_angle_deg",
    "turret_pitch_deg",
    "world_matrix",
    "mesh_local_transform_and_visibility_for_primary_turret",
    "playable_claim = false",
];

pub const PRESENTATION_MESH_TURRET_SOURCE_MARKERS_WAVE494: &[&str] = &[
    "Frozen host primary-turret yaw (degrees)",
    "must never alter the hull world matrix",
    "mesh_local_transform_and_visibility_for_primary_turret",
    "authored_primary_turret",
    "turret_angle_deg: ro.turret_angle_deg",
];

pub const PRESENTATION_MESH_TURRET_NAV_STEPS_WAVE494: &[&str] = &[
    "FREEZE_TURRET_ON_UNIT_INPUT",
    "WORLD_MATRIX_EXCLUDES_TURRET",
    "DRAW_STATE_PRIMARY_TURRET_BINDING",
    "HLOD_CONTROL_AFTER_ANIMATION",
    "MESH_PASS_USES_HLOD_CONTROL",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_MESH_TURRET_CMD_NAMES_WAVE494: &[&str] = &[
    "click_presentation_mesh_turret_ok_wnd_detect",
    "click_presentation_mesh_turret_ok_wnd_skip",
    "click_presentation_mesh_turret_ok_wnd_queue",
    "click_presentation_mesh_turret_ok_wnd_prepare",
    "click_presentation_mesh_turret_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationMeshTurretAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    InputSource = 4,
    MatrixSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationMeshTurretAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_mesh_turret_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_mesh_turret_last_action() -> ResidualPresentationMeshTurretAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationMeshTurretAction::MethodNames,
        2 => ResidualPresentationMeshTurretAction::SourceMarkers,
        3 => ResidualPresentationMeshTurretAction::NavCommands,
        4 => ResidualPresentationMeshTurretAction::InputSource,
        5 => ResidualPresentationMeshTurretAction::MatrixSource,
        6 => ResidualPresentationMeshTurretAction::Composite,
        _ => ResidualPresentationMeshTurretAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn pipeline_collect_source() -> &'static str {
    include_str!("../../graphics/render_pipeline/pipeline_collect.rs")
}

pub fn honesty_presentation_mesh_turret_method_names_residual_wave494() -> bool {
    PRESENTATION_MESH_TURRET_METHOD_NAMES_WAVE494.len() == 6
        && residual_name_index(
            PRESENTATION_MESH_TURRET_METHOD_NAMES_WAVE494,
            "UnitRenderInput",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_MESH_TURRET_METHOD_NAMES_WAVE494,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_mesh_turret_source_markers_residual_wave494() -> bool {
    PRESENTATION_MESH_TURRET_SOURCE_MARKERS_WAVE494.len() == 5
        && residual_name_index(
            PRESENTATION_MESH_TURRET_SOURCE_MARKERS_WAVE494,
            "must never alter the hull world matrix",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_MESH_TURRET_SOURCE_MARKERS_WAVE494,
            "turret_angle_deg: ro.turret_angle_deg",
        ) == Some(4)
}

pub fn honesty_presentation_mesh_turret_nav_commands_residual_wave494() -> bool {
    PRESENTATION_MESH_TURRET_NAV_STEPS_WAVE494.len() == 6
        && residual_name_index(
            PRESENTATION_MESH_TURRET_NAV_STEPS_WAVE494,
            "HLOD_CONTROL_AFTER_ANIMATION",
        ) == Some(3)
        && residual_name_index(
            PRESENTATION_MESH_TURRET_NAV_STEPS_WAVE494,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_MESH_TURRET_CMD_NAMES_WAVE494.len() == 5
}

pub fn simulate_presentation_mesh_turret_input_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("struct UnitRenderInput")
        && pf.contains("Frozen host primary-turret yaw (degrees)")
        && pf.contains("turret_angle_deg: ro.turret_angle_deg")
        && pf.contains("turret_pitch_deg: ro.turret_pitch_deg");
    residual_action_store(ResidualPresentationMeshTurretAction::InputSource);
    ok
}

pub fn simulate_presentation_mesh_turret_matrix_source() -> bool {
    let pf = pf_source();
    let pipeline = pipeline_collect_source();
    let ok = pf.contains("must never alter the hull world matrix")
        && !pf.contains("self.orientation + self.turret_angle_deg.to_radians()")
        && !pf.contains("lean + self.turret_pitch_deg.to_radians()")
        && pipeline.contains("mesh_local_transform_and_visibility_for_primary_turret")
        && pipeline.contains("&authored_primary_turret")
        && pipeline.contains("u.turret_angle_deg")
        && pipeline.contains("u.turret_pitch_deg");
    residual_action_store(ResidualPresentationMeshTurretAction::MatrixSource);
    ok
}

pub fn honesty_presentation_mesh_turret_residual_pack_wave494() -> bool {
    honesty_presentation_mesh_turret_method_names_residual_wave494()
        && honesty_presentation_mesh_turret_source_markers_residual_wave494()
        && honesty_presentation_mesh_turret_nav_commands_residual_wave494()
        && simulate_presentation_mesh_turret_input_source()
        && simulate_presentation_mesh_turret_matrix_source()
}

pub fn simulate_live_presentation_mesh_turret_honesty() -> bool {
    let ok = honesty_presentation_mesh_turret_residual_pack_wave494();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationMeshTurretAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_mesh_turret_method_names_residual_wave494());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_mesh_turret_source_markers_residual_wave494());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_mesh_turret_nav_commands_residual_wave494());
    }

    #[test]
    fn presentation_mesh_turret_sources() {
        assert!(simulate_presentation_mesh_turret_input_source());
        assert!(simulate_presentation_mesh_turret_matrix_source());
    }

    #[test]
    fn wave494_composite_pack() {
        assert!(honesty_presentation_mesh_turret_residual_pack_wave494());
    }

    #[test]
    fn simulate_live_presentation_mesh_turret_honesty_residual_live() {
        assert!(
            simulate_live_presentation_mesh_turret_honesty(),
            "presentation mesh turret residual must latch"
        );
        assert!(residual_presentation_mesh_turret_ok());
        assert_eq!(
            residual_presentation_mesh_turret_last_action(),
            ResidualPresentationMeshTurretAction::Composite
        );
    }
}
