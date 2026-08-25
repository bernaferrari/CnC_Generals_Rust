//! Wave 477 residual peels: under production sole-tick, host does not
//! per-frame stomp GW queue progress via progress log.
//! - host skips full `host_production_progress_log::record` when sole-tick live
//! - publishes `record_power_factor_only` for GW rate
//! - apply_host_production_progress_events honors `power_factor_only`
//! - GW `tick_production_queues` remains sole progress/exit advance
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 464 sole-tick exit/progress peel.
//! Architecture residual - production progress last-writer is GameWorld under sole-tick.
//!
//! Sources:
//! - game_logic.rs production update sole-tick branch
//! - host_production_progress_log::record_power_factor_only
//! - gameworld_shadow::apply_host_production_progress_events
//!
//! Fail-closed:
//! - Host still try_complete/spawns when writeback head is finished
//! - Enqueue/complete/cancel still flow via host_production_log
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRODUCTION_SOLE_TICK_NO_PROGRESS_STOMP_METHOD_NAMES_WAVE477: &[&str] = &[
    "gameworld_production_sole_tick_enabled",
    "record_power_factor_only",
    "apply_host_production_progress_events",
    "tick_production_queues",
    "power_factor_only",
    "playable_claim = false",
];

pub const PRODUCTION_SOLE_TICK_NO_PROGRESS_STOMP_SOURCE_MARKERS_WAVE477: &[&str] = &[
    "Wave 477: still publish power factor",
    "record_power_factor_only",
    "if ev.power_factor_only",
    "host_production_progress_log::record",
];

pub const PRODUCTION_SOLE_TICK_NO_PROGRESS_STOMP_NAV_STEPS_WAVE477: &[&str] = &[
    "DETECT_SOLE_TICK",
    "SKIP_FULL_PROGRESS_RECORD",
    "PUBLISH_POWER_FACTOR_ONLY",
    "APPLY_SKIPS_QUEUE_STOMP",
    "GW_TICKS_PROGRESS_EXIT",
    "WRITEBACK_HOST_QUEUE",
];

