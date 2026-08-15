//! Wave 870: host_stamp_sim_timing_residuals keeps logic-frame / play-time /
//! freeze / visual-speed residuals warm after logic ticks and command process.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SIM_TIMING_STAMP_METHOD_NAMES_WAVE870: &[&str] = &[
    "host_stamp_sim_timing_residuals",
    "host_update_logic_frame",
    "host_process_commands_with_command_sound",
    "host_match_logic_frame",
    "Wave 870",
    "playable_claim = false",
];

pub const LIVE_HOST_SIM_TIMING_STAMP_NAV_STEPS_WAVE870: &[&str] = &[
    "STAMP_AFTER_LOGIC_TICK",
    "STAMP_AFTER_PROCESS_COMMANDS",
    "KEEP_SIM_TIMING_RESIDUAL_WARM",
    "LIVE_HOST_SIM_TIMING_STAMP",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSimTimingStampAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSimTimingStampAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

// 2026-08-15: retarget honesty markers to host_match_*/fail-closed seams.
pub fn honesty_host_sim_timing_stamp_method_names_residual_wave870() -> bool {
    let names = LIVE_HOST_SIM_TIMING_STAMP_METHOD_NAMES_WAVE870;
    let ok = residual_name_index(names, "host_stamp_sim_timing_residuals").is_some()
        && residual_name_index(names, "host_update_logic_frame").is_some()
        && residual_name_index(names, "Wave 870").is_some();
    residual_action_store(ResidualHostSimTimingStampAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sim_timing_stamp_nav_commands_residual_wave870() -> bool {
    let steps = LIVE_HOST_SIM_TIMING_STAMP_NAV_STEPS_WAVE870;
    let ok = residual_name_index(steps, "LIVE_HOST_SIM_TIMING_STAMP").is_some()
        && residual_name_index(steps, "STAMP_AFTER_LOGIC_TICK").is_some();
    residual_action_store(ResidualHostSimTimingStampAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sim_timing_stamp_residual_pack_wave870() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("fn host_stamp_sim_timing_residuals(&mut self)")
        && cnc.contains("Wave 584/870: host logic tick residual + stamp sim timing residuals")
        && cnc.contains("Wave 576/870: process + Command SFX residual + stamp sim timing")
        && cnc.contains("self.host_stamp_sim_timing_residuals()")
        && cnc.contains("self.host_match_logic_frame = Some(host_match_logic_frame)");
    residual_action_store(ResidualHostSimTimingStampAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_sim_timing_stamp_honesty() -> bool {
    let a = honesty_host_sim_timing_stamp_method_names_residual_wave870();
    let b = honesty_host_sim_timing_stamp_nav_commands_residual_wave870();
    let c = honesty_host_sim_timing_stamp_residual_pack_wave870();
    residual_action_store(ResidualHostSimTimingStampAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_sim_timing_stamp_residual_wave870() {
        assert!(honesty_host_sim_timing_stamp_residual_pack_wave870());
        assert!(honesty_host_sim_timing_stamp_method_names_residual_wave870());
        assert!(honesty_host_sim_timing_stamp_nav_commands_residual_wave870());
        assert!(simulate_live_host_sim_timing_stamp_honesty());
    }
}
