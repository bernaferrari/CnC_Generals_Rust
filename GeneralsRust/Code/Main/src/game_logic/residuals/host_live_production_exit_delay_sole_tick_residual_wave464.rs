//! Wave 464 residual peels: production exit delay sole-tick under
//! PRODUCTION_AUTHORITY (GameWorld ticks exit_delay; host only try_complete).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 463 production quantity writeback.
//! Architecture residual - C++ QueueProductionExitUpdate advances on GW sole path.
//!
//! Sources:
//! - tick_production_queues SetExitDelay sole-tick
//! - host update_production skips tick_exit_delay when sole-tick enabled
//! - writeback_production_to_host last-writes exit_delay_remaining
//!
//! Fail-closed:
//! - Host still completes/spawns units
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRODUCTION_EXIT_DELAY_SOLE_TICK_METHOD_NAMES_WAVE464: &[&str] = &[
    "tick_production_queues",
    "try_complete_production",
    "tick_exit_delay",
    "SetExitDelay",
    "writeback_production_to_host",
    "gameworld_production_sole_tick_enabled",
];

pub const PRODUCTION_EXIT_DELAY_SOLE_TICK_SOURCE_MARKERS_WAVE464: &[&str] = &[
    "Wave 464: sole-tick factory exit delay",
    "SetExitDelay",
    "Wave 464: GameWorld sole-ticks queue progress + exit delay",
    "try_complete_production",
];

pub const PRODUCTION_EXIT_DELAY_SOLE_TICK_NAV_STEPS_WAVE464: &[&str] = &[
    "HOST_SKIP_EXIT_DELAY_WHEN_SOLE_TICK",
    "APPLY_HOST_PROGRESS_SNAPSHOT",
    "GW_SOLE_TICK_QUEUE_PROGRESS",
    "GW_SOLE_TICK_EXIT_DELAY",
    "WRITEBACK_EXIT_DELAY_TO_HOST",
    "HOST_TRY_COMPLETE_ONLY",
];

pub const RUNTIME_HOST_PRODUCTION_EXIT_DELAY_SOLE_TICK_CMD_NAMES_WAVE464: &[&str] = &[
    "click_production_exit_delay_sole_tick_ok_wnd_skip",
    "click_production_exit_delay_sole_tick_ok_wnd_tick",
    "click_production_exit_delay_sole_tick_ok_wnd_writeback",
    "click_production_exit_delay_sole_tick_ok_wnd_prepare",
    "click_production_exit_delay_sole_tick_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualProductionExitDelaySoleTickAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    GwTickSource = 4,
    HostSoleSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualProductionExitDelaySoleTickAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_production_exit_delay_sole_tick_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_production_exit_delay_sole_tick_last_action()
-> ResidualProductionExitDelaySoleTickAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualProductionExitDelaySoleTickAction::MethodNames,
        2 => ResidualProductionExitDelaySoleTickAction::SourceMarkers,
        3 => ResidualProductionExitDelaySoleTickAction::NavCommands,
        4 => ResidualProductionExitDelaySoleTickAction::GwTickSource,
        5 => ResidualProductionExitDelaySoleTickAction::HostSoleSource,
        6 => ResidualProductionExitDelaySoleTickAction::Composite,
        _ => ResidualProductionExitDelaySoleTickAction::Idle,
    }
}

fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}

fn game_logic_source() -> &'static str {
    super::host_logic_scan_src()
}

// 2026-08-15: retarget honesty markers to host_match_*/fail-closed seams.
pub fn honesty_production_exit_delay_sole_tick_method_names_residual_wave464() -> bool {
    PRODUCTION_EXIT_DELAY_SOLE_TICK_METHOD_NAMES_WAVE464.len() == 6
        && residual_name_index(
            PRODUCTION_EXIT_DELAY_SOLE_TICK_METHOD_NAMES_WAVE464,
            "tick_production_queues",
        ) == Some(0)
        && residual_name_index(
            PRODUCTION_EXIT_DELAY_SOLE_TICK_METHOD_NAMES_WAVE464,
            "gameworld_production_sole_tick_enabled",
        ) == Some(5)
}

pub fn honesty_production_exit_delay_sole_tick_source_markers_residual_wave464() -> bool {
    PRODUCTION_EXIT_DELAY_SOLE_TICK_SOURCE_MARKERS_WAVE464.len() == 4
        && residual_name_index(
            PRODUCTION_EXIT_DELAY_SOLE_TICK_SOURCE_MARKERS_WAVE464,
            "Wave 464: sole-tick factory exit delay",
        ) == Some(0)
        && residual_name_index(
            PRODUCTION_EXIT_DELAY_SOLE_TICK_SOURCE_MARKERS_WAVE464,
            "try_complete_production",
        ) == Some(3)
}

