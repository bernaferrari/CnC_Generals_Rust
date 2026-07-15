//! Host AI skirmish activity — production path for Medium AI non-idle proof.
//!
//! Covers both the synthetic host update path and the load_map preserve/rebind
//! residual: after map load (or explicit rebind), Medium GLA AI must take at
//! least one productive action (structure start, unit queue, or unit spawn).
//!
//! Wave 77 residual peels:
//! - AIData/AIPlayer structure/team timer defaults residual honesty pack
//! - poor/wealthy resource thresholds + build-speed modifiers residual
//! - skirmish base-defense extra distance residual

use crate::ai::AIDifficulty;
use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
use glam::Vec3;
use std::path::{Path, PathBuf};

// --- Wave 77: AI skirmish residual pack (AIPlayer / AIData defaults) ---

/// C++ LOGICFRAMES_PER_SECOND residual used for structure/team timer conversion.
pub const AI_SKIRMISH_LOGIC_FPS: u32 = gamelogic::ai::ai_player::LOGICFRAMES_PER_SECOND;
/// C++ m_structureSeconds residual default.
pub const AI_SKIRMISH_STRUCTURE_SECONDS: f32 = gamelogic::ai::ai_player::DEFAULT_STRUCTURE_SECONDS;
/// C++ m_teamSeconds residual default.
pub const AI_SKIRMISH_TEAM_SECONDS: f32 = gamelogic::ai::ai_player::DEFAULT_TEAM_SECONDS;
/// C++ m_resourcesPoor residual.
pub const AI_SKIRMISH_RESOURCES_POOR: i32 = gamelogic::ai::ai_player::RESOURCES_POOR;
/// C++ m_resourcesWealthy residual.
pub const AI_SKIRMISH_RESOURCES_WEALTHY: i32 = gamelogic::ai::ai_player::RESOURCES_WEALTHY;
/// C++ m_structuresPoorMod residual.
pub const AI_SKIRMISH_STRUCTURES_POOR_MOD: f32 = gamelogic::ai::ai_player::STRUCTURES_POOR_MODIFIER;
/// C++ m_structuresWealthyMod residual.
pub const AI_SKIRMISH_STRUCTURES_WEALTHY_MOD: f32 =
    gamelogic::ai::ai_player::STRUCTURES_WEALTHY_MODIFIER;
/// C++ m_teamsPoorMod residual.
pub const AI_SKIRMISH_TEAMS_POOR_MOD: f32 = gamelogic::ai::ai_player::TEAMS_POOR_MODIFIER;
/// C++ m_teamsWealthyMod residual.
pub const AI_SKIRMISH_TEAMS_WEALTHY_MOD: f32 = gamelogic::ai::ai_player::TEAMS_WEALTHY_MODIFIER;
/// C++ m_rebuildDelaySeconds residual.
pub const AI_SKIRMISH_REBUILD_DELAY_SECONDS: u32 = gamelogic::ai::ai_player::REBUILD_DELAY_SECONDS;
/// C++ m_skirmishBaseDefenseExtraDistance residual.
pub const AI_SKIRMISH_BASE_DEFENSE_EXTRA_DISTANCE: f32 =
    gamelogic::ai::ai_player::SKIRMISH_BASE_DEFENSE_EXTRA_DISTANCE;
/// Structure timer frames residual: structureSeconds * LOGICFRAMES_PER_SECOND.
pub const AI_SKIRMISH_STRUCTURE_TIMER_FRAMES: u32 =
    (AI_SKIRMISH_STRUCTURE_SECONDS as u32).saturating_mul(AI_SKIRMISH_LOGIC_FPS);
/// Team timer frames residual: teamSeconds * LOGICFRAMES_PER_SECOND.
pub const AI_SKIRMISH_TEAM_TIMER_FRAMES: u32 =
    (AI_SKIRMISH_TEAM_SECONDS as u32).saturating_mul(AI_SKIRMISH_LOGIC_FPS);

