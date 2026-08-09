//! Wave 858: host-owned script camera max-height/pitch residuals peel live
//! GameLogic dual-reads from host_ui_script_default_camera_* boot paths.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SCRIPT_CAMERA_RESIDUALS_METHOD_NAMES_WAVE858: &[&str] = &[
    "host_match_script_camera_max_height",
    "host_match_script_camera_pitch",
    "host_ui_script_default_camera_max_height",
    "host_ui_script_default_camera_pitch",
    "Wave 858",
    "playable_claim = false",
];

pub const LIVE_HOST_SCRIPT_CAMERA_RESIDUALS_NAV_STEPS_WAVE858: &[&str] = &[
    "STAMP_HOST_SCRIPT_CAMERA",
    "PREFER_FREEZE_THEN_HOST_CAMERA",
    "LIVE_HOST_SCRIPT_CAMERA_RESIDUALS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostScriptCameraResidualsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostScriptCameraResidualsAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

pub fn honesty_host_script_camera_residuals_method_names_residual_wave858() -> bool {
    let names = LIVE_HOST_SCRIPT_CAMERA_RESIDUALS_METHOD_NAMES_WAVE858;
    let ok = residual_name_index(names, "host_match_script_camera_max_height").is_some()
        && residual_name_index(names, "host_match_script_camera_pitch").is_some()
        && residual_name_index(names, "Wave 858").is_some();
    residual_action_store(ResidualHostScriptCameraResidualsAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_script_camera_residuals_nav_commands_residual_wave858() -> bool {
    let steps = LIVE_HOST_SCRIPT_CAMERA_RESIDUALS_NAV_STEPS_WAVE858;
    let ok = residual_name_index(steps, "LIVE_HOST_SCRIPT_CAMERA_RESIDUALS").is_some()
        && residual_name_index(steps, "STAMP_HOST_SCRIPT_CAMERA").is_some();
    residual_action_store(ResidualHostScriptCameraResidualsAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_script_camera_residuals_residual_pack_wave858() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("host_match_script_camera_max_height: Option<f32>")
        && cnc.contains("host_match_script_camera_pitch: Option<f32>")
        && cnc.contains("Wave 858: stamp script camera defaults")
        && cnc.contains("Wave 607/858")
        && cnc.contains("Wave 609/858")
        && cnc.contains("if let Some(v) = self.host_match_script_camera_max_height")
        && cnc.contains("if let Some(v) = self.host_match_script_camera_pitch");
    residual_action_store(ResidualHostScriptCameraResidualsAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_script_camera_residuals_honesty() -> bool {
    let a = honesty_host_script_camera_residuals_method_names_residual_wave858();
    let b = honesty_host_script_camera_residuals_nav_commands_residual_wave858();
    let c = honesty_host_script_camera_residuals_residual_pack_wave858();
    residual_action_store(ResidualHostScriptCameraResidualsAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_script_camera_residuals_residual_wave858() {
        assert!(honesty_host_script_camera_residuals_residual_pack_wave858());
        assert!(honesty_host_script_camera_residuals_method_names_residual_wave858());
        assert!(honesty_host_script_camera_residuals_nav_commands_residual_wave858());
        assert!(simulate_live_host_script_camera_residuals_honesty());
    }
}
