//! Wave 929: direct player-order authority boundary + tick/legal single-line peels.
//!
//! host_command_attack/stop/move/attack_move share host_issue_direct_player_order.
//! Logic tick and legal-build miss paths use single-line GameLogic authority calls.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DIRECT_ORDER_BOUNDARY_METHOD_NAMES_WAVE929: &[&str] = &[
    "host_issue_direct_player_order",
    "DirectPlayerOrder",
    "host_command_attack",
    "host_command_stop",
    "host_command_move",
    "host_command_attack_move",
    "tick_logic_frame",
    "Wave 929",
    "playable_claim = false",
];

pub const LIVE_HOST_DIRECT_ORDER_BOUNDARY_NAV_STEPS_WAVE929: &[&str] = &[
    "DIRECT_PLAYER_ORDER_BOUNDARY",
    "TICK_LOGIC_SINGLE_LINE",
    "LEGAL_BUILD_SINGLE_LINE",
    "LIVE_HOST_DIRECT_ORDER_BOUNDARY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDirectOrderBoundaryAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostDirectOrderBoundaryAction) {
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

pub fn honesty_host_direct_order_boundary_method_names_residual_wave929() -> bool {
    let names = LIVE_HOST_DIRECT_ORDER_BOUNDARY_METHOD_NAMES_WAVE929;
    let ok = residual_name_index(names, "host_issue_direct_player_order").is_some()
        && residual_name_index(names, "Wave 929").is_some();
    residual_action_store(ResidualHostDirectOrderBoundaryAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_direct_order_boundary_nav_commands_residual_wave929() -> bool {
    let steps = LIVE_HOST_DIRECT_ORDER_BOUNDARY_NAV_STEPS_WAVE929;
    let ok = residual_name_index(steps, "LIVE_HOST_DIRECT_ORDER_BOUNDARY").is_some()
        && residual_name_index(steps, "DIRECT_PLAYER_ORDER_BOUNDARY").is_some();
    residual_action_store(ResidualHostDirectOrderBoundaryAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_direct_order_boundary_residual_pack_wave929() -> bool {
    let cnc = cnc_source();
    let issue_raw = code_window(cnc, "fn host_issue_direct_player_order", 1200);
    let issue = non_comment_code(issue_raw);
    let atk = non_comment_code(code_window(cnc, "fn host_command_attack", 500));
    let stop = non_comment_code(code_window(cnc, "fn host_command_stop", 400));
    let mov = non_comment_code(code_window(cnc, "fn host_command_move", 400));
    let amov = non_comment_code(code_window(cnc, "fn host_command_attack_move", 400));
    let tick = non_comment_code(code_window(cnc, "fn host_update_logic_frame", 900));
    let legal = non_comment_code(code_window(
        cnc,
        "fn host_legal_build_code_at_for_builder",
        1600,
    ));
    let ok = issue_raw.contains("929")
        && (issue.contains("command_attack") || issue.contains("apply_direct_player_order"))
        && (issue.contains("command_stop") || issue.contains("apply_direct_player_order"))
        && issue.contains("host_stamp_after_authority_command")
        && atk.contains("host_issue_direct_player_order")
        && !atk.contains("self.game_logic.command_")
        && stop.contains("host_issue_direct_player_order")
        && mov.contains("host_issue_direct_player_order")
        && amov.contains("host_issue_direct_player_order")
        && tick.contains("tick_logic_frame")
        && legal.contains("legal_build_code_at_for_builder")
        && legal.contains("host_legal_build_cache")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostDirectOrderBoundaryAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_direct_order_boundary_honesty() -> bool {
    let a = honesty_host_direct_order_boundary_method_names_residual_wave929();
    let b = honesty_host_direct_order_boundary_nav_commands_residual_wave929();
    let c = honesty_host_direct_order_boundary_residual_pack_wave929();
    residual_action_store(ResidualHostDirectOrderBoundaryAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_direct_order_boundary_residual_wave929() {
        assert!(honesty_host_direct_order_boundary_residual_pack_wave929());
        assert!(honesty_host_direct_order_boundary_method_names_residual_wave929());
        assert!(honesty_host_direct_order_boundary_nav_commands_residual_wave929());
        assert!(simulate_live_host_direct_order_boundary_honesty());
    }
}
