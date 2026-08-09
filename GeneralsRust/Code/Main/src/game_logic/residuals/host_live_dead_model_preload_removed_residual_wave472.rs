//! Wave 472 residual peels: dead engine model preload free helpers removed.
//! - `preload_all_models(&GameLogic)` deleted (unreferenced; dual-read surface)
//! - `load_model_into_graphics_system_blocking` deleted with it
//! - Texture preload path remains (asset-manager driven, no GameLogic)
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 471 free-fn GameLogic-only-seed residual.
//! Architecture residual - Main no longer carries unused GameLogic model preload.
//!
//! Sources (cnc_game_engine.rs):
//! - no fn preload_all_models
//! - no fn load_model_into_graphics_system_blocking
//! - fn ensure_presentation_env_for_hints remains seed free-fn
//!
//! Fail-closed:
//! - Live asset/model load still happens via graphics/asset manager paths
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const DEAD_MODEL_PRELOAD_REMOVED_METHOD_NAMES_WAVE472: &[&str] = &[
    "preload_all_models removed",
    "load_model_into_graphics_system_blocking removed",
    "ensure_presentation_env_for_hints",
    "texture_preload_retained",
    "playable_claim = false",
];

pub const DEAD_MODEL_PRELOAD_REMOVED_SOURCE_MARKERS_WAVE472: &[&str] = &[
    "no fn preload_all_models",
    "no fn load_model_into_graphics_system_blocking",
    "WW3D TEXTURE PRELOAD",
    "fn ensure_presentation_env_for_hints",
];

pub const DEAD_MODEL_PRELOAD_REMOVED_NAV_STEPS_WAVE472: &[&str] = &[
    "ENUMERATE_DEAD_MODEL_PRELOAD",
    "DELETE_PRELOAD_ALL_MODELS",
    "DELETE_BLOCKING_MODEL_LOAD",
    "RETAIN_TEXTURE_PRELOAD",
    "FREE_FN_SURFACE_SHRINKS",
    "NO_GAMELOGIC_MODEL_PRELOAD",
];

pub const RUNTIME_HOST_DEAD_MODEL_PRELOAD_REMOVED_CMD_NAMES_WAVE472: &[&str] = &[
    "click_dead_model_preload_removed_ok_wnd_enum",
    "click_dead_model_preload_removed_ok_wnd_delete",
    "click_dead_model_preload_removed_ok_wnd_texture",
    "click_dead_model_preload_removed_ok_wnd_prepare",
    "click_dead_model_preload_removed_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualDeadModelPreloadRemovedAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    SourceAbsent = 4,
    TextureRetained = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualDeadModelPreloadRemovedAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_dead_model_preload_removed_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_dead_model_preload_removed_last_action() -> ResidualDeadModelPreloadRemovedAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualDeadModelPreloadRemovedAction::MethodNames,
        2 => ResidualDeadModelPreloadRemovedAction::SourceMarkers,
        3 => ResidualDeadModelPreloadRemovedAction::NavCommands,
        4 => ResidualDeadModelPreloadRemovedAction::SourceAbsent,
        5 => ResidualDeadModelPreloadRemovedAction::TextureRetained,
        6 => ResidualDeadModelPreloadRemovedAction::Composite,
        _ => ResidualDeadModelPreloadRemovedAction::Idle,
    }
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

pub fn honesty_dead_model_preload_removed_method_names_residual_wave472() -> bool {
    DEAD_MODEL_PRELOAD_REMOVED_METHOD_NAMES_WAVE472.len() == 5
        && residual_name_index(
            DEAD_MODEL_PRELOAD_REMOVED_METHOD_NAMES_WAVE472,
            "preload_all_models removed",
        ) == Some(0)
        && residual_name_index(
            DEAD_MODEL_PRELOAD_REMOVED_METHOD_NAMES_WAVE472,
            "playable_claim = false",
        ) == Some(4)
}

