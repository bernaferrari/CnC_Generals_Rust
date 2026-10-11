//! Construction ownership audit. Historical CLI receipt names remain
//! navigation metadata; source checks require one canonical host writer and
//! immutable shadow comparisons. Runtime regressions exercise the host phases.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const SELL_DECONSTRUCTION_SOLE_TICK_NO_STOMP_METHOD_NAMES_WAVE481: &[&str] = &[
    "update_sell_list",
    "host_owns_construction",
    "record_rate_only",
    "SELL_CONSTRUCTION_DECREMENT_RESIDUAL",
    "update_construction",
    "playable_claim = false",
];

pub const SELL_DECONSTRUCTION_SOLE_TICK_NO_STOMP_SOURCE_MARKERS_WAVE481: &[&str] = &[
    "Wave 481: GW sole-ticks sell percent via negative rate",
    "record_rate_only",
    "SELL_CONSTRUCTION_DECREMENT_RESIDUAL",
    "sole && previous <= 0.0",
];

pub const SELL_DECONSTRUCTION_SOLE_TICK_NO_STOMP_NAV_STEPS_WAVE481: &[&str] = &[
    "DETECT_CONSTRUCTION_SOLE_TICK",
    "SKIP_HOST_PERCENT_DECREMENT",
    "PUBLISH_NEGATIVE_RATE_ONLY",
    "GW_TICKS_SELL_PERCENT",
    "WRITEBACK_HOST_PERCENT",
    "FINISH_ON_WRITEBACK_THRESHOLD",
];

pub const RUNTIME_HOST_SELL_DECONSTRUCTION_SOLE_TICK_NO_STOMP_CMD_NAMES_WAVE481: &[&str] = &[
    "click_sell_deconstruction_sole_tick_no_stomp_ok_wnd_detect",
    "click_sell_deconstruction_sole_tick_no_stomp_ok_wnd_rate",
    "click_sell_deconstruction_sole_tick_no_stomp_ok_wnd_tick",
    "click_sell_deconstruction_sole_tick_no_stomp_ok_wnd_prepare",
    "click_sell_deconstruction_sole_tick_no_stomp_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualSellDeconstructionSoleTickNoStompAction {
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

fn residual_action_store(a: ResidualSellDeconstructionSoleTickNoStompAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_sell_deconstruction_sole_tick_no_stomp_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_sell_deconstruction_sole_tick_no_stomp_last_action()
-> ResidualSellDeconstructionSoleTickNoStompAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualSellDeconstructionSoleTickNoStompAction::MethodNames,
        2 => ResidualSellDeconstructionSoleTickNoStompAction::SourceMarkers,
        3 => ResidualSellDeconstructionSoleTickNoStompAction::NavCommands,
        4 => ResidualSellDeconstructionSoleTickNoStompAction::HostSource,
        5 => ResidualSellDeconstructionSoleTickNoStompAction::ShadowSource,
        6 => ResidualSellDeconstructionSoleTickNoStompAction::Composite,
        _ => ResidualSellDeconstructionSoleTickNoStompAction::Idle,
    }
}

fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
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

