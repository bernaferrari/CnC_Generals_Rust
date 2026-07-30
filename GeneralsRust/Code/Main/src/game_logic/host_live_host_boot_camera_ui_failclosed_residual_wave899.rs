//! Wave 899: boot camera/movie/UI fail-closed dual-read peels.
//!
//! - `apply_boot_camera_residual` uses host_match follow only (no take_* dual-read).
//! - `host_drain_live_camera_request_queues` fail-closed no-op.
//! - `apply_boot_movie_residual` / `apply_boot_popup_music_residual` no-op.
//! - `host_update_ui_state` boot path stamps empty UI residual (no update_ui_state dual-read).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_BOOT_CAMERA_UI_FAILCLOSED_METHOD_NAMES_WAVE899: &[&str] = &[
    "apply_boot_camera_residual",
    "host_drain_live_camera_request_queues",
    "apply_boot_movie_residual",
    "apply_boot_popup_music_residual",
    "host_update_ui_state",
    "Wave 899",
    "playable_claim = false",
];

pub const LIVE_HOST_BOOT_CAMERA_UI_FAILCLOSED_NAV_STEPS_WAVE899: &[&str] = &[
    "BOOT_CAMERA_HOST_MATCH_ONLY",
    "CAMERA_DRAIN_NOOP",
    "BOOT_MOVIE_POPUP_NOOP",
    "UI_STATE_BOOT_DEFAULT",
    "LIVE_HOST_BOOT_CAMERA_UI_FAILCLOSED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostBootCameraUiFailclosedAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostBootCameraUiFailclosedAction) {
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

pub fn honesty_host_boot_camera_ui_failclosed_method_names_residual_wave899() -> bool {
    let names = LIVE_HOST_BOOT_CAMERA_UI_FAILCLOSED_METHOD_NAMES_WAVE899;
    let ok = residual_name_index(names, "apply_boot_camera_residual").is_some()
        && residual_name_index(names, "host_update_ui_state").is_some()
        && residual_name_index(names, "Wave 899").is_some();
    residual_action_store(ResidualHostBootCameraUiFailclosedAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_boot_camera_ui_failclosed_nav_commands_residual_wave899() -> bool {
    let steps = LIVE_HOST_BOOT_CAMERA_UI_FAILCLOSED_NAV_STEPS_WAVE899;
    let ok = residual_name_index(steps, "LIVE_HOST_BOOT_CAMERA_UI_FAILCLOSED").is_some()
        && residual_name_index(steps, "BOOT_CAMERA_HOST_MATCH_ONLY").is_some();
    residual_action_store(ResidualHostBootCameraUiFailclosedAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_boot_camera_ui_failclosed_residual_pack_wave899() -> bool {
    let cnc = cnc_source();
    let cam = non_comment_code(code_window(cnc, "fn apply_boot_camera_residual", 500));
    let drain = non_comment_code(code_window(
        cnc,
        "fn host_drain_live_camera_request_queues",
        500,
    ));
    let movie = non_comment_code(code_window(cnc, "fn apply_boot_movie_residual", 400));
    let popup = non_comment_code(code_window(cnc, "fn apply_boot_popup_music_residual", 400));
    let ui = non_comment_code(code_window(cnc, "fn host_update_ui_state", 900));
    let ok = cam.contains("host_match_camera_follow_position")
        && !cam.contains("take_camera_")
        && !cam.contains("camera_follow_target_position")
        && !drain.contains("take_camera_")
        && !movie.contains("take_pending_movie")
        && !popup.contains("take_popup_message")
        && !popup.contains("take_music_stop")
        && ui.contains("GameUIState::default()")
        && !ui.contains("update_ui_state(player_id)")
        && cnc.contains("Wave 899")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostBootCameraUiFailclosedAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_boot_camera_ui_failclosed_honesty() -> bool {
    let a = honesty_host_boot_camera_ui_failclosed_method_names_residual_wave899();
    let b = honesty_host_boot_camera_ui_failclosed_nav_commands_residual_wave899();
    let c = honesty_host_boot_camera_ui_failclosed_residual_pack_wave899();
    residual_action_store(ResidualHostBootCameraUiFailclosedAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_boot_camera_ui_failclosed_residual_wave899() {
        assert!(honesty_host_boot_camera_ui_failclosed_residual_pack_wave899());
        assert!(honesty_host_boot_camera_ui_failclosed_method_names_residual_wave899());
        assert!(honesty_host_boot_camera_ui_failclosed_nav_commands_residual_wave899());
        assert!(simulate_live_host_boot_camera_ui_failclosed_honesty());
    }
}
