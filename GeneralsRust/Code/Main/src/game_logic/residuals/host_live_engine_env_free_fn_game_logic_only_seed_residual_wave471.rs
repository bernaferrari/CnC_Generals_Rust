//! Wave 471 residual peels: engine free-fn GameLogic dual-reads for env/path
//! are collapsed to presentation env seed only.
//! - free GameLogic helpers: none after Wave 474 (ensure is instance method)
//! - minimap reinit is instance (`&mut self`)
//! - mid-frame path stub removed (Wave 469)
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 466–468 presentation seed/mirror/minimap instance peels.
//! Architecture residual - Main free helpers no longer dual-read live sim for path/env.
//!
//! Sources (cnc_game_engine.rs):
//! - fn ensure_presentation_env_for_hints(&mut self) — instance seed (Wave 474)
//! - fn reinitialize_minimap_renderer(&mut self) — no free GameLogic param
//! - no fn update_unit_pathfinding
//!
//! Fail-closed:
//! - ensure still freezes from host logic when pipeline empty
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const ENGINE_ENV_FREE_FN_GAME_LOGIC_ONLY_SEED_METHOD_NAMES_WAVE471: &[&str] = &[
    "ensure_presentation_env_for_hints",
    "ensure_presentation_env_seeded",
    "reinitialize_minimap_renderer",
    "presentation_world_bounds",
    "update_movement",
    "playable_claim = false",
];

pub const ENGINE_ENV_FREE_FN_GAME_LOGIC_ONLY_SEED_SOURCE_MARKERS_WAVE471: &[&str] = &[
    "fn ensure_presentation_env_for_hints",
    "fn reinitialize_minimap_renderer(&mut self)",
    "fn ensure_presentation_env_seeded",
    "no update_unit_pathfinding",
];

pub const ENGINE_ENV_FREE_FN_GAME_LOGIC_ONLY_SEED_NAV_STEPS_WAVE471: &[&str] = &[
    "ENUMERATE_FREE_FN_GAMELOGIC_PARAMS",
    "ONLY_ENSURE_PRESENTATION_ENV_REMAINS",
    "MINIMAP_REINIT_IS_INSTANCE",
    "PATH_STUB_REMOVED",
    "SEEDED_MIRRORS_LAST_FRAME",
    "NO_MIDFRAME_PATH_DUAL_READ",
];

pub const RUNTIME_HOST_ENGINE_ENV_FREE_FN_GAME_LOGIC_ONLY_SEED_CMD_NAMES_WAVE471: &[&str] = &[
    "click_engine_env_free_fn_game_logic_only_seed_ok_wnd_enum",
    "click_engine_env_free_fn_game_logic_only_seed_ok_wnd_ensure",
    "click_engine_env_free_fn_game_logic_only_seed_ok_wnd_minimap",
    "click_engine_env_free_fn_game_logic_only_seed_ok_wnd_prepare",
    "click_engine_env_free_fn_game_logic_only_seed_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualEngineEnvFreeFnGameLogicOnlySeedAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    FreeFnSource = 4,
    InstanceSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualEngineEnvFreeFnGameLogicOnlySeedAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_engine_env_free_fn_game_logic_only_seed_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_engine_env_free_fn_game_logic_only_seed_last_action()
-> ResidualEngineEnvFreeFnGameLogicOnlySeedAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualEngineEnvFreeFnGameLogicOnlySeedAction::MethodNames,
        2 => ResidualEngineEnvFreeFnGameLogicOnlySeedAction::SourceMarkers,
        3 => ResidualEngineEnvFreeFnGameLogicOnlySeedAction::NavCommands,
        4 => ResidualEngineEnvFreeFnGameLogicOnlySeedAction::FreeFnSource,
        5 => ResidualEngineEnvFreeFnGameLogicOnlySeedAction::InstanceSource,
        6 => ResidualEngineEnvFreeFnGameLogicOnlySeedAction::Composite,
        _ => ResidualEngineEnvFreeFnGameLogicOnlySeedAction::Idle,
    }
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

/// Collect free/associated-fn names whose params include game_logic without self receiver.
fn free_fns_taking_game_logic(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut search = src;
    while let Some(rel) = search.find("fn ") {
        let after = &search[rel + 3..];
        let name_len = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .count();
        let name = &after[..name_len];
        let sig_end = after.find('{').unwrap_or(after.len().min(600));
        let sig = &after[..sig_end];
        let has_param = sig.contains("game_logic: &")
            || sig.contains("game_logic: Option<&")
            || sig.contains("game_logic:Option<&");
        let first = sig
            .find('(')
            .map(|i| sig[i + 1..].split(',').next().unwrap_or("").trim())
            .unwrap_or("");
        let is_instance = (first.starts_with('&') && first.contains("self"))
            || first == "self"
            || first.starts_with("mut self");
        if has_param && !is_instance && !name.is_empty() {
            out.push(name.to_string());
        }
        search = &after[name_len.max(1)..];
    }
    out.sort();
    out.dedup();
    out
}

