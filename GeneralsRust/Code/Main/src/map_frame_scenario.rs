//! Production-linked map load + stable logic-frame advance gate helpers.
//!
//! This is deliberately small and fixture-friendly: it drives the real Main
//! `GameLogic::load_map` / `GameLogic::update` entry points rather than a mock
//! GameState. When retail maps are absent, callers can still exercise the pure
//! frame-advance path with a synthetic world.

//! Wave 957: host_object/host_objects authority dual-read seal.
use crate::game_logic::script_loader::find_map_file;
use crate::game_logic::{GameLogic, GameMode};
use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
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


/// Per-frame logic CRC + owned state snapshot for one Lone Eagle skirmish.
///
/// `frame_crcs[i]` is [`GameLogic::logic_crc`] after frame `i+1`. Same seed and
/// same orders must yield identical sequences (parallel peers and solo).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoneEagleSkirmishFingerprint {
    pub frames_advanced: u32,
    pub frame_after: u32,
    pub object_count: usize,
    pub players_ptr: usize,
    pub sides_ptr: usize,
    pub map_loaded: bool,
    pub order_overlay: Option<String>,
    pub seed: u32,
    pub orders_token: u32,
    pub frame_crcs: Vec<u32>,
}

/// Fixed seed for isolation CRC runs (solo and parallel must share it).
pub const LONE_EAGLE_ISOLATION_SEED: u32 = 0x50_E1_50_E1;

/// Same retail root probe as `retail_skirmish_map_gate` / `player_orders_gate`.
fn retail_root_looks_valid(root: &Path) -> bool {
    let data = if root
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("Data"))
    {
        root.to_path_buf()
    } else {
        root.join("Data")
    };
    data.join("MapsZH.big").is_file()
        || data.join("EnglishZH.big").is_file()
        || root
            .parent()
            .map(|p| {
                p.join("extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map")
                    .is_file()
            })
            .unwrap_or(false)
}

/// Resolve and publish `GENERALS_ASSETS_DIR` when windows_game / retail is present.
/// Tests that need PlayerTemplate.ini or maps call this and skip when `None`.
pub fn ensure_assets_env_for_tests() -> Option<PathBuf> {
    ensure_lone_eagle_assets_env()
}

fn ensure_lone_eagle_assets_env() -> Option<PathBuf> {
    for key in ["GENERALS_RETAIL_DIR", "GENERALS_ASSETS_DIR"] {
        if let Ok(dir) = std::env::var(key) {
            let p = PathBuf::from(&dir);
            if retail_root_looks_valid(&p) {
                if std::env::var_os("GENERALS_ASSETS_DIR").is_none() {
                    // SAFETY: process env for asset discovery in gates/tests.
                    unsafe {
                        std::env::set_var("GENERALS_ASSETS_DIR", &p);
                    }
                }
                return Some(p);
            }
        }
    }
    for candidate in [
        PathBuf::from("windows_game/Command & Conquer Generals Zero Hour"),
        PathBuf::from("../windows_game/Command & Conquer Generals Zero Hour"),
        PathBuf::from("/Users/bernardoferrari/Downloads/cnc-zero-hour-work-preserved-20261010/windows_game/Command & Conquer Generals Zero Hour"),
        PathBuf::from("/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main/windows_game/Command & Conquer Generals Zero Hour"),
        PathBuf::from("/Users/bernardoferrari/Downloads/cnc-zero-hour-work-preserved-20261010/windows_game"),
        PathBuf::from("/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main/windows_game"),
    ] {
        if retail_root_looks_valid(&candidate) {
            let resolved = candidate.canonicalize().unwrap_or(candidate);
            // SAFETY: process env for asset discovery in gates/tests.
            unsafe {
                std::env::set_var("GENERALS_ASSETS_DIR", &resolved);
            }
            return Some(resolved);
        }
    }
    None
}

