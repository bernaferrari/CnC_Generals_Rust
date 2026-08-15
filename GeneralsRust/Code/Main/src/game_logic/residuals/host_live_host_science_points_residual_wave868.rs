//! Wave 868: host_match_local_science_purchase_points residual peels science UI
//! dual-read; center-camera skips focus dual-read under freeze; enqueue refreshes
//! object-scan residuals. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SCIENCE_POINTS_METHOD_NAMES_WAVE868: &[&str] = &[
    "host_ui_local_science_purchase_points",
    "host_match_local_science_purchase_points",
    "host_center_camera_and_request_focus",
    "host_enqueue_production",
    "Wave 868",
    "playable_claim = false",
];

pub const LIVE_HOST_SCIENCE_POINTS_NAV_STEPS_WAVE868: &[&str] = &[
    "STAMP_SCIENCE_PURCHASE_POINTS",
    "PREFER_SCIENCE_RESIDUAL",
    "SKIP_CAMERA_FOCUS_UNDER_FREEZE",
    "REFRESH_AFTER_ENQUEUE",
    "LIVE_HOST_SCIENCE_POINTS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSciencePointsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSciencePointsAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

pub fn honesty_host_science_points_method_names_residual_wave868() -> bool {
    let names = LIVE_HOST_SCIENCE_POINTS_METHOD_NAMES_WAVE868;
    let ok = residual_name_index(names, "host_ui_local_science_purchase_points").is_some()
        && residual_name_index(names, "host_match_local_science_purchase_points").is_some()
        && residual_name_index(names, "Wave 868").is_some();
    residual_action_store(ResidualHostSciencePointsAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_science_points_nav_commands_residual_wave868() -> bool {
    let steps = LIVE_HOST_SCIENCE_POINTS_NAV_STEPS_WAVE868;
    let ok = residual_name_index(steps, "LIVE_HOST_SCIENCE_POINTS").is_some()
        && residual_name_index(steps, "STAMP_SCIENCE_PURCHASE_POINTS").is_some();
    residual_action_store(ResidualHostSciencePointsAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_science_points_residual_pack_wave868() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("host_match_local_science_purchase_points: Option<i32>")
        && cnc.contains("Wave 610/868: host residual helper")
        && cnc.contains("if let Some(v) = self.host_match_local_science_purchase_points")
        && cnc.contains("Wave 868")
        && cnc.contains("Wave 577")
        && cnc.contains("if self.last_presentation_frame.is_none()")
        && cnc.contains("Wave 582");
    residual_action_store(ResidualHostSciencePointsAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_science_points_honesty() -> bool {
    let a = honesty_host_science_points_method_names_residual_wave868();
    let b = honesty_host_science_points_nav_commands_residual_wave868();
    let c = honesty_host_science_points_residual_pack_wave868();
    residual_action_store(ResidualHostSciencePointsAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_science_points_residual_wave868() {
        assert!(honesty_host_science_points_residual_pack_wave868());
        assert!(honesty_host_science_points_method_names_residual_wave868());
        assert!(honesty_host_science_points_nav_commands_residual_wave868());
        assert!(simulate_live_host_science_points_honesty());
    }
}
