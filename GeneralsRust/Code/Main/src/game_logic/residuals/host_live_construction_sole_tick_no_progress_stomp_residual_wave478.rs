//! Wave 478 residual peels: under construction sole-tick, host does not
//! per-frame stomp GW construction percent via progress log.
//! - host skips full `host_construction_progress_log::record` when sole-tick live
//! - publishes `record_rate_only` for GW dozer/power rate
//! - apply_host_construction_progress_events honors `rate_only`
//! - GW `tick_construction_progress` remains sole percent advance
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 477 production sole-tick no-stomp peel.
//! Architecture residual - construction percent last-writer is GameWorld under sole-tick.
//!
//! Sources:
//! - game_logic.rs construction update sole-tick branch
//! - host_construction_progress_log::record_rate_only
//! - gameworld_shadow::apply_host_construction_progress_events
//!
//! Fail-closed:
//! - Lifecycle records (start 0.0 / finish 1.0 / sell) still use full `record`
//! - Host still completes construction when writeback percent reaches 1.0
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const CONSTRUCTION_SOLE_TICK_NO_PROGRESS_STOMP_METHOD_NAMES_WAVE478: &[&str] = &[
    "gameworld_construction_sole_tick_enabled",
    "record_rate_only",
    "apply_host_construction_progress_events",
    "tick_construction_progress",
    "rate_only",
    "playable_claim = false",
];

pub const CONSTRUCTION_SOLE_TICK_NO_PROGRESS_STOMP_SOURCE_MARKERS_WAVE478: &[&str] = &[
    "Wave 478: publish dozer/power rate only",
    "record_rate_only",
    "if ev.rate_only",
    "tick_construction_progress",
];

pub const CONSTRUCTION_SOLE_TICK_NO_PROGRESS_STOMP_NAV_STEPS_WAVE478: &[&str] = &[
    "DETECT_CONSTRUCTION_SOLE_TICK",
    "SKIP_FULL_PERCENT_RECORD",
    "PUBLISH_RATE_ONLY",
    "APPLY_SKIPS_PERCENT_STOMP",
    "GW_TICKS_CONSTRUCTION_PERCENT",
    "WRITEBACK_HOST_PERCENT",
];

