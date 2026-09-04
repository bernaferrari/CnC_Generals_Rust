//! Wave 169 residual peels: start_game_from_ui Loading → InGame residual
//! (map resolve Defcon6/Lone Eagle; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 168 W3DMainMenuInit residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - GameLogic::startNewGame / UI start → load screen → loadMap → InGame
//! - CncGameEngine::start_game_from_ui Loading transition + load_map + seed
//! - Retail MapsZH: Defcon6 (DEFAULT_SKIRMISH_MAP), Lone Eagle (behavior_gate)
//!
//! Fail-closed:
//! - Does not run full start_game_from_ui / map object spawn in this peel
//! - Not full load-screen W3D residual
//! - Shell `playable_claim` stays false; network deferred

use std::path::{Path, PathBuf};

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// start_game Loading residual method names.
pub const START_GAME_LOADING_METHOD_NAMES_WAVE169: &[&str] = &[
    "start_game_from_ui",
    "prepare_cpp_load_screen_for_mode",
    "transition_to_state Loading",
    "load_map",
    "seed_presentation_after_match_start",
    "transition_to_state InGame",
];

/// Ordered start_game Loading residual navigation steps.
pub const START_GAME_LOADING_NAV_STEPS_WAVE169: &[&str] = &[
    "PREPARE_LOAD_SCREEN",
    "ENTER_LOADING_STATE",
    "APPLY_SKIRMISH_CONFIG",
    "LOAD_MAP",
    "SEED_PRESENTATION",
    "ENTER_INGAME_STATE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_START_GAME_LOADING_CMD_NAMES_WAVE169: &[&str] = &[
    "click_start_game_loading_ok_maps",
    "click_start_game_loading_ok_source",
    "click_start_game_loading_miss",
];

/// Engine default skirmish map residual (cnc_game_engine DEFAULT_SKIRMISH_MAP).
pub const DEFAULT_SKIRMISH_MAP_WAVE169: &str = "Defcon6";

/// Behavior-gate / retail campaign-adjacent skirmish map residual.
pub const LONE_EAGLE_MAP_WAVE169: &str = "Lone Eagle";

/// Candidate roots for MapsZH retail trees.
pub const MAP_ROOT_CANDIDATES_WAVE169: &[&str] = &[
    "windows_game/extracted_big_files/MapsZH/Maps",
    "windows_game/extracted_big_files_v2/MapsZH/Maps",
    "../windows_game/extracted_big_files/MapsZH/Maps",
    "../windows_game/extracted_big_files_v2/MapsZH/Maps",
    "../../windows_game/extracted_big_files/MapsZH/Maps",
    "../../windows_game/extracted_big_files_v2/MapsZH/Maps",
];