/// Resolve Lone Eagle.map via find_map_file (retail archives) or extracted trees.
pub fn lone_eagle_map_path() -> Option<String> {
    let _ = ensure_lone_eagle_assets_env();
    if let Some(found) = find_map_file("Lone Eagle") {
        return Some(found.display().to_string());
    }
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

/// Run one Lone Eagle skirmish for `frames` ticks.
///
/// `seed` is the logic RNG seed (same seed ⇒ same CRC when orders match).
/// `orders_token` is the owned order-stream id: it sets world-local command id /
/// map-string overlay and a CRC-visible player supply offset so different
/// orders diverge in both overlay and [`GameLogic::logic_crc`].
pub fn run_lone_eagle_skirmish(
    frames: u32,
    seed: u32,
    orders_token: u32,
) -> Option<LoneEagleSkirmishFingerprint> {
    let _retail = ensure_lone_eagle_assets_env()?;
    let map = lone_eagle_map_path()?;
    let mut logic = GameLogic::new();
    // Publish this instance before start/load/update — no process-bag fallback.
    logic.install_as_active_stores();
    logic.set_logic_random_seed(seed);
    // Same in-process retail admit bootstrap as initial_skirmish_owner_tests /
    // playable Lone Eagle hosts: GameData partition cell + golden skirmish slots.
    logic.admit_camera_application_movement(
        game_engine::common::ini::ini_game_data::CameraMovementRules::default()
            .overlay_ini_text("GameData\n PartitionCellSize = 40\nEnd\n")
            .ok()?,
    );
    // Match initial_skirmish_owner_tests::world(): admit → apply_skirmish → load_map
    // (no start_new_game before load — that path wiped/raced retail map stage).
    apply_skirmish_config(&mut logic, &golden_skirmish_config(&map)).ok()?;
    let services = std::sync::Arc::clone(&logic.world_services);
    // Prefer owned session flags on this instance (no ambient read).
    services
        .session_flags()
        .next_command_id
        .store(1 + orders_token, std::sync::atomic::Ordering::SeqCst);
    if orders_token != 0 {
        if let Ok(mut overlay) = services.map_string_overlay().write() {
            overlay.insert(
                "parallel_order".to_string(),
                orders_token.to_string(),
            );
        }
    }
    let loaded = logic.load_map(&map) || logic.load_map("Lone Eagle");
    // CRC-visible order effect: offset player-1 supplies by orders_token.
    // Same token ⇒ same CRC; different tokens diverge without touching peers.
    if orders_token != 0 {
        if let Some(player) = logic.get_player_mut(1) {
            player.resources.supplies = player
                .resources
                .supplies
                .saturating_add(orders_token);
        }
    }
    let before = logic.get_frame();
    let mut frame_crcs = Vec::with_capacity(frames as usize);
    for _ in 0..frames {
        let _ = logic.update();
        let _ = logic.headless_camera_view_offer(33);
        frame_crcs.push(logic.logic_crc());
    }
    let after = logic.get_frame();
    let players_ptr = std::sync::Arc::as_ptr(logic.world_services.players()) as usize;
    let sides_ptr = std::sync::Arc::as_ptr(logic.world_services.sides()) as usize;
    let order_overlay = services
        .map_string_overlay()
        .read()
        .ok()
        .and_then(|o| o.get("parallel_order").cloned());
    Some(LoneEagleSkirmishFingerprint {
        frames_advanced: after.saturating_sub(before),
        frame_after: after,
        object_count: logic.host_objects().registered_len(),
        players_ptr,
        sides_ptr,
        map_loaded: loaded,
        order_overlay,
        seed,
        orders_token,
        frame_crcs,
    })
}

fn assert_crc_match(label: &str, a: &LoneEagleSkirmishFingerprint, b: &LoneEagleSkirmishFingerprint) -> Result<(), String> {
    if a.frame_crcs != b.frame_crcs {
        return Err(format!(
            "{label}: per-frame CRC mismatch (isolation leak or nondeterminism)\n  a={:?}\n  b={:?}",
            a.frame_crcs, b.frame_crcs
        ));
    }
    if a.frames_advanced != b.frames_advanced
        || a.frame_after != b.frame_after
        || a.object_count != b.object_count
        || a.map_loaded != b.map_loaded
        || a.seed != b.seed
        || a.orders_token != b.orders_token
        || a.order_overlay != b.order_overlay
    {
        return Err(format!("{label}: state snapshot mismatch\n  a={a:?}\n  b={b:?}"));
    }
    Ok(())
}

/// Two-thread barrier retail check:
/// - same seed + same orders ⇒ identical per-frame CRC/state (peers + vs solo)
/// - different orders ⇒ CRC/state diverge; each still matches its solo run
pub fn run_parallel_lone_eagle_skirmishes() -> Result<(), String> {
    if ensure_lone_eagle_assets_env().is_none() {
        return Err("Lone Eagle retail assets missing (set GENERALS_ASSETS_DIR to windows_game/Command & Conquer Generals Zero Hour)".into());
    }
    if lone_eagle_map_path().is_none() {
        return Err("Lone Eagle.map not found after retail asset bootstrap".into());
    }
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;

    const FRAMES: u32 = 12;
    let seed = LONE_EAGLE_ISOLATION_SEED;

    // Solo baselines first — parallel must match these exactly.
    let solo_same = run_lone_eagle_skirmish(FRAMES, seed, 0)
        .ok_or_else(|| "solo same-orders skirmish failed".to_string())?;
    let solo_t0 = run_lone_eagle_skirmish(FRAMES, seed, 0)
        .ok_or_else(|| "solo token-0 skirmish failed".to_string())?;
    let solo_t77 = run_lone_eagle_skirmish(FRAMES, seed, 77)
        .ok_or_else(|| "solo token-77 skirmish failed".to_string())?;
    if !solo_same.map_loaded || !solo_t77.map_loaded {
        return Err(format!(
            "Lone Eagle load_map returned false with assets present: same={solo_same:?} t77={solo_t77:?}"
        ));
    }
    if solo_same.frame_crcs.is_empty() || solo_same.frames_advanced == 0 {
        return Err(format!("solo must advance frames with CRCs: {solo_same:?}"));
    }
    // Sanity: same seed+orders is deterministic even solo-vs-solo.
    assert_crc_match("solo-vs-solo same orders", &solo_same, &solo_t0)?;
    if solo_same.frame_crcs == solo_t77.frame_crcs {
        return Err(format!(
            "different orders must diverge CRC (got identical {:?})",
            solo_same.frame_crcs
        ));
    }

    let barrier = Arc::new(Barrier::new(2));
    let matched = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let barrier = Arc::clone(&barrier);
        let matched = Arc::clone(&matched);
        handles.push(thread::spawn(move || {
            barrier.wait();
            let fp = run_lone_eagle_skirmish(FRAMES, seed, 0).expect("same-order skirmish");
            matched.lock().unwrap().push(fp);
            barrier.wait();
        }));
    }
    for h in handles {
        h.join().map_err(|_| "same-order thread panicked".to_string())?;
    }
    let got = matched.lock().unwrap().clone();
    if got.len() != 2 {
        return Err(format!("expected 2 fingerprints, got {}", got.len()));
    }
    if got[0].players_ptr == got[1].players_ptr || got[0].sides_ptr == got[1].sides_ptr {
        return Err(format!("worlds must be distinct owners: {got:?}"));
    }
    if !(got[0].map_loaded && got[1].map_loaded) {
        return Err(format!(
            "Lone Eagle load_map returned false with assets present: {got:?}"
        ));
    }
    assert_crc_match("parallel peers same orders", &got[0], &got[1])?;
    assert_crc_match("parallel[0] vs solo same orders", &got[0], &solo_same)?;
    assert_crc_match("parallel[1] vs solo same orders", &got[1], &solo_same)?;

    let barrier = Arc::new(Barrier::new(2));
    let diverged = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();
    for token in [0u32, 77u32] {
        let barrier = Arc::clone(&barrier);
        let diverged = Arc::clone(&diverged);
        handles.push(thread::spawn(move || {
            barrier.wait();
            let fp = run_lone_eagle_skirmish(FRAMES, seed, token).expect("divergent skirmish");
            diverged.lock().unwrap().push((token, fp));
            barrier.wait();
        }));
    }
    for h in handles {
        h.join().map_err(|_| "diverge thread panicked".to_string())?;
    }
    let mut got = diverged.lock().unwrap().clone();
    got.sort_by_key(|(t, _)| *t);
    if !(got[0].1.map_loaded && got[1].1.map_loaded) {
        return Err(format!("diverge maps must load: {got:?}"));
    }
    if got[0].1.players_ptr == got[1].1.players_ptr || got[0].1.sides_ptr == got[1].1.sides_ptr {
        return Err(format!("diverge worlds must be distinct: {got:?}"));
    }
    if got[0].1.order_overlay == got[1].1.order_overlay {
        return Err(format!("orders must diverge: {got:?}"));
    }
    if got[1].1.order_overlay.as_deref() != Some("77") {
        return Err(format!("token 77 overlay missing: {got:?}"));
    }
    if got[0].1.frame_crcs == got[1].1.frame_crcs {
        return Err(format!(
            "different-order parallel CRCs must diverge: {:?}",
            got[0].1.frame_crcs
        ));
    }
    assert_crc_match("parallel token-0 vs solo", &got[0].1, &solo_t0)?;
    assert_crc_match("parallel token-77 vs solo", &got[1].1, &solo_t77)?;
    Ok(())
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

    #[test]
    fn parallel_lone_eagle_skirmishes_match_under_same_orders_and_diverge_under_different() {
        match run_parallel_lone_eagle_skirmishes() {
            Ok(()) => {}
            Err(msg) if msg.contains("assets missing") => {
                eprintln!("parallel_lone_eagle_skirmishes: SKIP {msg}");
            }
            Err(msg) => panic!("{msg}"),
        }
    }
}

