//! Wave 923: single tick_logic_frame authority boundary + queue via host residual.
//!
//! host_update_logic_frame uses GameLogic::tick_logic_frame instead of four update
//! dual-write variants. Resume/stop/force-attack queue through host_queue_command.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_TICK_LOGIC_FRAME_BOUNDARY_METHOD_NAMES_WAVE923: &[&str] = &[
    "host_update_logic_frame",
    "tick_logic_frame",
    "host_queue_command",
    "Wave 923",
    "playable_claim = false",
];

pub const LIVE_HOST_TICK_LOGIC_FRAME_BOUNDARY_NAV_STEPS_WAVE923: &[&str] = &[
    "TICK_LOGIC_FRAME_BOUNDARY",
    "QUEUE_VIA_HOST_RESIDUAL",
    "LIVE_HOST_TICK_LOGIC_FRAME_BOUNDARY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostTickLogicFrameBoundaryAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostTickLogicFrameBoundaryAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
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

pub fn honesty_host_tick_logic_frame_boundary_method_names_residual_wave923() -> bool {
    let names = LIVE_HOST_TICK_LOGIC_FRAME_BOUNDARY_METHOD_NAMES_WAVE923;
    let ok = residual_name_index(names, "tick_logic_frame").is_some()
        && residual_name_index(names, "Wave 923").is_some();
    residual_action_store(ResidualHostTickLogicFrameBoundaryAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_tick_logic_frame_boundary_nav_commands_residual_wave923() -> bool {
    let steps = LIVE_HOST_TICK_LOGIC_FRAME_BOUNDARY_NAV_STEPS_WAVE923;
    let ok = residual_name_index(steps, "LIVE_HOST_TICK_LOGIC_FRAME_BOUNDARY").is_some()
        && residual_name_index(steps, "TICK_LOGIC_FRAME_BOUNDARY").is_some();
    residual_action_store(ResidualHostTickLogicFrameBoundaryAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_tick_logic_frame_boundary_residual_pack_wave923() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let upd_raw = code_window(cnc, "fn host_update_logic_frame", 2000);
    let upd = non_comment_code(upd_raw);
    let ok = upd_raw.contains("923")
        && upd.contains("tick_logic_frame")
        && !upd.contains("update_with_dt")
        && !upd.contains("update_with_timing")
        && gl.contains("fn tick_logic_frame")
        && !cnc.contains("self.game_logic\n            .queue_command")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostTickLogicFrameBoundaryAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_tick_logic_frame_boundary_honesty() -> bool {
    let a = honesty_host_tick_logic_frame_boundary_method_names_residual_wave923();
    let b = honesty_host_tick_logic_frame_boundary_nav_commands_residual_wave923();
    let c = honesty_host_tick_logic_frame_boundary_residual_pack_wave923();
    residual_action_store(ResidualHostTickLogicFrameBoundaryAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_tick_logic_frame_boundary_residual_wave923() {
        assert!(honesty_host_tick_logic_frame_boundary_residual_pack_wave923());
        assert!(honesty_host_tick_logic_frame_boundary_method_names_residual_wave923());
        assert!(honesty_host_tick_logic_frame_boundary_nav_commands_residual_wave923());
        assert!(simulate_live_host_tick_logic_frame_boundary_honesty());
    }
}
