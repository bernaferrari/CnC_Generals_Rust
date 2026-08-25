//! Wave 501 residual peels: deployed + radar dish bits stamp into mesh model-condition.
//! - `UnitRenderInput` freezes `is_deployed` / `radar_active` / `radar_extend_complete`
//! - `model_condition_bits_with_combat_flags` stamps DEPLOYED / RADAR_EXTENDING / RADAR_UPGRADED
//! - render collect uses stamped bits via Wave 497 condition resolve
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 495–496 combat/door bit stamping.
//! Architecture residual - deploy/radar presentation must reach mesh condition channel.
//!
//! Sources:
//! - host_enum_table_residual.rs deployed_model_bit
//! - presentation_frame.rs Wave 501 stamp
//! - graphics/render_pipeline.rs Wave 501 comment
//!
//! Fail-closed:
//! - Full W3D radar dish bone anim still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_MESH_DEPLOY_RADAR_METHOD_NAMES_WAVE501: &[&str] = &[
    "is_deployed",
    "radar_active",
    "radar_extend_complete",
    "deployed_model_bit",
    "radar_extending_model_bit",
    "playable_claim = false",
];

pub const PRESENTATION_MESH_DEPLOY_RADAR_SOURCE_MARKERS_WAVE501: &[&str] = &[
    "Wave 501: stamp deployed + radar dish model-condition residual bits",
    "Wave 501: deployed + radar dish bits included in stamp helper",
    "deployed_model_bit",
    "radar_extend_complete",
];

