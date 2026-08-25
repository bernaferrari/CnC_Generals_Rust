//! Wave 473 residual peels: camera bootstrap/heights are presentation-only.
//! - `bootstrap_camera_for_loaded_map` drops Option<&GameLogic>
//! - `sample_startup_camera_heights` drops Option<&GameLogic>
//! - call sites pass presentation freeze only (no startup_camera_live_logic)
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 458 Option dual-read peel (superseded).
//! Architecture residual - camera free helpers no longer dual-read live sim.
//!
//! Sources (cnc_game_engine.rs):
//! - fn bootstrap_camera_for_loaded_map(... presentation ...)
//! - fn sample_startup_camera_heights(... presentation ...)
//! - no game_logic param on either helper
//!
//! Fail-closed:
//! - Missing presentation uses default bounds/fallback heights
//! - ensure_presentation_env_for_hints remains sole free &GameLogic helper
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const CAMERA_BOOTSTRAP_PRESENTATION_ONLY_METHOD_NAMES_WAVE473: &[&str] = &[
    "bootstrap_camera_for_loaded_map",
    "sample_startup_camera_heights",
    "startup_camera_presentation",
    "world_bounds_vec3",
    "sample_height",
    "playable_claim = false",
];

pub const CAMERA_BOOTSTRAP_PRESENTATION_ONLY_SOURCE_MARKERS_WAVE473: &[&str] = &[
    "Wave 473: presentation freeze only",
    "Wave 473: presentation height grid only",
    "no game_logic param bootstrap",
    "no startup_camera_live_logic",
];

pub const CAMERA_BOOTSTRAP_PRESENTATION_ONLY_NAV_STEPS_WAVE473: &[&str] = &[
    "DROP_BOOTSTRAP_GAMELOGIC_PARAM",
    "DROP_SAMPLE_HEIGHTS_GAMELOGIC_PARAM",
    "CALLSITE_PRESENTATION_ONLY",
    "DEFAULT_BOUNDS_FALLBACK",
    "ENSURE_SEED_REMAINS",
    "NO_CAMERA_LIVE_DUAL_READ",
];

pub const RUNTIME_HOST_CAMERA_BOOTSTRAP_PRESENTATION_ONLY_CMD_NAMES_WAVE473: &[&str] = &[
    "click_camera_bootstrap_presentation_only_ok_wnd_drop",
    "click_camera_bootstrap_presentation_only_ok_wnd_sample",
    "click_camera_bootstrap_presentation_only_ok_wnd_callsite",
    "click_camera_bootstrap_presentation_only_ok_wnd_prepare",
    "click_camera_bootstrap_presentation_only_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualCameraBootstrapPresentationOnlyAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    BootstrapSource = 4,
    SampleSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualCameraBootstrapPresentationOnlyAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_camera_bootstrap_presentation_only_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_camera_bootstrap_presentation_only_last_action()
-> ResidualCameraBootstrapPresentationOnlyAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualCameraBootstrapPresentationOnlyAction::MethodNames,
        2 => ResidualCameraBootstrapPresentationOnlyAction::SourceMarkers,
        3 => ResidualCameraBootstrapPresentationOnlyAction::NavCommands,
        4 => ResidualCameraBootstrapPresentationOnlyAction::BootstrapSource,
        5 => ResidualCameraBootstrapPresentationOnlyAction::SampleSource,
        6 => ResidualCameraBootstrapPresentationOnlyAction::Composite,
        _ => ResidualCameraBootstrapPresentationOnlyAction::Idle,
    }
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
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

