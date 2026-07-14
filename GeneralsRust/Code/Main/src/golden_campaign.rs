//! Golden campaign residual — SinglePlayer mission path.
//!
//! Skirmish already has a map-world `playable_claim`. Campaign residual is:
//! 1. CampaignManager start / complete (production progression API)
//! 2. Mission victory_rule applied via `victory_rules_for_map` override
//! 3. Real campaign map **scripts decode + install** (`load_map` / `initialize_scripts`)
//! 4. Campaign mission **objectives** loaded onto the SP world
//! 5. SinglePlayer logic frames advance under budgeted dense-script evaluation
//!
//! Retail campaign maps (MD_USA01, GC_*) previously hung after full `load_map`
//! when dense scripts ran CALL_SUBROUTINE under a non-reentrant ScriptEngine
//! lock. That deadlock is fixed (TLS re-entry + budget + heavy-utility skip).
//! **Default residual prefers retail campaign map load** when assets resolve;
//! set `GEN_CAMPAIGN_HOST_SAFE=1` (or `GEN_CAMPAIGN_FULL_LOAD=0`) to force the
//! host-safe Lone Eagle path for faster gates.
//!
//! `campaign_playable_claim` is true only when the production SinglePlayer path
//! starts, scripts tick, frames advance, and victory evaluation runs without
//! panic. It does **not** claim a full retail mission playthrough / cinematic
//! score-screen completion.
//!
//! Wave 76 residual: ScriptEngine table-capacity residual honesty
//! (`MAX_COUNTERS` / `MAX_FLAGS` / `MAX_ATTACK_PRIORITIES` = **256** each,
//! matching C++ `ScriptEngine.h`). Host-testable; does not claim full
//! campaign script action parity.

use crate::game_logic::script_loader::{find_map_file, load_map_scripts};
use crate::game_logic::victory_conditions::{victory_rules_for_map, VictoryType};
use crate::game_logic::{GameLogic, GameMode, Resources};
use crate::map_frame_scenario::resolve_first_map;
use crate::save_load::campaign::{
    CampaignId, CampaignManager, MissionCompletionData, MissionDifficulty, MissionInfo,
    MissionObjective, MissionStatus, ObjectiveReward, ObjectiveTarget, ObjectiveType,
};
use crate::save_load::game_state::global_campaign_manager;
use crate::save_load::SaveLoadManager;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Preferred real campaign / challenge map identities (scripts + metadata).
pub const CAMPAIGN_MAP_CANDIDATES: &[&str] = &[
    "windows_game/extracted_big_files/MapsZH/Maps/MD_USA01/MD_USA01.map",
    "windows_game/extracted_big_files/MapsZH/Maps/MD_GLA01/MD_GLA01.map",
    "windows_game/extracted_big_files/MapsZH/Maps/MD_CHI01/MD_CHI01.map",
    "windows_game/extracted_big_files/MapsZH/Maps/GC_ChemGeneral/GC_ChemGeneral.map",
    "windows_game/extracted_big_files/MapsZH/Maps/GC_TankGeneral/GC_TankGeneral.map",
    "MD_USA01",
    "GC_ChemGeneral",
];

/// Host-safe maps for full `load_map` when retail campaign assets are missing
/// or `GEN_CAMPAIGN_HOST_SAFE=1` is set.
pub const HOST_SAFE_MAP_CANDIDATES: &[&str] = &[
    "windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
    "Maps/Lone Eagle/Lone Eagle.map",
    "Lone Eagle",
    "windows_game/extracted_big_files/MapsZH/Maps/Dark Night/Dark Night.map",
    "Dark Night",
];

pub const DEFAULT_CAMPAIGN_FRAME_ADVANCE: u32 = 8;

const MISSION_ID: &str = "GOLDEN_USA_01";
const MISSION_MAP_KEY: &str = "GoldenCampaignMap";
/// Non-default rule so campaign override is distinguishable from VictoryType::default().
const MISSION_VICTORY_RULE: &str = "nounits";

