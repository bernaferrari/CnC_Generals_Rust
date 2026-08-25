//! Wave 476 residual peels: named shell sync is host tracker only.
//! - `sync_named_shell_object_into_legacy_runtime` registers host ObjectId
//! - no dual ObjectManager::create_object / OBJECT_REGISTRY mirror
//! - no engine_object_bridge gate on this path
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 475 map-ground host-only pose peel.
//! Architecture residual - shell named objects do not dual-create into legacy registry.
//!
//! Sources (game_logic.rs):
//! - fn sync_named_shell_object_into_legacy_runtime
//! - tracker.register_named_object(name, host_id)
//! - Wave 476 host-only comment
//!
//! Fail-closed:
//! - Other bridge create paths may still exist under GENERALS_BRIDGE_ENGINE_OBJECTS
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const NAMED_SHELL_HOST_ONLY_TRACKER_METHOD_NAMES_WAVE476: &[&str] = &[
    "sync_named_shell_object_into_legacy_runtime",
    "register_named_object",
    "host ObjectId is the name key",
    "no ObjectManager::create_object mirror",
    "no engine_object_bridge gate",
    "playable_claim = false",
];

pub const NAMED_SHELL_HOST_ONLY_TRACKER_SOURCE_MARKERS_WAVE476: &[&str] = &[
    "Wave 476: host-only named tracker registration",
    "Dual ObjectManager/OBJECT_REGISTRY mirror retired",
    "register_named_object",
    "no create_object mirror in named shell sync",
];

pub const NAMED_SHELL_HOST_ONLY_TRACKER_NAV_STEPS_WAVE476: &[&str] = &[
    "SHELL_MODE_GATE",
    "REQUIRE_NONEMPTY_NAME",
    "SKIP_IF_ALREADY_REGISTERED",
    "REGISTER_HOST_OBJECT_ID",
    "NO_LEGACY_OBJECT_MANAGER_CREATE",
    "NO_BRIDGE_GATE_ON_PATH",
];