/// Honesty: AI skirmish residual pack (Wave 77).
///
/// Freezes AIPlayer/AIData default timer + wealth modifiers + base-defense
/// extra distance used by Medium skirmish AI residual path.
/// Fail-closed: not full AI.ini side build list / live dozer pathfinding.
pub fn honesty_ai_skirmish_residual_pack_wave77() -> bool {
    // Retail Default/AIData.ini:
    // StructureSeconds=0, TeamSeconds=10, Wealthy=7000, Poor=2000,
    // Structures/TeamsPoorRate=0.6, Structures/TeamsWealthyRate=2.0
    AI_SKIRMISH_LOGIC_FPS == 30
        && (AI_SKIRMISH_STRUCTURE_SECONDS - 0.0).abs() < 0.01
        && (AI_SKIRMISH_TEAM_SECONDS - 10.0).abs() < 0.01
        && AI_SKIRMISH_RESOURCES_POOR == 2000
        && AI_SKIRMISH_RESOURCES_WEALTHY == 7000
        && AI_SKIRMISH_RESOURCES_WEALTHY > AI_SKIRMISH_RESOURCES_POOR
        && (AI_SKIRMISH_STRUCTURES_POOR_MOD - 0.6).abs() < 0.01
        && (AI_SKIRMISH_STRUCTURES_WEALTHY_MOD - 2.0).abs() < 0.01
        && (AI_SKIRMISH_TEAMS_POOR_MOD - 0.6).abs() < 0.01
        && (AI_SKIRMISH_TEAMS_WEALTHY_MOD - 2.0).abs() < 0.01
        && AI_SKIRMISH_REBUILD_DELAY_SECONDS == 30
        && (AI_SKIRMISH_BASE_DEFENSE_EXTRA_DISTANCE - 150.0).abs() < 0.01
        && AI_SKIRMISH_STRUCTURE_TIMER_FRAMES == 0 // 0s * 30 FPS
        && AI_SKIRMISH_TEAM_TIMER_FRAMES == 300 // 10s * 30 FPS
        // C++ divides timer by rate: poor 0.6 slows, wealthy 2.0 speeds.
        && AI_SKIRMISH_STRUCTURES_POOR_MOD > 0.0
        && AI_SKIRMISH_STRUCTURES_POOR_MOD < 1.0
        && AI_SKIRMISH_STRUCTURES_WEALTHY_MOD > 1.0
}

#[derive(Debug, Clone)]
pub struct AiSkirmishActivityResult {
    pub config_applied: bool,
    pub ai_players: usize,
    pub frames_advanced: u32,
    pub activity_count: u64,
    pub ai_structures: usize,
    pub ai_units_or_queue: usize,
    pub difficulty: String,
    pub status: String,
}

/// Outcome of Medium AI after apply_skirmish_config → load_map preserve path.
#[derive(Debug, Clone)]
pub struct AiLoadMapActivityResult {
    pub config_applied: bool,
    pub map_loaded: bool,
    pub map_identity: String,
    pub cash_after_load: u32,
    pub cash_ok: bool,
    pub ai_active: bool,
    pub difficulty_medium: bool,
    pub frames_advanced: u32,
    pub activity_count: u64,
    pub ai_structures: usize,
    pub ai_units_or_queue: usize,
    /// True when AI issued at least one productive action after load_map/rebind.
    pub productive: bool,
    pub status: String,
}

fn ensure_human_templates(logic: &mut GameLogic) {
    for (name, kind, hp) in [
        ("HumanCC", KindOf::CommandCenter, 2000.0),
        ("HumanRanger", KindOf::Infantry, 120.0),
    ] {
        if logic.templates.contains_key(name) {
            continue;
        }
        let mut t = ThingTemplate::new(name);
        t.set_health(hp);
        t.set_cost(100, 0);
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(kind);
        logic.templates.insert(name.into(), t);
    }
}

fn count_ai_structures(logic: &GameLogic) -> usize {
    logic
        .get_objects()
        .values()
        .filter(|o| o.team == Team::GLA && o.is_kind_of(KindOf::Structure))
        .count()
}

