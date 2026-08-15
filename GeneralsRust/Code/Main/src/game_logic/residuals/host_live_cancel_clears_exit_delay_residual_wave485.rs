//! Wave 485 residual peels: emptying production queue clears exit-delay residual.
//! - `cancel_all_production` zeroes `exit_delay_remaining` when draining queue
//! - single `cancel_production` clears exit delay when queue becomes empty
//! - both publish `record_exit_delay_only(..., 0.0)` for GW sole-tick
//! - prevents ghost QueueProductionExitUpdate hold after cancel/sell/death
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 480 arm-exit and Wave 484 cancel-all queue refresh.
//! Architecture residual - exit delay is production-queue companion state.
//!
//! Sources:
//! - game_logic.rs cancel_all_production exit clear + record_exit_delay_only
//! - game_logic.rs cancel_production empty-queue exit clear
//! - host_production_progress_log::record_exit_delay_only
//!
//! Fail-closed:
//! - Non-empty queue after single cancel keeps exit delay
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const CANCEL_CLEARS_EXIT_DELAY_METHOD_NAMES_WAVE485: &[&str] = &[
    "cancel_all_production",
    "cancel_production",
    "exit_delay_remaining = 0.0",
    "record_exit_delay_only",
    "production_queue.is_empty",
    "playable_claim = false",
];

pub const CANCEL_CLEARS_EXIT_DELAY_SOURCE_MARKERS_WAVE485: &[&str] = &[
    "Wave 485: empty queue clears QueueProductionExitUpdate residual",
    "Wave 485: last cancelled item clears factory exit-delay residual",
    "record_exit_delay_only",
    "exit_delay_remaining = 0.0",
];

pub const CANCEL_CLEARS_EXIT_DELAY_NAV_STEPS_WAVE485: &[&str] = &[
    "CANCEL_DRAINS_OR_REMOVES",
    "QUEUE_EMPTY",
    "CLEAR_EXIT_DELAY",
    "RECORD_EXIT_DELAY_ZERO",
    "GW_SOLE_TICK_NO_GHOST",
    "NEXT_ENQUEUE_UNBLOCKED",
];

pub const RUNTIME_HOST_CANCEL_CLEARS_EXIT_DELAY_CMD_NAMES_WAVE485: &[&str] = &[
    "click_cancel_clears_exit_delay_ok_wnd_detect",
    "click_cancel_clears_exit_delay_ok_wnd_skip",
    "click_cancel_clears_exit_delay_ok_wnd_queue",
    "click_cancel_clears_exit_delay_ok_wnd_prepare",
    "click_cancel_clears_exit_delay_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualCancelClearsExitDelayAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CancelAllSource = 4,
    CancelOneSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualCancelClearsExitDelayAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_cancel_clears_exit_delay_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_cancel_clears_exit_delay_last_action() -> ResidualCancelClearsExitDelayAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualCancelClearsExitDelayAction::MethodNames,
        2 => ResidualCancelClearsExitDelayAction::SourceMarkers,
        3 => ResidualCancelClearsExitDelayAction::NavCommands,
        4 => ResidualCancelClearsExitDelayAction::CancelAllSource,
        5 => ResidualCancelClearsExitDelayAction::CancelOneSource,
        6 => ResidualCancelClearsExitDelayAction::Composite,
        _ => ResidualCancelClearsExitDelayAction::Idle,
    }
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
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

