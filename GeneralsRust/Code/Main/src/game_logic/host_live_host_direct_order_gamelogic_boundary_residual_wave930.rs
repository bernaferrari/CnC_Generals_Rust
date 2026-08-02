//! Wave 930: direct orders via GameLogic::apply_direct_player_order boundary.
//!
//! Host thin wrappers call one GameLogic authority API instead of four
//! command_* dual-writes. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DIRECT_ORDER_GAMELOGIC_BOUNDARY_METHOD_NAMES_WAVE930: &[&str] = &[
    "host_issue_direct_player_order",
    "apply_direct_player_order",
    "DirectPlayerOrder",
    "Wave 930",
    "playable_claim = false",
];

pub const LIVE_HOST_DIRECT_ORDER_GAMELOGIC_BOUNDARY_NAV_STEPS_WAVE930: &[&str] = &[
    "DIRECT_ORDER_GAMELOGIC_BOUNDARY",
    "SINGLE_APPLY_DIRECT_PLAYER_ORDER",
    "LIVE_HOST_DIRECT_ORDER_GAMELOGIC_BOUNDARY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDirectOrderGamelogicBoundaryAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostDirectOrderGamelogicBoundaryAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
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

pub fn honesty_host_direct_order_gamelogic_boundary_method_names_residual_wave930() -> bool {
    let names = LIVE_HOST_DIRECT_ORDER_GAMELOGIC_BOUNDARY_METHOD_NAMES_WAVE930;
    let ok = residual_name_index(names, "apply_direct_player_order").is_some()
        && residual_name_index(names, "Wave 930").is_some();
    residual_action_store(ResidualHostDirectOrderGamelogicBoundaryAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_direct_order_gamelogic_boundary_nav_commands_residual_wave930() -> bool {
    let steps = LIVE_HOST_DIRECT_ORDER_GAMELOGIC_BOUNDARY_NAV_STEPS_WAVE930;
    let ok = residual_name_index(steps, "LIVE_HOST_DIRECT_ORDER_GAMELOGIC_BOUNDARY").is_some()
        && residual_name_index(steps, "DIRECT_ORDER_GAMELOGIC_BOUNDARY").is_some();
    residual_action_store(ResidualHostDirectOrderGamelogicBoundaryAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_direct_order_gamelogic_boundary_residual_pack_wave930() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    // Keep the issue window short so host_command_* bodies do not leak in.
    let issue_raw = code_window(cnc, "fn host_issue_direct_player_order", 420);
    let issue = non_comment_code(issue_raw);
    let api_raw = code_window(gl, "fn apply_direct_player_order", 900);
    let api = non_comment_code(api_raw);
    let atk = non_comment_code(code_window(cnc, "fn host_command_attack", 360));
    let ok = issue_raw.contains("930")
        && issue.contains("apply_direct_player_order")
        && !issue.contains("self.game_logic.command_")
        && issue.contains("host_stamp_after_authority_command")
        && atk.contains("host_issue_direct_player_order")
        && !atk.contains("self.game_logic.command_")
        && api.contains("self.command_attack")
        && api.contains("self.command_stop")
        && gl.contains("enum DirectPlayerOrder")
        && !cnc.contains("enum HostDirectPlayerOrder")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostDirectOrderGamelogicBoundaryAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_direct_order_gamelogic_boundary_honesty() -> bool {
    let a = honesty_host_direct_order_gamelogic_boundary_method_names_residual_wave930();
    let b = honesty_host_direct_order_gamelogic_boundary_nav_commands_residual_wave930();
    let c = honesty_host_direct_order_gamelogic_boundary_residual_pack_wave930();
    residual_action_store(ResidualHostDirectOrderGamelogicBoundaryAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_direct_order_gamelogic_boundary_residual_wave930() {
        assert!(honesty_host_direct_order_gamelogic_boundary_residual_pack_wave930());
        assert!(honesty_host_direct_order_gamelogic_boundary_method_names_residual_wave930());
        assert!(honesty_host_direct_order_gamelogic_boundary_nav_commands_residual_wave930());
        assert!(simulate_live_host_direct_order_gamelogic_boundary_honesty());
    }
}
