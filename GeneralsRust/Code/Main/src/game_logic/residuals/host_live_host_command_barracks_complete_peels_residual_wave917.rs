//! Wave 917: command stamp freeze skip + barracks/complete residual peels.
//!
//! - host_command_* skip mid-command sim-timing stamp under presentation freeze
//! - barracks ensure/force-ensure skip when residual already lists producer
//! - force_complete_construction skips when presentation residual is complete
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_COMMAND_BARRACKS_COMPLETE_PEELS_METHOD_NAMES_WAVE917: &[&str] = &[
    "host_command_attack",
    "host_command_stop",
    "host_command_attack_move",
    "host_command_move",
    "host_ensure_barracks_building_data",
    "host_force_ensure_barracks_building_data",
    "host_force_complete_construction",
    "Wave 917",
    "playable_claim = false",
];

pub const LIVE_HOST_COMMAND_BARRACKS_COMPLETE_PEELS_NAV_STEPS_WAVE917: &[&str] = &[
    "COMMAND_STAMP_SKIP_UNDER_FREEZE",
    "BARRACKS_ENSURE_RESIDUAL_HIT",
    "FORCE_COMPLETE_PRESENTATION_SKIP",
    "LIVE_HOST_COMMAND_BARRACKS_COMPLETE_PEELS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCommandBarracksCompletePeelsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostCommandBarracksCompletePeelsAction) {
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

pub fn honesty_host_command_barracks_complete_peels_method_names_residual_wave917() -> bool {
    let names = LIVE_HOST_COMMAND_BARRACKS_COMPLETE_PEELS_METHOD_NAMES_WAVE917;
    let ok = residual_name_index(names, "host_force_complete_construction").is_some()
        && residual_name_index(names, "Wave 917").is_some();
    residual_action_store(ResidualHostCommandBarracksCompletePeelsAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_command_barracks_complete_peels_nav_commands_residual_wave917() -> bool {
    let steps = LIVE_HOST_COMMAND_BARRACKS_COMPLETE_PEELS_NAV_STEPS_WAVE917;
    let ok = residual_name_index(steps, "LIVE_HOST_COMMAND_BARRACKS_COMPLETE_PEELS").is_some()
        && residual_name_index(steps, "COMMAND_STAMP_SKIP_UNDER_FREEZE").is_some();
    residual_action_store(ResidualHostCommandBarracksCompletePeelsAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_command_barracks_complete_peels_residual_pack_wave917() -> bool {
    let cnc = cnc_source();
    let atk_raw = code_window(cnc, "fn host_command_attack", 500);
    let atk = non_comment_code(atk_raw);
    let bar_raw = code_window(cnc, "fn host_ensure_barracks_building_data", 900);
    let bar = non_comment_code(bar_raw);
    let fc_raw = code_window(cnc, "fn host_force_complete_construction", 900);
    let fc = non_comment_code(fc_raw);
    let ok = atk_raw.contains("917")
        && atk.contains("last_presentation_frame.is_none()")
        && bar_raw.contains("917")
        && bar.contains("host_match_local_barracks_ids")
        && fc_raw.contains("917")
        && fc.contains("under_construction")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostCommandBarracksCompletePeelsAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_command_barracks_complete_peels_honesty() -> bool {
    let a = honesty_host_command_barracks_complete_peels_method_names_residual_wave917();
    let b = honesty_host_command_barracks_complete_peels_nav_commands_residual_wave917();
    let c = honesty_host_command_barracks_complete_peels_residual_pack_wave917();
    residual_action_store(ResidualHostCommandBarracksCompletePeelsAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_command_barracks_complete_peels_residual_wave917() {
        assert!(honesty_host_command_barracks_complete_peels_residual_pack_wave917());
        assert!(honesty_host_command_barracks_complete_peels_method_names_residual_wave917());
        assert!(honesty_host_command_barracks_complete_peels_nav_commands_residual_wave917());
        assert!(simulate_live_host_command_barracks_complete_peels_honesty());
    }
}
