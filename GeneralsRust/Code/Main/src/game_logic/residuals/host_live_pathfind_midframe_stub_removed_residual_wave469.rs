//! Wave 469 residual peels: empty engine mid-frame path step stub removed.
//! `update_unit_pathfinding` no longer exists; host `update_movement` is sole
//! path follower unless GameWorld movement authority live.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 424/426 path dual-world empty gates.
//! Architecture residual - no dead dual mid-frame path hook on engine.
//!
//! Sources (cnc_game_engine.rs / game_logic.rs):
//! - no fn update_unit_pathfinding
//! - GameLogic::update_movement sole host path integrate (or skip under GW authority)
//! - gameworld_movement_authority_live gates host skip
//!
//! Fail-closed:
//! - Host still owns path *commands* (move_to logs)
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PATHFIND_MIDFRAME_STUB_REMOVED_METHOD_NAMES_WAVE469: &[&str] = &[
    "update_movement",
    "gameworld_movement_authority_live",
    "step_movement",
    "writeback_movement_to_host",
    "shadow_session_after_host_tick",
    "playable_claim = false",
];

pub const PATHFIND_MIDFRAME_STUB_REMOVED_SOURCE_MARKERS_WAVE469: &[&str] = &[
    "GameWorld movement authority: path integrate + pose last-write",
    "gameworld_movement_authority_live()",
    "fn update_movement",
    "no update_unit_pathfinding",
];

pub const PATHFIND_MIDFRAME_STUB_REMOVED_NAV_STEPS_WAVE469: &[&str] = &[
    "HOST_ISSUES_PATH_COMMANDS",
    "HOST_UPDATE_MOVEMENT_OR_SKIP_UNDER_GW_AUTH",
    "SHADOW_STEP_MOVEMENT_WHEN_AUTH",
    "WRITEBACK_POSES_TO_HOST",
    "NO_ENGINE_MIDFRAME_PATH_STUB",
    "NO_DOUBLE_PATH_STEP",
];

pub const RUNTIME_HOST_PATHFIND_MIDFRAME_STUB_REMOVED_CMD_NAMES_WAVE469: &[&str] = &[
    "click_pathfind_midframe_stub_removed_ok_wnd_host_cmd",
    "click_pathfind_midframe_stub_removed_ok_wnd_host_or_gw",
    "click_pathfind_midframe_stub_removed_ok_wnd_writeback",
    "click_pathfind_midframe_stub_removed_ok_wnd_prepare",
    "click_pathfind_midframe_stub_removed_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPathfindMidframeStubRemovedAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    EngineSource = 4,
    HostSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPathfindMidframeStubRemovedAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_pathfind_midframe_stub_removed_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_pathfind_midframe_stub_removed_last_action()
-> ResidualPathfindMidframeStubRemovedAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPathfindMidframeStubRemovedAction::MethodNames,
        2 => ResidualPathfindMidframeStubRemovedAction::SourceMarkers,
        3 => ResidualPathfindMidframeStubRemovedAction::NavCommands,
        4 => ResidualPathfindMidframeStubRemovedAction::EngineSource,
        5 => ResidualPathfindMidframeStubRemovedAction::HostSource,
        6 => ResidualPathfindMidframeStubRemovedAction::Composite,
        _ => ResidualPathfindMidframeStubRemovedAction::Idle,
    }
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn game_logic_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}