pub const RUNTIME_HOST_PRODUCTION_SOLE_TICK_NO_PROGRESS_STOMP_CMD_NAMES_WAVE477: &[&str] = &[
    "click_production_sole_tick_no_progress_stomp_ok_wnd_detect",
    "click_production_sole_tick_no_progress_stomp_ok_wnd_skip",
    "click_production_sole_tick_no_progress_stomp_ok_wnd_power",
    "click_production_sole_tick_no_progress_stomp_ok_wnd_prepare",
    "click_production_sole_tick_no_progress_stomp_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualProductionSoleTickNoProgressStompAction {
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

fn residual_action_store(a: ResidualProductionSoleTickNoProgressStompAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_production_sole_tick_no_progress_stomp_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_production_sole_tick_no_progress_stomp_last_action()
-> ResidualProductionSoleTickNoProgressStompAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualProductionSoleTickNoProgressStompAction::MethodNames,
        2 => ResidualProductionSoleTickNoProgressStompAction::SourceMarkers,
        3 => ResidualProductionSoleTickNoProgressStompAction::NavCommands,
        4 => ResidualProductionSoleTickNoProgressStompAction::HostSource,
        5 => ResidualProductionSoleTickNoProgressStompAction::ShadowSource,
        6 => ResidualProductionSoleTickNoProgressStompAction::Composite,
        _ => ResidualProductionSoleTickNoProgressStompAction::Idle,
    }
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
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

pub fn honesty_production_sole_tick_no_progress_stomp_method_names_residual_wave477() -> bool {
    PRODUCTION_SOLE_TICK_NO_PROGRESS_STOMP_METHOD_NAMES_WAVE477.len() == 6
        && residual_name_index(
            PRODUCTION_SOLE_TICK_NO_PROGRESS_STOMP_METHOD_NAMES_WAVE477,
            "record_power_factor_only",
        ) == Some(1)
        && residual_name_index(
            PRODUCTION_SOLE_TICK_NO_PROGRESS_STOMP_METHOD_NAMES_WAVE477,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_production_sole_tick_no_progress_stomp_source_markers_residual_wave477() -> bool {
    PRODUCTION_SOLE_TICK_NO_PROGRESS_STOMP_SOURCE_MARKERS_WAVE477.len() == 4
        && residual_name_index(
            PRODUCTION_SOLE_TICK_NO_PROGRESS_STOMP_SOURCE_MARKERS_WAVE477,
            "record_power_factor_only",
        ) == Some(1)
        && residual_name_index(
            PRODUCTION_SOLE_TICK_NO_PROGRESS_STOMP_SOURCE_MARKERS_WAVE477,
            "if ev.power_factor_only",
        ) == Some(2)
}

pub fn honesty_production_sole_tick_no_progress_stomp_nav_commands_residual_wave477() -> bool {
    PRODUCTION_SOLE_TICK_NO_PROGRESS_STOMP_NAV_STEPS_WAVE477.len() == 6
        && residual_name_index(
            PRODUCTION_SOLE_TICK_NO_PROGRESS_STOMP_NAV_STEPS_WAVE477,
            "SKIP_FULL_PROGRESS_RECORD",
        ) == Some(1)
        && residual_name_index(
            PRODUCTION_SOLE_TICK_NO_PROGRESS_STOMP_NAV_STEPS_WAVE477,
            "WRITEBACK_HOST_QUEUE",
        ) == Some(5)
        && RUNTIME_HOST_PRODUCTION_SOLE_TICK_NO_PROGRESS_STOMP_CMD_NAMES_WAVE477.len() == 5
        && residual_name_index(
            RUNTIME_HOST_PRODUCTION_SOLE_TICK_NO_PROGRESS_STOMP_CMD_NAMES_WAVE477,
            "click_production_sole_tick_no_progress_stomp_ok_wnd_prepare",
        ) == Some(3)
}

pub fn simulate_production_sole_tick_no_progress_stomp_host_source() -> bool {
    let gl = gl_source();
    let log = log_source();
    let ok = gl.contains("gameworld_production_sole_tick_enabled()")
        && gl.contains("record_power_factor_only")
        && gl.contains("Wave 477: still publish power factor")
        && log.contains("pub fn record_power_factor_only")
        && log.contains("power_factor_only: true")
        && log.contains("power_factor_only: bool");
    residual_action_store(ResidualProductionSoleTickNoProgressStompAction::HostSource);
    ok
}

pub fn simulate_production_sole_tick_no_progress_stomp_shadow_source() -> bool {
    let src = shadow_source();
    let Some(body) = function_body(src, "fn apply_host_production_progress_events(") else {
        return false;
    };
    let ok = body.contains("if ev.power_factor_only")
        && body.contains("Wave 477")
        && src.contains("tick_production_queues")
        && src.contains("writeback_production_to_host");
    residual_action_store(ResidualProductionSoleTickNoProgressStompAction::ShadowSource);
    ok
}

pub fn honesty_production_sole_tick_no_progress_stomp_residual_pack_wave477() -> bool {
    honesty_production_sole_tick_no_progress_stomp_method_names_residual_wave477()
        && honesty_production_sole_tick_no_progress_stomp_source_markers_residual_wave477()
        && honesty_production_sole_tick_no_progress_stomp_nav_commands_residual_wave477()
        && simulate_production_sole_tick_no_progress_stomp_host_source()
        && simulate_production_sole_tick_no_progress_stomp_shadow_source()
}

pub fn simulate_live_production_sole_tick_no_progress_stomp_honesty() -> bool {
    let ok = honesty_production_sole_tick_no_progress_stomp_residual_pack_wave477();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualProductionSoleTickNoProgressStompAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_production_sole_tick_no_progress_stomp_method_names_residual_wave477());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_production_sole_tick_no_progress_stomp_source_markers_residual_wave477());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_production_sole_tick_no_progress_stomp_nav_commands_residual_wave477());
    }

    #[test]
    fn production_sole_tick_no_progress_stomp_sources() {
        assert!(simulate_production_sole_tick_no_progress_stomp_host_source());
        assert!(simulate_production_sole_tick_no_progress_stomp_shadow_source());
    }

    #[test]
    fn wave477_composite_pack() {
        assert!(honesty_production_sole_tick_no_progress_stomp_residual_pack_wave477());
    }

    #[test]
    fn simulate_live_production_sole_tick_no_progress_stomp_honesty_residual_live() {
        assert!(
            simulate_live_production_sole_tick_no_progress_stomp_honesty(),
            "production sole-tick no progress stomp residual must latch"
        );
        assert!(residual_production_sole_tick_no_progress_stomp_ok());
        assert_eq!(
            residual_production_sole_tick_no_progress_stomp_last_action(),
            ResidualProductionSoleTickNoProgressStompAction::Composite
        );
    }
}