fn count_ai_units_or_queue(logic: &GameLogic) -> usize {
    logic
        .get_objects()
        .values()
        .filter(|o| {
            o.team == Team::GLA
                && (o.is_kind_of(KindOf::Infantry)
                    || o.is_kind_of(KindOf::Vehicle)
                    || o.building_data
                        .as_ref()
                        .map(|b| !b.production_queue.is_empty())
                        .unwrap_or(false))
        })
        .count()
}

/// Resolve Lone Eagle (or synthetic identity) for load_map residual tests.
fn resolve_skirmish_map_path() -> (String, Option<PathBuf>) {
    const CANDIDATES: &[&str] = &[
        "windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
        "windows_game/extracted_big_files_v2/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
        "Maps/Lone Eagle/Lone Eagle.map",
        "Lone Eagle",
    ];
    // Walk from CARGO_MANIFEST_DIR (Main crate) and cwd parents so tests work
    // whether cargo's CWD is Code/Main, GeneralsRust, or the repo root.
    let mut roots: Vec<PathBuf> = Vec::new();
    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd.clone());
        roots.push(cwd.join(".."));
        roots.push(cwd.join("../.."));
        roots.push(cwd.join("../../.."));
    }
    // Main crate is GeneralsRust/Code/Main → three parents = repo root.
    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.."));
    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."));

    for root in &roots {
        for cand in CANDIDATES {
            let p = if Path::new(cand).is_absolute() {
                PathBuf::from(cand)
            } else {
                root.join(cand)
            };
            if p.is_file() {
                return (cand.to_string(), Some(p));
            }
        }
    }
    if let Some((id, path)) = crate::map_frame_scenario::resolve_first_map(CANDIDATES) {
        return (id, Some(path));
    }
    ("Lone Eagle".into(), None)
}

/// Run Medium GLA AI through host update path and measure production-linked activity.
pub fn run_medium_ai_skirmish_activity(frames: u32) -> AiSkirmishActivityResult {
    let config = golden_skirmish_config("AIActivityMap");
    let mut logic = GameLogic::new();
    let config_applied = apply_skirmish_config(&mut logic, &config).is_ok();
    ensure_human_templates(&mut logic);

    // Human presence for enemy assessment.
    let _ = logic.create_object("HumanCC", Team::USA, Vec3::new(-100.0, 0.0, -100.0));
    let _ = logic.create_object("HumanRanger", Team::USA, Vec3::new(-90.0, 0.0, -90.0));

    // Seed constructed GLA factories so production can run once AI queues teams.
    // AI still must start additional builds via process_building_queue.
    logic.ensure_ai_faction_templates(Team::GLA);
    for (name, pos) in [
        ("GLA_Barracks", Vec3::new(200.0, 0.0, 200.0)),
        ("GLA_ArmsDealer", Vec3::new(230.0, 0.0, 200.0)),
    ] {
        if let Some(id) = logic.create_object(name, Team::GLA, pos) {
            if let Some(obj) = logic.get_object_mut(id) {
                obj.status.under_construction = false;
                obj.construction_percent = 1.0;
            }
        }
    }

    let ai_players = logic.host_ai_player_count();
    let difficulty = logic.get_ai_status(1).unwrap_or_else(|| "missing".into());

    let frame_before = logic.get_frame();
    for _ in 0..frames.max(1) {
        logic.update();
    }
    let frames_advanced = logic.get_frame().saturating_sub(frame_before);

    let activity_count = logic.host_ai_activity_count();
    let ai_structures = count_ai_structures(&logic);
    let ai_units_or_queue = count_ai_units_or_queue(&logic);

    // Multi-interval depth: require more than a single one-shot action.
    let multi_action = activity_count >= 2
        || ai_structures >= 3
        || (activity_count >= 1 && ai_units_or_queue >= 1);
    let status = if config_applied && ai_players >= 1 && frames_advanced > 0 && multi_action {
        "success".into()
    } else {
        "partial".into()
    };

    let _ = AIDifficulty::Medium; // keep import path live for difficulty enum use
    AiSkirmishActivityResult {
        config_applied,
        ai_players,
        frames_advanced,
        activity_count,
        ai_structures,
        ai_units_or_queue,
        difficulty,
        status,
    }
}