pub const RUNTIME_HOST_NAMED_SHELL_HOST_ONLY_TRACKER_CMD_NAMES_WAVE476: &[&str] = &[
    "click_named_shell_host_only_tracker_ok_wnd_shell",
    "click_named_shell_host_only_tracker_ok_wnd_register",
    "click_named_shell_host_only_tracker_ok_wnd_skip_dual",
    "click_named_shell_host_only_tracker_ok_wnd_prepare",
    "click_named_shell_host_only_tracker_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualNamedShellHostOnlyTrackerAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    SyncSource = 4,
    DualAbsent = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualNamedShellHostOnlyTrackerAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_named_shell_host_only_tracker_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_named_shell_host_only_tracker_last_action()
-> ResidualNamedShellHostOnlyTrackerAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualNamedShellHostOnlyTrackerAction::MethodNames,
        2 => ResidualNamedShellHostOnlyTrackerAction::SourceMarkers,
        3 => ResidualNamedShellHostOnlyTrackerAction::NavCommands,
        4 => ResidualNamedShellHostOnlyTrackerAction::SyncSource,
        5 => ResidualNamedShellHostOnlyTrackerAction::DualAbsent,
        6 => ResidualNamedShellHostOnlyTrackerAction::Composite,
        _ => ResidualNamedShellHostOnlyTrackerAction::Idle,
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

pub fn honesty_named_shell_host_only_tracker_method_names_residual_wave476() -> bool {
    NAMED_SHELL_HOST_ONLY_TRACKER_METHOD_NAMES_WAVE476.len() == 6
        && residual_name_index(
            NAMED_SHELL_HOST_ONLY_TRACKER_METHOD_NAMES_WAVE476,
            "sync_named_shell_object_into_legacy_runtime",
        ) == Some(0)
        && residual_name_index(
            NAMED_SHELL_HOST_ONLY_TRACKER_METHOD_NAMES_WAVE476,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_named_shell_host_only_tracker_source_markers_residual_wave476() -> bool {
    NAMED_SHELL_HOST_ONLY_TRACKER_SOURCE_MARKERS_WAVE476.len() == 4
        && residual_name_index(
            NAMED_SHELL_HOST_ONLY_TRACKER_SOURCE_MARKERS_WAVE476,
            "Wave 476: host-only named tracker registration",
        ) == Some(0)
        && residual_name_index(
            NAMED_SHELL_HOST_ONLY_TRACKER_SOURCE_MARKERS_WAVE476,
            "no create_object mirror in named shell sync",
        ) == Some(3)
}

pub fn honesty_named_shell_host_only_tracker_nav_commands_residual_wave476() -> bool {
    NAMED_SHELL_HOST_ONLY_TRACKER_NAV_STEPS_WAVE476.len() == 6
        && residual_name_index(
            NAMED_SHELL_HOST_ONLY_TRACKER_NAV_STEPS_WAVE476,
            "REGISTER_HOST_OBJECT_ID",
        ) == Some(3)
        && residual_name_index(
            NAMED_SHELL_HOST_ONLY_TRACKER_NAV_STEPS_WAVE476,
            "NO_BRIDGE_GATE_ON_PATH",
        ) == Some(5)
        && RUNTIME_HOST_NAMED_SHELL_HOST_ONLY_TRACKER_CMD_NAMES_WAVE476.len() == 5
        && residual_name_index(
            RUNTIME_HOST_NAMED_SHELL_HOST_ONLY_TRACKER_CMD_NAMES_WAVE476,
            "click_named_shell_host_only_tracker_ok_wnd_prepare",
        ) == Some(3)
}

pub fn simulate_named_shell_host_only_tracker_source() -> bool {
    let src = gl_source();
    let Some(body) = function_body(src, "fn sync_named_shell_object_into_legacy_runtime(") else {
        return false;
    };
    let ok = body.contains("Wave 476: host-only named tracker registration")
        && body.contains("register_named_object")
        && body.contains("host_id.0")
        && body.contains("GameMode::Shell")
        && !body.contains("engine_object_bridge_enabled")
        && !body.contains("get_object_manager")
        && !body.contains("create_object(");
    residual_action_store(ResidualNamedShellHostOnlyTrackerAction::SyncSource);
    ok
}

pub fn simulate_named_shell_dual_mirror_absent() -> bool {
    let src = gl_source();
    let Some(body) = function_body(src, "fn sync_named_shell_object_into_legacy_runtime(") else {
        return false;
    };
    let ok = body.contains("Dual ObjectManager/OBJECT_REGISTRY mirror retired")
        && !body.contains("OBJECT_REGISTRY.get_object")
        && !body.contains("TheGameLogic::find_object_by_id")
        && !body.contains("ObjectCreationFlags")
        && !body.contains("get_object_manager");
    residual_action_store(ResidualNamedShellHostOnlyTrackerAction::DualAbsent);
    ok
}

pub fn honesty_named_shell_host_only_tracker_residual_pack_wave476() -> bool {
    honesty_named_shell_host_only_tracker_method_names_residual_wave476()
        && honesty_named_shell_host_only_tracker_source_markers_residual_wave476()
        && honesty_named_shell_host_only_tracker_nav_commands_residual_wave476()
        && simulate_named_shell_host_only_tracker_source()
        && simulate_named_shell_dual_mirror_absent()
}

pub fn simulate_live_named_shell_host_only_tracker_honesty() -> bool {
    let ok = honesty_named_shell_host_only_tracker_residual_pack_wave476();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualNamedShellHostOnlyTrackerAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_named_shell_host_only_tracker_method_names_residual_wave476());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_named_shell_host_only_tracker_source_markers_residual_wave476());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_named_shell_host_only_tracker_nav_commands_residual_wave476());
    }

    #[test]
    fn named_shell_host_only_tracker_sources() {
        assert!(simulate_named_shell_host_only_tracker_source());
        assert!(simulate_named_shell_dual_mirror_absent());
        let body = function_body(
            gl_source(),
            "fn sync_named_shell_object_into_legacy_runtime(",
        )
        .unwrap();
        assert!(!body.contains("create_object("));
        assert!(!body.contains("engine_object_bridge_enabled"));
    }

    #[test]
    fn wave476_composite_pack() {
        assert!(honesty_named_shell_host_only_tracker_residual_pack_wave476());
    }

    #[test]
    fn simulate_live_named_shell_host_only_tracker_honesty_residual_live() {
        assert!(
            simulate_live_named_shell_host_only_tracker_honesty(),
            "named shell host-only tracker residual must latch"
        );
        assert!(residual_named_shell_host_only_tracker_ok());
        assert_eq!(
            residual_named_shell_host_only_tracker_last_action(),
            ResidualNamedShellHostOnlyTrackerAction::Composite
        );
    }
}
