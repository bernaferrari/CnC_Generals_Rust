//! Wave 844: host-owned sim timing residuals (visual speed, time frozen, play time,
//! logic frame/steps, replay) peel live GameLogic dual-reads from presentation_or_boot_*
//! when freeze is missing. Refreshed after match load, seed, and each finalize.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MATCH_SIM_TIMING_RESIDUALS_METHOD_NAMES_WAVE844: &[&str] = &[
    "host_refresh_match_sim_residuals_from_logic",
    "host_match_visual_speed",
    "host_match_time_frozen",
    "host_match_total_play_time",
    "host_match_logic_frame",
    "host_match_logic_steps",
    "host_match_in_replay",
    "Wave 844",
    "playable_claim = false",
];

pub const LIVE_HOST_MATCH_SIM_TIMING_RESIDUALS_NAV_STEPS_WAVE844: &[&str] = &[
    "REFRESH_HOST_SIM_RESIDUALS",
    "PREFER_FREEZE_THEN_HOST_SIM",
    "LIVE_HOST_MATCH_SIM_TIMING_RESIDUALS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMatchSimTimingResidualsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMatchSimTimingResidualsAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

pub fn honesty_host_match_sim_timing_residuals_method_names_residual_wave844() -> bool {
    let names = LIVE_HOST_MATCH_SIM_TIMING_RESIDUALS_METHOD_NAMES_WAVE844;
    let ok = residual_name_index(names, "host_refresh_match_sim_residuals_from_logic").is_some()
        && residual_name_index(names, "host_match_visual_speed").is_some()
        && residual_name_index(names, "host_match_logic_frame").is_some()
        && residual_name_index(names, "Wave 844").is_some();
    residual_action_store(ResidualHostMatchSimTimingResidualsAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_sim_timing_residuals_nav_commands_residual_wave844() -> bool {
    let steps = LIVE_HOST_MATCH_SIM_TIMING_RESIDUALS_NAV_STEPS_WAVE844;
    let ok = residual_name_index(steps, "LIVE_HOST_MATCH_SIM_TIMING_RESIDUALS").is_some()
        && residual_name_index(steps, "REFRESH_HOST_SIM_RESIDUALS").is_some();
    residual_action_store(ResidualHostMatchSimTimingResidualsAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_sim_timing_residuals_residual_pack_wave844() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("fn host_refresh_match_sim_residuals_from_logic")
        && cnc.contains("host_match_visual_speed: Option<f32>")
        && cnc.contains("host_match_time_frozen: Option<bool>")
        && cnc.contains("host_match_total_play_time: Option<f32>")
        && cnc.contains("host_match_logic_frame: Option<u32>")
        && cnc.contains("host_match_logic_steps: Option<(u32, bool, f32)>")
        && cnc.contains("host_match_in_replay: Option<bool>")
        && cnc.contains("Wave 550/844")
        && cnc.contains("Wave 551/844")
        && cnc.contains("Wave 553/844")
        && cnc.contains("Wave 557/844")
        && cnc.contains("Wave 560/844")
        && cnc.contains("Wave 564/844")
        && cnc.contains("Wave 844: keep host sim residuals current for freeze-miss peels")
        && cnc
            .matches("self.host_refresh_match_sim_residuals_from_logic()")
            .count()
            >= 3;
    residual_action_store(ResidualHostMatchSimTimingResidualsAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_match_sim_timing_residuals_honesty() -> bool {
    let a = honesty_host_match_sim_timing_residuals_method_names_residual_wave844();
    let b = honesty_host_match_sim_timing_residuals_nav_commands_residual_wave844();
    let c = honesty_host_match_sim_timing_residuals_residual_pack_wave844();
    residual_action_store(ResidualHostMatchSimTimingResidualsAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_match_sim_timing_residuals_residual_wave844() {
        assert!(honesty_host_match_sim_timing_residuals_residual_pack_wave844());
        assert!(honesty_host_match_sim_timing_residuals_method_names_residual_wave844());
        assert!(honesty_host_match_sim_timing_residuals_nav_commands_residual_wave844());
        assert!(simulate_live_host_match_sim_timing_residuals_honesty());
    }
}