/// Medium AI after apply_skirmish_config → load_map (preserve) → frames.
///
/// Fail-closed residual: not full C++ checkReadyTeams parity — only that the
/// host AI update path issues at least one productive build/produce action after
/// map load rebind, with cash retained (or topped up from empty).
pub fn run_medium_ai_after_load_map(frames: u32) -> AiLoadMapActivityResult {
    let (map_identity, map_path) = resolve_skirmish_map_path();
    let config = golden_skirmish_config(&map_identity);
    let mut logic = GameLogic::new();
    let config_applied = apply_skirmish_config(&mut logic, &config).is_ok();
    logic.ensure_ai_faction_templates(Team::USA);
    logic.ensure_ai_faction_templates(Team::GLA);
    ensure_human_templates(&mut logic);

    let cash_before = logic
        .get_player(1)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);

    let map_loaded = if let Some(ref path) = map_path {
        let s = path.to_string_lossy();
        logic.load_map(&s) || logic.load_map(&map_identity)
    } else {
        false
    };
    if !map_loaded {
        // Explicit rebind residual when retail map is unavailable in the workspace.
        logic.rebind_host_ai_after_map_load();
    }

    let cash_after_load = logic
        .get_player(1)
        .map(|p| p.resources.supplies)
        .unwrap_or(0);
    // Cash must survive preserve, or be topped up from empty by rebind residual.
    let cash_ok = cash_after_load >= 1_000
        && (cash_after_load >= cash_before.min(10_000) || cash_before == 0);

    // Human presence for enemy assessment after map wipe.
    let _ = logic.create_object("HumanCC", Team::USA, Vec3::new(-100.0, 0.0, -100.0));
    let _ = logic.create_object("HumanRanger", Team::USA, Vec3::new(-90.0, 0.0, -90.0));

    let difficulty_medium = matches!(logic.host_ai_difficulty(1), Some(AIDifficulty::Medium));
    let ai_active = logic.is_host_ai_active(1);

    let frame_before = logic.get_frame();
    for _ in 0..frames.max(1) {
        logic.update();
    }
    let frames_advanced = logic.get_frame().saturating_sub(frame_before);

    let activity_count = logic.host_ai_activity_count();
    let ai_structures = count_ai_structures(&logic);
    let ai_units_or_queue = count_ai_units_or_queue(&logic);

    // Productive: multi-structure base, unit production, or activity counter.
    let productive = activity_count >= 1
        && (ai_structures >= 2 || ai_units_or_queue >= 1 || activity_count >= 2);

    let status = if config_applied
        && cash_ok
        && ai_active
        && difficulty_medium
        && frames_advanced > 0
        && productive
    {
        "success".into()
    } else {
        "partial".into()
    };

    AiLoadMapActivityResult {
        config_applied,
        map_loaded,
        map_identity,
        cash_after_load,
        cash_ok,
        ai_active,
        difficulty_medium,
        frames_advanced,
        activity_count,
        ai_structures,
        ai_units_or_queue,
        productive,
        status,
    }
}

pub fn format_ai_activity_report(r: &AiSkirmishActivityResult) -> String {
    format!(
        "config_applied={} ai_players={} frames={} activity={} structures={} units_or_queue={} difficulty={} status={}",
        r.config_applied,
        r.ai_players,
        r.frames_advanced,
        r.activity_count,
        r.ai_structures,
        r.ai_units_or_queue,
        r.difficulty,
        r.status
    )
}

