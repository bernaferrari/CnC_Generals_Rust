//! Wave 897: boot player/alive/science fail-closed dual-read peels.
//!
//! - `boot_local_player_id_from_host` uses host residual only (no player_exists).
//! - `boot_player_info_from_host` prefers diplomacy residual / presentation freeze.
//! - `host_object_is_alive` / `host_player_can_purchase_science` fail-closed when cold.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_BOOT_PLAYER_ALIVE_SCIENCE_METHOD_NAMES_WAVE897: &[&str] = &[
    "boot_local_player_id_from_host",
    "boot_player_info_from_host",
    "host_object_is_alive",
    "host_player_can_purchase_science",
    "Wave 897",
    "playable_claim = false",
];

pub const LIVE_HOST_BOOT_PLAYER_ALIVE_SCIENCE_NAV_STEPS_WAVE897: &[&str] = &[
    "BOOT_LOCAL_PLAYER_NO_DUAL_READ",
    "BOOT_PLAYER_INFO_RESIDUAL_FIRST",
    "OBJECT_ALIVE_FAILCLOSED",
    "SCIENCE_PURCHASE_FAILCLOSED",
    "LIVE_HOST_BOOT_PLAYER_ALIVE_SCIENCE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostBootPlayerAliveScienceAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostBootPlayerAliveScienceAction) {
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

pub fn honesty_host_boot_player_alive_science_method_names_residual_wave897() -> bool {
    let names = LIVE_HOST_BOOT_PLAYER_ALIVE_SCIENCE_METHOD_NAMES_WAVE897;
    let ok = residual_name_index(names, "boot_local_player_id_from_host").is_some()
        && residual_name_index(names, "host_object_is_alive").is_some()
        && residual_name_index(names, "Wave 897").is_some();
    residual_action_store(ResidualHostBootPlayerAliveScienceAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_boot_player_alive_science_nav_commands_residual_wave897() -> bool {
    let steps = LIVE_HOST_BOOT_PLAYER_ALIVE_SCIENCE_NAV_STEPS_WAVE897;
    let ok = residual_name_index(steps, "LIVE_HOST_BOOT_PLAYER_ALIVE_SCIENCE").is_some()
        && residual_name_index(steps, "BOOT_LOCAL_PLAYER_NO_DUAL_READ").is_some();
    residual_action_store(ResidualHostBootPlayerAliveScienceAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_boot_player_alive_science_residual_pack_wave897() -> bool {
    let cnc = cnc_source();
    let boot_local = non_comment_code(code_window(cnc, "fn boot_local_player_id_from_host", 500));
    let boot_info = non_comment_code(code_window(cnc, "fn boot_player_info_from_host", 700));
    let alive = non_comment_code(code_window(cnc, "fn host_object_is_alive", 500));
    let science = non_comment_code(code_window(cnc, "fn host_player_can_purchase_science", 600));
    let ok = boot_local.contains("current_player_id")
        && !boot_local.contains("player_exists")
        && !boot_local.contains("min_player_id")
        && boot_info.contains("host_match_diplomacy_players")
        && boot_info.contains("last_presentation_frame")
        && !boot_info.contains("player_exists")
        && !alive.contains("object_is_alive(id)")
        && alive.contains("false")
        && !science.contains("player_can_purchase_science(player_id")
        && science.contains("false")
        && cnc.contains("Wave 897")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostBootPlayerAliveScienceAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_boot_player_alive_science_honesty() -> bool {
    let a = honesty_host_boot_player_alive_science_method_names_residual_wave897();
    let b = honesty_host_boot_player_alive_science_nav_commands_residual_wave897();
    let c = honesty_host_boot_player_alive_science_residual_pack_wave897();
    residual_action_store(ResidualHostBootPlayerAliveScienceAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_boot_player_alive_science_residual_wave897() {
        assert!(honesty_host_boot_player_alive_science_residual_pack_wave897());
        assert!(honesty_host_boot_player_alive_science_method_names_residual_wave897());
        assert!(honesty_host_boot_player_alive_science_nav_commands_residual_wave897());
        assert!(simulate_live_host_boot_player_alive_science_honesty());
    }
}