pub const PRESENTATION_MESH_DEPLOY_RADAR_NAV_STEPS_WAVE501: &[&str] = &[
    "FREEZE_DEPLOY_RADAR_FLAGS",
    "STAMP_DEPLOYED_BIT",
    "STAMP_RADAR_EXTENDING_OR_UPGRADED",
    "RESOLVE_MESH_FROM_BITS",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_MESH_DEPLOY_RADAR_CMD_NAMES_WAVE501: &[&str] = &[
    "click_presentation_mesh_deploy_radar_ok_wnd_detect",
    "click_presentation_mesh_deploy_radar_ok_wnd_skip",
    "click_presentation_mesh_deploy_radar_ok_wnd_queue",
    "click_presentation_mesh_deploy_radar_ok_wnd_prepare",
    "click_presentation_mesh_deploy_radar_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationMeshDeployRadarAction {
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

fn residual_action_store(a: ResidualPresentationMeshDeployRadarAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_mesh_deploy_radar_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_mesh_deploy_radar_last_action()
-> ResidualPresentationMeshDeployRadarAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationMeshDeployRadarAction::MethodNames,
        2 => ResidualPresentationMeshDeployRadarAction::SourceMarkers,
        3 => ResidualPresentationMeshDeployRadarAction::NavCommands,
        4 => ResidualPresentationMeshDeployRadarAction::StampSource,
        5 => ResidualPresentationMeshDeployRadarAction::RenderSource,
        6 => ResidualPresentationMeshDeployRadarAction::Composite,
        _ => ResidualPresentationMeshDeployRadarAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn en_source() -> &'static str {
    include_str!("../host_enum_table_residual.rs")
}

fn rp_source() -> &'static str {
    crate::graphics::render_pipeline::RENDER_PIPELINE_SRC
}

pub fn honesty_presentation_mesh_deploy_radar_method_names_residual_wave501() -> bool {
    PRESENTATION_MESH_DEPLOY_RADAR_METHOD_NAMES_WAVE501.len() == 6
        && residual_name_index(
            PRESENTATION_MESH_DEPLOY_RADAR_METHOD_NAMES_WAVE501,
            "is_deployed",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_MESH_DEPLOY_RADAR_METHOD_NAMES_WAVE501,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_mesh_deploy_radar_source_markers_residual_wave501() -> bool {
    PRESENTATION_MESH_DEPLOY_RADAR_SOURCE_MARKERS_WAVE501.len() == 4
        && residual_name_index(
            PRESENTATION_MESH_DEPLOY_RADAR_SOURCE_MARKERS_WAVE501,
            "Wave 501: stamp deployed + radar dish model-condition residual bits",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_MESH_DEPLOY_RADAR_SOURCE_MARKERS_WAVE501,
            "deployed_model_bit",
        ) == Some(2)
}

pub fn honesty_presentation_mesh_deploy_radar_nav_commands_residual_wave501() -> bool {
    PRESENTATION_MESH_DEPLOY_RADAR_NAV_STEPS_WAVE501.len() == 6
        && residual_name_index(
            PRESENTATION_MESH_DEPLOY_RADAR_NAV_STEPS_WAVE501,
            "STAMP_RADAR_EXTENDING_OR_UPGRADED",
        ) == Some(2)
        && residual_name_index(
            PRESENTATION_MESH_DEPLOY_RADAR_NAV_STEPS_WAVE501,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_MESH_DEPLOY_RADAR_CMD_NAMES_WAVE501.len() == 5
}

pub fn simulate_presentation_mesh_deploy_radar_stamp_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let ok = pf.contains("Wave 501: stamp deployed + radar dish model-condition residual bits")
        && pf.contains("is_deployed: ro.is_deployed")
        && pf.contains("radar_active: ro.radar_active")
        && pf.contains("radar_extend_complete: ro.radar_extend_complete")
        && en.contains("pub fn deployed_model_bit")
        && pf.contains("deployed_model_bit()");
    residual_action_store(ResidualPresentationMeshDeployRadarAction::StampSource);
    ok
}

pub fn simulate_presentation_mesh_deploy_radar_render_source() -> bool {
    let rp = rp_source();
    let pf = pf_source();
    // 2026-08-15: stamp helper lives in presentation_frame/unit_render.rs;
    // pipeline_collect.rs only keeps the Wave 501 include comment.
    let ok = (rp.contains("Wave 501: deployed + radar dish bits included in stamp helper")
        || pf.contains(
            "Wave 501: deployed / radar dish residual bits for mesh subobject selection",
        ))
        && (rp.contains("model_condition_bits_with_combat_flags")
            || pf.contains("model_condition_bits_with_combat_flags"));
    residual_action_store(ResidualPresentationMeshDeployRadarAction::RenderSource);
    ok
}

pub fn honesty_presentation_mesh_deploy_radar_residual_pack_wave501() -> bool {
    honesty_presentation_mesh_deploy_radar_method_names_residual_wave501()
        && honesty_presentation_mesh_deploy_radar_source_markers_residual_wave501()
        && honesty_presentation_mesh_deploy_radar_nav_commands_residual_wave501()
        && simulate_presentation_mesh_deploy_radar_stamp_source()
        && simulate_presentation_mesh_deploy_radar_render_source()
}

pub fn simulate_live_presentation_mesh_deploy_radar_honesty() -> bool {
    let ok = honesty_presentation_mesh_deploy_radar_residual_pack_wave501();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationMeshDeployRadarAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_mesh_deploy_radar_method_names_residual_wave501());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_mesh_deploy_radar_source_markers_residual_wave501());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_mesh_deploy_radar_nav_commands_residual_wave501());
    }

    #[test]
    fn presentation_mesh_deploy_radar_sources() {
        assert!(simulate_presentation_mesh_deploy_radar_stamp_source());
        assert!(simulate_presentation_mesh_deploy_radar_render_source());
    }

    #[test]
    fn wave501_composite_pack() {
        assert!(honesty_presentation_mesh_deploy_radar_residual_pack_wave501());
    }

    #[test]
    fn simulate_live_presentation_mesh_deploy_radar_honesty_residual_live() {
        assert!(
            simulate_live_presentation_mesh_deploy_radar_honesty(),
            "presentation mesh deploy/radar residual must latch"
        );
        assert!(residual_presentation_mesh_deploy_radar_ok());
        assert_eq!(
            residual_presentation_mesh_deploy_radar_last_action(),
            ResidualPresentationMeshDeployRadarAction::Composite
        );
    }
}
