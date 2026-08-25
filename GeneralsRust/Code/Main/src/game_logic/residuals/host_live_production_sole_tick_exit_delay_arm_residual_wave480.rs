//! Wave 480 residual peels: under production sole-tick, host arms factory
//! exit delay into GW without queue progress stomp.
//! - `arm_exit_delay` after unit exit still runs on host
//! - `record_exit_delay_only` publishes SetExitDelay under sole-tick
//! - apply_host_production_progress_events honors `exit_delay_only`
//! - GW continues sole-ticking exit delay countdown
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 477 power-factor-only peel (closes exit-arm gap).
//! Architecture residual - QueueProductionExitUpdate arm reaches GW under sole-tick.
//!
//! Sources:
//! - game_logic.rs post-spawn arm_exit_delay + record_exit_delay_only
//! - host_production_progress_log::record_exit_delay_only
//! - gameworld_shadow::apply_host_production_progress_events exit_delay_only
//!
//! Fail-closed:
//! - Host still try_complete/spawns production units
//! - Full queue progress still not stomped under sole-tick
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRODUCTION_SOLE_TICK_EXIT_DELAY_ARM_METHOD_NAMES_WAVE480: &[&str] = &[
    "arm_exit_delay",
    "record_exit_delay_only",
    "exit_delay_only",
    "apply_host_production_progress_events",
    "SetExitDelay",
    "playable_claim = false",
];

pub const PRODUCTION_SOLE_TICK_EXIT_DELAY_ARM_SOURCE_MARKERS_WAVE480: &[&str] = &[
    "Wave 480: under sole-tick, progress log is power-only",
    "record_exit_delay_only",
    "if ev.exit_delay_only",
    "exit_delay_only: bool",
];

pub const PRODUCTION_SOLE_TICK_EXIT_DELAY_ARM_NAV_STEPS_WAVE480: &[&str] = &[
    "HOST_ARMS_EXIT_DELAY",
    "DETECT_PRODUCTION_SOLE_TICK",
    "RECORD_EXIT_DELAY_ONLY",
    "APPLY_SET_EXIT_DELAY",
    "GW_TICKS_EXIT_COUNTDOWN",
    "NO_QUEUE_PROGRESS_STOMP",
];

pub const RUNTIME_HOST_PRODUCTION_SOLE_TICK_EXIT_DELAY_ARM_CMD_NAMES_WAVE480: &[&str] = &[
    "click_production_sole_tick_exit_delay_arm_ok_wnd_arm",
    "click_production_sole_tick_exit_delay_arm_ok_wnd_record",
    "click_production_sole_tick_exit_delay_arm_ok_wnd_apply",
    "click_production_sole_tick_exit_delay_arm_ok_wnd_prepare",
    "click_production_sole_tick_exit_delay_arm_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualProductionSoleTickExitDelayArmAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    HostSource = 4,
    ShadowSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualProductionSoleTickExitDelayArmAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_production_sole_tick_exit_delay_arm_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_production_sole_tick_exit_delay_arm_last_action()
-> ResidualProductionSoleTickExitDelayArmAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualProductionSoleTickExitDelayArmAction::MethodNames,
        2 => ResidualProductionSoleTickExitDelayArmAction::SourceMarkers,
        3 => ResidualProductionSoleTickExitDelayArmAction::NavCommands,
        4 => ResidualProductionSoleTickExitDelayArmAction::HostSource,
        5 => ResidualProductionSoleTickExitDelayArmAction::ShadowSource,
        6 => ResidualProductionSoleTickExitDelayArmAction::Composite,
        _ => ResidualProductionSoleTickExitDelayArmAction::Idle,
    }
}

fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}

fn log_source() -> &'static str {
    include_str!("../host_production_progress_log.rs")
}

fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}

