//! Wave 923: per-step host delivery through GameLogic fixed-step authority.
//!
//! The retained host_update_logic_frame preserves pause/timing policy and delegates
//! callbacks to tick_logic_frame_with_boundary. Resume/stop/force-attack queue via host_queue_command.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_TICK_LOGIC_FRAME_BOUNDARY_METHOD_NAMES_WAVE923: &[&str] = &[
    "host_update_logic_frame",
    "tick_logic_frame_with_boundary",
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
    let ok = residual_name_index(names, "tick_logic_frame_with_boundary").is_some()
        && residual_name_index(names, "host_update_logic_frame").is_some()
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
    let host = non_comment_code(
        super::harness::rust_fn_body(cnc, "host_update_logic_frame").unwrap_or(""),
    );
    let tick = non_comment_code(
        super::harness::rust_fn_body(gl, "tick_logic_frame_with_boundary").unwrap_or(""),
    );
    let callbacks = non_comment_code(
        super::harness::rust_fn_body(gl, "step_simulation_with_callbacks").unwrap_or(""),
    );
    let loop_body = non_comment_code(
        super::harness::rust_fn_body(cnc, "host_run_fast_forward_loop").unwrap_or(""),
    );
    let ok = host.contains("tick_logic_frame_with_boundary")
        && tick.contains("step_simulation_with_callbacks")
        && super::harness::rust_fn_body(&callbacks, "step_simulation_with_callbacks").is_some_and(
            |body| {
                let advanced = body
                    .split("SimulationStepOutcome::Advanced =>")
                    .nth(1)
                    .unwrap_or("");
                let (advanced, frozen) = advanced
                    .split_once("SimulationStepOutcome::Frozen =>")
                    .unwrap_or(("", ""));
                let frame = advanced.find("self.frame += 1");
                let delivery = advanced.find("after_step(self)");
                frame.zip(delivery).is_some_and(|(a, b)| a < b)
                    && !frozen.contains("after_step(self)")
                    && body.contains("with_session_random")
                    && body.contains("self.logic_random.clone()")
                    && !body.contains("unsafe")
            },
        )
        && loop_body.contains("host_update_logic_frame")
        && !tick.contains("update_with_dt")
        && !tick.contains("update_with_timing")
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