pub fn honesty_camera_bootstrap_presentation_only_method_names_residual_wave473() -> bool {
    CAMERA_BOOTSTRAP_PRESENTATION_ONLY_METHOD_NAMES_WAVE473.len() == 6
        && residual_name_index(
            CAMERA_BOOTSTRAP_PRESENTATION_ONLY_METHOD_NAMES_WAVE473,
            "bootstrap_camera_for_loaded_map",
        ) == Some(0)
        && residual_name_index(
            CAMERA_BOOTSTRAP_PRESENTATION_ONLY_METHOD_NAMES_WAVE473,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_camera_bootstrap_presentation_only_source_markers_residual_wave473() -> bool {
    CAMERA_BOOTSTRAP_PRESENTATION_ONLY_SOURCE_MARKERS_WAVE473.len() == 4
        && residual_name_index(
            CAMERA_BOOTSTRAP_PRESENTATION_ONLY_SOURCE_MARKERS_WAVE473,
            "Wave 473: presentation freeze only",
        ) == Some(0)
        && residual_name_index(
            CAMERA_BOOTSTRAP_PRESENTATION_ONLY_SOURCE_MARKERS_WAVE473,
            "no startup_camera_live_logic",
        ) == Some(3)
}

pub fn honesty_camera_bootstrap_presentation_only_nav_commands_residual_wave473() -> bool {
    CAMERA_BOOTSTRAP_PRESENTATION_ONLY_NAV_STEPS_WAVE473.len() == 6
        && residual_name_index(
            CAMERA_BOOTSTRAP_PRESENTATION_ONLY_NAV_STEPS_WAVE473,
            "DROP_BOOTSTRAP_GAMELOGIC_PARAM",
        ) == Some(0)
        && residual_name_index(
            CAMERA_BOOTSTRAP_PRESENTATION_ONLY_NAV_STEPS_WAVE473,
            "NO_CAMERA_LIVE_DUAL_READ",
        ) == Some(5)
        && RUNTIME_HOST_CAMERA_BOOTSTRAP_PRESENTATION_ONLY_CMD_NAMES_WAVE473.len() == 5
        && residual_name_index(
            RUNTIME_HOST_CAMERA_BOOTSTRAP_PRESENTATION_ONLY_CMD_NAMES_WAVE473,
            "click_camera_bootstrap_presentation_only_ok_wnd_prepare",
        ) == Some(3)
}

pub fn simulate_camera_bootstrap_presentation_only_source() -> bool {
    let src = cnc_source();
    let Some(boot) = function_body(src, "fn bootstrap_camera_for_loaded_map(") else {
        return false;
    };
    let ok = boot.contains("Wave 473: presentation freeze only")
        && boot.contains("presentation: Option")
        && !boot.contains("game_logic")
        && boot.contains("world_bounds_vec3()")
        && !src.contains("startup_camera_live_logic");
    residual_action_store(ResidualCameraBootstrapPresentationOnlyAction::BootstrapSource);
    ok
}

pub fn simulate_camera_sample_heights_presentation_only_source() -> bool {
    let src = cnc_source();
    let Some(sample) = function_body(src, "fn sample_startup_camera_heights(") else {
        return false;
    };
    let ok = sample.contains("Wave 473: presentation height grid only")
        && sample.contains("sample_height")
        && !sample.contains("game_logic")
        && src.contains("fn ensure_presentation_env_for_hints");
    residual_action_store(ResidualCameraBootstrapPresentationOnlyAction::SampleSource);
    ok
}

pub fn honesty_camera_bootstrap_presentation_only_residual_pack_wave473() -> bool {
    honesty_camera_bootstrap_presentation_only_method_names_residual_wave473()
        && honesty_camera_bootstrap_presentation_only_source_markers_residual_wave473()
        && honesty_camera_bootstrap_presentation_only_nav_commands_residual_wave473()
        && simulate_camera_bootstrap_presentation_only_source()
        && simulate_camera_sample_heights_presentation_only_source()
}

pub fn simulate_live_camera_bootstrap_presentation_only_honesty() -> bool {
    let ok = honesty_camera_bootstrap_presentation_only_residual_pack_wave473();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualCameraBootstrapPresentationOnlyAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_camera_bootstrap_presentation_only_method_names_residual_wave473());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_camera_bootstrap_presentation_only_source_markers_residual_wave473());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_camera_bootstrap_presentation_only_nav_commands_residual_wave473());
    }

    #[test]
    fn camera_bootstrap_presentation_only_sources() {
        assert!(simulate_camera_bootstrap_presentation_only_source());
        assert!(simulate_camera_sample_heights_presentation_only_source());
        let src = cnc_source();
        assert!(!src.contains("startup_camera_live_logic"));
        let boot = function_body(src, "fn bootstrap_camera_for_loaded_map(").unwrap();
        assert!(!boot.contains("game_logic"));
        let sample = function_body(src, "fn sample_startup_camera_heights(").unwrap();
        assert!(!sample.contains("game_logic"));
    }

    #[test]
    fn wave473_composite_pack() {
        assert!(honesty_camera_bootstrap_presentation_only_residual_pack_wave473());
    }

    #[test]
    fn simulate_live_camera_bootstrap_presentation_only_honesty_residual_live() {
        assert!(
            simulate_live_camera_bootstrap_presentation_only_honesty(),
            "camera bootstrap presentation-only residual must latch"
        );
        assert!(residual_camera_bootstrap_presentation_only_ok());
        assert_eq!(
            residual_camera_bootstrap_presentation_only_last_action(),
            ResidualCameraBootstrapPresentationOnlyAction::Composite
        );
    }
}