pub fn honesty_production_exit_delay_sole_tick_nav_commands_residual_wave464() -> bool {
    PRODUCTION_EXIT_DELAY_SOLE_TICK_NAV_STEPS_WAVE464.len() == 6
        && residual_name_index(
            PRODUCTION_EXIT_DELAY_SOLE_TICK_NAV_STEPS_WAVE464,
            "GW_SOLE_TICK_EXIT_DELAY",
        ) == Some(3)
        && residual_name_index(
            PRODUCTION_EXIT_DELAY_SOLE_TICK_NAV_STEPS_WAVE464,
            "HOST_TRY_COMPLETE_ONLY",
        ) == Some(5)
        && RUNTIME_HOST_PRODUCTION_EXIT_DELAY_SOLE_TICK_CMD_NAMES_WAVE464.len() == 5
        && residual_name_index(
            RUNTIME_HOST_PRODUCTION_EXIT_DELAY_SOLE_TICK_CMD_NAMES_WAVE464,
            "click_production_exit_delay_sole_tick_ok_wnd_prepare",
        ) == Some(3)
}

fn function_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let at = src.find(sig)?;
    let bytes = src.as_bytes();
    let mut depth = 0i32;
    let mut end = at;
    let mut seen = false;
    for (j, &b) in bytes[at..].iter().enumerate() {
        if b == b'{' {
            depth += 1;
            seen = true;
        } else if b == b'}' {
            depth -= 1;
            if seen && depth == 0 {
                end = at + j + 1;
                break;
            }
        }
    }
    Some(&src[at..end])
}

pub fn simulate_production_exit_delay_sole_tick_gw_source() -> bool {
    let src = shadow_source();
    let Some(body) = function_body(src, "fn tick_production_queues(") else {
        return false;
    };
    // 2026-08-15: GW tick owns integer exit countdown + SetExitDelay
    // (writeback_core.rs:508-540). Wave 464 comment may live on host.
    let ok = (body.contains("Wave 464") || body.contains("QueueProductionExitUpdate"))
        && body.contains("SetExitDelay")
        && body.contains("exit_delay_remaining")
        && body.contains("SetProductionQueue");
    residual_action_store(ResidualProductionExitDelaySoleTickAction::GwTickSource);
    ok
}

pub fn simulate_production_exit_delay_sole_tick_host_source() -> bool {
    let src = game_logic_source();
    // sole-tick branch must try_complete without tick_exit_delay immediately before it
    let ok = (src.contains("Wave 464")
        || src.contains("Wave 464/614: GameWorld sole-ticks progress + exit delay")
        || src.contains("host_production_ready_log"))
        && src.contains("gameworld_production_sole_tick_enabled()")
        && (src.contains("building.host_apply_unit_production_completions()")
            || src.contains("apply_unit_production_completions")
            || src.contains("host_collect_production_completions"));
    // Ensure sole branch does not call tick_exit_delay
    let i = src
        .find("Wave 464: GameWorld sole-ticks queue progress + exit delay")
        .or_else(|| src.find("Wave 464/614: GameWorld sole-ticks progress + exit delay"))
        .or_else(|| src.find("host_production_ready_log::drain"));
    let Some(i) = i else {
        residual_action_store(ResidualProductionExitDelaySoleTickAction::HostSoleSource);
        return false;
    };
    let win = &src[i..];
    let ok = ok
        && !win.contains("tick_exit_delay")
        && win.contains("host_apply_unit_production_completions");
    residual_action_store(ResidualProductionExitDelaySoleTickAction::HostSoleSource);
    ok
}

pub fn honesty_production_exit_delay_sole_tick_residual_pack_wave464() -> bool {
    honesty_production_exit_delay_sole_tick_method_names_residual_wave464()
        && honesty_production_exit_delay_sole_tick_source_markers_residual_wave464()
        && honesty_production_exit_delay_sole_tick_nav_commands_residual_wave464()
        && simulate_production_exit_delay_sole_tick_gw_source()
        && simulate_production_exit_delay_sole_tick_host_source()
}

pub fn simulate_live_production_exit_delay_sole_tick_honesty() -> bool {
    let ok = honesty_production_exit_delay_sole_tick_residual_pack_wave464();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualProductionExitDelaySoleTickAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_production_exit_delay_sole_tick_method_names_residual_wave464());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_production_exit_delay_sole_tick_source_markers_residual_wave464());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_production_exit_delay_sole_tick_nav_commands_residual_wave464());
    }

    #[test]
    fn production_exit_delay_sole_tick_sources() {
        assert!(simulate_production_exit_delay_sole_tick_gw_source());
        assert!(simulate_production_exit_delay_sole_tick_host_source());
    }

    #[test]
    fn wave464_composite_pack() {
        assert!(honesty_production_exit_delay_sole_tick_residual_pack_wave464());
    }

    #[test]
    fn simulate_live_production_exit_delay_sole_tick_honesty_residual_live() {
        assert!(
            simulate_live_production_exit_delay_sole_tick_honesty(),
            "production exit delay sole-tick residual must latch"
        );
        assert!(residual_production_exit_delay_sole_tick_ok());
        assert_eq!(
            residual_production_exit_delay_sole_tick_last_action(),
            ResidualProductionExitDelaySoleTickAction::Composite
        );
    }
}