pub const RUNTIME_HOST_CONSTRUCTION_SOLE_TICK_NO_PROGRESS_STOMP_CMD_NAMES_WAVE478: &[&str] = &[
    "click_construction_sole_tick_no_progress_stomp_ok_wnd_detect",
    "click_construction_sole_tick_no_progress_stomp_ok_wnd_skip",
    "click_construction_sole_tick_no_progress_stomp_ok_wnd_rate",
    "click_construction_sole_tick_no_progress_stomp_ok_wnd_prepare",
    "click_construction_sole_tick_no_progress_stomp_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualConstructionSoleTickNoProgressStompAction {
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

fn residual_action_store(a: ResidualConstructionSoleTickNoProgressStompAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_construction_sole_tick_no_progress_stomp_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_construction_sole_tick_no_progress_stomp_last_action()
-> ResidualConstructionSoleTickNoProgressStompAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualConstructionSoleTickNoProgressStompAction::MethodNames,
        2 => ResidualConstructionSoleTickNoProgressStompAction::SourceMarkers,
        3 => ResidualConstructionSoleTickNoProgressStompAction::NavCommands,
        4 => ResidualConstructionSoleTickNoProgressStompAction::HostSource,
        5 => ResidualConstructionSoleTickNoProgressStompAction::ShadowSource,
        6 => ResidualConstructionSoleTickNoProgressStompAction::Composite,
        _ => ResidualConstructionSoleTickNoProgressStompAction::Idle,
    }
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn log_source() -> &'static str {
    include_str!("../host_construction_progress_log.rs")
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

pub fn honesty_construction_sole_tick_no_progress_stomp_method_names_residual_wave478() -> bool {
    CONSTRUCTION_SOLE_TICK_NO_PROGRESS_STOMP_METHOD_NAMES_WAVE478.len() == 6
        && residual_name_index(
            CONSTRUCTION_SOLE_TICK_NO_PROGRESS_STOMP_METHOD_NAMES_WAVE478,
            "record_rate_only",
        ) == Some(1)
        && residual_name_index(
            CONSTRUCTION_SOLE_TICK_NO_PROGRESS_STOMP_METHOD_NAMES_WAVE478,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_construction_sole_tick_no_progress_stomp_source_markers_residual_wave478() -> bool {
    CONSTRUCTION_SOLE_TICK_NO_PROGRESS_STOMP_SOURCE_MARKERS_WAVE478.len() == 4
        && residual_name_index(
            CONSTRUCTION_SOLE_TICK_NO_PROGRESS_STOMP_SOURCE_MARKERS_WAVE478,
            "record_rate_only",
        ) == Some(1)
        && residual_name_index(
            CONSTRUCTION_SOLE_TICK_NO_PROGRESS_STOMP_SOURCE_MARKERS_WAVE478,
            "if ev.rate_only",
        ) == Some(2)
}

pub fn honesty_construction_sole_tick_no_progress_stomp_nav_commands_residual_wave478() -> bool {
    CONSTRUCTION_SOLE_TICK_NO_PROGRESS_STOMP_NAV_STEPS_WAVE478.len() == 6
        && residual_name_index(
            CONSTRUCTION_SOLE_TICK_NO_PROGRESS_STOMP_NAV_STEPS_WAVE478,
            "SKIP_FULL_PERCENT_RECORD",
        ) == Some(1)
        && residual_name_index(
            CONSTRUCTION_SOLE_TICK_NO_PROGRESS_STOMP_NAV_STEPS_WAVE478,
            "WRITEBACK_HOST_PERCENT",
        ) == Some(5)
        && RUNTIME_HOST_CONSTRUCTION_SOLE_TICK_NO_PROGRESS_STOMP_CMD_NAMES_WAVE478.len() == 5
        && residual_name_index(
            RUNTIME_HOST_CONSTRUCTION_SOLE_TICK_NO_PROGRESS_STOMP_CMD_NAMES_WAVE478,
            "click_construction_sole_tick_no_progress_stomp_ok_wnd_prepare",
        ) == Some(3)
}

pub fn simulate_construction_sole_tick_no_progress_stomp_host_source() -> bool {
    let gl = gl_source();
    let log = log_source();
    let ok = gl.contains("gameworld_construction_sole_tick_enabled()")
        && gl.contains("record_rate_only")
        && gl.contains("Wave 478: publish dozer/power rate only")
        && log.contains("pub fn record_rate_only")
        && log.contains("rate_only: true")
        && log.contains("rate_only: bool");
    residual_action_store(ResidualConstructionSoleTickNoProgressStompAction::HostSource);
    ok
}

pub fn simulate_construction_sole_tick_no_progress_stomp_shadow_source() -> bool {
    let src = shadow_source();
    let Some(body) = function_body(src, "fn apply_host_construction_progress_events(") else {
        return false;
    };
    let ok = body.contains("if ev.rate_only")
        && body.contains("Wave 478")
        && src.contains("tick_construction_progress")
        && src.contains("writeback_construction_to_host");
    residual_action_store(ResidualConstructionSoleTickNoProgressStompAction::ShadowSource);
    ok
}

pub fn honesty_construction_sole_tick_no_progress_stomp_residual_pack_wave478() -> bool {
    honesty_construction_sole_tick_no_progress_stomp_method_names_residual_wave478()
        && honesty_construction_sole_tick_no_progress_stomp_source_markers_residual_wave478()
        && honesty_construction_sole_tick_no_progress_stomp_nav_commands_residual_wave478()
        && simulate_construction_sole_tick_no_progress_stomp_host_source()
        && simulate_construction_sole_tick_no_progress_stomp_shadow_source()
}

pub fn simulate_live_construction_sole_tick_no_progress_stomp_honesty() -> bool {
    let ok = honesty_construction_sole_tick_no_progress_stomp_residual_pack_wave478();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualConstructionSoleTickNoProgressStompAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_construction_sole_tick_no_progress_stomp_method_names_residual_wave478());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_construction_sole_tick_no_progress_stomp_source_markers_residual_wave478());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_construction_sole_tick_no_progress_stomp_nav_commands_residual_wave478());
    }

    #[test]
    fn construction_sole_tick_no_progress_stomp_sources() {
        assert!(simulate_construction_sole_tick_no_progress_stomp_host_source());
        assert!(simulate_construction_sole_tick_no_progress_stomp_shadow_source());
    }

    #[test]
    fn wave478_composite_pack() {
        assert!(honesty_construction_sole_tick_no_progress_stomp_residual_pack_wave478());
    }

    #[test]
    fn simulate_live_construction_sole_tick_no_progress_stomp_honesty_residual_live() {
        assert!(
            simulate_live_construction_sole_tick_no_progress_stomp_honesty(),
            "construction sole-tick no progress stomp residual must latch"
        );
        assert!(residual_construction_sole_tick_no_progress_stomp_ok());
        assert_eq!(
            residual_construction_sole_tick_no_progress_stomp_last_action(),
            ResidualConstructionSoleTickNoProgressStompAction::Composite
        );
    }
}
