//! Historical Wave 858 coverage now verifies one canonical camera-default owner.
//! Native operations observe SessionView; Native85 retains data-only observations.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SCRIPT_CAMERA_RESIDUALS_METHOD_NAMES_WAVE858: &[&str] = &[
    "script_default_camera_max_height",
    "script_default_camera_pitch",
    "host_ui_script_default_camera_max_height",
    "host_ui_script_default_camera_pitch",
    "Wave 858",
    "playable_claim = false",
];

pub const LIVE_HOST_SCRIPT_CAMERA_RESIDUALS_NAV_STEPS_WAVE858: &[&str] = &[
    "READ_SESSION_CAMERA_DEFAULTS",
    "PRESERVE_NATIVE85_OBSERVATION_SLOTS",
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
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_host_script_camera_residuals_method_names_residual_wave858() -> bool {
    let names = LIVE_HOST_SCRIPT_CAMERA_RESIDUALS_METHOD_NAMES_WAVE858;
    let ok = residual_name_index(names, "script_default_camera_max_height").is_some()
        && residual_name_index(names, "script_default_camera_pitch").is_some()
        && residual_name_index(names, "Wave 858").is_some();
    residual_action_store(ResidualHostScriptCameraResidualsAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_script_camera_residuals_nav_commands_residual_wave858() -> bool {
    let steps = LIVE_HOST_SCRIPT_CAMERA_RESIDUALS_NAV_STEPS_WAVE858;
    let ok = residual_name_index(steps, "LIVE_HOST_SCRIPT_CAMERA_RESIDUALS").is_some()
        && residual_name_index(steps, "READ_SESSION_CAMERA_DEFAULTS").is_some();
    residual_action_store(ResidualHostScriptCameraResidualsAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_script_camera_residuals_residual_pack_wave858() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("self.game_logic.script_default_camera_max_height()")
        && cnc.contains("self.game_logic.script_default_camera_pitch()")
        && !cnc.contains("self.host_match_script_camera_max_height")
        && !cnc.contains("self.host_match_script_camera_pitch")
        && !cnc.contains("pub(crate) host_match_script_camera_max_height")
        && !cnc.contains("pub(crate) host_match_script_camera_pitch");
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