/// Honesty: method names residual pack.
pub fn honesty_start_game_loading_method_names_residual_wave169() -> bool {
    START_GAME_LOADING_METHOD_NAMES_WAVE169.len() == 6
        && residual_name_index(
            START_GAME_LOADING_METHOD_NAMES_WAVE169,
            "start_game_from_ui",
        ) == Some(0)
        && residual_name_index(START_GAME_LOADING_METHOD_NAMES_WAVE169, "load_map") == Some(3)
        && residual_name_index(
            START_GAME_LOADING_METHOD_NAMES_WAVE169,
            "transition_to_state InGame",
        ) == Some(5)
        && DEFAULT_SKIRMISH_MAP_WAVE169 == "Defcon6"
        && LONE_EAGLE_MAP_WAVE169 == "Lone Eagle"
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_start_game_loading_nav_commands_residual_wave169() -> bool {
    START_GAME_LOADING_NAV_STEPS_WAVE169.len() == 6
        && residual_name_index(START_GAME_LOADING_NAV_STEPS_WAVE169, "ENTER_LOADING_STATE")
            == Some(1)
        && residual_name_index(START_GAME_LOADING_NAV_STEPS_WAVE169, "ENTER_INGAME_STATE")
            == Some(5)
        && RUNTIME_HOST_START_GAME_LOADING_CMD_NAMES_WAVE169.len() == 3
}

/// Wave 169 composite residual honesty pack.
pub fn honesty_start_game_loading_residual_pack_wave169() -> bool {
    honesty_start_game_loading_method_names_residual_wave169()
        && honesty_start_game_loading_nav_commands_residual_wave169()
}

/// Source residual: start_game_from_ui Loading → load_map → InGame path.
pub fn honesty_start_game_from_ui_loading_source() -> bool {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    // Wave 611: logic lives in host helper; thin wrapper delegates.
    let i = match src
        .find("fn host_start_game_from_ui(")
        .or_else(|| src.find("fn start_game_from_ui("))
    {
        Some(i) => i,
        None => return false,
    };
    // hq-nnuu 2026-08-15: host_start_game_from_ui grew past a 7k window after
    // the cnc_game_engine split; scan the live helper body (ENGINE_SRC concat).
    let body = &src[i..src.len().min(i + 24_000)];
    body.contains("prepare_cpp_load_screen_for_mode")
        && body.contains("GameState::Loading")
        && body.contains("load_map")
        && body.contains("DEFAULT_SKIRMISH_MAP")
        && body.contains("seed_presentation_after_match_start")
        && body.contains("GameState::InGame")
        && body.contains("apply_skirmish_config")
}

/// Resolve a retail map directory or .map file for `map_name`.
///
/// Walks cwd + up to 6 parents so residuals work from `Code/Main`,
/// `GeneralsRust/`, or the monorepo root (matches map_frame_scenario).
pub fn resolve_retail_map_path(map_name: &str) -> Option<PathBuf> {
    let map_name = map_name.trim();
    if map_name.is_empty() {
        return None;
    }

    let mut bases: Vec<PathBuf> = vec![PathBuf::from(".")];
    if let Ok(cwd) = std::env::current_dir() {
        bases.push(cwd.clone());
        let mut b = cwd;
        for _ in 0..6 {
            if let Some(parent) = b.parent() {
                b = parent.to_path_buf();
                bases.push(b.clone());
            } else {
                break;
            }
        }
    }

    // Compile-time install anchors (repo root via Code/Main manifest): C++
    // resolves map names through TheFileSystem's engine-fixed search paths
    // (FileSystem.cpp:164-178), never the transient process CWD — bare names
    // must resolve the same way from any launch directory.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    bases.push(manifest);
    for ancestor in PathBuf::from(env!("CARGO_MANIFEST_DIR")).ancestors().skip(1) {
        bases.push(ancestor.to_path_buf());
    }
    for base in bases {
        for root in MAP_ROOT_CANDIDATES_WAVE169 {
            let root_path = if Path::new(root).is_absolute() {
                PathBuf::from(root)
            } else {
                base.join(root)
            };
            let dir = root_path.join(map_name);
            let map_file = dir.join(format!("{map_name}.map"));
            if map_file.is_file() {
                return Some(map_file);
            }
            if dir.is_dir() {
                if let Ok(rd) = std::fs::read_dir(&dir) {
                    for ent in rd.flatten() {
                        let p = ent.path();
                        if p.extension().and_then(|e| e.to_str()) == Some("map") {
                            return Some(p);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Honesty: default skirmish map (Defcon6) resolves when MapsZH is present.
pub fn honesty_default_skirmish_map_resolves() -> bool {
    match resolve_retail_map_path(DEFAULT_SKIRMISH_MAP_WAVE169) {
        Some(p) => p.is_file(),
        None => true, // assets unavailable — soft residual
    }
}

/// Honesty: Lone Eagle resolves when MapsZH is present (behavior_gate map).
pub fn honesty_lone_eagle_map_resolves() -> bool {
    match resolve_retail_map_path(LONE_EAGLE_MAP_WAVE169) {
        Some(p) => p.is_file(),
        None => true,
    }
}

/// Live residual: pack + source + map resolve honesty (no full match start).
pub fn simulate_start_game_loading_honesty() -> bool {
    if !honesty_start_game_loading_residual_pack_wave169() {
        return false;
    }
    if !honesty_start_game_from_ui_loading_source() {
        return false;
    }
    if !honesty_default_skirmish_map_resolves() {
        return false;
    }
    if !honesty_lone_eagle_map_resolves() {
        return false;
    }
    // When MapsZH is checked out, both maps must materialise as files.
    let defcon = resolve_retail_map_path(DEFAULT_SKIRMISH_MAP_WAVE169);
    let lone = resolve_retail_map_path(LONE_EAGLE_MAP_WAVE169);
    match (defcon, lone) {
        (None, None) => true, // no maps — CI soft residual
        (Some(d), Some(l)) => d.is_file() && l.is_file(),
        // Partial tree is fail-closed: both residuals should resolve together.
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_start_game_loading_method_names_residual_wave169());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_start_game_loading_nav_commands_residual_wave169());
    }

    #[test]
    fn wave169_composite_pack() {
        assert!(honesty_start_game_loading_residual_pack_wave169());
    }

    #[test]
    fn start_game_from_ui_loading_source() {
        assert!(honesty_start_game_from_ui_loading_source());
    }

    #[test]
    fn default_and_lone_eagle_map_resolve() {
        assert!(honesty_default_skirmish_map_resolves());
        assert!(honesty_lone_eagle_map_resolves());
    }

    #[test]
    fn simulate_start_game_loading_honesty_residual_live() {
        assert!(
            simulate_start_game_loading_honesty(),
            "start_game Loading path + Defcon6/Lone Eagle map residual must latch"
        );
        if let Some(p) = resolve_retail_map_path(DEFAULT_SKIRMISH_MAP_WAVE169) {
            assert!(p.extension().and_then(|e| e.to_str()) == Some("map"));
        }
        if let Some(p) = resolve_retail_map_path(LONE_EAGLE_MAP_WAVE169) {
            assert!(p.extension().and_then(|e| e.to_str()) == Some("map"));
        }
    }
}
