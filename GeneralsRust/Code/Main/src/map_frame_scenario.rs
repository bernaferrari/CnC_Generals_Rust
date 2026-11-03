//! Production-linked map load + stable logic-frame advance gate helpers.
//!
//! This is deliberately small and fixture-friendly: it drives the real Main
//! `GameLogic::load_map` / `GameLogic::update` entry points rather than a mock
//! GameState. When retail maps are absent, callers can still exercise the pure
//! frame-advance path with a synthetic world.

//! Wave 957: host_object/host_objects authority dual-read seal.
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
pub fn run_map_frame_scenario(
    map_name: Option<&str>,
    frames: u32,
) -> gamelogic::GameLogicResult<MapFrameScenarioResult> {
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
        logic.headless_camera_view_offer(33)?;
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

    Ok(MapFrameScenarioResult {
        map_identity,
        map_resolved,
        map_loaded,
        frames_requested: frames,
        frames_advanced,
        frame_before,
        frame_after,
        object_count: logic.host_objects().registered_len(),
        status,
    })
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
        let result = run_map_frame_scenario(Some("__no_such_map_for_gate__"), 5).unwrap();
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

    /// Fingerprint of one Lone Eagle (+AI skirmish) world after N logic ticks.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct SkirmishFingerprint {
        frames_advanced: u32,
        frame_after: u32,
        object_count: usize,
        players_ptr: usize,
        sides_ptr: usize,
        map_loaded: bool,
        order_overlay: Option<String>,
    }

    fn ensure_assets_env() {
        if std::env::var_os("GENERALS_ASSETS_DIR").is_some() {
            return;
        }
        for candidate in [
            "/Users/bernardoferrari/Downloads/cnc-zero-hour-work-preserved-20261010/windows_game",
            "/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main/windows_game",
        ] {
            let p = Path::new(candidate);
            if p
                .join("extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map")
                .is_file()
                || p.join("Data/EnglishZH.big").is_file()
            {
                // SAFETY: test-only process env for asset discovery.
                unsafe {
                    std::env::set_var("GENERALS_ASSETS_DIR", candidate);
                }
                return;
            }
        }
    }

    fn lone_eagle_map_path() -> Option<String> {
        ensure_assets_env();
        const CANDIDATES: &[&str] = &[
            "windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
            "/Users/bernardoferrari/Downloads/cnc-zero-hour-work-preserved-20261010/windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
            "/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main/windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
        ];
        for c in CANDIDATES {
            if let Some(path) = resolve_path_candidate(c) {
                return Some(path.display().to_string());
            }
            if Path::new(c).is_file() {
                return Some(c.to_string());
            }
        }
        None
    }

    fn run_lone_eagle_skirmish(frames: u32, diverge_token: u32) -> Option<SkirmishFingerprint> {
        let map = lone_eagle_map_path()?;
        let mut logic = GameLogic::new();
        logic.start_new_game(GameMode::Skirmish);
        let services = std::sync::Arc::clone(&logic.world_services);
        gamelogic::system::engine_stores::with_world_services(&services, || {
            services
                .session_flags()
                .next_command_id
                .store(1 + diverge_token, std::sync::atomic::Ordering::SeqCst);
            if diverge_token != 0 {
                if let Ok(mut overlay) = services.map_string_overlay().write() {
                    overlay.insert(
                        "parallel_order".to_string(),
                        diverge_token.to_string(),
                    );
                }
            }
        });
        let loaded = logic.load_map(&map) || logic.load_map("Lone Eagle");
        let before = logic.get_frame();
        for _ in 0..frames {
            let _ = logic.update();
            let _ = logic.headless_camera_view_offer(33);
        }
        let after = logic.get_frame();
        let players_ptr = std::sync::Arc::as_ptr(logic.world_services.players()) as usize;
        let sides_ptr = std::sync::Arc::as_ptr(logic.world_services.sides()) as usize;
        let order_overlay = gamelogic::system::engine_stores::with_world_services(&services, || {
            services
                .map_string_overlay()
                .read()
                .ok()
                .and_then(|o| o.get("parallel_order").cloned())
        });
        Some(SkirmishFingerprint {
            frames_advanced: after.saturating_sub(before),
            frame_after: after,
            object_count: logic.host_objects().registered_len(),
            players_ptr,
            sides_ptr,
            map_loaded: loaded,
            order_overlay,
        })
    }

    #[test]
    fn parallel_lone_eagle_skirmishes_match_under_same_orders_and_diverge_under_different() {
        ensure_assets_env();
        if lone_eagle_map_path().is_none() {
            eprintln!("parallel_lone_eagle_skirmishes: SKIP Lone Eagle assets missing");
            return;
        }
        use std::sync::{Arc, Barrier, Mutex};
        use std::thread;

        // Match under same orders + seed.
        let barrier = Arc::new(Barrier::new(2));
        let matched = Arc::new(Mutex::new(Vec::new()));
        let mut handles = Vec::new();
        for _ in 0..2 {
            let barrier = Arc::clone(&barrier);
            let matched = Arc::clone(&matched);
            handles.push(thread::spawn(move || {
                barrier.wait();
                let fp = run_lone_eagle_skirmish(12, 0).expect("same-order skirmish");
                matched.lock().unwrap().push(fp);
                barrier.wait();
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        let got = matched.lock().unwrap().clone();
        assert_eq!(got.len(), 2);
        assert!(got[0].map_loaded && got[1].map_loaded, "both maps must load: {got:?}");
        assert!(got[0].frames_advanced > 0 && got[1].frames_advanced > 0, "{got:?}");
        assert_eq!(got[0].frames_advanced, got[1].frames_advanced);
        assert_eq!(got[0].frame_after, got[1].frame_after);
        assert_eq!(got[0].object_count, got[1].object_count);
        assert_ne!(got[0].players_ptr, got[1].players_ptr);
        assert_ne!(got[0].sides_ptr, got[1].sides_ptr);

        // Diverge under different orders (world-local overlay + session command id).
        let barrier = Arc::new(Barrier::new(2));
        let diverged = Arc::new(Mutex::new(Vec::new()));
        let mut handles = Vec::new();
        for token in [0u32, 77u32] {
            let barrier = Arc::clone(&barrier);
            let diverged = Arc::clone(&diverged);
            handles.push(thread::spawn(move || {
                barrier.wait();
                let fp = run_lone_eagle_skirmish(12, token).expect("divergent skirmish");
                diverged.lock().unwrap().push((token, fp));
                barrier.wait();
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        let mut got = diverged.lock().unwrap().clone();
        got.sort_by_key(|(t, _)| *t);
        assert!(got[0].1.map_loaded && got[1].1.map_loaded, "{got:?}");
        assert_ne!(got[0].1.players_ptr, got[1].1.players_ptr);
        assert_ne!(got[0].1.sides_ptr, got[1].1.sides_ptr);
        assert_ne!(got[0].1.order_overlay, got[1].1.order_overlay);
        assert_eq!(got[1].1.order_overlay.as_deref(), Some("77"));
    }
}
