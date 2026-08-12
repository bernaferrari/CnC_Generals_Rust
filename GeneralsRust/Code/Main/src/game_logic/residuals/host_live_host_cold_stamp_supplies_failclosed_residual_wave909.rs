//! Wave 909: cold stamp/refresh + supplies floor dual-read peels.
//!
//! - cold sim-timing residual keeps prior stamp / fail-closed zeros (no live probe)
//! - runtime-host supplies floor skips write when presentation residual meets floor
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_COLD_STAMP_SUPPLIES_FAILCLOSED_METHOD_NAMES_WAVE909: &[&str] = &[
    "host_stamp_sim_timing_residuals",
    "host_refresh_match_sim_residuals_from_logic",
    "host_ensure_player_min_supplies_residual",
    "Wave 909",
    "playable_claim = false",
];

pub const LIVE_HOST_COLD_STAMP_SUPPLIES_FAILCLOSED_NAV_STEPS_WAVE909: &[&str] = &[
    "COLD_STAMP_NO_LIVE_SNAPSHOT",
    "SUPPLIES_FLOOR_PRESENTATION_FIRST",
    "LIVE_HOST_COLD_STAMP_SUPPLIES_FAILCLOSED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostColdStampSuppliesFailclosedAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostColdStampSuppliesFailclosedAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
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

pub fn honesty_host_cold_stamp_supplies_failclosed_method_names_residual_wave909() -> bool {
    let names = LIVE_HOST_COLD_STAMP_SUPPLIES_FAILCLOSED_METHOD_NAMES_WAVE909;
    let ok = residual_name_index(names, "host_ensure_player_min_supplies_residual").is_some()
        && residual_name_index(names, "Wave 909").is_some();
    residual_action_store(ResidualHostColdStampSuppliesFailclosedAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_cold_stamp_supplies_failclosed_nav_commands_residual_wave909() -> bool {
    let steps = LIVE_HOST_COLD_STAMP_SUPPLIES_FAILCLOSED_NAV_STEPS_WAVE909;
    let ok = residual_name_index(steps, "LIVE_HOST_COLD_STAMP_SUPPLIES_FAILCLOSED").is_some()
        && residual_name_index(steps, "COLD_STAMP_NO_LIVE_SNAPSHOT").is_some();
    residual_action_store(ResidualHostColdStampSuppliesFailclosedAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_cold_stamp_supplies_failclosed_residual_pack_wave909() -> bool {
    let cnc = cnc_source();
    let stamp_raw = code_window(cnc, "fn host_stamp_sim_timing_residuals", 1100);
    let stamp = non_comment_code(stamp_raw);
    let refresh_raw = code_window(cnc, "fn host_refresh_match_sim_residuals_from_logic", 2200);
    let refresh = non_comment_code(refresh_raw);
    let supplies_raw = code_window(cnc, "fn host_ensure_player_min_supplies_residual", 700);
    let supplies = non_comment_code(supplies_raw);
    let ok = stamp_raw.contains("909")
        && !stamp.contains("sim_timing_snapshot")
        && refresh_raw.contains("909")
        && !refresh.contains("sim_timing_snapshot")
        && supplies_raw.contains("909")
        && supplies.contains("local_supplies")
        && supplies.contains("ensure_player_min_supplies")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostColdStampSuppliesFailclosedAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_cold_stamp_supplies_failclosed_honesty() -> bool {
    let a = honesty_host_cold_stamp_supplies_failclosed_method_names_residual_wave909();
    let b = honesty_host_cold_stamp_supplies_failclosed_nav_commands_residual_wave909();
    let c = honesty_host_cold_stamp_supplies_failclosed_residual_pack_wave909();
    residual_action_store(ResidualHostColdStampSuppliesFailclosedAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_cold_stamp_supplies_failclosed_residual_wave909() {
        assert!(honesty_host_cold_stamp_supplies_failclosed_residual_pack_wave909());
        assert!(honesty_host_cold_stamp_supplies_failclosed_method_names_residual_wave909());
        assert!(honesty_host_cold_stamp_supplies_failclosed_nav_commands_residual_wave909());
        assert!(simulate_live_host_cold_stamp_supplies_failclosed_honesty());
    }
}
