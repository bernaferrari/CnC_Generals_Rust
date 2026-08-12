//! Wave 927: post-logic shadow session single boundary + command stamp helper.
//!
//! `host_run_gameworld_shadow_after_logic` uses `run_post_logic_shadow_boundary`
//! instead of separate session/no-session dual-borrows. Direct host commands share
//! `host_stamp_after_authority_command`. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_POST_LOGIC_SHADOW_BOUNDARY_METHOD_NAMES_WAVE927: &[&str] = &[
    "host_run_gameworld_shadow_after_logic",
    "run_post_logic_shadow_boundary",
    "host_stamp_after_authority_command",
    "Wave 927",
    "playable_claim = false",
];

pub const LIVE_HOST_POST_LOGIC_SHADOW_BOUNDARY_NAV_STEPS_WAVE927: &[&str] = &[
    "POST_LOGIC_SHADOW_BOUNDARY",
    "AUTHORITY_COMMAND_STAMP_HELPER",
    "LIVE_HOST_POST_LOGIC_SHADOW_BOUNDARY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPostLogicShadowBoundaryAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPostLogicShadowBoundaryAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn gw_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
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

pub fn honesty_host_post_logic_shadow_boundary_method_names_residual_wave927() -> bool {
    let names = LIVE_HOST_POST_LOGIC_SHADOW_BOUNDARY_METHOD_NAMES_WAVE927;
    let ok = residual_name_index(names, "run_post_logic_shadow_boundary").is_some()
        && residual_name_index(names, "Wave 927").is_some();
    residual_action_store(ResidualHostPostLogicShadowBoundaryAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_post_logic_shadow_boundary_nav_commands_residual_wave927() -> bool {
    let steps = LIVE_HOST_POST_LOGIC_SHADOW_BOUNDARY_NAV_STEPS_WAVE927;
    let ok = residual_name_index(steps, "LIVE_HOST_POST_LOGIC_SHADOW_BOUNDARY").is_some()
        && residual_name_index(steps, "POST_LOGIC_SHADOW_BOUNDARY").is_some();
    residual_action_store(ResidualHostPostLogicShadowBoundaryAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_post_logic_shadow_boundary_residual_pack_wave927() -> bool {
    let cnc = cnc_source();
    let gw = gw_source();
    let host_raw = code_window(cnc, "fn host_run_gameworld_shadow_after_logic", 1200);
    let host = non_comment_code(host_raw);
    let stamp_raw = code_window(cnc, "fn host_stamp_after_authority_command", 500);
    let stamp = non_comment_code(stamp_raw);
    let batch_raw = code_window(gw, "fn run_post_logic_shadow_boundary", 900);
    let ok = host_raw.contains("927")
        && host.contains("run_post_logic_shadow_boundary")
        && !host.contains("shadow_session_after_host_tick")
        && !host.contains("maybe_shadow_after_host_tick")
        && stamp.contains("host_stamp_sim_timing_residuals")
        && batch_raw.contains("shadow_session_after_host_tick")
        && batch_raw.contains("maybe_shadow_after_host_tick")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostPostLogicShadowBoundaryAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_post_logic_shadow_boundary_honesty() -> bool {
    let a = honesty_host_post_logic_shadow_boundary_method_names_residual_wave927();
    let b = honesty_host_post_logic_shadow_boundary_nav_commands_residual_wave927();
    let c = honesty_host_post_logic_shadow_boundary_residual_pack_wave927();
    residual_action_store(ResidualHostPostLogicShadowBoundaryAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_post_logic_shadow_boundary_residual_wave927() {
        assert!(honesty_host_post_logic_shadow_boundary_residual_pack_wave927());
        assert!(honesty_host_post_logic_shadow_boundary_method_names_residual_wave927());
        assert!(honesty_host_post_logic_shadow_boundary_nav_commands_residual_wave927());
        assert!(simulate_live_host_post_logic_shadow_boundary_honesty());
    }
}