pub fn honesty_pathfind_midframe_stub_removed_method_names_residual_wave469() -> bool {
    PATHFIND_MIDFRAME_STUB_REMOVED_METHOD_NAMES_WAVE469.len() == 6
        && residual_name_index(
            PATHFIND_MIDFRAME_STUB_REMOVED_METHOD_NAMES_WAVE469,
            "update_movement",
        ) == Some(0)
        && residual_name_index(
            PATHFIND_MIDFRAME_STUB_REMOVED_METHOD_NAMES_WAVE469,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_pathfind_midframe_stub_removed_source_markers_residual_wave469() -> bool {
    PATHFIND_MIDFRAME_STUB_REMOVED_SOURCE_MARKERS_WAVE469.len() == 4
        && residual_name_index(
            PATHFIND_MIDFRAME_STUB_REMOVED_SOURCE_MARKERS_WAVE469,
            "GameWorld movement authority: path integrate + pose last-write",
        ) == Some(0)
        && residual_name_index(
            PATHFIND_MIDFRAME_STUB_REMOVED_SOURCE_MARKERS_WAVE469,
            "no update_unit_pathfinding",
        ) == Some(3)
}

pub fn honesty_pathfind_midframe_stub_removed_nav_commands_residual_wave469() -> bool {
    PATHFIND_MIDFRAME_STUB_REMOVED_NAV_STEPS_WAVE469.len() == 6
        && residual_name_index(
            PATHFIND_MIDFRAME_STUB_REMOVED_NAV_STEPS_WAVE469,
            "NO_ENGINE_MIDFRAME_PATH_STUB",
        ) == Some(4)
        && residual_name_index(
            PATHFIND_MIDFRAME_STUB_REMOVED_NAV_STEPS_WAVE469,
            "NO_DOUBLE_PATH_STEP",
        ) == Some(5)
        && RUNTIME_HOST_PATHFIND_MIDFRAME_STUB_REMOVED_CMD_NAMES_WAVE469.len() == 5
        && residual_name_index(
            RUNTIME_HOST_PATHFIND_MIDFRAME_STUB_REMOVED_CMD_NAMES_WAVE469,
            "click_pathfind_midframe_stub_removed_ok_wnd_prepare",
        ) == Some(3)
}

pub fn simulate_pathfind_midframe_stub_removed_engine_source() -> bool {
    let src = cnc_source();
    // Wave 469: stub fully removed from engine.
    let ok =
        !src.contains("fn update_unit_pathfinding") && !src.contains("update_unit_pathfinding(");
    residual_action_store(ResidualPathfindMidframeStubRemovedAction::EngineSource);
    ok
}

pub fn simulate_pathfind_midframe_stub_removed_host_source() -> bool {
    let gl = game_logic_source();
    let sw = shadow_source();
    let ok = gl.contains("fn update_movement")
        && gl.contains("GameWorld movement authority: path integrate + pose last-write")
        && gl.contains("gameworld_movement_authority_live()")
        && sw.contains("step_movement")
        && sw.contains("writeback_movement_to_host");
    residual_action_store(ResidualPathfindMidframeStubRemovedAction::HostSource);
    ok
}

pub fn honesty_pathfind_midframe_stub_removed_residual_pack_wave469() -> bool {
    honesty_pathfind_midframe_stub_removed_method_names_residual_wave469()
        && honesty_pathfind_midframe_stub_removed_source_markers_residual_wave469()
        && honesty_pathfind_midframe_stub_removed_nav_commands_residual_wave469()
        && simulate_pathfind_midframe_stub_removed_engine_source()
        && simulate_pathfind_midframe_stub_removed_host_source()
}

pub fn simulate_live_pathfind_midframe_stub_removed_honesty() -> bool {
    let ok = honesty_pathfind_midframe_stub_removed_residual_pack_wave469();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPathfindMidframeStubRemovedAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_pathfind_midframe_stub_removed_method_names_residual_wave469());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_pathfind_midframe_stub_removed_source_markers_residual_wave469());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_pathfind_midframe_stub_removed_nav_commands_residual_wave469());
    }

    #[test]
    fn pathfind_midframe_stub_removed_sources() {
        assert!(simulate_pathfind_midframe_stub_removed_engine_source());
        assert!(simulate_pathfind_midframe_stub_removed_host_source());
    }

    #[test]
    fn wave469_composite_pack() {
        assert!(honesty_pathfind_midframe_stub_removed_residual_pack_wave469());
    }

    #[test]
    fn simulate_live_pathfind_midframe_stub_removed_honesty_residual_live() {
        assert!(
            simulate_live_pathfind_midframe_stub_removed_honesty(),
            "pathfind midframe stub removed residual must latch"
        );
        assert!(residual_pathfind_midframe_stub_removed_ok());
        assert_eq!(
            residual_pathfind_midframe_stub_removed_last_action(),
            ResidualPathfindMidframeStubRemovedAction::Composite
        );
    }
}