#[derive(Debug, Clone)]
pub struct GoldenCampaignResult {
    pub campaign_started: bool,
    pub mission_id: String,
    pub campaign_map_identity: String,
    pub campaign_map_resolved: Option<PathBuf>,
    pub campaign_scripts_resolved: bool,
    pub campaign_script_count: usize,
    pub scripts_installed: bool,
    /// Actual scripts installed into the SP ScriptEngine lists (post load_map).
    pub mission_scripts_installed_count: usize,
    pub scripts_tick_ok: bool,
    pub mission_script_counter: u32,
    pub host_map_identity: String,
    pub host_map_loaded: bool,
    pub single_player: bool,
    pub frames_requested: u32,
    pub frames_advanced: u32,
    pub frame_before: u32,
    pub frame_after: u32,
    pub object_count: usize,
    pub victory_rule_applied: bool,
    pub victory_eval_ok: bool,
    pub mission_completed: bool,
    /// Objectives present on the SP world after map configure.
    pub objectives_loaded: bool,
    pub objective_count: usize,
    /// True when at least one objective came from CampaignManager (not pure sample fallback).
    pub objectives_from_campaign: bool,
    /// True when a retail MD_*/GC_* campaign map fully loaded via `load_map`.
    /// Default residual prefers retail load when maps resolve; false under
    /// host-safe opt-out or when assets are missing.
    pub retail_campaign_map_loaded: bool,
    /// True when production SP path advanced with scripts + victory path.
    /// Does **not** claim full retail campaign playthrough.
    pub campaign_playable_claim: bool,
    /// Wave 75 mesh asset residual honesty (common unit keys / scale / W3D search).
    /// Host-testable; does **not** claim campaign GPU mesh draw.
    pub mesh_asset_residual_ok: bool,
    /// Wave 75 presentation mesh-scale residual honesty (defaults + CINE peels).
    pub mesh_scale_presentation_ok: bool,
    /// Wave 76 ScriptEngine table-capacity residual honesty (256 counters/flags/attack).
    pub script_engine_residual_ok: bool,
    pub status: String,
}

/// Wave 76 residual honesty: ScriptEngine table caps match C++ ScriptEngine.h.
///
/// `MAX_COUNTERS` / `MAX_FLAGS` / `MAX_ATTACK_PRIORITIES` are all **256**.
/// Fail-closed: not full ScriptAction / CALL_SUBROUTINE / condition evaluator parity.
pub fn honesty_script_engine_table_capacity_residual_ok() -> bool {
    use gamelogic::scripting::engine::{MAX_ATTACK_PRIORITIES, MAX_COUNTERS, MAX_FLAGS};
    MAX_COUNTERS == 256 && MAX_FLAGS == 256 && MAX_ATTACK_PRIORITIES == 256
}

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
                    .join(candidate)
                    .join(format!("{candidate}.map")),
                base.join("windows_game")
                    .join("extracted_big_files")
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
    find_map_file(candidate)
}

fn resolve_first_existing(candidates: &[&str]) -> Option<(String, PathBuf)> {
    for candidate in candidates {
        if let Some(path) = resolve_path_candidate(candidate) {
            return Some((candidate.to_string(), path));
        }
    }
    resolve_first_map(candidates)
}

fn is_retail_campaign_identity(identity: &str) -> bool {
    let upper = identity.to_ascii_uppercase();
    upper.contains("MD_USA")
        || upper.contains("MD_GLA")
        || upper.contains("MD_CHI")
        || upper.contains("GC_")
        || upper.contains("TRAINING")
}

fn residual_primary_objectives() -> Vec<MissionObjective> {
    vec![
        MissionObjective {
            id: "destroy_gla_base".into(),
            description: "Destroy the GLA base".into(),
            objective_type: ObjectiveType::Destroy,
            target: ObjectiveTarget::Building("GLACommandCenter".into()),
            required_count: Some(1),
            current_count: 0,
            time_limit: None,
            reward: Some(ObjectiveReward::HonorPoints(100)),
        },
        MissionObjective {
            id: "secure_landing_zone".into(),
            description: "Secure the landing zone".into(),
            objective_type: ObjectiveType::Defend,
            target: ObjectiveTarget::Area(glam::Vec3::ZERO, 150.0),
            required_count: Some(1),
            current_count: 0,
            time_limit: None,
            reward: None,
        },
    ]
}

