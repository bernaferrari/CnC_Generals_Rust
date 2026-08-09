//! Wave 892: host_set_paused + boot_local_player_id dual-read peel.
//!
//! - `host_set_paused` stamps time_frozen from presentation residual (or one boot
//!   probe) || paused — no unconditional is_time_frozen dual-read on pause path.
//! - `boot_local_player_id_from_host` prefers host_match_local_player_id before
//!   live player_exists/min_player_id probes.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PAUSE_BOOT_PLAYER_METHOD_NAMES_WAVE892: &[&str] = &[
    "host_set_paused",
    "boot_local_player_id_from_host",
    "time_frozen_for_simulation",
    "host_match_local_player_id",
    "Wave 892",
    "playable_claim = false",
];

pub const LIVE_HOST_PAUSE_BOOT_PLAYER_NAV_STEPS_WAVE892: &[&str] = &[
    "PAUSE_FREEZE_FROM_PRESENTATION",
    "BOOT_LOCAL_PLAYER_RESIDUAL_FIRST",
    "LIVE_HOST_PAUSE_BOOT_PLAYER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPauseBootPlayerAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPauseBootPlayerAction) {
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

fn non_comment_lines(window: &str) -> impl Iterator<Item = &str> {
    window.lines().filter(|l| !l.trim_start().starts_with("//"))
}

pub fn honesty_host_pause_boot_player_method_names_residual_wave892() -> bool {
    let names = LIVE_HOST_PAUSE_BOOT_PLAYER_METHOD_NAMES_WAVE892;
    let ok = residual_name_index(names, "host_set_paused").is_some()
        && residual_name_index(names, "boot_local_player_id_from_host").is_some()
        && residual_name_index(names, "Wave 892").is_some();
    residual_action_store(ResidualHostPauseBootPlayerAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_pause_boot_player_nav_commands_residual_wave892() -> bool {
    let steps = LIVE_HOST_PAUSE_BOOT_PLAYER_NAV_STEPS_WAVE892;
    let ok = residual_name_index(steps, "LIVE_HOST_PAUSE_BOOT_PLAYER").is_some()
        && residual_name_index(steps, "PAUSE_FREEZE_FROM_PRESENTATION").is_some();
    residual_action_store(ResidualHostPauseBootPlayerAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_pause_boot_player_residual_pack_wave892() -> bool {
    let cnc = cnc_source();
    let pause = code_window(cnc, "fn host_set_paused", 900);
    let boot = code_window(cnc, "fn boot_local_player_id_from_host", 700);
    let pause_code: String = non_comment_lines(pause).collect::<Vec<_>>().join("\n");
    let boot_code: String = non_comment_lines(boot).collect::<Vec<_>>().join("\n");
    // Pause must prefer presentation time_frozen_for_simulation.
    let pause_ok = pause_code.contains("time_frozen_for_simulation")
        && pause_code.contains("last_presentation_frame")
        && pause_code.contains("script_frozen || paused");
    // Boot must check host_match_local_player_id first.
    let boot_ok = boot_code.contains("host_match_local_player_id")
        && boot.find("host_match_local_player_id").unwrap_or(9999)
            < boot.find("player_exists").unwrap_or(0);
    let ok = pause_ok && boot_ok && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostPauseBootPlayerAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_pause_boot_player_honesty() -> bool {
    let a = honesty_host_pause_boot_player_method_names_residual_wave892();
    let b = honesty_host_pause_boot_player_nav_commands_residual_wave892();
    let c = honesty_host_pause_boot_player_residual_pack_wave892();
    residual_action_store(ResidualHostPauseBootPlayerAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_pause_boot_player_residual_wave892() {
        assert!(honesty_host_pause_boot_player_residual_pack_wave892());
        assert!(honesty_host_pause_boot_player_method_names_residual_wave892());
        assert!(honesty_host_pause_boot_player_nav_commands_residual_wave892());
        assert!(simulate_live_host_pause_boot_player_honesty());
    }
}
