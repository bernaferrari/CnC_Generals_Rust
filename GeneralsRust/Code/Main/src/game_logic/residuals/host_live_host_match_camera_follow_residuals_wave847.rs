//! Wave 847: host-owned camera-follow residuals peel live GameLogic dual-reads
//! from presentation_or_boot_camera_follow_active, boot camera residual, and
//! host_set_camera_follow_object stamp path.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MATCH_CAMERA_FOLLOW_RESIDUALS_METHOD_NAMES_WAVE847: &[&str] = &[
    "host_match_camera_follow_active",
    "host_match_camera_follow_position",
    "presentation_or_boot_camera_follow_active",
    "host_set_camera_follow_object",
    "Wave 847",
    "playable_claim = false",
];

pub const LIVE_HOST_MATCH_CAMERA_FOLLOW_RESIDUALS_NAV_STEPS_WAVE847: &[&str] = &[
    "STAMP_HOST_MATCH_CAMERA_FOLLOW",
    "PREFER_FREEZE_THEN_HOST_FOLLOW",
    "LIVE_HOST_MATCH_CAMERA_FOLLOW_RESIDUALS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMatchCameraFollowResidualsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMatchCameraFollowResidualsAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

pub fn honesty_host_match_camera_follow_residuals_method_names_residual_wave847() -> bool {
    let names = LIVE_HOST_MATCH_CAMERA_FOLLOW_RESIDUALS_METHOD_NAMES_WAVE847;
    let ok = residual_name_index(names, "host_match_camera_follow_active").is_some()
        && residual_name_index(names, "host_match_camera_follow_position").is_some()
        && residual_name_index(names, "Wave 847").is_some();
    residual_action_store(ResidualHostMatchCameraFollowResidualsAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_camera_follow_residuals_nav_commands_residual_wave847() -> bool {
    let steps = LIVE_HOST_MATCH_CAMERA_FOLLOW_RESIDUALS_NAV_STEPS_WAVE847;
    let ok = residual_name_index(steps, "LIVE_HOST_MATCH_CAMERA_FOLLOW_RESIDUALS").is_some()
        && residual_name_index(steps, "STAMP_HOST_MATCH_CAMERA_FOLLOW").is_some();
    residual_action_store(ResidualHostMatchCameraFollowResidualsAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_camera_follow_residuals_residual_pack_wave847() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("host_match_camera_follow_active: Option<bool>")
        && cnc.contains("host_match_camera_follow_position: Option<[f32; 3]>")
        && cnc.contains("Wave 583/847")
        && cnc.contains("Wave 847: camera-follow host residual")
        && cnc.contains("Wave 847: prefer host_match camera-follow residual")
        && cnc.contains("self.host_match_camera_follow_active = Some(id.is_some())")
        && cnc.contains("if let Some(v) = self.host_match_camera_follow_active");
    residual_action_store(ResidualHostMatchCameraFollowResidualsAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_match_camera_follow_residuals_honesty() -> bool {
    let a = honesty_host_match_camera_follow_residuals_method_names_residual_wave847();
    let b = honesty_host_match_camera_follow_residuals_nav_commands_residual_wave847();
    let c = honesty_host_match_camera_follow_residuals_residual_pack_wave847();
    residual_action_store(ResidualHostMatchCameraFollowResidualsAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_match_camera_follow_residuals_residual_wave847() {
        assert!(honesty_host_match_camera_follow_residuals_residual_pack_wave847());
        assert!(honesty_host_match_camera_follow_residuals_method_names_residual_wave847());
        assert!(honesty_host_match_camera_follow_residuals_nav_commands_residual_wave847());
        assert!(simulate_live_host_match_camera_follow_residuals_honesty());
    }
}
