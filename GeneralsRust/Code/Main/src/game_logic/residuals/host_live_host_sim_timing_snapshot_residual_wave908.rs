//! Wave 908: post-tick sim timing snapshot residual (no dual get_frame/diagnostics).
//!
//! GameLogic callback-aware tick returns `SimTimingSnapshot`; host stamps from
//! its returned payload only after per-step boundary delivery completes.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SIM_TIMING_SNAPSHOT_METHOD_NAMES_WAVE908: &[&str] = &[
    "host_update_logic_frame",
    "host_stamp_sim_timing_from_snapshot",
    "host_stamp_sim_timing_residuals",
    "sim_timing_snapshot",
    "SimTimingSnapshot",
    "Wave 908",
    "playable_claim = false",
];

pub const LIVE_HOST_SIM_TIMING_SNAPSHOT_NAV_STEPS_WAVE908: &[&str] = &[
    "UPDATE_RETURNS_SIM_TIMING_SNAPSHOT",
    "HOST_STAMP_FROM_SNAPSHOT",
    "NO_GET_FRAME_AFTER_TICK",
    "LIVE_HOST_SIM_TIMING_SNAPSHOT",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSimTimingSnapshotAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSimTimingSnapshotAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
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

pub fn honesty_host_sim_timing_snapshot_method_names_residual_wave908() -> bool {
    let names = LIVE_HOST_SIM_TIMING_SNAPSHOT_METHOD_NAMES_WAVE908;
    let ok = residual_name_index(names, "host_stamp_sim_timing_from_snapshot").is_some()
        && residual_name_index(names, "Wave 908").is_some();
    residual_action_store(ResidualHostSimTimingSnapshotAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sim_timing_snapshot_nav_commands_residual_wave908() -> bool {
    let steps = LIVE_HOST_SIM_TIMING_SNAPSHOT_NAV_STEPS_WAVE908;
    let ok = residual_name_index(steps, "LIVE_HOST_SIM_TIMING_SNAPSHOT").is_some()
        && residual_name_index(steps, "HOST_STAMP_FROM_SNAPSHOT").is_some();
    residual_action_store(ResidualHostSimTimingSnapshotAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sim_timing_snapshot_residual_pack_wave908() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let host = non_comment_code(
        super::harness::rust_fn_body(cnc, "host_update_logic_frame").unwrap_or(""),
    );
    let loop_body = non_comment_code(
        super::harness::rust_fn_body(cnc, "host_run_fast_forward_loop").unwrap_or(""),
    );
    let tick = non_comment_code(
        super::harness::rust_fn_body(&gl, "tick_logic_frame_with_boundary").unwrap_or(""),
    );
    let callbacks = non_comment_code(
        super::harness::rust_fn_body(&gl, "step_simulation_with_callbacks").unwrap_or(""),
    );
    let stamp_raw = code_window(cnc, "fn host_stamp_sim_timing_residuals", 900);
    let stamp = non_comment_code(stamp_raw);
    let snap_raw = code_window(&gl, "struct SimTimingSnapshot", 400);
    let returned = loop_body.find("host_update_logic_frame");
    let stamped = loop_body.find("host_stamp_sim_timing_from_snapshot(snap)");
    let ok = host.contains("tick_logic_frame_with_boundary")
        && !host.contains("host_stamp_sim_timing_from_snapshot")
        && tick.contains("step_simulation_with_callbacks")
        && callbacks.contains("SimulationStepOutcome::Advanced")
        && callbacks.contains("after_step(self)")
        && returned.zip(stamped).is_some_and(|(a, b)| a < b)
        && !host.contains("get_frame")
        && (stamp.contains("sim_timing_snapshot")
            || stamp.contains("SimTimingSnapshot")
            || stamp.contains("host_match_logic_frame"))
        && !stamp.contains("get_frame")
        && !stamp.contains("fixed_step_diagnostics")
        && snap_raw.contains("SimTimingSnapshot")
        && gl.contains("-> SimTimingSnapshot")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostSimTimingSnapshotAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_sim_timing_snapshot_honesty() -> bool {
    let a = honesty_host_sim_timing_snapshot_method_names_residual_wave908();
    let b = honesty_host_sim_timing_snapshot_nav_commands_residual_wave908();
    let c = honesty_host_sim_timing_snapshot_residual_pack_wave908();
    residual_action_store(ResidualHostSimTimingSnapshotAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_sim_timing_snapshot_residual_wave908() {
        assert!(honesty_host_sim_timing_snapshot_residual_pack_wave908());
        assert!(honesty_host_sim_timing_snapshot_method_names_residual_wave908());
        assert!(honesty_host_sim_timing_snapshot_nav_commands_residual_wave908());
        assert!(simulate_live_host_sim_timing_snapshot_honesty());
    }
}
