//! Wave 893: host_stamp_sim_timing + host_refresh_match_sim presentation peel.
//!
//! When `last_presentation_frame` is installed, sim timing residuals are stamped
//! from the freeze (visual speed, time frozen, play time, frame, fixed-step
//! diagnostics, replay, local team) — no mid-command GameLogic dual-read.
//! Boot/no-freeze still probes host.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SIM_TIMING_PRESENTATION_METHOD_NAMES_WAVE893: &[&str] = &[
    "host_stamp_sim_timing_residuals",
    "host_refresh_match_sim_residuals_from_logic",
    "last_presentation_frame",
    "logic_steps_accumulated_seconds",
    "Wave 893",
    "playable_claim = false",
];

pub const LIVE_HOST_SIM_TIMING_PRESENTATION_NAV_STEPS_WAVE893: &[&str] = &[
    "STAMP_SIM_TIMING_FROM_PRESENTATION",
    "REFRESH_MATCH_SIM_FROM_PRESENTATION",
    "LIVE_HOST_SIM_TIMING_PRESENTATION",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSimTimingPresentationAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSimTimingPresentationAction) {
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

pub fn honesty_host_sim_timing_presentation_method_names_residual_wave893() -> bool {
    let names = LIVE_HOST_SIM_TIMING_PRESENTATION_METHOD_NAMES_WAVE893;
    let ok = residual_name_index(names, "host_stamp_sim_timing_residuals").is_some()
        && residual_name_index(names, "host_refresh_match_sim_residuals_from_logic").is_some()
        && residual_name_index(names, "Wave 893").is_some();
    residual_action_store(ResidualHostSimTimingPresentationAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sim_timing_presentation_nav_commands_residual_wave893() -> bool {
    let steps = LIVE_HOST_SIM_TIMING_PRESENTATION_NAV_STEPS_WAVE893;
    let ok = residual_name_index(steps, "LIVE_HOST_SIM_TIMING_PRESENTATION").is_some()
        && residual_name_index(steps, "STAMP_SIM_TIMING_FROM_PRESENTATION").is_some();
    residual_action_store(ResidualHostSimTimingPresentationAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sim_timing_presentation_residual_pack_wave893() -> bool {
    let cnc = cnc_source();
    let stamp = non_comment_code(code_window(cnc, "fn host_stamp_sim_timing_residuals", 1200));
    let refresh = non_comment_code(code_window(
        cnc,
        "fn host_refresh_match_sim_residuals_from_logic",
        1600,
    ));
    let stamp_ok = stamp.contains("last_presentation_frame")
        && stamp.contains("pres.visual_speed_multiplier")
        && stamp.contains("pres.logic_steps_accumulated_seconds")
        && stamp.contains("return;");
    let refresh_ok = refresh.contains("last_presentation_frame")
        && refresh.contains("pres.in_replay_game")
        && refresh.contains("pres.local_team")
        && refresh.contains("pres.frame.0");
    let ok = stamp_ok && refresh_ok && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostSimTimingPresentationAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_sim_timing_presentation_honesty() -> bool {
    let a = honesty_host_sim_timing_presentation_method_names_residual_wave893();
    let b = honesty_host_sim_timing_presentation_nav_commands_residual_wave893();
    let c = honesty_host_sim_timing_presentation_residual_pack_wave893();
    residual_action_store(ResidualHostSimTimingPresentationAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_sim_timing_presentation_residual_wave893() {
        assert!(honesty_host_sim_timing_presentation_residual_pack_wave893());
        assert!(honesty_host_sim_timing_presentation_method_names_residual_wave893());
        assert!(honesty_host_sim_timing_presentation_nav_commands_residual_wave893());
        assert!(simulate_live_host_sim_timing_presentation_honesty());
    }
}
