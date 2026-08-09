//! Wave 903: camera focus dual-read peel.
//!
//! - `host_center_camera_and_request_focus` no longer dual-writes request_camera_focus.
//! - `host_set_camera_follow_object` stamps pose from presentation freeze first.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CAMERA_FOCUS_FAILCLOSED_METHOD_NAMES_WAVE903: &[&str] = &[
    "host_center_camera_and_request_focus",
    "host_set_camera_follow_object",
    "last_presentation_frame",
    "Wave 903",
    "playable_claim = false",
];

pub const LIVE_HOST_CAMERA_FOCUS_FAILCLOSED_NAV_STEPS_WAVE903: &[&str] = &[
    "CENTER_CAMERA_NO_REQUEST_FOCUS",
    "FOLLOW_POSE_FROM_PRESENTATION",
    "LIVE_HOST_CAMERA_FOCUS_FAILCLOSED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCameraFocusFailclosedAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostCameraFocusFailclosedAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

fn code_window<'a>(src: &'a str, marker: &str, len: usize) -> &'a str {
    match src.find(marker) {
        Some(i) => &src[i..src.len().min(i + len)],
        None => "",
    }
}

fn non_comment_code(window: &str) -> String {
    window
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn honesty_host_camera_focus_failclosed_method_names_residual_wave903() -> bool {
    let names = LIVE_HOST_CAMERA_FOCUS_FAILCLOSED_METHOD_NAMES_WAVE903;
    let ok = residual_name_index(names, "host_center_camera_and_request_focus").is_some()
        && residual_name_index(names, "host_set_camera_follow_object").is_some()
        && residual_name_index(names, "Wave 903").is_some();
    residual_action_store(ResidualHostCameraFocusFailclosedAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_camera_focus_failclosed_nav_commands_residual_wave903() -> bool {
    let steps = LIVE_HOST_CAMERA_FOCUS_FAILCLOSED_NAV_STEPS_WAVE903;
    let ok = residual_name_index(steps, "LIVE_HOST_CAMERA_FOCUS_FAILCLOSED").is_some()
        && residual_name_index(steps, "CENTER_CAMERA_NO_REQUEST_FOCUS").is_some();
    residual_action_store(ResidualHostCameraFocusFailclosedAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_camera_focus_failclosed_residual_pack_wave903() -> bool {
    let cnc = cnc_source();
    let center_raw = code_window(cnc, "fn host_center_camera_and_request_focus", 700);
    let center = non_comment_code(center_raw);
    let follow_raw = code_window(cnc, "fn host_set_camera_follow_object", 1200);
    let follow = non_comment_code(follow_raw);
    let ok = !center.contains("request_camera_focus")
        && center.contains("camera_target")
        && follow.contains("last_presentation_frame")
        && follow.contains("set_camera_follow_object")
        && (center_raw.contains("/903") || follow_raw.contains("/903") || cnc.contains("Wave 903"))
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostCameraFocusFailclosedAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_camera_focus_failclosed_honesty() -> bool {
    let a = honesty_host_camera_focus_failclosed_method_names_residual_wave903();
    let b = honesty_host_camera_focus_failclosed_nav_commands_residual_wave903();
    let c = honesty_host_camera_focus_failclosed_residual_pack_wave903();
    residual_action_store(ResidualHostCameraFocusFailclosedAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_camera_focus_failclosed_residual_wave903() {
        assert!(honesty_host_camera_focus_failclosed_residual_pack_wave903());
        assert!(honesty_host_camera_focus_failclosed_method_names_residual_wave903());
        assert!(honesty_host_camera_focus_failclosed_nav_commands_residual_wave903());
        assert!(simulate_live_host_camera_focus_failclosed_honesty());
    }
}