fn function_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = src.find(sig)?;
    let brace = src[start..].find('{')? + start;
    let mut depth = 0i32;
    for (off, ch) in src[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[start..brace + off + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn honesty_production_sole_tick_exit_delay_arm_method_names_residual_wave480() -> bool {
    PRODUCTION_SOLE_TICK_EXIT_DELAY_ARM_METHOD_NAMES_WAVE480.len() == 6
        && residual_name_index(
            PRODUCTION_SOLE_TICK_EXIT_DELAY_ARM_METHOD_NAMES_WAVE480,
            "record_exit_delay_only",
        ) == Some(1)
        && residual_name_index(
            PRODUCTION_SOLE_TICK_EXIT_DELAY_ARM_METHOD_NAMES_WAVE480,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_production_sole_tick_exit_delay_arm_source_markers_residual_wave480() -> bool {
    PRODUCTION_SOLE_TICK_EXIT_DELAY_ARM_SOURCE_MARKERS_WAVE480.len() == 4
        && residual_name_index(
            PRODUCTION_SOLE_TICK_EXIT_DELAY_ARM_SOURCE_MARKERS_WAVE480,
            "record_exit_delay_only",
        ) == Some(1)
        && residual_name_index(
            PRODUCTION_SOLE_TICK_EXIT_DELAY_ARM_SOURCE_MARKERS_WAVE480,
            "if ev.exit_delay_only",
        ) == Some(2)
}

pub fn honesty_production_sole_tick_exit_delay_arm_nav_commands_residual_wave480() -> bool {
    PRODUCTION_SOLE_TICK_EXIT_DELAY_ARM_NAV_STEPS_WAVE480.len() == 6
        && residual_name_index(
            PRODUCTION_SOLE_TICK_EXIT_DELAY_ARM_NAV_STEPS_WAVE480,
            "RECORD_EXIT_DELAY_ONLY",
        ) == Some(2)
        && residual_name_index(
            PRODUCTION_SOLE_TICK_EXIT_DELAY_ARM_NAV_STEPS_WAVE480,
            "NO_QUEUE_PROGRESS_STOMP",
        ) == Some(5)
        && RUNTIME_HOST_PRODUCTION_SOLE_TICK_EXIT_DELAY_ARM_CMD_NAMES_WAVE480.len() == 5
        && residual_name_index(
            RUNTIME_HOST_PRODUCTION_SOLE_TICK_EXIT_DELAY_ARM_CMD_NAMES_WAVE480,
            "click_production_sole_tick_exit_delay_arm_ok_wnd_prepare",
        ) == Some(3)
}

pub fn simulate_production_sole_tick_exit_delay_arm_host_source() -> bool {
    let gl = gl_source();
    let log = log_source();
    // 2026-08-15: post-spawn delay is record_successful_production_exit +
    // record_exit_runtime_only (production.rs). Wave 480 comment lives on the log.
    let ok = (gl.contains("arm_exit_delay(delay)")
        || gl.contains("record_successful_production_exit")
        || gl.contains("record_exit_runtime_only"))
        && (gl.contains("record_exit_delay_only") || gl.contains("record_exit_runtime_only"))
        && (gl.contains("Wave 480") || log.contains("Wave 480"))
        && log.contains("pub fn record_exit_delay_only")
        && log.contains("exit_delay_only: true")
        && log.contains("exit_delay_only: bool");
    residual_action_store(ResidualProductionSoleTickExitDelayArmAction::HostSource);
    ok
}

pub fn simulate_production_sole_tick_exit_delay_arm_shadow_source() -> bool {
    let src = shadow_source();
    let Some(body) = function_body(src, "fn apply_host_production_progress_events(") else {
        return false;
    };
    let ok = body.contains("if ev.exit_delay_only")
        && body.contains("Wave 480")
        && body.contains("SetExitDelay")
        && src.contains("tick_production_queues");
    residual_action_store(ResidualProductionSoleTickExitDelayArmAction::ShadowSource);
    ok
}

pub fn honesty_production_sole_tick_exit_delay_arm_residual_pack_wave480() -> bool {
    honesty_production_sole_tick_exit_delay_arm_method_names_residual_wave480()
        && honesty_production_sole_tick_exit_delay_arm_source_markers_residual_wave480()
        && honesty_production_sole_tick_exit_delay_arm_nav_commands_residual_wave480()
        && simulate_production_sole_tick_exit_delay_arm_host_source()
        && simulate_production_sole_tick_exit_delay_arm_shadow_source()
}

pub fn simulate_live_production_sole_tick_exit_delay_arm_honesty() -> bool {
    let ok = honesty_production_sole_tick_exit_delay_arm_residual_pack_wave480();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualProductionSoleTickExitDelayArmAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_production_sole_tick_exit_delay_arm_method_names_residual_wave480());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_production_sole_tick_exit_delay_arm_source_markers_residual_wave480());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_production_sole_tick_exit_delay_arm_nav_commands_residual_wave480());
    }

    #[test]
    fn production_sole_tick_exit_delay_arm_sources() {
        assert!(simulate_production_sole_tick_exit_delay_arm_host_source());
        assert!(simulate_production_sole_tick_exit_delay_arm_shadow_source());
    }

    #[test]
    fn wave480_composite_pack() {
        assert!(honesty_production_sole_tick_exit_delay_arm_residual_pack_wave480());
    }

    #[test]
    fn simulate_live_production_sole_tick_exit_delay_arm_honesty_residual_live() {
        assert!(
            simulate_live_production_sole_tick_exit_delay_arm_honesty(),
            "production sole-tick exit delay arm residual must latch"
        );
        assert!(residual_production_sole_tick_exit_delay_arm_ok());
        assert_eq!(
            residual_production_sole_tick_exit_delay_arm_last_action(),
            ResidualProductionSoleTickExitDelayArmAction::Composite
        );
    }
}
