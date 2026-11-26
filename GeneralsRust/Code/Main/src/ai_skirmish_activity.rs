//! Host AI skirmish activity — Medium AI non-idle proof on the retail route.
//!
//! Every run starts the retail Lone Eagle skirmish the way the application
//! does ([`crate::parallel_harness::HeadlessSkirmish`]): the retail
//! definitions are admitted before NewGame (GameEngine.cpp:428-458), the golden
//! lobby is applied and the map places each side's starting base. Medium GLA
//! AI must then take productive actions (structure start, unit queue, or unit
//! spawn) from authored content alone; there are no hand templates and no
//! cash top-up (C++ AIPlayer never grants money).
//!
//! Wave 77 residual peels:
//! - AIData/AIPlayer structure/team timer defaults residual honesty pack
//! - poor/wealthy resource thresholds + build-speed modifiers residual
//! - skirmish base-defense extra distance residual

//!
//! Wave 956: host_object/host_objects authority dual-read seal.
//! Wave 958: host_object dual-read seal (tests + residual).
use crate::ai::AIDifficulty;
use crate::game_logic::{AIState, GameLogic, KindOf};
use crate::parallel_harness::{
    HARNESS_MAP, HarnessAssetError, HeadlessSkirmish, SharedSkirmishAssets,
};
use glam::Vec3;

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

/// Status of a run whose retail definitions or map are not installed.
pub const RETAIL_MISSING: &str = "retail_missing";

/// The AI slot of [`crate::skirmish_config::golden_skirmish_config`]
/// (slot 1, Medium GLA).
const AI_PLAYER: u32 = 1;
/// The human slot (slot 0, USA).
const HUMAN_PLAYER: u32 = 0;

/// Start the retail Lone Eagle skirmish exactly as the application does:
/// every definition admitted before NewGame (GameEngine.cpp:428-458), the
/// golden lobby applied, the map loaded and its starting buildings placed
/// (GameLogic.cpp:1949-1961). `Err` names the user-provided retail content
/// that is not installed. Installed content the world refuses is a defect,
/// not an absence, so it panics rather than reading as a skip.
fn start_retail_skirmish() -> Result<HeadlessSkirmish, String> {
    let assets = match SharedSkirmishAssets::load_retail() {
        Ok(assets) => assets,
        Err(HarnessAssetError::Missing(what)) => return Err(what),
        Err(error) => panic!("retail skirmish content refused: {error}"),
    };
    Ok(HeadlessSkirmish::start(&assets, 0)
        .unwrap_or_else(|error| panic!("retail skirmish failed to start: {error}")))
}

fn map_identity() -> String {
    HARNESS_MAP.into()
}

fn ai_owned<'a>(
    logic: &'a GameLogic,
) -> impl Iterator<Item = &'a crate::game_logic::object::Object> {
    logic
        .host_objects()
        .registered_values()
        .filter(|o| o.is_alive() && o.owner_player_id == Some(AI_PLAYER))
}

fn count_ai_structures(logic: &GameLogic) -> usize {
    ai_owned(logic)
        .filter(|o| o.is_kind_of(KindOf::Structure))
        .count()
}

fn count_ai_units_or_queue(logic: &GameLogic) -> usize {
    ai_owned(logic)
        .filter(|o| {
            o.is_kind_of(KindOf::Infantry)
                || o.is_kind_of(KindOf::Vehicle)
                || o.building_data
                    .as_ref()
                    .is_some_and(|b| !b.production_queue.is_empty())
        })
        .count()
}

fn advance(game: &mut HeadlessSkirmish, frames: u32) -> u32 {
    let before = game.frame();
    for _ in 0..frames {
        game.advance();
    }
    game.frame().saturating_sub(before)
}

/// Run Medium GLA AI on the retail skirmish and measure its activity.
pub fn run_medium_ai_skirmish_activity(frames: u32) -> AiSkirmishActivityResult {
    let mut game = match start_retail_skirmish() {
        Ok(game) => game,
        Err(reason) => {
            return AiSkirmishActivityResult {
                config_applied: false,
                ai_players: 0,
                frames_advanced: 0,
                activity_count: 0,
                ai_structures: 0,
                ai_units_or_queue: 0,
                difficulty: reason,
                status: RETAIL_MISSING.into(),
            };
        }
    };
    let ai_players = game.logic().host_ai_player_count();
    let difficulty = game
        .logic()
        .get_ai_status(AI_PLAYER)
        .unwrap_or_else(|| "missing".into());
    let frames_advanced = advance(&mut game, frames.max(1));
    let logic = game.logic();
    let activity_count = logic.host_ai_activity_count();
    let ai_structures = count_ai_structures(logic);
    let ai_units_or_queue = count_ai_units_or_queue(logic);
    let multi_action = activity_count >= 2
        || ai_structures >= 3
        || (activity_count >= 1 && ai_units_or_queue >= 1);
    let status = if ai_players >= 1 && frames_advanced > 0 && multi_action {
        "success"
    } else {
        "partial"
    };
    AiSkirmishActivityResult {
        config_applied: true,
        ai_players,
        frames_advanced,
        activity_count,
        ai_structures,
        ai_units_or_queue,
        difficulty,
        status: status.into(),
    }
}

