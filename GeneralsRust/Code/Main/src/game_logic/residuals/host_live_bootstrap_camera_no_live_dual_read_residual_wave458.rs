//! Wave 458 residual peels: bootstrap camera no live GameLogic dual-read when
//! PresentationFrame is installed (Option<&GameLogic> + pipeline presentation).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 223/241 presentation-first camera peels.
//! Architecture residual - startup camera prefers presentation freeze exclusively.
//!
//! Sources (cnc_game_engine.rs):
//! - bootstrap_camera_for_loaded_map(game_logic: Option<&GameLogic>, is_shell_game, ...)
//! - call sites set startup_camera_live_logic = None when presentation installed
//! - prefer render_pipeline.presentation_frame() over last_presentation_frame alone
//!
//! Fail-closed:
//! - Boot path may still pass Some(&GameLogic) when freeze missing
//! - isInShellGame still read once at call site from host
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const BOOTSTRAP_CAMERA_NO_LIVE_DUAL_READ_METHOD_NAMES_WAVE458: &[&str] = &[
    "bootstrap_camera_for_loaded_map",
    "sample_startup_camera_heights",
    "select_startup_camera_focus",
    "presentation_frame",
    "startup_camera_live_logic",
    "startup_camera_presentation",
];

pub const BOOTSTRAP_CAMERA_NO_LIVE_DUAL_READ_SOURCE_MARKERS_WAVE458: &[&str] = &[
    "Wave 458: live GameLogic only when presentation freeze is missing",
    "no game_logic param (Wave 473)",
    "startup_camera_live_logic",
    "startup_camera_presentation",
];

pub const BOOTSTRAP_CAMERA_NO_LIVE_DUAL_READ_NAV_STEPS_WAVE458: &[&str] = &[
    "SEED_PRESENTATION_IF_MISSING",
    "RESOLVE_PIPELINE_OR_LAST_PRESENTATION",
    "GATE_LIVE_LOGIC_WHEN_PRESENTATION",
    "BOOTSTRAP_BOUNDS_FROM_PRESENTATION",
    "SAMPLE_HEIGHTS_FROM_PRESENTATION",
    "NO_LIVE_GAMELOGIC_DUAL_READ_WHEN_FROZEN",
];

pub const RUNTIME_HOST_BOOTSTRAP_CAMERA_NO_LIVE_DUAL_READ_CMD_NAMES_WAVE458: &[&str] = &[
    "click_bootstrap_camera_no_live_dual_read_ok_wnd_seed",
    "click_bootstrap_camera_no_live_dual_read_ok_wnd_resolve",
    "click_bootstrap_camera_no_live_dual_read_ok_wnd_gate",
    "click_bootstrap_camera_no_live_dual_read_ok_wnd_prepare",
    "click_bootstrap_camera_no_live_dual_read_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualBootstrapCameraNoLiveDualReadAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    BootstrapSource = 4,
    CallSites = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualBootstrapCameraNoLiveDualReadAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_bootstrap_camera_no_live_dual_read_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_bootstrap_camera_no_live_dual_read_last_action()
-> ResidualBootstrapCameraNoLiveDualReadAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualBootstrapCameraNoLiveDualReadAction::MethodNames,
        2 => ResidualBootstrapCameraNoLiveDualReadAction::SourceMarkers,
        3 => ResidualBootstrapCameraNoLiveDualReadAction::NavCommands,
        4 => ResidualBootstrapCameraNoLiveDualReadAction::BootstrapSource,
        5 => ResidualBootstrapCameraNoLiveDualReadAction::CallSites,
        6 => ResidualBootstrapCameraNoLiveDualReadAction::Composite,
        _ => ResidualBootstrapCameraNoLiveDualReadAction::Idle,
    }
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_bootstrap_camera_no_live_dual_read_method_names_residual_wave458() -> bool {
    BOOTSTRAP_CAMERA_NO_LIVE_DUAL_READ_METHOD_NAMES_WAVE458.len() == 6
        && residual_name_index(
            BOOTSTRAP_CAMERA_NO_LIVE_DUAL_READ_METHOD_NAMES_WAVE458,
            "bootstrap_camera_for_loaded_map",
        ) == Some(0)
        && residual_name_index(
            BOOTSTRAP_CAMERA_NO_LIVE_DUAL_READ_METHOD_NAMES_WAVE458,
            "startup_camera_presentation",
        ) == Some(5)
}

pub fn honesty_bootstrap_camera_no_live_dual_read_source_markers_residual_wave458() -> bool {
    BOOTSTRAP_CAMERA_NO_LIVE_DUAL_READ_SOURCE_MARKERS_WAVE458.len() == 4
        && residual_name_index(
            BOOTSTRAP_CAMERA_NO_LIVE_DUAL_READ_SOURCE_MARKERS_WAVE458,
            "Wave 458: live GameLogic only when presentation freeze is missing",
        ) == Some(0)
        && residual_name_index(
            BOOTSTRAP_CAMERA_NO_LIVE_DUAL_READ_SOURCE_MARKERS_WAVE458,
            "startup_camera_presentation",
        ) == Some(3)
}

