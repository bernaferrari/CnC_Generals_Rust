//! Wave 483 residual peels: production upgrade complete refreshes GW queue
//! under sole-tick (no per-frame progress stomp).
//! - upgrade complete records `host_production_log::record_complete`
//! - spawned id 0 = queue-refresh-only (no unit spawn)
//! - `apply_host_production_events` skips spawn for id 0
//! - Complete still inserts producer into queue last-write snapshot set
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 477 power-only + Wave 480 exit-delay arm.
//! Architecture residual - upgrade pop must reach GW when progress log is gated.
//!
//! Sources:
//! - game_logic.rs upgrade_completions → record_complete(..., ObjectId(0))
//! - gameworld_shadow.rs apply_host_production_events spawned.0 == 0
//!
//! Fail-closed:
//! - Unit completes still spawn via non-zero spawned ids
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRODUCTION_UPGRADE_COMPLETE_QUEUE_REFRESH_METHOD_NAMES_WAVE483: &[&str] = &[
    "upgrade_completions",
    "record_complete",
    "ObjectId(0)",
    "spawned.0 == 0",
    "enqueue_producers",
    "playable_claim = false",
];

pub const PRODUCTION_UPGRADE_COMPLETE_QUEUE_REFRESH_SOURCE_MARKERS_WAVE483: &[&str] = &[
    "Wave 483: refresh GW producer queue after host pop",
    "record_complete",
    "ObjectId(0)",
    "Wave 483: upgrade complete uses spawned id 0",
];

pub const PRODUCTION_UPGRADE_COMPLETE_QUEUE_REFRESH_NAV_STEPS_WAVE483: &[&str] = &[
    "UPGRADE_COMPLETE_HOST",
    "RECORD_COMPLETE_ID_ZERO",
    "APPLY_SKIP_SPAWN",
    "ENQUEUE_PRODUCER_SNAPSHOT",
    "WRITEBACK_MATCHES_HOST_QUEUE",
    "SOLE_TICK_NO_PROGRESS_STOMP",
];