pub fn honesty_dead_model_preload_removed_source_markers_residual_wave472() -> bool {
    DEAD_MODEL_PRELOAD_REMOVED_SOURCE_MARKERS_WAVE472.len() == 4
        && residual_name_index(
            DEAD_MODEL_PRELOAD_REMOVED_SOURCE_MARKERS_WAVE472,
            "no fn preload_all_models",
        ) == Some(0)
        && residual_name_index(
            DEAD_MODEL_PRELOAD_REMOVED_SOURCE_MARKERS_WAVE472,
            "fn ensure_presentation_env_for_hints",
        ) == Some(3)
}

pub fn honesty_dead_model_preload_removed_nav_commands_residual_wave472() -> bool {
    DEAD_MODEL_PRELOAD_REMOVED_NAV_STEPS_WAVE472.len() == 6
        && residual_name_index(
            DEAD_MODEL_PRELOAD_REMOVED_NAV_STEPS_WAVE472,
            "DELETE_PRELOAD_ALL_MODELS",
        ) == Some(1)
        && residual_name_index(
            DEAD_MODEL_PRELOAD_REMOVED_NAV_STEPS_WAVE472,
            "NO_GAMELOGIC_MODEL_PRELOAD",
        ) == Some(5)
        && RUNTIME_HOST_DEAD_MODEL_PRELOAD_REMOVED_CMD_NAMES_WAVE472.len() == 5
        && residual_name_index(
            RUNTIME_HOST_DEAD_MODEL_PRELOAD_REMOVED_CMD_NAMES_WAVE472,
            "click_dead_model_preload_removed_ok_wnd_prepare",
        ) == Some(3)
}

pub fn simulate_dead_model_preload_removed_source() -> bool {
    let src = cnc_source();
    let ok = !src.contains("fn preload_all_models")
        && !src.contains("fn load_model_into_graphics_system_blocking")
        && !src.contains("PRELOAD: Starting model preloading");
    residual_action_store(ResidualDeadModelPreloadRemovedAction::SourceAbsent);
    ok
}

pub fn simulate_texture_preload_retained_source() -> bool {
    let src = cnc_source();
    let ok = src.contains("WW3D TEXTURE PRELOAD")
        || src.contains("Preload textures from all cached models")
        || src.contains("texture preload");
    // ensure seed free-fn still present
    let ok = ok && src.contains("fn ensure_presentation_env_for_hints");
    residual_action_store(ResidualDeadModelPreloadRemovedAction::TextureRetained);
    ok
}

pub fn honesty_dead_model_preload_removed_residual_pack_wave472() -> bool {
    honesty_dead_model_preload_removed_method_names_residual_wave472()
        && honesty_dead_model_preload_removed_source_markers_residual_wave472()
        && honesty_dead_model_preload_removed_nav_commands_residual_wave472()
        && simulate_dead_model_preload_removed_source()
        && simulate_texture_preload_retained_source()
}

pub fn simulate_live_dead_model_preload_removed_honesty() -> bool {
    let ok = honesty_dead_model_preload_removed_residual_pack_wave472();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualDeadModelPreloadRemovedAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_dead_model_preload_removed_method_names_residual_wave472());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_dead_model_preload_removed_source_markers_residual_wave472());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_dead_model_preload_removed_nav_commands_residual_wave472());
    }

    #[test]
    fn dead_model_preload_removed_sources() {
        assert!(simulate_dead_model_preload_removed_source());
        assert!(simulate_texture_preload_retained_source());
        assert!(!cnc_source().contains("fn preload_all_models"));
        assert!(!cnc_source().contains("fn load_model_into_graphics_system_blocking"));
    }

    #[test]
    fn wave472_composite_pack() {
        assert!(honesty_dead_model_preload_removed_residual_pack_wave472());
    }

    #[test]
    fn simulate_live_dead_model_preload_removed_honesty_residual_live() {
        assert!(
            simulate_live_dead_model_preload_removed_honesty(),
            "dead model preload removed residual must latch"
        );
        assert!(residual_dead_model_preload_removed_ok());
        assert_eq!(
            residual_dead_model_preload_removed_last_action(),
            ResidualDeadModelPreloadRemovedAction::Composite
        );
    }
}