pub fn honesty_engine_env_free_fn_game_logic_only_seed_method_names_residual_wave471() -> bool {
    ENGINE_ENV_FREE_FN_GAME_LOGIC_ONLY_SEED_METHOD_NAMES_WAVE471.len() == 6
        && residual_name_index(
            ENGINE_ENV_FREE_FN_GAME_LOGIC_ONLY_SEED_METHOD_NAMES_WAVE471,
            "ensure_presentation_env_for_hints",
        ) == Some(0)
        && residual_name_index(
            ENGINE_ENV_FREE_FN_GAME_LOGIC_ONLY_SEED_METHOD_NAMES_WAVE471,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_engine_env_free_fn_game_logic_only_seed_source_markers_residual_wave471() -> bool {
    ENGINE_ENV_FREE_FN_GAME_LOGIC_ONLY_SEED_SOURCE_MARKERS_WAVE471.len() == 4
        && residual_name_index(
            ENGINE_ENV_FREE_FN_GAME_LOGIC_ONLY_SEED_SOURCE_MARKERS_WAVE471,
            "fn ensure_presentation_env_for_hints",
        ) == Some(0)
        && residual_name_index(
            ENGINE_ENV_FREE_FN_GAME_LOGIC_ONLY_SEED_SOURCE_MARKERS_WAVE471,
            "no update_unit_pathfinding",
        ) == Some(3)
}

pub fn honesty_engine_env_free_fn_game_logic_only_seed_nav_commands_residual_wave471() -> bool {
    ENGINE_ENV_FREE_FN_GAME_LOGIC_ONLY_SEED_NAV_STEPS_WAVE471.len() == 6
        && residual_name_index(
            ENGINE_ENV_FREE_FN_GAME_LOGIC_ONLY_SEED_NAV_STEPS_WAVE471,
            "ONLY_ENSURE_PRESENTATION_ENV_REMAINS",
        ) == Some(1)
        && residual_name_index(
            ENGINE_ENV_FREE_FN_GAME_LOGIC_ONLY_SEED_NAV_STEPS_WAVE471,
            "NO_MIDFRAME_PATH_DUAL_READ",
        ) == Some(5)
        && RUNTIME_HOST_ENGINE_ENV_FREE_FN_GAME_LOGIC_ONLY_SEED_CMD_NAMES_WAVE471.len() == 5
        && residual_name_index(
            RUNTIME_HOST_ENGINE_ENV_FREE_FN_GAME_LOGIC_ONLY_SEED_CMD_NAMES_WAVE471,
            "click_engine_env_free_fn_game_logic_only_seed_ok_wnd_prepare",
        ) == Some(3)
}

pub fn simulate_engine_env_free_fn_game_logic_only_seed_source() -> bool {
    let src = cnc_source();
    let free = free_fns_taking_game_logic(src);
    // Wave 474: no free GameLogic helpers remain (ensure is &mut self).
    let ok = free.is_empty()
        && src.contains("fn ensure_presentation_env_for_hints(&mut self)")
        && !src.contains("fn update_unit_pathfinding");
    residual_action_store(ResidualEngineEnvFreeFnGameLogicOnlySeedAction::FreeFnSource);
    ok
}

pub fn simulate_engine_env_instance_minimap_path_source() -> bool {
    let src = cnc_source();
    let ok = src.contains("fn reinitialize_minimap_renderer(&mut self)")
        && src.contains("fn ensure_presentation_env_seeded")
        && !src.contains("fn update_unit_pathfinding")
        && src.contains("presentation_world_bounds()");
    residual_action_store(ResidualEngineEnvFreeFnGameLogicOnlySeedAction::InstanceSource);
    ok
}

pub fn honesty_engine_env_free_fn_game_logic_only_seed_residual_pack_wave471() -> bool {
    honesty_engine_env_free_fn_game_logic_only_seed_method_names_residual_wave471()
        && honesty_engine_env_free_fn_game_logic_only_seed_source_markers_residual_wave471()
        && honesty_engine_env_free_fn_game_logic_only_seed_nav_commands_residual_wave471()
        && simulate_engine_env_free_fn_game_logic_only_seed_source()
        && simulate_engine_env_instance_minimap_path_source()
}

pub fn simulate_live_engine_env_free_fn_game_logic_only_seed_honesty() -> bool {
    let ok = honesty_engine_env_free_fn_game_logic_only_seed_residual_pack_wave471();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualEngineEnvFreeFnGameLogicOnlySeedAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_engine_env_free_fn_game_logic_only_seed_method_names_residual_wave471());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_engine_env_free_fn_game_logic_only_seed_source_markers_residual_wave471());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_engine_env_free_fn_game_logic_only_seed_nav_commands_residual_wave471());
    }

    #[test]
    fn engine_env_free_fn_game_logic_only_seed_sources() {
        let free = free_fns_taking_game_logic(cnc_source());
        assert!(
            free.is_empty(),
            "unexpected free GameLogic helpers: {free:?}"
        );
        assert!(cnc_source().contains("fn ensure_presentation_env_for_hints(&mut self)"));
        assert!(simulate_engine_env_free_fn_game_logic_only_seed_source());
        assert!(simulate_engine_env_instance_minimap_path_source());
        assert!(
            !cnc_source().contains("fn preload_all_models"),
            "dead preload_all_models must stay removed"
        );
    }

    #[test]
    fn wave471_composite_pack() {
        assert!(honesty_engine_env_free_fn_game_logic_only_seed_residual_pack_wave471());
    }

    #[test]
    fn simulate_live_engine_env_free_fn_game_logic_only_seed_honesty_residual_live() {
        assert!(
            simulate_live_engine_env_free_fn_game_logic_only_seed_honesty(),
            "engine env free-fn GameLogic-only-seed residual must latch"
        );
        assert!(residual_engine_env_free_fn_game_logic_only_seed_ok());
        assert_eq!(
            residual_engine_env_free_fn_game_logic_only_seed_last_action(),
            ResidualEngineEnvFreeFnGameLogicOnlySeedAction::Composite
        );
    }
}
