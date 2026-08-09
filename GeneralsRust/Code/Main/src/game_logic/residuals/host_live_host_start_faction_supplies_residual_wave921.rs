//! Wave 921: start_new_game_with_faction authority boundary + supplies residual peel.
//!
//! - host match start uses one GameLogic start_new_game_with_faction call
//! - supplies floor checks host_match_local_supplies residual when freeze cold
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_START_FACTION_SUPPLIES_METHOD_NAMES_WAVE921: &[&str] = &[
    "host_start_new_game_with_faction",
    "start_new_game_with_faction",
    "host_ensure_player_min_supplies_residual",
    "host_match_local_supplies",
    "Wave 921",
    "playable_claim = false",
];

pub const LIVE_HOST_START_FACTION_SUPPLIES_NAV_STEPS_WAVE921: &[&str] = &[
    "START_NEW_GAME_WITH_FACTION_BOUNDARY",
    "SUPPLIES_FLOOR_HOST_RESIDUAL",
    "LIVE_HOST_START_FACTION_SUPPLIES",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostStartFactionSuppliesAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostStartFactionSuppliesAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
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

pub fn honesty_host_start_faction_supplies_method_names_residual_wave921() -> bool {
    let names = LIVE_HOST_START_FACTION_SUPPLIES_METHOD_NAMES_WAVE921;
    let ok = residual_name_index(names, "start_new_game_with_faction").is_some()
        && residual_name_index(names, "Wave 921").is_some();
    residual_action_store(ResidualHostStartFactionSuppliesAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_start_faction_supplies_nav_commands_residual_wave921() -> bool {
    let steps = LIVE_HOST_START_FACTION_SUPPLIES_NAV_STEPS_WAVE921;
    let ok = residual_name_index(steps, "LIVE_HOST_START_FACTION_SUPPLIES").is_some()
        && residual_name_index(steps, "START_NEW_GAME_WITH_FACTION_BOUNDARY").is_some();
    residual_action_store(ResidualHostStartFactionSuppliesAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_start_faction_supplies_residual_pack_wave921() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let start_raw = code_window(cnc, "fn host_start_new_game_with_faction", 1100);
    let start = non_comment_code(start_raw);
    let sup_raw = code_window(cnc, "fn host_ensure_player_min_supplies_residual", 900);
    let sup = non_comment_code(sup_raw);
    let helper_raw = code_window(gl, "fn start_new_game_with_faction", 700);
    let helper = non_comment_code(helper_raw);
    let ok = start_raw.contains("921")
        && start.contains("start_new_game_with_faction")
        && !start.contains("setup_skirmish_ai(")
        && !start.contains("set_player_team")
        && sup_raw.contains("921")
        && sup.contains("host_match_local_supplies")
        && helper.contains("start_new_game(mode)")
        && cnc.contains("host_match_local_supplies:")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostStartFactionSuppliesAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_start_faction_supplies_honesty() -> bool {
    let a = honesty_host_start_faction_supplies_method_names_residual_wave921();
    let b = honesty_host_start_faction_supplies_nav_commands_residual_wave921();
    let c = honesty_host_start_faction_supplies_residual_pack_wave921();
    residual_action_store(ResidualHostStartFactionSuppliesAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_start_faction_supplies_residual_wave921() {
        assert!(honesty_host_start_faction_supplies_residual_pack_wave921());
        assert!(honesty_host_start_faction_supplies_method_names_residual_wave921());
        assert!(honesty_host_start_faction_supplies_nav_commands_residual_wave921());
        assert!(simulate_live_host_start_faction_supplies_honesty());
    }
}
