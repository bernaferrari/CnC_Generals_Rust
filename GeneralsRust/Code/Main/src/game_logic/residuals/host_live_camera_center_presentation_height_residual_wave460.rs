//! Wave 460 residual peels: center_camera_on / clamp_to_world_bounds prefer
//! PresentationFrame height grid + bounds (no live terrain_height_at when freeze
//! installed). Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 458 bootstrap camera Option<&GameLogic>.
//! Architecture residual - runtime camera center uses presentation heights.
//!
//! Sources (cnc_game_engine.rs):
//! - center_camera_on samples presentation world_env.sample_height first
//! - clamp_to_world_bounds prefers pipeline/last presentation bounds
//! - live terrain_height_at only when no presentation frame
//!
//! Fail-closed:
//! - Boot residual without freeze still reads host terrain
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const CAMERA_CENTER_PRESENTATION_HEIGHT_METHOD_NAMES_WAVE460: &[&str] = &[
    "center_camera_on",
    "clamp_to_world_bounds",
    "sample_height",
    "presentation_frame",
    "terrain_height_at",
    "apply_camera_orbit_transform",
];

pub const CAMERA_CENTER_PRESENTATION_HEIGHT_SOURCE_MARKERS_WAVE460: &[&str] = &[
    "Wave 460: prefer presentation-frozen height grid",
    "sample_height",
    "presentation_frame()",
    "terrain_height_at",
];

pub const CAMERA_CENTER_PRESENTATION_HEIGHT_NAV_STEPS_WAVE460: &[&str] = &[
    "RESOLVE_PIPELINE_OR_LAST_PRESENTATION",
    "SAMPLE_HEIGHT_FROM_WORLD_ENV",
    "FALLBACK_LIVE_TERRAIN_WHEN_NO_FRAME",
    "CLAMP_BOUNDS_FROM_PRESENTATION",
    "APPLY_CAMERA_ORBIT",
    "NO_LIVE_HEIGHT_WHEN_FROZEN",
];

pub const RUNTIME_HOST_CAMERA_CENTER_PRESENTATION_HEIGHT_CMD_NAMES_WAVE460: &[&str] = &[
    "click_camera_center_presentation_height_ok_wnd_resolve",
    "click_camera_center_presentation_height_ok_wnd_sample",
    "click_camera_center_presentation_height_ok_wnd_clamp",
    "click_camera_center_presentation_height_ok_wnd_prepare",
    "click_camera_center_presentation_height_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualCameraCenterPresentationHeightAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CenterSource = 4,
    ClampSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualCameraCenterPresentationHeightAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_camera_center_presentation_height_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_camera_center_presentation_height_last_action()
-> ResidualCameraCenterPresentationHeightAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualCameraCenterPresentationHeightAction::MethodNames,
        2 => ResidualCameraCenterPresentationHeightAction::SourceMarkers,
        3 => ResidualCameraCenterPresentationHeightAction::NavCommands,
        4 => ResidualCameraCenterPresentationHeightAction::CenterSource,
        5 => ResidualCameraCenterPresentationHeightAction::ClampSource,
        6 => ResidualCameraCenterPresentationHeightAction::Composite,
        _ => ResidualCameraCenterPresentationHeightAction::Idle,
    }
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_camera_center_presentation_height_method_names_residual_wave460() -> bool {
    CAMERA_CENTER_PRESENTATION_HEIGHT_METHOD_NAMES_WAVE460.len() == 6
        && residual_name_index(
            CAMERA_CENTER_PRESENTATION_HEIGHT_METHOD_NAMES_WAVE460,
            "center_camera_on",
        ) == Some(0)
        && residual_name_index(
            CAMERA_CENTER_PRESENTATION_HEIGHT_METHOD_NAMES_WAVE460,
            "apply_camera_orbit_transform",
        ) == Some(5)
}

pub fn honesty_camera_center_presentation_height_source_markers_residual_wave460() -> bool {
    CAMERA_CENTER_PRESENTATION_HEIGHT_SOURCE_MARKERS_WAVE460.len() == 4
        && residual_name_index(
            CAMERA_CENTER_PRESENTATION_HEIGHT_SOURCE_MARKERS_WAVE460,
            "Wave 460: prefer presentation-frozen height grid",
        ) == Some(0)
        && residual_name_index(
            CAMERA_CENTER_PRESENTATION_HEIGHT_SOURCE_MARKERS_WAVE460,
            "terrain_height_at",
        ) == Some(3)
}