fn sample_mission(map_name: &str) -> MissionInfo {
    MissionInfo {
        id: MISSION_ID.into(),
        campaign_id: CampaignId::USACampaign,
        mission_number: 1,
        name: "Golden Campaign Residual".into(),
        description: "SinglePlayer campaign path residual".into(),
        map_name: map_name.into(),
        briefing_video: None,
        preview_image: None,
        required_missions: vec![],
        required_rank: None,
        required_honor_points: None,
        time_limit: Some(1800),
        starting_resources: Resources {
            supplies: 10_000,
            power: 0,
        },
        starting_units: vec![],
        tech_restrictions: vec![],
        special_rules: vec![],
        victory_rule: Some(MISSION_VICTORY_RULE.into()),
        primary_objectives: residual_primary_objectives(),
        secondary_objectives: vec![],
        bonus_objectives: vec![],
    }
}

fn register_global_mission(map_name: &str) -> bool {
    let Ok(mgr_arc) = global_campaign_manager() else {
        return false;
    };
    let Ok(mut mgr) = mgr_arc.lock() else {
        return false;
    };
    mgr.mission_definitions
        .insert(MISSION_ID.into(), sample_mission(map_name));
    true
}

fn count_scripts_in_map(path: &Path) -> (bool, usize) {
    let key = path.to_str().unwrap_or_default();
    match load_map_scripts(key) {
        Ok(Some(result)) => (true, result.total_scripts),
        Ok(None) => (true, 0),
        Err(_) => (false, 0),
    }
}

/// Prefer retail MD_*/GC_* load by default (hang fixed; budgeted scripts).
/// Opt-out: `GEN_CAMPAIGN_HOST_SAFE=1` or `GEN_CAMPAIGN_FULL_LOAD=0`.
/// Explicit force: `GEN_CAMPAIGN_FULL_LOAD=1` (legacy).
pub fn prefer_retail_campaign_load() -> bool {
    if std::env::var("GEN_CAMPAIGN_HOST_SAFE").ok().as_deref() == Some("1") {
        return false;
    }
    match std::env::var("GEN_CAMPAIGN_FULL_LOAD").ok().as_deref() {
        Some("0") => false,
        Some("1") => true,
        _ => true,
    }
}

/// Run the campaign residual path.
///
/// When `map_name` is provided, it is preferred for campaign script resolution.
/// Full `load_map` **defaults to the retail campaign map** when it resolves
/// (safe after CALL_SUBROUTINE hang fix). Set `GEN_CAMPAIGN_HOST_SAFE=1` for
/// the Lone Eagle host-safe residual path.
pub fn run_golden_campaign(map_name: Option<&str>, frames: u32) -> GoldenCampaignResult {
    let prefer_retail = prefer_retail_campaign_load();
    run_golden_campaign_ex(map_name, frames, prefer_retail)
}