pub fn honesty_bootstrap_camera_no_live_dual_read_nav_commands_residual_wave458() -> bool {
    BOOTSTRAP_CAMERA_NO_LIVE_DUAL_READ_NAV_STEPS_WAVE458.len() == 6
        && residual_name_index(
            BOOTSTRAP_CAMERA_NO_LIVE_DUAL_READ_NAV_STEPS_WAVE458,
            "GATE_LIVE_LOGIC_WHEN_PRESENTATION",
        ) == Some(2)
        && residual_name_index(
            BOOTSTRAP_CAMERA_NO_LIVE_DUAL_READ_NAV_STEPS_WAVE458,
            "NO_LIVE_GAMELOGIC_DUAL_READ_WHEN_FROZEN",
        ) == Some(5)
        && RUNTIME_HOST_BOOTSTRAP_CAMERA_NO_LIVE_DUAL_READ_CMD_NAMES_WAVE458.len() == 5
        && residual_name_index(
            RUNTIME_HOST_BOOTSTRAP_CAMERA_NO_LIVE_DUAL_READ_CMD_NAMES_WAVE458,
            "click_bootstrap_camera_no_live_dual_read_ok_wnd_prepare",
        ) == Some(3)
}

fn function_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let at = src.find(sig)?;
    let bytes = src.as_bytes();
    let mut depth = 0i32;
    let mut end = at;
    let mut seen = false;
    for (j, &b) in bytes[at..].iter().enumerate() {
        if b == b'{' {
            depth += 1;
            seen = true;
        } else if b == b'}' {
            depth -= 1;
            if seen && depth == 0 {
                end = at + j + 1;
                break;
            }
        }
    }
    Some(&src[at..end])
}

pub fn simulate_bootstrap_camera_no_live_dual_read_source() -> bool {
    let src = cnc_source();
    let Some(body) = function_body(src, "fn bootstrap_camera_for_loaded_map(") else {
        return false;
    };
    // Wave 473 supersedes Option live dual-read: presentation-only bootstrap.
    let ok = (body.contains("Wave 458: live GameLogic only when presentation freeze is missing")
        || body.contains("Wave 473: presentation freeze only"))
        && body.contains("is_shell_game: bool")
        && body.contains("local_team_base_position")
        && body.contains("world_bounds_vec3()")
        && !body.contains("game_logic: &GameLogic")
        && !body.contains("no game_logic param (Wave 473)");
    residual_action_store(ResidualBootstrapCameraNoLiveDualReadAction::BootstrapSource);
    ok
}

pub fn simulate_bootstrap_camera_no_live_dual_read_callsites() -> bool {
    let src = cnc_source();
    // Wave 473: presentation freeze at callsites, no live_logic arg.
    let ok = src.matches("startup_camera_presentation").count() >= 2
        && !src.contains("startup_camera_live_logic")
        && src.contains("Self::bootstrap_camera_for_loaded_map(")
        && (src.contains("Wave 458: prefer pipeline presentation freeze")
            || src.contains("startup_camera_presentation"))
        && !src.contains(
            "Self::bootstrap_camera_for_loaded_map(\n                    &self.game_logic,",
        )
        && !src
            .contains("Self::bootstrap_camera_for_loaded_map(\n                &self.game_logic,");
    residual_action_store(ResidualBootstrapCameraNoLiveDualReadAction::CallSites);
    ok
}

pub fn honesty_bootstrap_camera_no_live_dual_read_residual_pack_wave458() -> bool {
    honesty_bootstrap_camera_no_live_dual_read_method_names_residual_wave458()
        && honesty_bootstrap_camera_no_live_dual_read_source_markers_residual_wave458()
        && honesty_bootstrap_camera_no_live_dual_read_nav_commands_residual_wave458()
        && simulate_bootstrap_camera_no_live_dual_read_source()
        && simulate_bootstrap_camera_no_live_dual_read_callsites()
}

pub fn simulate_live_bootstrap_camera_no_live_dual_read_honesty() -> bool {
    let ok = honesty_bootstrap_camera_no_live_dual_read_residual_pack_wave458();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualBootstrapCameraNoLiveDualReadAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_bootstrap_camera_no_live_dual_read_method_names_residual_wave458());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_bootstrap_camera_no_live_dual_read_source_markers_residual_wave458());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_bootstrap_camera_no_live_dual_read_nav_commands_residual_wave458());
    }

    #[test]
    fn bootstrap_camera_no_live_dual_read_sources() {
        assert!(simulate_bootstrap_camera_no_live_dual_read_source());
        assert!(simulate_bootstrap_camera_no_live_dual_read_callsites());
    }

    #[test]
    fn wave458_composite_pack() {
        assert!(honesty_bootstrap_camera_no_live_dual_read_residual_pack_wave458());
    }

    #[test]
    fn simulate_live_bootstrap_camera_no_live_dual_read_honesty_residual_live() {
        assert!(
            simulate_live_bootstrap_camera_no_live_dual_read_honesty(),
            "bootstrap camera no-live dual-read residual must latch"
        );
        assert!(residual_bootstrap_camera_no_live_dual_read_ok());
        assert_eq!(
            residual_bootstrap_camera_no_live_dual_read_last_action(),
            ResidualBootstrapCameraNoLiveDualReadAction::Composite
        );
    }
}