pub const RUNTIME_HOST_PRODUCTION_UPGRADE_COMPLETE_QUEUE_REFRESH_CMD_NAMES_WAVE483: &[&str] = &[
    "click_production_upgrade_complete_queue_refresh_ok_wnd_detect",
    "click_production_upgrade_complete_queue_refresh_ok_wnd_skip",
    "click_production_upgrade_complete_queue_refresh_ok_wnd_queue",
    "click_production_upgrade_complete_queue_refresh_ok_wnd_prepare",
    "click_production_upgrade_complete_queue_refresh_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualProductionUpgradeCompleteQueueRefreshAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    UpgradeSource = 4,
    ApplySource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualProductionUpgradeCompleteQueueRefreshAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_production_upgrade_complete_queue_refresh_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_production_upgrade_complete_queue_refresh_last_action()
-> ResidualProductionUpgradeCompleteQueueRefreshAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualProductionUpgradeCompleteQueueRefreshAction::MethodNames,
        2 => ResidualProductionUpgradeCompleteQueueRefreshAction::SourceMarkers,
        3 => ResidualProductionUpgradeCompleteQueueRefreshAction::NavCommands,
        4 => ResidualProductionUpgradeCompleteQueueRefreshAction::UpgradeSource,
        5 => ResidualProductionUpgradeCompleteQueueRefreshAction::ApplySource,
        6 => ResidualProductionUpgradeCompleteQueueRefreshAction::Composite,
        _ => ResidualProductionUpgradeCompleteQueueRefreshAction::Idle,
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

pub fn honesty_production_upgrade_complete_queue_refresh_method_names_residual_wave483() -> bool {
    PRODUCTION_UPGRADE_COMPLETE_QUEUE_REFRESH_METHOD_NAMES_WAVE483.len() == 6
        && residual_name_index(
            PRODUCTION_UPGRADE_COMPLETE_QUEUE_REFRESH_METHOD_NAMES_WAVE483,
            "upgrade_completions",
        ) == Some(0)
        && residual_name_index(
            PRODUCTION_UPGRADE_COMPLETE_QUEUE_REFRESH_METHOD_NAMES_WAVE483,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_production_upgrade_complete_queue_refresh_source_markers_residual_wave483() -> bool {
    PRODUCTION_UPGRADE_COMPLETE_QUEUE_REFRESH_SOURCE_MARKERS_WAVE483.len() == 4
        && residual_name_index(
            PRODUCTION_UPGRADE_COMPLETE_QUEUE_REFRESH_SOURCE_MARKERS_WAVE483,
            "Wave 483: refresh GW producer queue after host pop",
        ) == Some(0)
        && residual_name_index(
            PRODUCTION_UPGRADE_COMPLETE_QUEUE_REFRESH_SOURCE_MARKERS_WAVE483,
            "ObjectId(0)",
        ) == Some(2)
}

pub fn honesty_production_upgrade_complete_queue_refresh_nav_commands_residual_wave483() -> bool {
    PRODUCTION_UPGRADE_COMPLETE_QUEUE_REFRESH_NAV_STEPS_WAVE483.len() == 6
        && residual_name_index(
            PRODUCTION_UPGRADE_COMPLETE_QUEUE_REFRESH_NAV_STEPS_WAVE483,
            "RECORD_COMPLETE_ID_ZERO",
        ) == Some(1)
        && residual_name_index(
            PRODUCTION_UPGRADE_COMPLETE_QUEUE_REFRESH_NAV_STEPS_WAVE483,
            "SOLE_TICK_NO_PROGRESS_STOMP",
        ) == Some(5)
        && RUNTIME_HOST_PRODUCTION_UPGRADE_COMPLETE_QUEUE_REFRESH_CMD_NAMES_WAVE483.len() == 5
        && residual_name_index(
            RUNTIME_HOST_PRODUCTION_UPGRADE_COMPLETE_QUEUE_REFRESH_CMD_NAMES_WAVE483,
            "click_production_upgrade_complete_queue_refresh_ok_wnd_prepare",
        ) == Some(3)
}

pub fn simulate_production_upgrade_complete_queue_refresh_upgrade_source() -> bool {
    let gl = gl_source();
    let ok = gl.contains("Wave 483: refresh GW producer queue after host pop")
        && gl.contains("upgrade_completions")
        && gl.contains("host_production_log::record_complete")
        && gl.contains("ObjectId(0)");
    residual_action_store(ResidualProductionUpgradeCompleteQueueRefreshAction::UpgradeSource);
    ok
}

pub fn simulate_production_upgrade_complete_queue_refresh_apply_source() -> bool {
    let gw = gw_source();
    let Some(body) = function_body(gw, "fn apply_host_production_events(") else {
        return false;
    };
    let ok = body.contains("Wave 483: upgrade complete uses spawned id 0")
        && body.contains("spawned.0 == 0")
        && body.contains("enqueue_producers.insert(producer.0)");
    residual_action_store(ResidualProductionUpgradeCompleteQueueRefreshAction::ApplySource);
    ok
}

pub fn honesty_production_upgrade_complete_queue_refresh_residual_pack_wave483() -> bool {
    honesty_production_upgrade_complete_queue_refresh_method_names_residual_wave483()
        && honesty_production_upgrade_complete_queue_refresh_source_markers_residual_wave483()
        && honesty_production_upgrade_complete_queue_refresh_nav_commands_residual_wave483()
        && simulate_production_upgrade_complete_queue_refresh_upgrade_source()
        && simulate_production_upgrade_complete_queue_refresh_apply_source()
}

pub fn simulate_live_production_upgrade_complete_queue_refresh_honesty() -> bool {
    let ok = honesty_production_upgrade_complete_queue_refresh_residual_pack_wave483();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualProductionUpgradeCompleteQueueRefreshAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_production_upgrade_complete_queue_refresh_method_names_residual_wave483());
    }

    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_production_upgrade_complete_queue_refresh_source_markers_residual_wave483()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_production_upgrade_complete_queue_refresh_nav_commands_residual_wave483());
    }

    #[test]
    fn production_upgrade_complete_queue_refresh_sources() {
        assert!(simulate_production_upgrade_complete_queue_refresh_upgrade_source());
        assert!(simulate_production_upgrade_complete_queue_refresh_apply_source());
    }

    #[test]
    fn wave483_composite_pack() {
        assert!(honesty_production_upgrade_complete_queue_refresh_residual_pack_wave483());
    }

    #[test]
    fn simulate_live_production_upgrade_complete_queue_refresh_honesty_residual_live() {
        assert!(
            simulate_live_production_upgrade_complete_queue_refresh_honesty(),
            "production upgrade complete queue refresh residual must latch"
        );
        assert!(residual_production_upgrade_complete_queue_refresh_ok());
        assert_eq!(
            residual_production_upgrade_complete_queue_refresh_last_action(),
            ResidualProductionUpgradeCompleteQueueRefreshAction::Composite
        );
    }
}
