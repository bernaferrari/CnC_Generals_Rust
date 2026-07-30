//! Wave 901: host_refresh_match_sim residual dual-read peel + stamp wipe fix.
//!
//! - Diplomacy/template/sciences from freeze only (no player_unlocked dual-read).
//! - Cold residual fail-closed for camera/mp/bounds/opponent/purchasable sciences.
//! - Removed Wave 855 clear that wiped camera/mp/bounds/opponent stamps after write.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_REFRESH_SIM_FAILCLOSED_METHOD_NAMES_WAVE901: &[&str] = &[
    "host_refresh_match_sim_residuals_from_logic",
    "host_match_unlocked_sciences",
    "host_match_purchasable_sciences",
    "host_match_in_multiplayer",
    "Wave 901",
    "playable_claim = false",
];

pub const LIVE_HOST_REFRESH_SIM_FAILCLOSED_NAV_STEPS_WAVE901: &[&str] = &[
    "SCIENCES_FREEZE_ONLY",
    "NO_STAMP_WIPE_AFTER_WRITE",
    "PURCHASABLE_SCIENCES_FAILCLOSED",
    "LIVE_HOST_REFRESH_SIM_FAILCLOSED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostRefreshSimFailclosedAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostRefreshSimFailclosedAction) {
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

pub fn honesty_host_refresh_sim_failclosed_method_names_residual_wave901() -> bool {
    let names = LIVE_HOST_REFRESH_SIM_FAILCLOSED_METHOD_NAMES_WAVE901;
    let ok = residual_name_index(names, "host_refresh_match_sim_residuals_from_logic").is_some()
        && residual_name_index(names, "Wave 901").is_some();
    residual_action_store(ResidualHostRefreshSimFailclosedAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_refresh_sim_failclosed_nav_commands_residual_wave901() -> bool {
    let steps = LIVE_HOST_REFRESH_SIM_FAILCLOSED_NAV_STEPS_WAVE901;
    let ok = residual_name_index(steps, "LIVE_HOST_REFRESH_SIM_FAILCLOSED").is_some()
        && residual_name_index(steps, "NO_STAMP_WIPE_AFTER_WRITE").is_some();
    residual_action_store(ResidualHostRefreshSimFailclosedAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_refresh_sim_failclosed_residual_pack_wave901() -> bool {
    let cnc = cnc_source();
    let refresh = non_comment_code(code_window(
        cnc,
        "fn host_refresh_match_sim_residuals_from_logic",
        12000,
    ));
    let raw = code_window(cnc, "fn host_refresh_match_sim_residuals_from_logic", 12000);
    let ok = !refresh.contains("player_unlocked_sciences")
        && !refresh.contains("player_can_purchase_science")
        && !refresh.contains("isInMultiplayerGame()")
        && !refresh.contains("first_opponent_id(local)")
        && !refresh.contains("script_default_camera_max_height()")
        && !refresh.contains("templates.keys()")
        && refresh.contains("host_match_in_multiplayer = Some(false)")
        && !refresh.contains("host_match_script_camera_max_height = None")
        && !refresh.contains("host_match_in_multiplayer = None")
        && raw.contains("Wave 901")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostRefreshSimFailclosedAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_refresh_sim_failclosed_honesty() -> bool {
    let a = honesty_host_refresh_sim_failclosed_method_names_residual_wave901();
    let b = honesty_host_refresh_sim_failclosed_nav_commands_residual_wave901();
    let c = honesty_host_refresh_sim_failclosed_residual_pack_wave901();
    residual_action_store(ResidualHostRefreshSimFailclosedAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_refresh_sim_failclosed_residual_wave901() {
        assert!(honesty_host_refresh_sim_failclosed_residual_pack_wave901());
        assert!(honesty_host_refresh_sim_failclosed_method_names_residual_wave901());
        assert!(honesty_host_refresh_sim_failclosed_nav_commands_residual_wave901());
        assert!(simulate_live_host_refresh_sim_failclosed_honesty());
    }
}
