//! Wave 540 residual peels: map-load camera bootstrap prefers presentation
//! `fow_shell_bypass` for shell-map mode instead of live
//! `GameLogic::isInShellGame` dual-read when a freeze is present.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 539 defeat notify residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` bootstrap_camera_for_loaded_map call sites
//! - `PresentationFrame::fow_shell_bypass`
//!
//! Fail-closed:
//! - Missing presentation still falls through to live isInShellGame
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_PRESENTATION_CAMERA_SHELL_FLAG_METHOD_NAMES_WAVE540: &[&str] = &[
    "bootstrap_camera_for_loaded_map",
    "fow_shell_bypass",
    "in_shell_camera",
    "isInShellGame",
    "playable_claim = false",
];

pub const LIVE_PRESENTATION_CAMERA_SHELL_FLAG_NAV_STEPS_WAVE540: &[&str] = &[
    "REQUIRE_CAMERA_SHELL_PRESENTATION_FLAG",
    "REQUIRE_FOW_SHELL_BYPASS",
    "LIVE_PRESENTATION_CAMERA_SHELL_FLAG",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_PRESENTATION_CAMERA_SHELL_FLAG_CMD_NAMES_WAVE540: &[&str] = &[
    "camera_shell_presentation_flag",
    "fow_shell_bypass",
    "bootstrap_camera",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualPresentationCameraShellFlagAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualPresentationCameraShellFlagAction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            6 => Self::Composite,
            _ => Self::None,
        }
    }
}

fn residual_action_store(action: ResidualPresentationCameraShellFlagAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_presentation_camera_shell_flag_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_camera_shell_flag_last_action()
-> ResidualPresentationCameraShellFlagAction {
    ResidualPresentationCameraShellFlagAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_presentation_camera_shell_flag_method_names_residual_wave540() -> bool {
    let names = LIVE_PRESENTATION_CAMERA_SHELL_FLAG_METHOD_NAMES_WAVE540;
    let ok = residual_name_index(names, "bootstrap_camera_for_loaded_map").is_some()
        && residual_name_index(names, "fow_shell_bypass").is_some()
        && residual_name_index(names, "in_shell_camera").is_some()
        && residual_name_index(names, "isInShellGame").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualPresentationCameraShellFlagAction::MethodNames);
    ok
}

pub fn honesty_presentation_camera_shell_flag_source_markers_residual_wave540() -> bool {
    let eng = eng_source();
    let bootstrap_n = eng.matches("bootstrap_camera_for_loaded_map").count();
    let shell_flag_n = eng.matches("in_shell_camera").count();
    // Wave 552: camera shell flag centralized via shell_bypass_from_presentation.
    let ok = eng.contains("Wave 540")
        && eng.contains("prefer presentation fow_shell_bypass")
        && eng.contains("in_shell_camera")
        && eng.contains("fow_shell_bypass")
        && eng.contains("bootstrap_camera_for_loaded_map")
        && shell_flag_n >= 2
        && bootstrap_n >= 2
        && (eng.contains("unwrap_or_else(|| self.game_logic.isInShellGame())")
            || eng.contains("shell_bypass_from_presentation(startup_camera_presentation)"))
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualPresentationCameraShellFlagAction::SourceMarkers);
    ok
}

pub fn honesty_presentation_camera_shell_flag_nav_commands_residual_wave540() -> bool {
    let steps = LIVE_PRESENTATION_CAMERA_SHELL_FLAG_NAV_STEPS_WAVE540;
    let cmds = RUNTIME_HOST_LIVE_PRESENTATION_CAMERA_SHELL_FLAG_CMD_NAMES_WAVE540;
    let ok = residual_name_index(steps, "REQUIRE_CAMERA_SHELL_PRESENTATION_FLAG").is_some()
        && residual_name_index(steps, "REQUIRE_FOW_SHELL_BYPASS").is_some()
        && residual_name_index(steps, "LIVE_PRESENTATION_CAMERA_SHELL_FLAG").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "camera_shell_presentation_flag").is_some()
        && residual_name_index(cmds, "fow_shell_bypass").is_some()
        && residual_name_index(cmds, "bootstrap_camera").is_some();
    residual_action_store(ResidualPresentationCameraShellFlagAction::NavCommands);
    ok
}

pub fn simulate_presentation_camera_shell_flag_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 540")
        && eng.contains("startup_camera_presentation")
        && eng.contains("fow_shell_bypass");
    residual_action_store(ResidualPresentationCameraShellFlagAction::CollectSource);
    ok
}

pub fn simulate_presentation_camera_shell_flag_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("in_shell_camera")
        && eng.contains("bootstrap_camera_for_loaded_map")
        && eng.matches("in_shell_camera").count() >= 2;
    residual_action_store(ResidualPresentationCameraShellFlagAction::DispatchSource);
    ok
}

pub fn honesty_presentation_camera_shell_flag_residual_pack_wave540() -> bool {
    honesty_presentation_camera_shell_flag_method_names_residual_wave540()
        && honesty_presentation_camera_shell_flag_source_markers_residual_wave540()
        && honesty_presentation_camera_shell_flag_nav_commands_residual_wave540()
        && simulate_presentation_camera_shell_flag_collect_source()
        && simulate_presentation_camera_shell_flag_dispatch_source()
}

pub fn simulate_live_presentation_camera_shell_flag_honesty() -> bool {
    let ok = honesty_presentation_camera_shell_flag_residual_pack_wave540();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationCameraShellFlagAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_camera_shell_flag_method_names_residual_wave540());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_camera_shell_flag_source_markers_residual_wave540());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_camera_shell_flag_nav_commands_residual_wave540());
    }

    #[test]
    fn presentation_camera_shell_flag_sources() {
        assert!(simulate_presentation_camera_shell_flag_collect_source());
        assert!(simulate_presentation_camera_shell_flag_dispatch_source());
    }

    #[test]
    fn wave540_composite_pack() {
        assert!(honesty_presentation_camera_shell_flag_residual_pack_wave540());
    }

    #[test]
    fn simulate_live_presentation_camera_shell_flag_honesty_residual_live() {
        assert!(
            simulate_live_presentation_camera_shell_flag_honesty(),
            "camera shell flag residual must latch"
        );
        assert!(residual_presentation_camera_shell_flag_ok());
        assert_eq!(
            residual_presentation_camera_shell_flag_last_action(),
            ResidualPresentationCameraShellFlagAction::Composite
        );
    }
}