fn load_map_result(game: &mut HeadlessSkirmish, frames: u32) -> AiLoadMapActivityResult {
    let cash_after_load = game
        .logic()
        .get_player(AI_PLAYER)
        .map(|p| p.effective_supplies())
        .unwrap_or(0);
    // The golden lobby grants 10,000 starting cash (C++ GameInfo money).
    let cash_ok = cash_after_load >= 1_000;
    let difficulty_medium = matches!(
        game.logic().host_ai_difficulty(AI_PLAYER),
        Some(AIDifficulty::Medium)
    );
    let ai_active = game.logic().is_host_ai_active(AI_PLAYER);
    let frames_advanced = advance(game, frames.max(1));
    let logic = game.logic();
    let activity_count = logic.host_ai_activity_count();
    let ai_structures = count_ai_structures(logic);
    let ai_units_or_queue = count_ai_units_or_queue(logic);
    let productive = activity_count >= 1
        && (ai_structures >= 2 || ai_units_or_queue >= 1 || activity_count >= 2);
    let status = if cash_ok && ai_active && difficulty_medium && frames_advanced > 0 && productive {
        "success"
    } else {
        "partial"
    };
    AiLoadMapActivityResult {
        config_applied: true,
        map_loaded: true,
        map_identity: map_identity(),
        cash_after_load,
        cash_ok,
        ai_active,
        difficulty_medium,
        frames_advanced,
        activity_count,
        ai_structures,
        ai_units_or_queue,
        productive,
        status: status.into(),
    }
}

fn missing_load_map_result(reason: String) -> AiLoadMapActivityResult {
    AiLoadMapActivityResult {
        config_applied: false,
        map_loaded: false,
        map_identity: reason,
        cash_after_load: 0,
        cash_ok: false,
        ai_active: false,
        difficulty_medium: false,
        frames_advanced: 0,
        activity_count: 0,
        ai_structures: 0,
        ai_units_or_queue: 0,
        productive: false,
        status: RETAIL_MISSING.into(),
    }
}