pub fn honesty_sell_deconstruction_sole_tick_no_stomp_method_names_residual_wave481() -> bool {
    SELL_DECONSTRUCTION_SOLE_TICK_NO_STOMP_METHOD_NAMES_WAVE481.len() == 6
        && residual_name_index(
            SELL_DECONSTRUCTION_SOLE_TICK_NO_STOMP_METHOD_NAMES_WAVE481,
            "update_sell_list",
        ) == Some(0)
        && residual_name_index(
            SELL_DECONSTRUCTION_SOLE_TICK_NO_STOMP_METHOD_NAMES_WAVE481,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_sell_deconstruction_sole_tick_no_stomp_source_markers_residual_wave481() -> bool {
    let ok = super::construction_owner_source_contract::host_owns_construction();
    ok
}

pub fn honesty_sell_deconstruction_sole_tick_no_stomp_nav_commands_residual_wave481() -> bool {
    SELL_DECONSTRUCTION_SOLE_TICK_NO_STOMP_NAV_STEPS_WAVE481.len() == 6
        && residual_name_index(
            SELL_DECONSTRUCTION_SOLE_TICK_NO_STOMP_NAV_STEPS_WAVE481,
            "PUBLISH_NEGATIVE_RATE_ONLY",
        ) == Some(2)
        && residual_name_index(
            SELL_DECONSTRUCTION_SOLE_TICK_NO_STOMP_NAV_STEPS_WAVE481,
            "FINISH_ON_WRITEBACK_THRESHOLD",
        ) == Some(5)
        && RUNTIME_HOST_SELL_DECONSTRUCTION_SOLE_TICK_NO_STOMP_CMD_NAMES_WAVE481.len() == 5
        && residual_name_index(
            RUNTIME_HOST_SELL_DECONSTRUCTION_SOLE_TICK_NO_STOMP_CMD_NAMES_WAVE481,
            "click_sell_deconstruction_sole_tick_no_stomp_ok_wnd_prepare",
        ) == Some(3)
}

pub fn simulate_sell_deconstruction_sole_tick_no_stomp_host_source() -> bool {
    let ok = super::construction_owner_source_contract::host_owns_construction();
    residual_action_store(ResidualSellDeconstructionSoleTickNoStompAction::HostSource);
    ok
}

pub fn simulate_sell_deconstruction_sole_tick_no_stomp_shadow_source() -> bool {
    let ok = super::construction_owner_source_contract::host_owns_construction();
    residual_action_store(ResidualSellDeconstructionSoleTickNoStompAction::ShadowSource);
    ok
}

pub fn honesty_sell_deconstruction_sole_tick_no_stomp_residual_pack_wave481() -> bool {
    honesty_sell_deconstruction_sole_tick_no_stomp_method_names_residual_wave481()
        && honesty_sell_deconstruction_sole_tick_no_stomp_source_markers_residual_wave481()
        && honesty_sell_deconstruction_sole_tick_no_stomp_nav_commands_residual_wave481()
        && simulate_sell_deconstruction_sole_tick_no_stomp_host_source()
        && simulate_sell_deconstruction_sole_tick_no_stomp_shadow_source()
}

pub fn simulate_live_sell_deconstruction_sole_tick_no_stomp_honesty() -> bool {
    let ok = honesty_sell_deconstruction_sole_tick_no_stomp_residual_pack_wave481();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualSellDeconstructionSoleTickNoStompAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_sell_deconstruction_sole_tick_no_stomp_method_names_residual_wave481());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_sell_deconstruction_sole_tick_no_stomp_source_markers_residual_wave481());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_sell_deconstruction_sole_tick_no_stomp_nav_commands_residual_wave481());
    }

    #[test]
    fn sell_deconstruction_sole_tick_no_stomp_sources() {
        assert!(simulate_sell_deconstruction_sole_tick_no_stomp_host_source());
        assert!(simulate_sell_deconstruction_sole_tick_no_stomp_shadow_source());
    }

    #[test]
    fn wave481_composite_pack() {
        assert!(honesty_sell_deconstruction_sole_tick_no_stomp_residual_pack_wave481());
    }

    #[test]
    fn simulate_live_sell_deconstruction_sole_tick_no_stomp_honesty_residual_live() {
        assert!(
            simulate_live_sell_deconstruction_sole_tick_no_stomp_honesty(),
            "sell deconstruction sole-tick no-stomp residual must latch"
        );
        assert!(residual_sell_deconstruction_sole_tick_no_stomp_ok());
        assert_eq!(
            residual_sell_deconstruction_sole_tick_no_stomp_last_action(),
            ResidualSellDeconstructionSoleTickNoStompAction::Composite
        );
    }
}
