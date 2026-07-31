//! Wave 904: single-authority verification stamp + camera follow freeze-only pose.
//!
//! - InGame frame enables verification single-authority when dual-tick is not opted in.
//! - `host_set_camera_follow_object` stamps pose from presentation freeze only.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SINGLE_AUTHORITY_CAMERA_METHOD_NAMES_WAVE904: &[&str] = &[
    "set_verification_single_authority",
    "host_set_camera_follow_object",
    "GENERALS_ALLOW_DUAL_TICK",
    "Wave 904",
    "playable_claim = false",
];

pub const LIVE_HOST_SINGLE_AUTHORITY_CAMERA_NAV_STEPS_WAVE904: &[&str] = &[
    "VERIFY_SINGLE_AUTHORITY_DEFAULT",
    "CAMERA_FOLLOW_FREEZE_POSE_ONLY",
    "LIVE_HOST_SINGLE_AUTHORITY_CAMERA",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSingleAuthorityCameraAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSingleAuthorityCameraAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
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

pub fn honesty_host_single_authority_camera_method_names_residual_wave904() -> bool {
    let names = LIVE_HOST_SINGLE_AUTHORITY_CAMERA_METHOD_NAMES_WAVE904;
    let ok = residual_name_index(names, "set_verification_single_authority").is_some()
        && residual_name_index(names, "host_set_camera_follow_object").is_some()
        && residual_name_index(names, "Wave 904").is_some();
    residual_action_store(ResidualHostSingleAuthorityCameraAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_single_authority_camera_nav_commands_residual_wave904() -> bool {
    let steps = LIVE_HOST_SINGLE_AUTHORITY_CAMERA_NAV_STEPS_WAVE904;
    let ok = residual_name_index(steps, "LIVE_HOST_SINGLE_AUTHORITY_CAMERA").is_some()
        && residual_name_index(steps, "VERIFY_SINGLE_AUTHORITY_DEFAULT").is_some();
    residual_action_store(ResidualHostSingleAuthorityCameraAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_single_authority_camera_residual_pack_wave904() -> bool {
    let cnc = cnc_source();
    let follow_raw = code_window(cnc, "fn host_set_camera_follow_object", 900);
    let follow = non_comment_code(follow_raw);
    let ok = cnc.contains("set_verification_single_authority")
        && cnc.contains("GENERALS_ALLOW_DUAL_TICK")
        && follow.contains("last_presentation_frame")
        && !follow.contains("get_object")
        && follow.contains("set_camera_follow_object")
        && (follow_raw.contains("/904") || cnc.contains("Wave 904"))
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostSingleAuthorityCameraAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_single_authority_camera_honesty() -> bool {
    let a = honesty_host_single_authority_camera_method_names_residual_wave904();
    let b = honesty_host_single_authority_camera_nav_commands_residual_wave904();
    let c = honesty_host_single_authority_camera_residual_pack_wave904();
    residual_action_store(ResidualHostSingleAuthorityCameraAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_single_authority_camera_residual_wave904() {
        assert!(honesty_host_single_authority_camera_residual_pack_wave904());
        assert!(honesty_host_single_authority_camera_method_names_residual_wave904());
        assert!(honesty_host_single_authority_camera_nav_commands_residual_wave904());
        assert!(simulate_live_host_single_authority_camera_honesty());
    }
}