pub fn format_ai_load_map_report(r: &AiLoadMapActivityResult) -> String {
    format!(
        "config_applied={} map_loaded={} map={} cash={} cash_ok={} ai_active={} medium={} frames={} activity={} structures={} units_or_queue={} productive={} status={}",
        r.config_applied,
        r.map_loaded,
        r.map_identity,
        r.cash_after_load,
        r.cash_ok,
        r.ai_active,
        r.difficulty_medium,
        r.frames_advanced,
        r.activity_count,
        r.ai_structures,
        r.ai_units_or_queue,
        r.productive,
        r.status
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::BuildingType;

    #[test]
    fn medium_ai_is_non_idle_on_host_update_path() {
        let result = run_medium_ai_skirmish_activity(120);
        assert!(result.config_applied, "skirmish config must apply");
        assert!(
            result.ai_players >= 1,
            "Medium AI slot must register: {}",
            result.difficulty
        );
        assert!(result.frames_advanced > 0);
        assert!(
            result.activity_count >= 2
                || result.ai_structures >= 3
                || (result.activity_count >= 1 && result.ai_units_or_queue >= 1),
            "AI must show multi-interval activity: {}",
            format_ai_activity_report(&result)
        );
        assert_eq!(
            result.status,
            "success",
            "{}",
            format_ai_activity_report(&result)
        );
    }

    #[test]
    fn medium_ai_activity_grows_across_update_windows() {
        let config = golden_skirmish_config("AIGrowthMap");
        let mut logic = GameLogic::new();
        assert!(apply_skirmish_config(&mut logic, &config).is_ok());
        ensure_human_templates(&mut logic);
        let _ = logic.create_object("HumanCC", Team::USA, Vec3::new(-100.0, 0.0, -100.0));
        logic.ensure_ai_faction_templates(Team::GLA);
        logic.ensure_skirmish_ai_starting_cash(20_000);
        for _ in 0..30 {
            logic.update();
        }
        let after_first = logic.host_ai_activity_count();
        let structs_first = count_ai_structures(&logic);
        // Host residual base layout is multi-structure; can_afford must not gate on
        // template power draw or GLA stalls after the first Command Center.
        assert!(
            after_first >= 2 || structs_first >= 2,
            "first AI window must start multiple structures: act={after_first} structs={structs_first}"
        );
        for _ in 0..90 {
            logic.update();
        }
        let after_more = logic.host_ai_activity_count();
        let structs_more = count_ai_structures(&logic);
        let units_more = count_ai_units_or_queue(&logic);
        // After layout is filled, further growth is units/production or rebuilds.
        assert!(
            after_more >= after_first
                && (structs_more >= structs_first)
                && (after_more > after_first || structs_more > 1 || units_more >= 1),
            "AI must remain productive: act {after_first}->{after_more} structs {structs_first}->{structs_more} units={units_more}"
        );
    }

    /// Highest-value residual: after load_map preserve/rebind, Medium AI still
    /// builds/produces (not full checkReadyTeams C++ parity).
    #[test]
    fn medium_ai_takes_productive_action_after_load_map() {
        let result = run_medium_ai_after_load_map(150);
        assert!(
            result.config_applied,
            "skirmish config must apply: {}",
            format_ai_load_map_report(&result)
        );
        assert!(
            result.cash_ok,
            "AI cash must survive load_map or be topped up: {}",
            format_ai_load_map_report(&result)
        );
        assert!(
            result.cash_after_load >= 1_000,
            "AI needs enough cash to build: {}",
            format_ai_load_map_report(&result)
        );
        assert!(
            result.ai_active && result.difficulty_medium,
            "Medium AI must remain active after rebind: {}",
            format_ai_load_map_report(&result)
        );
        assert!(result.frames_advanced > 0);
        assert!(
            result.productive,
            "AI must take a productive action after load_map: {}",
            format_ai_load_map_report(&result)
        );
        assert_eq!(
            result.status,
            "success",
            "{}",
            format_ai_load_map_report(&result)
        );
    }

    /// Unit production residual: ArmsDealer is a vehicle factory and host AI
    /// queues GLA_Technical after factories exist (post rebind path).
    #[test]
    fn medium_ai_produces_units_from_factories_after_rebind() {
        let config = golden_skirmish_config("AIProduceMap");
        let mut logic = GameLogic::new();
        assert!(apply_skirmish_config(&mut logic, &config).is_ok());
        logic.ensure_ai_faction_templates(Team::GLA);
        ensure_human_templates(&mut logic);
        let _ = logic.create_object("HumanCC", Team::USA, Vec3::new(-100.0, 0.0, -100.0));

        // World wipe + rebind (load_map residual without requiring retail map bytes).
        logic.objects.clear();
        logic.rebind_host_ai_after_map_load();
        assert!(
            logic
                .get_player(1)
                .map(|p| p.resources.supplies)
                .unwrap_or(0)
                >= 10_000
        );

        // Seed constructed factories after rebind (map may not place GLA_*).
        for (name, pos) in [
            ("GLA_Barracks", Vec3::new(200.0, 0.0, 200.0)),
            ("GLA_ArmsDealer", Vec3::new(230.0, 0.0, 200.0)),
        ] {
            if let Some(id) = logic.create_object(name, Team::GLA, pos) {
                if let Some(obj) = logic.get_object_mut(id) {
                    obj.status.under_construction = false;
                    obj.construction_percent = 1.0;
                }
            }
        }

        // ArmsDealer must classify as WarFactory so Technical enqueue succeeds.
        let arms_type = logic
            .get_objects()
            .values()
            .find(|o| o.template_name == "GLA_ArmsDealer")
            .and_then(|o| o.building_data.as_ref().map(|b| b.building_type));
        assert_eq!(arms_type, Some(BuildingType::WarFactory));

        for _ in 0..120 {
            logic.update();
        }

        let units_or_queue = count_ai_units_or_queue(&logic);
        let activity = logic.host_ai_activity_count();
        assert!(
            units_or_queue >= 1 || activity >= 2,
            "AI must queue/produce units after rebind (units_or_queue={units_or_queue} activity={activity})"
        );
        // Strong preference: actual production enqueue or unit spawn.
        assert!(
            units_or_queue >= 1,
            "expected GLA unit or production queue after factories present (activity={activity})"
        );
    }

    /// Cash top-up residual: empty AI wallet after preserve is restored on rebind.
    #[test]
    fn rebind_tops_up_empty_ai_cash() {
        let config = golden_skirmish_config("AICashMap");
        let mut logic = GameLogic::new();
        assert!(apply_skirmish_config(&mut logic, &config).is_ok());
        if let Some(p) = logic.get_player_mut(1) {
            p.resources.supplies = 0;
        }
        logic.rebind_host_ai_after_map_load();
        assert_eq!(
            logic.get_player(1).map(|p| p.resources.supplies),
            Some(10_000),
            "empty AI cash must be topped up after rebind"
        );
    }

    /// Wave 77 residual: AI skirmish structure/team timer + wealth mod pack honesty.
    #[test]
    fn ai_skirmish_residual_pack_wave77_honesty() {
        assert!(honesty_ai_skirmish_residual_pack_wave77());
        assert_eq!(AI_SKIRMISH_LOGIC_FPS, 30);
        assert!((AI_SKIRMISH_STRUCTURE_SECONDS - 0.0).abs() < 0.01);
        assert!((AI_SKIRMISH_TEAM_SECONDS - 10.0).abs() < 0.01);
        assert_eq!(AI_SKIRMISH_STRUCTURE_TIMER_FRAMES, 0);
        assert_eq!(AI_SKIRMISH_TEAM_TIMER_FRAMES, 300);
        assert_eq!(AI_SKIRMISH_RESOURCES_POOR, 2000);
        assert_eq!(AI_SKIRMISH_RESOURCES_WEALTHY, 7000);
        assert!((AI_SKIRMISH_STRUCTURES_POOR_MOD - 0.6).abs() < 0.01);
        assert!((AI_SKIRMISH_TEAMS_POOR_MOD - 0.6).abs() < 0.01);
        assert!((AI_SKIRMISH_BASE_DEFENSE_EXTRA_DISTANCE - 150.0).abs() < 0.01);
        assert_eq!(AI_SKIRMISH_REBUILD_DELAY_SECONDS, 30);
    }
}