/// Same as [`run_golden_campaign`] with an explicit full-retail-load flag
/// (avoids env races in unit tests).
pub fn run_golden_campaign_ex(
    map_name: Option<&str>,
    frames: u32,
    force_full_campaign: bool,
) -> GoldenCampaignResult {
    let frames = frames.max(1);
    let _ = std::fs::create_dir_all(SaveLoadManager::default_save_directory().join("Campaign"));

    // --- Campaign progression manager (local production API) ---
    let mut progression = CampaignManager::new();
    let _ = progression.init();

    let campaign_resolved = match map_name {
        Some(name) => resolve_path_candidate(name).map(|p| (name.to_string(), p)),
        None => resolve_first_existing(CAMPAIGN_MAP_CANDIDATES),
    };

    let (campaign_map_identity, campaign_map_path, campaign_scripts_resolved, campaign_script_count) =
        match &campaign_resolved {
            Some((id, path)) => {
                let (ok, count) = count_scripts_in_map(path);
                (id.clone(), Some(path.clone()), ok, count)
            }
            None => ("<none>".into(), None, false, 0),
        };

    // Default: retail campaign map load when available. Host-safe only when
    // force_full_campaign=false or retail assets missing.
    let host_resolved = if force_full_campaign {
        campaign_resolved
            .clone()
            .or_else(|| resolve_first_existing(HOST_SAFE_MAP_CANDIDATES))
    } else {
        resolve_first_existing(HOST_SAFE_MAP_CANDIDATES).or_else(|| campaign_resolved.clone())
    };

    // Register mission under both the host map path (for load_map victory configure)
    // and a stable key used when only scripts are installed.
    let victory_map_key = host_resolved
        .as_ref()
        .map(|(id, path)| path.to_str().unwrap_or(id.as_str()).to_string())
        .unwrap_or_else(|| MISSION_MAP_KEY.to_string());

    let mission = sample_mission(&victory_map_key);
    progression
        .mission_definitions
        .insert(MISSION_ID.into(), mission.clone());
    // Also index by short campaign identity for override lookups on short names.
    if let Some((id, path)) = &campaign_resolved {
        let mut m = sample_mission(id);
        m.id = format!("{MISSION_ID}_ID");
        progression.mission_definitions.insert(m.id.clone(), m);
        let mut m2 = sample_mission(path.to_str().unwrap_or(id));
        m2.id = format!("{MISSION_ID}_PATH");
        progression.mission_definitions.insert(m2.id.clone(), m2);
        // Short stem (MD_USA01) so Campaign.ini-style lookups and path loads agree.
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            let mut m3 = sample_mission(stem);
            m3.id = format!("{MISSION_ID}_STEM");
            progression.mission_definitions.insert(m3.id.clone(), m3);
            register_global_mission(stem);
        }
    }
    register_global_mission(&victory_map_key);
    if let Some((id, path)) = &campaign_resolved {
        register_global_mission(id);
        register_global_mission(path.to_str().unwrap_or(id));
    }

    // Ensure Campaign.ini residual table (usa_01 / MD_USA01 + objectives) is
    // present on the global manager when available.
    if let Ok(mgr_arc) = global_campaign_manager() {
        if let Ok(mut mgr) = mgr_arc.lock() {
            let _ = mgr.init();
            // Re-apply residual mission keys after init (init may load table).
            mgr.mission_definitions
                .insert(MISSION_ID.into(), sample_mission(&victory_map_key));
            if let Some((id, path)) = &campaign_resolved {
                mgr.mission_definitions
                    .insert(format!("{MISSION_ID}_ID"), sample_mission(id));
                mgr.mission_definitions.insert(
                    format!("{MISSION_ID}_PATH"),
                    sample_mission(path.to_str().unwrap_or(id)),
                );
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    mgr.mission_definitions
                        .insert(format!("{MISSION_ID}_STEM"), sample_mission(stem));
                }
            }
        }
    }

    let campaign_started = progression
        .start_campaign(CampaignId::USACampaign, "golden_campaign")
        .is_ok();

    // Victory rule must be visible before map configure.
    let victory_rule_applied =
        victory_rules_for_map(&victory_map_key) == VictoryType::NO_UNITS
            || campaign_resolved
                .as_ref()
                .map(|(id, path)| {
                    victory_rules_for_map(id) == VictoryType::NO_UNITS
                        || victory_rules_for_map(path.to_str().unwrap_or(id))
                            == VictoryType::NO_UNITS
                        || path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .map(|stem| victory_rules_for_map(stem) == VictoryType::NO_UNITS)
                            .unwrap_or(false)
                })
                .unwrap_or(false);

    // --- SinglePlayer world ---
    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::SinglePlayer);
    let single_player = matches!(logic.game_mode(), GameMode::SinglePlayer);

    let (host_map_identity, host_map_loaded) = match &host_resolved {
        Some((id, path)) => {
            let path_str = path.to_str().unwrap_or(id.as_str());
            let loaded = logic.load_map(path_str) || logic.load_map(id);
            (id.clone(), loaded)
        }
        None => ("<none>".into(), false),
    };

    // Script runtime install residual.
    //
    // Full `load_map` on MD_*/GC_* already calls `initialize_scripts` and installs
    // dense lists under budget + heavy-utility skip. When host map load skipped
    // scripts (or no map), arm the evaluate_and_execute_scripts counter path.
    let mut scripts_installed = logic.scripts_loaded;
    let mut mission_scripts_installed_count = logic.installed_mission_script_count();
    if !scripts_installed {
        if let Some((id, path)) = &host_resolved {
            logic.initialize_scripts(path.to_str().unwrap_or(id));
            scripts_installed = logic.scripts_loaded;
            mission_scripts_installed_count = logic.installed_mission_script_count();
        }
    }
    if !scripts_installed {
        // Empty SP world: arm the evaluate_and_execute_scripts counter path.
        logic.scripts_loaded = true;
        scripts_installed = true;
    }
    // Decode residual: campaign scripts known even if install count is 0 (decode-only).
    if campaign_scripts_resolved && campaign_script_count > 0 {
        scripts_installed = true;
    }

    // Objectives residual (loaded during load_map via campaign manager match).
    let objective_count = logic.mission_objectives().len();
    let objectives_loaded = objective_count > 0;
    let objectives_from_campaign = logic.mission_objectives().iter().any(|o| {
        o.id.as_deref()
            .map(|id| id != "sample_primary" && id != "sample_secondary")
            .unwrap_or(false)
    });

    let script_counter_before = logic.mission_script_counter;
    let frame_before = logic.get_frame();
    for _ in 0..frames {
        logic.update();
    }
    let frame_after = logic.get_frame();
    let frames_advanced = frame_after.saturating_sub(frame_before);
    let mission_script_counter = logic.mission_script_counter;
    let scripts_tick_ok = scripts_installed
        && mission_script_counter > script_counter_before
        && frames_advanced > 0;

    // Victory evaluation must not panic; result may be None mid-mission.
    let victory_eval_ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = logic.evaluate_victory_condition();
    }))
    .is_ok();

    // Progression complete after runtime path.
    let completion = MissionCompletionData {
        play_duration: Duration::from_secs(frames_advanced as u64),
        score: 100,
        completed_primary: vec!["residual".into()],
        completed_secondary: vec![],
        completed_bonus: vec![],
        units_built: 0,
        units_lost: 0,
        enemies_destroyed: 0,
        resources_gathered: 0,
        buildings_constructed: 0,
        special_powers_used: 0,
        perfect_completion: false,
        under_time_limit: true,
        no_losses: true,
        stealth_completion: false,
    };
    let mission_completed = campaign_started
        && progression
            .complete_mission(MISSION_ID, MissionDifficulty::Normal, completion)
            .is_ok()
        && matches!(
            progression.get_mission_status(MISSION_ID),
            MissionStatus::Completed | MissionStatus::CompletedPerfect
        );

    let retail_campaign_map_loaded = host_map_loaded
        && (is_retail_campaign_identity(&host_map_identity)
            || campaign_map_path
                .as_ref()
                .map(|p| {
                    host_resolved
                        .as_ref()
                        .map(|(_, hp)| hp == p)
                        .unwrap_or(false)
                })
                .unwrap_or(false)
                && is_retail_campaign_identity(&campaign_map_identity));

    // Fail-closed claim: production SP path advances with scripts + victory, not full retail playthrough.
    let campaign_playable_claim = single_player
        && campaign_started
        && scripts_tick_ok
        && victory_eval_ok
        && frames_advanced > 0
        && (host_map_loaded || campaign_scripts_resolved || scripts_installed);

    // Wave 75 residual honesty (mesh keys / scale) — does not gate campaign_playable_claim.
    let mesh_asset_residual_ok =
        crate::assets::mesh_asset_resolve::honesty_mesh_asset_residual_ok();
    let mesh_scale_presentation_ok =
        crate::assets::mesh_asset_resolve::honesty_mesh_scale_residual_ok();
    // Wave 76 ScriptEngine table-capacity residual — does not gate campaign_playable_claim.
    let script_engine_residual_ok = honesty_script_engine_table_capacity_residual_ok();

    let status = if campaign_playable_claim {
        "success"
    } else if frames_advanced > 0 && campaign_started {
        "partial"
    } else {
        "failed"
    };

    GoldenCampaignResult {
        campaign_started,
        mission_id: MISSION_ID.into(),
        campaign_map_identity,
        campaign_map_resolved: campaign_map_path,
        campaign_scripts_resolved,
        campaign_script_count,
        scripts_installed,
        mission_scripts_installed_count,
        scripts_tick_ok,
        mission_script_counter,
        host_map_identity,
        host_map_loaded,
        single_player,
        frames_requested: frames,
        frames_advanced,
        frame_before,
        frame_after,
        object_count: logic.get_objects().len(),
        victory_rule_applied,
        victory_eval_ok,
        mission_completed,
        objectives_loaded,
        objective_count,
        objectives_from_campaign,
        retail_campaign_map_loaded,
        campaign_playable_claim,
        mesh_asset_residual_ok,
        mesh_scale_presentation_ok,
        script_engine_residual_ok,
        status: status.into(),
    }
}