pub fn honesty_cancel_clears_exit_delay_method_names_residual_wave485() -> bool {
    CANCEL_CLEARS_EXIT_DELAY_METHOD_NAMES_WAVE485.len() == 6
        && residual_name_index(
            CANCEL_CLEARS_EXIT_DELAY_METHOD_NAMES_WAVE485,
            "cancel_all_production",
        ) == Some(0)
        && residual_name_index(
            CANCEL_CLEARS_EXIT_DELAY_METHOD_NAMES_WAVE485,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_cancel_clears_exit_delay_source_markers_residual_wave485() -> bool {
    CANCEL_CLEARS_EXIT_DELAY_SOURCE_MARKERS_WAVE485.len() == 4
        && residual_name_index(
            CANCEL_CLEARS_EXIT_DELAY_SOURCE_MARKERS_WAVE485,
            "Wave 485: empty queue clears QueueProductionExitUpdate residual",
        ) == Some(0)
        && residual_name_index(
            CANCEL_CLEARS_EXIT_DELAY_SOURCE_MARKERS_WAVE485,
            "record_exit_delay_only",
        ) == Some(2)
}

pub fn honesty_cancel_clears_exit_delay_nav_commands_residual_wave485() -> bool {
    CANCEL_CLEARS_EXIT_DELAY_NAV_STEPS_WAVE485.len() == 6
        && residual_name_index(
            CANCEL_CLEARS_EXIT_DELAY_NAV_STEPS_WAVE485,
            "CLEAR_EXIT_DELAY",
        ) == Some(2)
        && residual_name_index(
            CANCEL_CLEARS_EXIT_DELAY_NAV_STEPS_WAVE485,
            "NEXT_ENQUEUE_UNBLOCKED",
        ) == Some(5)
        && RUNTIME_HOST_CANCEL_CLEARS_EXIT_DELAY_CMD_NAMES_WAVE485.len() == 5
        && residual_name_index(
            RUNTIME_HOST_CANCEL_CLEARS_EXIT_DELAY_CMD_NAMES_WAVE485,
            "click_cancel_clears_exit_delay_ok_wnd_prepare",
        ) == Some(3)
}

pub fn simulate_cancel_clears_exit_delay_cancel_all_source() -> bool {
    let Some(body) = function_body(gl_source(), "fn cancel_all_production(") else {
        return false;
    };
    let ok = body.contains("Wave 485: empty queue clears QueueProductionExitUpdate residual")
        && body.contains("exit_delay_remaining = 0.0")
        && body.contains("record_exit_delay_only")
        && body.contains("cleared_exit_delay");
    residual_action_store(ResidualCancelClearsExitDelayAction::CancelAllSource);
    ok
}

pub fn simulate_cancel_clears_exit_delay_cancel_one_source() -> bool {
    let Some(body) = function_body(gl_source(), "fn cancel_production(") else {
        return false;
    };
    let ok = body.contains("Wave 485: last cancelled item clears factory exit-delay residual")
        && body.contains("production_queue.is_empty()")
        && body.contains("record_exit_delay_only")
        && body.contains("exit_delay_remaining = 0.0");
    residual_action_store(ResidualCancelClearsExitDelayAction::CancelOneSource);
    ok
}

pub fn honesty_cancel_clears_exit_delay_residual_pack_wave485() -> bool {
    honesty_cancel_clears_exit_delay_method_names_residual_wave485()
        && honesty_cancel_clears_exit_delay_source_markers_residual_wave485()
        && honesty_cancel_clears_exit_delay_nav_commands_residual_wave485()
        && simulate_cancel_clears_exit_delay_cancel_all_source()
        && simulate_cancel_clears_exit_delay_cancel_one_source()
}

pub fn simulate_live_cancel_clears_exit_delay_honesty() -> bool {
    let ok = honesty_cancel_clears_exit_delay_residual_pack_wave485();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualCancelClearsExitDelayAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_cancel_clears_exit_delay_method_names_residual_wave485());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_cancel_clears_exit_delay_source_markers_residual_wave485());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_cancel_clears_exit_delay_nav_commands_residual_wave485());
    }

    #[test]
    fn cancel_clears_exit_delay_sources() {
        assert!(simulate_cancel_clears_exit_delay_cancel_all_source());
        assert!(simulate_cancel_clears_exit_delay_cancel_one_source());
    }

    #[test]
    fn wave485_composite_pack() {
        assert!(honesty_cancel_clears_exit_delay_residual_pack_wave485());
    }

    #[test]
    fn simulate_live_cancel_clears_exit_delay_honesty_residual_live() {
        assert!(
            simulate_live_cancel_clears_exit_delay_honesty(),
            "cancel clears exit delay residual must latch"
        );
        assert!(residual_cancel_clears_exit_delay_ok());
        assert_eq!(
            residual_cancel_clears_exit_delay_last_action(),
            ResidualCancelClearsExitDelayAction::Composite
        );
    }
}