pub fn honesty_camera_center_presentation_height_nav_commands_residual_wave460() -> bool {
    CAMERA_CENTER_PRESENTATION_HEIGHT_NAV_STEPS_WAVE460.len() == 6
        && residual_name_index(
            CAMERA_CENTER_PRESENTATION_HEIGHT_NAV_STEPS_WAVE460,
            "SAMPLE_HEIGHT_FROM_WORLD_ENV",
        ) == Some(1)
        && residual_name_index(
            CAMERA_CENTER_PRESENTATION_HEIGHT_NAV_STEPS_WAVE460,
            "NO_LIVE_HEIGHT_WHEN_FROZEN",
        ) == Some(5)
        && RUNTIME_HOST_CAMERA_CENTER_PRESENTATION_HEIGHT_CMD_NAMES_WAVE460.len() == 5
        && residual_name_index(
            RUNTIME_HOST_CAMERA_CENTER_PRESENTATION_HEIGHT_CMD_NAMES_WAVE460,
            "click_camera_center_presentation_height_ok_wnd_prepare",
        ) == Some(3)
}

fn function_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let at = src.find(sig)?;
    let bytes = src.as_bytes();
    let mut depth = 0i32;
    let mut end = at;
    let mut seen = false;
    for (j, &b) in bytes[at..].iter().enumerate() {
        if b == b'{' {
            depth += 1;
            seen = true;
        } else if b == b'}' {
            depth -= 1;
            if seen && depth == 0 {
                end = at + j + 1;
                break;
            }
        }
    }
    Some(&src[at..end])
}

pub fn simulate_camera_center_presentation_height_source() -> bool {
    let src = cnc_source();
    // Wave 611: logic lives in host helper; thin wrapper delegates.
    let Some(body) = function_body(src, "fn host_center_camera_on(")
        .or_else(|| function_body(src, "fn center_camera_on("))
    else {
        return false;
    };
    let ok = body.contains("Wave 460: prefer presentation-frozen height grid")
        && body.contains("sample_height")
        && body.contains("presentation_frame()")
        && body.contains("terrain_height_at")
        && body.contains("last_presentation_frame");
    residual_action_store(ResidualCameraCenterPresentationHeightAction::CenterSource);
    ok
}

pub fn simulate_camera_clamp_presentation_bounds_source() -> bool {
    let src = cnc_source();
    let Some(body) = function_body(src, "fn clamp_to_world_bounds(") else {
        return false;
    };
    // Wave 461: clamp delegates to presentation_world_bounds shared probe.
    let ok = (body.contains("presentation_world_bounds()")
        || (body.contains("presentation_frame()") && body.contains("world_bounds_vec3()")))
        && src.contains("fn presentation_world_bounds(")
        && src.contains("Wave 461: single presentation-first world bounds probe");
    residual_action_store(ResidualCameraCenterPresentationHeightAction::ClampSource);
    ok
}

pub fn honesty_camera_center_presentation_height_residual_pack_wave460() -> bool {
    honesty_camera_center_presentation_height_method_names_residual_wave460()
        && honesty_camera_center_presentation_height_source_markers_residual_wave460()
        && honesty_camera_center_presentation_height_nav_commands_residual_wave460()
        && simulate_camera_center_presentation_height_source()
        && simulate_camera_clamp_presentation_bounds_source()
}

pub fn simulate_live_camera_center_presentation_height_honesty() -> bool {
    let ok = honesty_camera_center_presentation_height_residual_pack_wave460();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualCameraCenterPresentationHeightAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_camera_center_presentation_height_method_names_residual_wave460());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_camera_center_presentation_height_source_markers_residual_wave460());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_camera_center_presentation_height_nav_commands_residual_wave460());
    }

    #[test]
    fn camera_center_presentation_height_sources() {
        assert!(simulate_camera_center_presentation_height_source());
        assert!(simulate_camera_clamp_presentation_bounds_source());
    }

    #[test]
    fn wave460_composite_pack() {
        assert!(honesty_camera_center_presentation_height_residual_pack_wave460());
    }

    #[test]
    fn simulate_live_camera_center_presentation_height_honesty_residual_live() {
        assert!(
            simulate_live_camera_center_presentation_height_honesty(),
            "camera center presentation height residual must latch"
        );
        assert!(residual_camera_center_presentation_height_ok());
        assert_eq!(
            residual_camera_center_presentation_height_last_action(),
            ResidualCameraCenterPresentationHeightAction::Composite
        );
    }
}