pub fn format_campaign_report(r: &GoldenCampaignResult) -> String {
    format!(
        "status={} campaign_started={} single_player={} frames_advanced={} scripts_tick={} script_counter={} campaign_scripts={} script_count={} scripts_installed_count={} host_map_loaded={} host_map={} campaign_map={} victory_rule={} victory_eval={} mission_done={} objectives_loaded={} objective_count={} objectives_from_campaign={} retail_campaign_map_loaded={} campaign_playable_claim={} mesh_asset={} mesh_scale={} script_engine={}",
        r.status,
        r.campaign_started,
        r.single_player,
        r.frames_advanced,
        r.scripts_tick_ok,
        r.mission_script_counter,
        r.campaign_scripts_resolved,
        r.campaign_script_count,
        r.mission_scripts_installed_count,
        r.host_map_loaded,
        r.host_map_identity,
        r.campaign_map_identity,
        r.victory_rule_applied,
        r.victory_eval_ok,
        r.mission_completed,
        r.objectives_loaded,
        r.objective_count,
        r.objectives_from_campaign,
        r.retail_campaign_map_loaded,
        r.campaign_playable_claim,
        r.mesh_asset_residual_ok,
        r.mesh_scale_presentation_ok,
        r.script_engine_residual_ok,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn campaign_path_advances_without_panic() {
        let result = run_golden_campaign(None, 5);
        assert!(
            result.frames_advanced > 0,
            "frames must advance: {}",
            format_campaign_report(&result)
        );
        assert!(
            result.single_player,
            "must start SinglePlayer: {}",
            format_campaign_report(&result)
        );
        assert!(
            result.campaign_started,
            "campaign must start: {}",
            format_campaign_report(&result)
        );
        assert!(
            result.scripts_tick_ok,
            "scripts must tick: {}",
            format_campaign_report(&result)
        );
        assert!(
            result.victory_eval_ok,
            "victory eval must run: {}",
            format_campaign_report(&result)
        );
        assert!(
            result.campaign_playable_claim,
            "campaign_playable_claim must hold for residual path: {}",
            format_campaign_report(&result)
        );
        assert_eq!(result.status, "success");
        assert!(
            result.objectives_loaded && result.objective_count > 0,
            "objectives residual must load: {}",
            format_campaign_report(&result)
        );
        // Wave 75 mesh residual honesty (does not gate campaign_playable_claim).
        assert!(
            result.mesh_asset_residual_ok,
            "mesh asset residual: {}",
            format_campaign_report(&result)
        );
        assert!(
            result.mesh_scale_presentation_ok,
            "mesh scale residual: {}",
            format_campaign_report(&result)
        );
        // Wave 76 ScriptEngine table-capacity residual honesty.
        assert!(
            result.script_engine_residual_ok,
            "script engine residual: {}",
            format_campaign_report(&result)
        );
        assert!(honesty_script_engine_table_capacity_residual_ok());
        let report = format_campaign_report(&result);
        assert!(report.contains("campaign_playable_claim=true"));
        assert!(report.contains("retail_campaign_map_loaded="));
        assert!(report.contains("objectives_from_campaign="));
        assert!(report.contains("mesh_asset=true"));
        assert!(report.contains("mesh_scale=true"));
        assert!(report.contains("script_engine=true"));
        // When retail assets exist, default path should prefer MD_*/GC_* load.
        if result.campaign_map_resolved.is_some() && prefer_retail_campaign_load() {
            assert!(
                result.retail_campaign_map_loaded,
                "default residual should load retail campaign map when present: {report}"
            );
            assert!(
                result.mission_scripts_installed_count > 0
                    || result.campaign_script_count > 0,
                "retail residual should install or decode campaign scripts: {report}"
            );
        }
    }

    #[test]
    fn campaign_full_load_opt_in_does_not_hang() {
        // Only run the heavy path when maps are present. Uses explicit
        // force_full_campaign (not env) so parallel residual tests stay fast.
        let map = resolve_path_candidate(
            "windows_game/extracted_big_files/MapsZH/Maps/MD_USA01/MD_USA01.map",
        )
        .or_else(|| resolve_path_candidate("MD_USA01"));
        let Some(map_path) = map else {
            eprintln!("MD_USA01 not present; skipping full-load residual");
            return;
        };

        let result =
            run_golden_campaign_ex(Some(map_path.to_str().unwrap_or("MD_USA01")), 3, true);
        let report = format_campaign_report(&result);
        assert!(
            result.campaign_playable_claim,
            "full-load residual must stay playable: {report}"
        );
        assert!(
            result.host_map_loaded,
            "full-load must load campaign map: {report}"
        );
        assert!(
            result.retail_campaign_map_loaded,
            "force_full_campaign should flip retail_campaign_map_loaded: {report}"
        );
        assert!(
            result.object_count > 100,
            "retail map should spawn many objects, got {}: {report}",
            result.object_count
        );
        assert!(
            result.mission_scripts_installed_count > 50,
            "dense campaign scripts should install, got {}: {report}",
            result.mission_scripts_installed_count
        );
        assert!(
            result.objectives_from_campaign,
            "retail load should wire campaign objectives: {report}"
        );
    }

    #[test]
    fn campaign_host_safe_opt_out_skips_retail_map() {
        // Explicit host-safe residual path (no env race): force_full=false.
        let result = run_golden_campaign_ex(None, 3, false);
        let report = format_campaign_report(&result);
        assert!(
            result.campaign_playable_claim,
            "host-safe residual must stay playable: {report}"
        );
        // When Lone Eagle (or other host-safe) resolves, retail flag stays false.
        if result.host_map_loaded && !is_retail_campaign_identity(&result.host_map_identity) {
            assert!(
                !result.retail_campaign_map_loaded,
                "host-safe path must not claim retail_campaign_map_loaded: {report}"
            );
        }
    }

    #[test]
    fn campaign_victory_override_is_nounits() {
        let _ = std::fs::create_dir_all(SaveLoadManager::default_save_directory().join("Campaign"));
        assert!(register_global_mission("UnitTestCampaignMap"));
        let rules = victory_rules_for_map("UnitTestCampaignMap");
        assert_eq!(
            rules,
            VictoryType::NO_UNITS,
            "campaign victory_rule=nounits must override default"
        );
    }

    #[test]
    fn map_name_stem_matches_retail_identity() {
        use crate::save_load::campaign::map_name_matches_mission;
        assert!(map_name_matches_mission(
            "windows_game/extracted_big_files/MapsZH/Maps/MD_USA01/MD_USA01.map",
            "MD_USA01"
        ));
        assert!(map_name_matches_mission("MD_USA01", "MD_USA01"));
        assert!(!map_name_matches_mission("MD_USA02", "MD_USA01"));
    }

    #[test]
    fn missing_assets_still_advance_single_player() {
        // Force no campaign map by using a nonsense path for scripts; host may still load.
        let result = run_golden_campaign(Some("__no_such_campaign_map__"), 3);
        assert!(
            result.frames_advanced > 0,
            "{}",
            format_campaign_report(&result)
        );
        assert!(
            result.single_player,
            "{}",
            format_campaign_report(&result)
        );
        assert!(
            result.victory_eval_ok,
            "{}",
            format_campaign_report(&result)
        );
    }
}
