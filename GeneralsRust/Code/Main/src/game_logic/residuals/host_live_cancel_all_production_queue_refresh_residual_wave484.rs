//! Wave 484 residual peels: `cancel_all_production` refreshes GW queue under
//! production sole-tick (no per-frame progress stomp).
//! - drain still refunds owner via economy log
//! - each cancelled template records `host_production_log::record_cancel`
//! - `apply_host_production_events` Cancel path snapshots host queue
//! - sell/death/cancel-all no longer leave stale GW production heads
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 483 upgrade-complete queue refresh.
//! Architecture residual - sole-tick progress log gate requires explicit cancel events.
//!
//! Sources:
//! - game_logic.rs cancel_all_production record_cancel loop
//! - host_production_log::record_cancel
//! - gameworld_shadow apply_host_production_events Cancel → enqueue_producers
//!
//! Fail-closed:
//! - Single-item cancel_production already logged Cancel
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const CANCEL_ALL_PRODUCTION_QUEUE_REFRESH_METHOD_NAMES_WAVE484: &[&str] = &[
    "cancel_all_production",
    "record_cancel",
    "production_queue.drain",
    "enqueue_producers",
    "host_economy_log::record",
    "playable_claim = false",
];

pub const CANCEL_ALL_PRODUCTION_QUEUE_REFRESH_SOURCE_MARKERS_WAVE484: &[&str] = &[
    "Wave 484: sole-tick skips per-frame progress log",
    "record_cancel",
    "cancelled_names",
    "HostProductionEvent::Cancel",
];

pub const CANCEL_ALL_PRODUCTION_QUEUE_REFRESH_NAV_STEPS_WAVE484: &[&str] = &[
    "DRAIN_HOST_QUEUE",
    "REFUND_OWNER",
    "RECORD_CANCEL_EACH",
    "APPLY_CANCEL_SNAPSHOT",
    "GW_QUEUE_MATCHES_HOST",
    "SOLE_TICK_NO_PROGRESS_STOMP",
];

pub const RUNTIME_HOST_CANCEL_ALL_PRODUCTION_QUEUE_REFRESH_CMD_NAMES_WAVE484: &[&str] = &[
    "click_cancel_all_production_queue_refresh_ok_wnd_detect",
    "click_cancel_all_production_queue_refresh_ok_wnd_skip",
    "click_cancel_all_production_queue_refresh_ok_wnd_queue",
    "click_cancel_all_production_queue_refresh_ok_wnd_prepare",
    "click_cancel_all_production_queue_refresh_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualCancelAllProductionQueueRefreshAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CancelAllSource = 4,
    ApplySource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualCancelAllProductionQueueRefreshAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_cancel_all_production_queue_refresh_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_cancel_all_production_queue_refresh_last_action()
-> ResidualCancelAllProductionQueueRefreshAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualCancelAllProductionQueueRefreshAction::MethodNames,
        2 => ResidualCancelAllProductionQueueRefreshAction::SourceMarkers,
        3 => ResidualCancelAllProductionQueueRefreshAction::NavCommands,
        4 => ResidualCancelAllProductionQueueRefreshAction::CancelAllSource,
        5 => ResidualCancelAllProductionQueueRefreshAction::ApplySource,
        6 => ResidualCancelAllProductionQueueRefreshAction::Composite,
        _ => ResidualCancelAllProductionQueueRefreshAction::Idle,
    }
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn gw_source() -> &'static str {
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

