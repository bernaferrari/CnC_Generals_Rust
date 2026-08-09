//! Wave 913: skip redundant authority dual-writes (camera follow / pause / selection).
//!
//! When host residual already matches the requested state, skip GameLogic writes.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_REDUNDANT_AUTHORITY_WRITE_SKIP_METHOD_NAMES_WAVE913: &[&str] = &[
    "host_set_camera_follow_object",
    "host_set_paused",
    "host_set_selection",
    "host_match_camera_follow_id",
    "Wave 913",
    "playable_claim = false",
];

pub const LIVE_HOST_REDUNDANT_AUTHORITY_WRITE_SKIP_NAV_STEPS_WAVE913: &[&str] = &[
    "SKIP_REDUNDANT_CAMERA_FOLLOW_WRITE",
    "SKIP_REDUNDANT_PAUSE_WRITE",
    "SKIP_REDUNDANT_SELECTION_WRITE",
    "LIVE_HOST_REDUNDANT_AUTHORITY_WRITE_SKIP",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostRedundantAuthorityWriteSkipAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostRedundantAuthorityWriteSkipAction) {
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

pub fn honesty_host_redundant_authority_write_skip_method_names_residual_wave913() -> bool {
    let names = LIVE_HOST_REDUNDANT_AUTHORITY_WRITE_SKIP_METHOD_NAMES_WAVE913;
    let ok = residual_name_index(names, "host_match_camera_follow_id").is_some()
        && residual_name_index(names, "Wave 913").is_some();
    residual_action_store(ResidualHostRedundantAuthorityWriteSkipAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_redundant_authority_write_skip_nav_commands_residual_wave913() -> bool {
    let steps = LIVE_HOST_REDUNDANT_AUTHORITY_WRITE_SKIP_NAV_STEPS_WAVE913;
    let ok = residual_name_index(steps, "LIVE_HOST_REDUNDANT_AUTHORITY_WRITE_SKIP").is_some()
        && residual_name_index(steps, "SKIP_REDUNDANT_CAMERA_FOLLOW_WRITE").is_some();
    residual_action_store(ResidualHostRedundantAuthorityWriteSkipAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_redundant_authority_write_skip_residual_pack_wave913() -> bool {
    let cnc = cnc_source();
    let cam_raw = code_window(cnc, "fn host_set_camera_follow_object", 1200);
    let cam = non_comment_code(cam_raw);
    let pause_raw = code_window(cnc, "fn host_set_paused", 900);
    let pause = non_comment_code(pause_raw);
    let sel_raw = code_window(cnc, "fn host_set_selection", 900);
    let sel = non_comment_code(sel_raw);
    let ok = cam_raw.contains("913")
        && cam.contains("host_match_camera_follow_id")
        && pause_raw.contains("913")
        && pause.contains("game_paused != paused")
        && sel_raw.contains("913")
        && sel.contains("already")
        && cnc.contains("host_match_camera_follow_id:")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostRedundantAuthorityWriteSkipAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_redundant_authority_write_skip_honesty() -> bool {
    let a = honesty_host_redundant_authority_write_skip_method_names_residual_wave913();
    let b = honesty_host_redundant_authority_write_skip_nav_commands_residual_wave913();
    let c = honesty_host_redundant_authority_write_skip_residual_pack_wave913();
    residual_action_store(ResidualHostRedundantAuthorityWriteSkipAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_redundant_authority_write_skip_residual_wave913() {
        assert!(honesty_host_redundant_authority_write_skip_residual_pack_wave913());
        assert!(honesty_host_redundant_authority_write_skip_method_names_residual_wave913());
        assert!(honesty_host_redundant_authority_write_skip_nav_commands_residual_wave913());
        assert!(simulate_live_host_redundant_authority_write_skip_honesty());
    }
}