/// Medium AI after the retail map load: its starting base, cash and
/// difficulty survive NewGame and it acts within `frames`.
pub fn run_medium_ai_after_load_map(frames: u32) -> AiLoadMapActivityResult {
    match start_retail_skirmish() {
        Ok(mut game) => load_map_result(&mut game, frames),
        Err(reason) => missing_load_map_result(reason),
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

/// Retail map + Medium AI + a concurrent human move order on one world.
#[derive(Debug, Clone)]
pub struct AiRetailSkirmishOrdersResult {
    pub map_loaded: bool,
    pub map_identity: String,
    pub ai: AiLoadMapActivityResult,
    pub human_unit_id: Option<crate::game_logic::ObjectId>,
    pub human_order_issued: bool,
    pub human_order_visible: bool,
    pub status: String,
}

/// Run the retail skirmish with Medium AI and issue a human move mid-run
/// through the same `queue_command` path the UI uses.
pub fn run_ai_retail_skirmish_with_player_orders(frames: u32) -> AiRetailSkirmishOrdersResult {
    let mut game = match start_retail_skirmish() {
        Ok(game) => game,
        Err(reason) => {
            return AiRetailSkirmishOrdersResult {
                map_loaded: false,
                map_identity: reason.clone(),
                ai: missing_load_map_result(reason),
                human_unit_id: None,
                human_order_issued: false,
                human_order_visible: false,
                status: "map_missing".into(),
            };
        }
    };
    let half = (frames.max(2) / 2).max(1);
    let _ = advance(&mut game, half);
    // The human slot's starting mobile unit (the map's starting Dozer).
    let human_unit_id = game
        .logic()
        .host_objects()
        .registered_values()
        .find(|o| {
            o.is_alive()
                && o.owner_player_id == Some(HUMAN_PLAYER)
                && !o.is_kind_of(KindOf::Structure)
                && o.is_mobile()
        })
        .map(|o| o.id);
    let pos_before =
        human_unit_id.and_then(|id| game.logic().host_object(id).map(|o| o.get_position()));
    let human_order_issued = if let (Some(id), Some(at)) = (human_unit_id, pos_before) {
        game.issue(
            HUMAN_PLAYER,
            1,
            vec![id],
            crate::command_system::CommandType::Move {
                destination: at + Vec3::new(60.0, 0.0, 60.0),
            },
        );
        true
    } else {
        false
    };
    let ai = load_map_result(&mut game, frames.max(2) - half);
    let human_order_visible = match (human_unit_id, pos_before) {
        (Some(id), Some(before)) => game.logic().host_object(id).is_some_and(|o| {
            let after = o.get_position();
            let (dx, dz) = (after.x - before.x, after.z - before.z);
            dx * dx + dz * dz > 1.0 || o.ai_state() != AIState::Idle
        }),
        _ => false,
    };
    let status = if ai.status == "success" && human_order_issued && human_order_visible {
        "success"
    } else {
        "partial"
    };
    AiRetailSkirmishOrdersResult {
        map_loaded: true,
        map_identity: map_identity(),
        ai,
        human_unit_id,
        human_order_issued,
        human_order_visible,
        status: status.into(),
    }
}

pub fn format_ai_retail_skirmish_orders_report(r: &AiRetailSkirmishOrdersResult) -> String {
    format!(
        "map_loaded={} map={} ai_status={} activity={} structures={} units_or_queue={} human_order_issued={} human_order_visible={} status={}",
        r.map_loaded,
        r.map_identity,
        r.ai.status,
        r.ai.activity_count,
        r.ai.ai_structures,
        r.ai.ai_units_or_queue,
        r.human_order_issued,
        r.human_order_visible,
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

    /// Retail definitions and Lone Eagle are user-provided; without them the
    /// retail route has nothing to run.
    fn retail_or_skip(game: Result<HeadlessSkirmish, String>) -> Option<HeadlessSkirmish> {
        match game {
            Ok(game) => Some(game),
            Err(reason) => {
                eprintln!("skip: retail skirmish content unavailable: {reason}");
                None
            }
        }
    }

    #[test]
    fn medium_ai_is_non_idle_on_host_update_path() {
        let result = run_medium_ai_skirmish_activity(120);
        if result.status == RETAIL_MISSING {
            eprintln!("skip: {}", format_ai_activity_report(&result));
            return;
        }
        assert!(
            result.ai_players >= 1,
            "Medium AI slot must register: {}",
            result.difficulty
        );
        assert!(result.frames_advanced >= 120);
        assert_eq!(
            result.status,
            "success",
            "{}",
            format_ai_activity_report(&result)
        );
    }

    #[test]
    fn medium_ai_activity_grows_across_update_windows() {
        let Some(mut game) = retail_or_skip(start_retail_skirmish()) else {
            return;
        };
        advance(&mut game, 30);
        let after_first = game.logic().host_ai_activity_count();
        let structs_first = count_ai_structures(game.logic());
        assert!(
            structs_first >= 1,
            "the map places the AI's starting Command Center"
        );
        advance(&mut game, 90);
        let after_more = game.logic().host_ai_activity_count();
        let structs_more = count_ai_structures(game.logic());
        let units_more = count_ai_units_or_queue(game.logic());
        assert!(
            after_more >= after_first
                && structs_more >= structs_first
                && (after_more > after_first || structs_more > structs_first || units_more >= 1),
            "AI must remain productive: act {after_first}->{after_more} structs {structs_first}->{structs_more} units={units_more}"
        );
    }

    #[test]
    fn medium_ai_and_human_orders_on_retail_skirmish_map() {
        let result = run_ai_retail_skirmish_with_player_orders(180);
        if !result.map_loaded {
            eprintln!("skip: {}", format_ai_retail_skirmish_orders_report(&result));
            return;
        }
        assert_eq!(
            result.status,
            "success",
            "{}",
            format_ai_retail_skirmish_orders_report(&result)
        );
        assert!(result.ai.productive);
        assert!(result.human_order_issued && result.human_order_visible);
    }

    #[test]
    fn medium_ai_takes_productive_action_after_load_map() {
        let result = run_medium_ai_after_load_map(150);
        if result.status == RETAIL_MISSING {
            eprintln!("skip: {}", format_ai_load_map_report(&result));
            return;
        }
        assert!(result.cash_ok, "{}", format_ai_load_map_report(&result));
        assert!(
            result.ai_active && result.difficulty_medium,
            "{}",
            format_ai_load_map_report(&result)
        );
        assert!(result.productive, "{}", format_ai_load_map_report(&result));
        assert_eq!(
            result.status,
            "success",
            "{}",
            format_ai_load_map_report(&result)
        );
    }

    /// Medium GLA AI on the retail map eventually queues or produces units.
    #[test]
    fn medium_ai_produces_units_on_retail_map() {
        let Some(mut game) = retail_or_skip(start_retail_skirmish()) else {
            return;
        };
        let mut units_or_queue = 0;
        for _ in 0..60 {
            advance(&mut game, 60);
            units_or_queue = count_ai_units_or_queue(game.logic());
            if units_or_queue >= 1 {
                break;
            }
        }
        assert!(
            units_or_queue >= 1,
            "expected a GLA unit or production queue (activity={} structures={})",
            game.logic().host_ai_activity_count(),
            count_ai_structures(game.logic())
        );
    }

    #[test]
    fn ai_skirmish_timer_and_wealth_defaults_match_retail() {
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
        assert!(AI_SKIRMISH_RESOURCES_WEALTHY > AI_SKIRMISH_RESOURCES_POOR);
        assert!((AI_SKIRMISH_STRUCTURES_WEALTHY_MOD - 2.0).abs() < 0.01);
        assert!((AI_SKIRMISH_TEAMS_WEALTHY_MOD - 2.0).abs() < 0.01);
        assert!(AI_SKIRMISH_STRUCTURES_POOR_MOD > 0.0);
        assert!(AI_SKIRMISH_STRUCTURES_POOR_MOD < 1.0);
        assert!(AI_SKIRMISH_STRUCTURES_WEALTHY_MOD > 1.0);
    }
}
