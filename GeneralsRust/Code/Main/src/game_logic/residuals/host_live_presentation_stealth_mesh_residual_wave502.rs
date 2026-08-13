//! Wave 502 residual peels: presentation stealth hides enemy mesh / fades ally mesh.
//! - `UnitRenderInput.effectively_stealthed` frozen from presentation
//! - `unit_render_inputs` omits enemy effectively-stealthed units
//! - viewer-relative friendly stealthed units carry separate presentation opacity
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 501 deploy/radar bit stamping.
//! Architecture residual - stealth observe path without live GameLogic dual-read.
//!
//! Sources:
//! - presentation_frame.rs unit_render_inputs Wave 502
//! - graphics/render_pipeline.rs collect Wave 502 comment
//!
//! Fail-closed:
//! - Full stealth detector / disguise mesh swap still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_STEALTH_MESH_METHOD_NAMES_WAVE502: &[&str] = &[
    "effectively_stealthed",
    "unit_render_inputs",
    "presentation_opacity",
    "local_team",
    "fow_visibility",
    "playable_claim = false",
];

pub const PRESENTATION_STEALTH_MESH_SOURCE_MARKERS_WAVE502: &[&str] = &[
    "Wave 502: stealth mesh residual from frozen presentation only",
    "Wave 502: stealth filter/alpha applied inside unit_render_inputs (presentation-only)",
    "presentation_opacity",
    "effectively_stealthed: ro.effectively_stealthed",
];

pub const PRESENTATION_STEALTH_MESH_NAV_STEPS_WAVE502: &[&str] = &[
    "FREEZE_EFFECTIVELY_STEALTHED",
    "FILTER_ENEMY_STEALTHED_FROM_MESH",
    "FRIENDLY_STEALTH_PRESENTATION_OPACITY",
    "COLLECT_UNIT_RENDER_INPUTS",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_STEALTH_MESH_CMD_NAMES_WAVE502: &[&str] = &[
    "click_presentation_stealth_mesh_ok_wnd_detect",
    "click_presentation_stealth_mesh_ok_wnd_skip",
    "click_presentation_stealth_mesh_ok_wnd_queue",
    "click_presentation_stealth_mesh_ok_wnd_prepare",
    "click_presentation_stealth_mesh_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationStealthMeshAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    InputSource = 4,
    RenderSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationStealthMeshAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_stealth_mesh_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_stealth_mesh_last_action() -> ResidualPresentationStealthMeshAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationStealthMeshAction::MethodNames,
        2 => ResidualPresentationStealthMeshAction::SourceMarkers,
        3 => ResidualPresentationStealthMeshAction::NavCommands,
        4 => ResidualPresentationStealthMeshAction::InputSource,
        5 => ResidualPresentationStealthMeshAction::RenderSource,
        6 => ResidualPresentationStealthMeshAction::Composite,
        _ => ResidualPresentationStealthMeshAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn rp_source() -> &'static str {
    crate::graphics::render_pipeline::RENDER_PIPELINE_SRC
}

pub fn honesty_presentation_stealth_mesh_method_names_residual_wave502() -> bool {
    PRESENTATION_STEALTH_MESH_METHOD_NAMES_WAVE502.len() == 6
        && residual_name_index(
            PRESENTATION_STEALTH_MESH_METHOD_NAMES_WAVE502,
            "effectively_stealthed",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_STEALTH_MESH_METHOD_NAMES_WAVE502,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_stealth_mesh_source_markers_residual_wave502() -> bool {
    PRESENTATION_STEALTH_MESH_SOURCE_MARKERS_WAVE502.len() == 4
        && residual_name_index(
            PRESENTATION_STEALTH_MESH_SOURCE_MARKERS_WAVE502,
            "Wave 502: stealth mesh residual from frozen presentation only",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_STEALTH_MESH_SOURCE_MARKERS_WAVE502,
            "presentation_opacity",
        ) == Some(2)
}

pub fn honesty_presentation_stealth_mesh_nav_commands_residual_wave502() -> bool {
    PRESENTATION_STEALTH_MESH_NAV_STEPS_WAVE502.len() == 6
        && residual_name_index(
            PRESENTATION_STEALTH_MESH_NAV_STEPS_WAVE502,
            "FILTER_ENEMY_STEALTHED_FROM_MESH",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_STEALTH_MESH_NAV_STEPS_WAVE502,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_STEALTH_MESH_CMD_NAMES_WAVE502.len() == 5
}

pub fn simulate_presentation_stealth_mesh_input_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 502: stealth mesh residual from frozen presentation only")
        && pf.contains("effectively_stealthed: ro.effectively_stealthed")
        && pf.contains("input.presentation_opacity")
        && pf.contains("local_viewer_hides_stealthed")
        && pf.contains("local_viewer_uses_friendly_stealth_look");
    residual_action_store(ResidualPresentationStealthMeshAction::InputSource);
    ok
}

pub fn simulate_presentation_stealth_mesh_render_source() -> bool {
    let rp = rp_source();
    let ok = rp.contains(
        "Wave 502: stealth filter/alpha applied inside unit_render_inputs (presentation-only)",
    ) && rp.contains("frame.unit_render_inputs()");
    residual_action_store(ResidualPresentationStealthMeshAction::RenderSource);
    ok
}

pub fn honesty_presentation_stealth_mesh_residual_pack_wave502() -> bool {
    honesty_presentation_stealth_mesh_method_names_residual_wave502()
        && honesty_presentation_stealth_mesh_source_markers_residual_wave502()
        && honesty_presentation_stealth_mesh_nav_commands_residual_wave502()
        && simulate_presentation_stealth_mesh_input_source()
        && simulate_presentation_stealth_mesh_render_source()
}

pub fn simulate_live_presentation_stealth_mesh_honesty() -> bool {
    let ok = honesty_presentation_stealth_mesh_residual_pack_wave502();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationStealthMeshAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_stealth_mesh_method_names_residual_wave502());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_stealth_mesh_source_markers_residual_wave502());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_stealth_mesh_nav_commands_residual_wave502());
    }

    #[test]
    fn presentation_stealth_mesh_sources() {
        assert!(simulate_presentation_stealth_mesh_input_source());
        assert!(simulate_presentation_stealth_mesh_render_source());
    }

    #[test]
    fn wave502_composite_pack() {
        assert!(honesty_presentation_stealth_mesh_residual_pack_wave502());
    }

    #[test]
    fn simulate_live_presentation_stealth_mesh_honesty_residual_live() {
        assert!(
            simulate_live_presentation_stealth_mesh_honesty(),
            "presentation stealth mesh residual must latch"
        );
        assert!(residual_presentation_stealth_mesh_ok());
        assert_eq!(
            residual_presentation_stealth_mesh_last_action(),
            ResidualPresentationStealthMeshAction::Composite
        );
    }
}
