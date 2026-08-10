//! Split from `ai/ai_player.rs` for module-size parity.
//! Observable AIPlayer behavior is unchanged.

#![allow(unused_imports)]

use super::*;

/// Invalid skill set selection constant
pub const INVALID_SKILLSET_SELECTION: i32 = -1;

/// Maximum number of structures to repair simultaneously
pub const MAX_STRUCTURES_TO_REPAIR: usize = 2;

// Constants from C++ AIData (AI.cpp)
// These match the defaults and are critical for AI behavior

/// Frames per second for timing calculations (C++ LOGICFRAMES_PER_SECOND)
pub const LOGICFRAMES_PER_SECOND: u32 = 30;

/// C++ `AIPlayer::doBaseBuilding` recheck: `m_buildDelay = 2*LOGICFRAMES_PER_SECOND`.
pub const BUILD_DELAY_RECHECK_FRAMES: u32 = 2 * LOGICFRAMES_PER_SECOND;

/// C++ `AIPlayer::doTeamBuilding` recheck: `m_teamDelay = 5*LOGICFRAMES_PER_SECOND`.
pub const TEAM_DELAY_RECHECK_FRAMES: u32 = 5 * LOGICFRAMES_PER_SECOND;

/// Default delay between team production in seconds.
/// Retail `Default/AIData.ini` TeamSeconds = 10 (C++ AIPlayer ctor uses AIData).
pub const DEFAULT_TEAM_SECONDS: f32 = 10.0;

/// Default delay between structure production in seconds.
/// Retail `Default/AIData.ini` StructureSeconds = 0 (try every ready tick).
pub const DEFAULT_STRUCTURE_SECONDS: f32 = 0.0;

/// Resource threshold for "poor" AI (retail AIData Poor = 2000).
pub const RESOURCES_POOR: i32 = 2000;

/// Resource threshold for "wealthy" AI (retail AIData Wealthy = 7000).
pub const RESOURCES_WEALTHY: i32 = 7000;

/// Build speed modifier when poor (retail AIData StructuresPoorRate = 0.6).
/// C++ divides the timer by this rate: 0.6 → slower when poor.
pub const STRUCTURES_POOR_MODIFIER: f32 = 0.6;

/// Build speed modifier when wealthy (retail AIData StructuresWealthyRate = 2.0).
pub const STRUCTURES_WEALTHY_MODIFIER: f32 = 2.0;

/// Team build speed modifier when poor (retail AIData TeamsPoorRate = 0.6).
pub const TEAMS_POOR_MODIFIER: f32 = 0.6;

/// Team build speed modifier when wealthy (retail AIData TeamsWealthyRate = 2.0).
pub const TEAMS_WEALTHY_MODIFIER: f32 = 2.0;

/// Delay before rebuilding destroyed structure in seconds.
/// Retail `Default/AIData.ini` RebuildDelayTimeSeconds = 30.
pub const REBUILD_DELAY_SECONDS: u32 = 30;

/// Team resource multiplier for affordability check (C++ m_teamResourcesToBuild)
pub const TEAM_RESOURCES_TO_BUILD: f32 = 0.5;

/// Supply center safe radius in units (C++ m_supplyCenterSafeRadius)
pub const SUPPLY_CENTER_SAFE_RADIUS: f32 = 100.0;

/// Skirmish base defense extra distance.
/// Retail `Default/AIData.ini` SkirmishBaseDefenseExtraDistance = 150.0.
pub const SKIRMISH_BASE_DEFENSE_EXTRA_DISTANCE: f32 = 150.0;

/// Close distance for supply center pathfinding (C++ SUPPLY_CENTER_CLOSE_DIST)
/// 20 * PATHFIND_CELL_SIZE_F where PATHFIND_CELL_SIZE_F = 10.0
pub const SUPPLY_CENTER_CLOSE_DIST: f32 = 200.0;

/// Huge distance constant for enemy prioritization (C++ HUGE_DIST)
pub const HUGE_DIST: f32 = 100000.0;

/// AI Player implementation
#[derive(Debug)]
pub struct AIPlayer {
    /// Player we represent
    pub(super) player_id: u32,

    /// Team build and ready queues
    pub(super) team_build_queue: VecDeque<TeamInQueue>,
    pub(super) team_ready_queue: VecDeque<TeamInQueue>,

    /// Timing and delays
    pub(super) ready_to_build_team: bool,
    pub(super) ready_to_build_structure: bool,
    pub(super) team_timer: u32,
    pub(super) structure_timer: u32,
    pub(super) team_seconds: Real,
    pub(super) structure_seconds: Real,
    pub(super) build_delay: u32,
    pub(super) team_delay: u32,
    pub(super) frame_last_building_built: u32,

    /// AI configuration
    pub(super) difficulty: GameDifficulty,
    pub(super) skillset_selector: i32,

    /// Base information
    pub(super) base_center: Coord3D,
    pub(super) base_center_set: bool,
    pub(super) base_radius: Real,

    /// Bridge repair system
    pub(super) structures_to_repair: [Option<ObjectID>; MAX_STRUCTURES_TO_REPAIR],
    pub(super) repair_dozer: Option<ObjectID>,
    pub(super) repair_dozer_origin: Coord3D,
    pub(super) structures_in_queue: i32,
    pub(super) dozer_queued_for_repair: bool,
    pub(super) dozer_is_repairing: bool,
    pub(super) bridge_timer: u32,

    /// Supply tracking
    pub(super) supply_source_attack_check_frame: u32,
    pub(super) attacked_supply_center: Option<ObjectID>,
    pub(super) current_warehouse_id: Option<ObjectID>,

    /// AI strategy state
    pub(super) strategy_state: AiStrategyState,

    /// Economic state
    pub(super) economic_state: AiEconomicState,

    /// Military state
    pub(super) military_state: AiMilitaryState,

    /// Construction priorities
    pub(super) construction_priorities: Vec<ConstructionPriority>,

    /// Threat assessment
    pub(super) threat_assessment: ThreatAssessment,

    /// Strategic decision maker (new integrated system)
    pub(super) strategic_decision_maker: StrategicDecisionMaker,

    /// Difficulty handler (new integrated system)
    pub(super) difficulty_handler: DifficultyHandler,

    /// Build order optimizer (new integrated system)
    pub(super) build_order_optimizer: BuildOrderOptimizer,

    /// Threat assessment system (new integrated system)
    pub(super) threat_system: ThreatAssessmentSystem,
}