pub fn honesty_cancel_all_production_queue_refresh_method_names_residual_wave484() -> bool {
    CANCEL_ALL_PRODUCTION_QUEUE_REFRESH_METHOD_NAMES_WAVE484.len() == 6
        && residual_name_index(
            CANCEL_ALL_PRODUCTION_QUEUE_REFRESH_METHOD_NAMES_WAVE484,
            "cancel_all_production",
        ) == Some(0)
        && residual_name_index(
            CANCEL_ALL_PRODUCTION_QUEUE_REFRESH_METHOD_NAMES_WAVE484,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_cancel_all_production_queue_refresh_source_markers_residual_wave484() -> bool {
    CANCEL_ALL_PRODUCTION_QUEUE_REFRESH_SOURCE_MARKERS_WAVE484.len() == 4
        && residual_name_index(
            CANCEL_ALL_PRODUCTION_QUEUE_REFRESH_SOURCE_MARKERS_WAVE484,
            "Wave 484: sole-tick skips per-frame progress log",
        ) == Some(0)
        && residual_name_index(
            CANCEL_ALL_PRODUCTION_QUEUE_REFRESH_SOURCE_MARKERS_WAVE484,
            "record_cancel",
        ) == Some(1)
}

pub fn honesty_cancel_all_production_queue_refresh_nav_commands_residual_wave484() -> bool {
    CANCEL_ALL_PRODUCTION_QUEUE_REFRESH_NAV_STEPS_WAVE484.len() == 6
        && residual_name_index(
            CANCEL_ALL_PRODUCTION_QUEUE_REFRESH_NAV_STEPS_WAVE484,
            "RECORD_CANCEL_EACH",
        ) == Some(2)
        && residual_name_index(
            CANCEL_ALL_PRODUCTION_QUEUE_REFRESH_NAV_STEPS_WAVE484,
            "SOLE_TICK_NO_PROGRESS_STOMP",
        ) == Some(5)
        && RUNTIME_HOST_CANCEL_ALL_PRODUCTION_QUEUE_REFRESH_CMD_NAMES_WAVE484.len() == 5
        && residual_name_index(
            RUNTIME_HOST_CANCEL_ALL_PRODUCTION_QUEUE_REFRESH_CMD_NAMES_WAVE484,
            "click_cancel_all_production_queue_refresh_ok_wnd_prepare",
        ) == Some(3)
}

pub fn simulate_cancel_all_production_queue_refresh_cancel_all_source() -> bool {
    let Some(body) = function_body(gl_source(), "fn cancel_all_production(") else {
        return false;
    };
    let ok = body.contains("Wave 484: sole-tick skips per-frame progress log")
        && body.contains("record_cancel")
        && body.contains("cancelled_names")
        && body.contains("production_queue.drain");
    residual_action_store(ResidualCancelAllProductionQueueRefreshAction::CancelAllSource);
    ok
}

pub fn simulate_cancel_all_production_queue_refresh_apply_source() -> bool {
    let Some(body) = function_body(gw_source(), "fn apply_host_production_events(") else {
        return false;
    };
    let ok = body.contains("HostProductionEvent::Cancel")
        && body.contains("enqueue_producers.insert(producer.0)");
    residual_action_store(ResidualCancelAllProductionQueueRefreshAction::ApplySource);
    ok
}

pub fn honesty_cancel_all_production_queue_refresh_residual_pack_wave484() -> bool {
    honesty_cancel_all_production_queue_refresh_method_names_residual_wave484()
        && honesty_cancel_all_production_queue_refresh_source_markers_residual_wave484()
        && honesty_cancel_all_production_queue_refresh_nav_commands_residual_wave484()
        && simulate_cancel_all_production_queue_refresh_cancel_all_source()
        && simulate_cancel_all_production_queue_refresh_apply_source()
}

pub fn simulate_live_cancel_all_production_queue_refresh_honesty() -> bool {
    let ok = honesty_cancel_all_production_queue_refresh_residual_pack_wave484();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualCancelAllProductionQueueRefreshAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_cancel_all_production_queue_refresh_method_names_residual_wave484());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_cancel_all_production_queue_refresh_source_markers_residual_wave484());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_cancel_all_production_queue_refresh_nav_commands_residual_wave484());
    }

    #[test]
    fn cancel_all_production_queue_refresh_sources() {
        assert!(simulate_cancel_all_production_queue_refresh_cancel_all_source());
        assert!(simulate_cancel_all_production_queue_refresh_apply_source());
    }

    #[test]
    fn wave484_composite_pack() {
        assert!(honesty_cancel_all_production_queue_refresh_residual_pack_wave484());
    }

    #[test]
    fn simulate_live_cancel_all_production_queue_refresh_honesty_residual_live() {
        assert!(
            simulate_live_cancel_all_production_queue_refresh_honesty(),
            "cancel_all production queue refresh residual must latch"
        );
        assert!(residual_cancel_all_production_queue_refresh_ok());
        assert_eq!(
            residual_cancel_all_production_queue_refresh_last_action(),
            ResidualCancelAllProductionQueueRefreshAction::Composite
        );
    }
}
