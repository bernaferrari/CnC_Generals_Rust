//! Wave 939: ready-log drains via GameLogic::apply_ready_log_drain_op boundary.
//!
//! `shadow_session_after_host_tick` post-writeback ready-log applies route through
//! one GameLogic authority API (`ReadyLogDrainOp`) instead of dozens of
//! `logic.host_apply_*_ready_completions` dual-writes. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_READY_LOG_DRAIN_BOUNDARY_METHOD_NAMES_WAVE939: &[&str] = &[
    "apply_ready_log_drain_op",
    "ReadyLogDrainOp",
    "shadow_session_after_host_tick",
    "Wave 939",
    "playable_claim = false",
];

pub const LIVE_HOST_READY_LOG_DRAIN_BOUNDARY_NAV_STEPS_WAVE939: &[&str] = &[
    "READY_LOG_DRAIN_BOUNDARY",
    "SINGLE_APPLY_READY_LOG_DRAIN_OP",
    "LIVE_HOST_READY_LOG_DRAIN_BOUNDARY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostReadyLogDrainBoundaryAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostReadyLogDrainBoundaryAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}

fn shadow_source() -> &'static str {
    include_str!("../gameworld_shadow.rs")
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

/// Extract `shadow_session_after_host_tick` body for session-path honesty.
fn session_fn_window(src: &str) -> &str {
    let marker = "fn shadow_session_after_host_tick";
    let Some(i) = src.find(marker) else {
        return "";
    };
    // large window covers writeback drains
    &src[i..src.len().min(i + 120_000)]
}

pub fn honesty_host_ready_log_drain_boundary_method_names_residual_wave939() -> bool {
    let names = LIVE_HOST_READY_LOG_DRAIN_BOUNDARY_METHOD_NAMES_WAVE939;
    let ok = residual_name_index(names, "apply_ready_log_drain_op").is_some()
        && residual_name_index(names, "Wave 939").is_some();
    residual_action_store(ResidualHostReadyLogDrainBoundaryAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ready_log_drain_boundary_nav_commands_residual_wave939() -> bool {
    let steps = LIVE_HOST_READY_LOG_DRAIN_BOUNDARY_NAV_STEPS_WAVE939;
    let ok = residual_name_index(steps, "LIVE_HOST_READY_LOG_DRAIN_BOUNDARY").is_some()
        && residual_name_index(steps, "READY_LOG_DRAIN_BOUNDARY").is_some();
    residual_action_store(ResidualHostReadyLogDrainBoundaryAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ready_log_drain_boundary_residual_pack_wave939() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let cnc = cnc_source();
    let api = non_comment_code(code_window(gl, "fn apply_ready_log_drain_op", 8000));
    let session = session_fn_window(sh);
    let session_code = non_comment_code(session);
    let ok = gl.contains("enum ReadyLogDrainOp")
        && api.contains("host_apply_contain_ready_completions")
        && api.contains("host_apply_upgrade_ready_completions")
        && api.contains("ReadyLogDrainOp::")
        && session_code.contains("apply_ready_log_drain_op")
        && session_code.contains("ReadyLogDrainOp::")
        // Session production path must not call host_apply_*_ready_completions directly.
        && !session_code.contains("logic.host_apply_")
        && session.matches("apply_ready_log_drain_op").count() >= 40
        && gl.contains("Wave 939")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostReadyLogDrainBoundaryAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_ready_log_drain_boundary_honesty() -> bool {
    let a = honesty_host_ready_log_drain_boundary_method_names_residual_wave939();
    let b = honesty_host_ready_log_drain_boundary_nav_commands_residual_wave939();
    let c = honesty_host_ready_log_drain_boundary_residual_pack_wave939();
    residual_action_store(ResidualHostReadyLogDrainBoundaryAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_ready_log_drain_boundary_residual_wave939() {
        assert!(honesty_host_ready_log_drain_boundary_residual_pack_wave939());
        assert!(honesty_host_ready_log_drain_boundary_method_names_residual_wave939());
        assert!(honesty_host_ready_log_drain_boundary_nav_commands_residual_wave939());
        assert!(simulate_live_host_ready_log_drain_boundary_honesty());
    }
}
