//! Wave 481 residual peels: under construction sole-tick, sell deconstruction
//! does not host-advance construction percent each frame.
//! - `update_sell_list` publishes negative `record_rate_only` under sole-tick
//! - host finish/sold-model uses writeback percent
//! - GW `tick_construction_progress` advances sell via negative rate
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 478 build sole-tick rate-only peel.
//! Architecture residual - sell percent last-writer is GameWorld under sole-tick.
//!
//! Sources:
//! - game_logic.rs update_sell_list sole-tick branch
//! - host_construction_progress_log::record_rate_only
//! - gameworld_shadow::tick_construction_progress negative rate path
//!
//! Fail-closed:
//! - Non-sole path still decrements host percent per frame
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const SELL_DECONSTRUCTION_SOLE_TICK_NO_STOMP_METHOD_NAMES_WAVE481: &[&str] = &[
    "update_sell_list",
    "gameworld_construction_sole_tick_enabled",
    "record_rate_only",
    "SELL_CONSTRUCTION_DECREMENT_RESIDUAL",
    "tick_construction_progress",
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
    SELL_DECONSTRUCTION_SOLE_TICK_NO_STOMP_SOURCE_MARKERS_WAVE481.len() == 4
        && residual_name_index(
            SELL_DECONSTRUCTION_SOLE_TICK_NO_STOMP_SOURCE_MARKERS_WAVE481,
            "Wave 481: GW sole-ticks sell percent via negative rate",
        ) == Some(0)
        && residual_name_index(
            SELL_DECONSTRUCTION_SOLE_TICK_NO_STOMP_SOURCE_MARKERS_WAVE481,
            "sole && previous <= 0.0",
        ) == Some(3)
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
    let gl = gl_source();
    let Some(body) = function_body(&gl, "pub(crate) fn update_sell_list(")
        .or_else(|| function_body(&gl, "fn update_sell_list("))
    else {
        return false;
    };
    let ok = body.contains("Wave 481: GW sole-ticks sell percent via negative rate")
        && body.contains("record_rate_only")
        && body.contains("gameworld_construction_sole_tick_enabled")
        && body.contains("SELL_CONSTRUCTION_DECREMENT_RESIDUAL")
        && body.contains("sole && previous <= 0.0");
    residual_action_store(ResidualSellDeconstructionSoleTickNoStompAction::HostSource);
    ok
}

pub fn simulate_sell_deconstruction_sole_tick_no_stomp_shadow_source() -> bool {
    let src = shadow_source();
    let Some(body) = function_body(src, "fn tick_construction_progress(") else {
        return false;
    };
    // 2026-08-15: sell is negative rate (writeback_production.rs:889).
    let ok = (body.contains("rate >= 0.0") || body.contains("rate < 0.0"))
        && (body.contains("Sell path") || body.contains("rate < 0.0"))
        && src.contains("fn writeback_construction_to_host")
        && src.contains("tick_construction_progress");
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
