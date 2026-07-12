//! Production-linked map load + stable logic-frame advance gate helpers.
//!
//! This is deliberately small and fixture-friendly: it drives the real Main
//! `GameLogic::load_map` / `GameLogic::update` entry points rather than a mock
//! GameState. When retail maps are absent, callers can still exercise the pure
//! frame-advance path with a synthetic world.

use crate::game_logic::script_loader::find_map_file;
use crate::game_logic::{GameLogic, GameMode};
use std::path::{Path, PathBuf};

/// Default number of logic frames to advance for the gate scenario.
pub const DEFAULT_MAP_FRAME_ADVANCE: u32 = 30;

/// Candidate retail/dev map identities tried when no explicit map is supplied.
pub const DEFAULT_MAP_CANDIDATES: &[&str] = &[
    "Maps/ShellMapMD/ShellMapMD.map",
    "Maps\\ShellMapMD\\ShellMapMD.map",
    "ShellMapMD",
    "Maps/Alpine Assault/Alpine Assault.map",
    "Maps/Tornado Alley/Tornado Alley.map",
    "windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
    "windows_game/extracted_big_files/MapsZH/Maps/Dark Night/Dark Night.map",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapFrameScenarioResult {
    pub map_identity: String,
    pub map_resolved: Option<PathBuf>,
    pub map_loaded: bool,
    pub frames_requested: u32,
    pub frames_advanced: u32,
    pub frame_before: u32,
    pub frame_after: u32,
    pub object_count: usize,
    pub status: MapFrameStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapFrameStatus {
    /// Map resolved, loaded, and frames advanced without panic.
    Success,
    /// No map assets available; frame advance still exercised on an empty world.
    AssetsUnavailable,
    /// Map path resolved but `load_map` returned false.
    LoadFailed,
}

impl MapFrameStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::AssetsUnavailable => "assets_unavailable",
            Self::LoadFailed => "load_failed",
        }
    }
}

/// Resolve a candidate against cwd and parent dirs (repo root layouts).
///
/// Walks up to 5 parents so gates run from `Code/Main`, `GeneralsRust/`, or
/// the monorepo root can all see `windows_game/...` at the repo root.
fn resolve_path_candidate(candidate: &str) -> Option<PathBuf> {
    let direct = Path::new(candidate);
    if direct.is_file() {
        return Some(direct.to_path_buf());
    }
    if let Ok(cwd) = std::env::current_dir() {
        let mut base = cwd.clone();
        for _ in 0..6 {
            let candidates = [
                base.join(candidate),
                base.join("windows_game")
                    .join("extracted_big_files")
                    .join("MapsZH")
                    .join("Maps")
                    .join(candidate),
                base.join("windows_game")
                    .join("extracted_big_files_v2")
                    .join("MapsZH")
                    .join("Maps")
                    .join(candidate),
            ];
            for path in candidates {
                if path.is_file() {
                    return Some(path);
                }
            }
            match base.parent() {
                Some(parent) => base = parent.to_path_buf(),
                None => break,
            }
        }
    }
    None
}

/// Resolve the first existing map from candidates (absolute paths or find_map_file).
pub fn resolve_first_map(candidates: &[&str]) -> Option<(String, PathBuf)> {
    for candidate in candidates {
        if let Some(path) = resolve_path_candidate(candidate) {
            return Some((candidate.to_string(), path));
        }
        if let Some(found) = find_map_file(candidate) {
            return Some((candidate.to_string(), found));
        }
    }
    None
}

/// Load a map through the production host path and advance `frames` logic updates.
///
/// When `map_name` is `None`, tries [`DEFAULT_MAP_CANDIDATES`]. If no map is found,
/// still advances frames on a skirmish world so the update path is covered and the
/// status reports `AssetsUnavailable` honestly.
pub fn run_map_frame_scenario(map_name: Option<&str>, frames: u32) -> MapFrameScenarioResult {
    let frames = frames.max(1);
    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::Skirmish);

    let resolved = match map_name {
        Some(name) => {
            if let Some(path) = resolve_path_candidate(name) {
                Some((name.to_string(), path))
            } else {
                find_map_file(name).map(|p| (name.to_string(), p))
            }
        }
        None => resolve_first_map(DEFAULT_MAP_CANDIDATES),
    };

    let (map_identity, map_resolved, map_loaded, status_seed) = match resolved {
        Some((identity, path)) => {
            let loaded = logic.load_map(path.to_str().unwrap_or(identity.as_str()))
                || logic.load_map(&identity);
            let status = if loaded {
                MapFrameStatus::Success
            } else {
                MapFrameStatus::LoadFailed
            };
            (identity, Some(path), loaded, status)
        }
        None => (
            map_name.unwrap_or("<none>").to_string(),
            None,
            false,
            MapFrameStatus::AssetsUnavailable,
        ),
    };

    let frame_before = logic.get_frame();
    for _ in 0..frames {
        logic.update();
    }
    let frame_after = logic.get_frame();
    let frames_advanced = frame_after.saturating_sub(frame_before);

    // Prefer Success only when map actually loaded; otherwise keep seed status.
    let status = if map_loaded && frames_advanced > 0 {
        MapFrameStatus::Success
    } else if map_loaded && frames_advanced == 0 {
        // Map loaded but frame counter did not move — still report load success
        // with zero advances so callers can assert.
        MapFrameStatus::Success
    } else {
        status_seed
    };

    MapFrameScenarioResult {
        map_identity,
        map_resolved,
        map_loaded,
        frames_requested: frames,
        frames_advanced,
        frame_before,
        frame_after,
        object_count: logic.get_objects().len(),
        status,
    }
}

/// Format a single-line machine-readable summary for gate logs.
pub fn format_map_frame_report(result: &MapFrameScenarioResult) -> String {
    format!(
        "map_identity={} status={} map_loaded={} frames_requested={} frames_advanced={} frame_before={} frame_after={} object_count={} resolved={}",
        result.map_identity,
        result.status.as_str(),
        result.map_loaded,
        result.frames_requested,
        result.frames_advanced,
        result.frame_before,
        result.frame_after,
        result.object_count,
        result
            .map_resolved
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "-".to_string()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advances_stable_frames_without_map_assets() {
        // Drive the production update path even when maps are absent.
        let result = run_map_frame_scenario(Some("__no_such_map_for_gate__"), 5);
        assert_eq!(result.frames_requested, 5);
        assert!(
            result.frames_advanced > 0,
            "logic frames must advance: {:?}",
            result
        );
        assert_eq!(
            result.frame_after,
            result.frame_before + result.frames_advanced
        );
        assert!(
            matches!(
                result.status,
                MapFrameStatus::AssetsUnavailable | MapFrameStatus::LoadFailed
            ),
            "missing map must not fake Success: {:?}",
            result
        );
        let report = format_map_frame_report(&result);
        assert!(report.contains("frames_advanced="));
        assert!(report.contains("map_identity="));
    }

    #[test]
    fn default_frame_count_is_positive() {
        assert!(DEFAULT_MAP_FRAME_ADVANCE > 0);
    }
}
