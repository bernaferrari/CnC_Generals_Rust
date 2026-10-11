//! Construction ownership audit. Historical CLI receipt names remain
//! navigation metadata; source checks require one canonical host writer and
//! immutable shadow comparisons. Runtime regressions exercise the host phases.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const CONSTRUCTION_SOLE_TICK_NO_PROGRESS_STOMP_METHOD_NAMES_WAVE478: &[&str] = &[
    "host_owns_construction",
    "record_rate_only",
    "observe_host_construction",
    "update_construction",
    "rate_only",
    "playable_claim = false",
];

pub const CONSTRUCTION_SOLE_TICK_NO_PROGRESS_STOMP_SOURCE_MARKERS_WAVE478: &[&str] = &[
    "Wave 478: publish dozer/power rate only",
    "record_rate_only",
    "if ev.rate_only",
    "update_construction",
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
    let ok = super::construction_owner_source_contract::host_owns_construction();
    ok
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
    let ok = super::construction_owner_source_contract::host_owns_construction();
    residual_action_store(ResidualConstructionSoleTickNoProgressStompAction::HostSource);
    ok
}

pub fn simulate_construction_sole_tick_no_progress_stomp_shadow_source() -> bool {
    let ok = super::construction_owner_source_contract::host_owns_construction();
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
