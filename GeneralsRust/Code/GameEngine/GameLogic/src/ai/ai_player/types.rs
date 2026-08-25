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

/// C++ `AIPlayer::isLocationSafe` scan candidate (host leftover-call).
#[derive(Debug, Clone, Copy)]
pub struct LeftoverLocationSafeCandidate {
    pub x: f32,
    pub y: f32,
    pub is_destroyed: bool,
    pub is_effectively_dead: bool,
    pub is_harvester: bool,
    pub is_dozer: bool,
    pub stealthed: bool,
    pub detected: bool,
    pub disguised: bool,
    pub is_enemy: bool,
    pub is_bridge: bool,
    pub is_bridge_tower: bool,
}

/// C++ `TheAI->getAiData()->m_supplyCenterSafeRadius` + template bounding circle.
/// Leftover fallback is `SUPPLY_CENTER_SAFE_RADIUS` (100) when AIData is missing or ≤0.
pub fn leftover_is_location_safe_radius(
    aidata_supply_center_safe_radius: Option<f32>,
    template_bounding_circle: f32,
) -> f32 {
    let mut radius = aidata_supply_center_safe_radius
        .filter(|r| *r > 0.0)
        .unwrap_or(SUPPLY_CENTER_SAFE_RADIUS);
    radius += template_bounding_circle;
    radius
}

/// C++ `AIPlayer::isLocationSafe` partition filters. True → enemy blocks placement.
pub fn leftover_is_location_safe_enemy_blocks(c: &LeftoverLocationSafeCandidate) -> bool {
    // PartitionFilterAlive
    if c.is_destroyed || c.is_effectively_dead {
        return false;
    }
    // PartitionFilterRejectByKindOf harvester/dozer
    if c.is_harvester || c.is_dozer {
        return false;
    }
    // PartitionFilterRejectByObjectStatus stealthed (unless detected/disguised)
    if c.stealthed && !c.detected && !c.disguised {
        return false;
    }
    // PartitionFilterPlayerAffiliation: enemies only
    if !c.is_enemy {
        return false;
    }
    // PartitionFilterInsignificantBuildings(true, false)
    if c.is_bridge || c.is_bridge_tower {
        return false;
    }
    true
}

/// C++ `AIPlayer::isLocationSafe`. Leftover partition range is center-to-center.
/// Any filtered enemy inside `radius` → unsafe.
pub fn leftover_is_location_safe(
    pos_x: f32,
    pos_y: f32,
    radius: f32,
    candidates: impl IntoIterator<Item = LeftoverLocationSafeCandidate>,
) -> bool {
    let radius_sqr = radius * radius;
    for c in candidates {
        let dx = c.x - pos_x;
        let dy = c.y - pos_y;
        if dx * dx + dy * dy > radius_sqr {
            continue;
        }
        if leftover_is_location_safe_enemy_blocks(&c) {
            return false;
        }
    }
    true
}

/// C++ `AIPlayer::findSupplyCenter` warehouse snapshot (host leftover-call).
#[derive(Debug, Clone, Copy)]
pub struct LeftoverSupplyCenterCandidate {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub bounding_circle: f32,
    pub available_cash: i32,
    pub is_structure: bool,
    pub is_supply_source: bool,
    pub has_warehouse_dock: bool,
    pub is_enemy: bool,
}

/// Owned `KINDOF_CASH_GENERATOR` near a warehouse (host leftover-call).
#[derive(Debug, Clone, Copy)]
pub struct LeftoverOwnedCashGenerator {
    pub x: f32,
    pub y: f32,
}

/// C++ `boxes * TheGlobalData->m_baseValuePerSupplyBox`.
pub fn leftover_warehouse_available_cash(boxes: i32) -> i32 {
    boxes * BASE_VALUE_PER_SUPPLY_BOX
}

/// C++ `AIPlayer::findSupplyCenter` pick: warehouse dock, skip ENEMIES, skip own
/// cash-gen within `SUPPLY_CENTER_CLOSE_DIST` + bounding circle, skip 60/40
/// closer to enemy structure-bounds midpoint, closest to `m_baseCenter`,
/// halve cash floor until ≤ 100.
pub fn leftover_find_supply_center(
    candidates: &[LeftoverSupplyCenterCandidate],
    own_cash_gens: &[LeftoverOwnedCashGenerator],
    base_x: f32,
    base_y: f32,
    enemy_center: Option<(f32, f32)>,
    minimum_cash: i32,
) -> Option<u32> {
    let mut cash_floor = minimum_cash.max(0);
    loop {
        let mut best: Option<(f32, u32)> = None;
        for c in candidates {
            if !c.is_structure || !c.is_supply_source || !c.has_warehouse_dock {
                continue;
            }
            if c.is_enemy {
                continue;
            }
            if c.available_cash < cash_floor {
                continue;
            }
            let radius = SUPPLY_CENTER_CLOSE_DIST + c.bounding_circle;
            let already_have = own_cash_gens.iter().any(|g| {
                let dx = g.x - c.x;
                let dy = g.y - c.y;
                dx * dx + dy * dy <= radius * radius
            });
            if already_have {
                continue;
            }
            let dx = c.x - base_x;
            let dy = c.y - base_y;
            let dist_sqr = dx * dx + dy * dy;
            if let Some((ex, ey)) = enemy_center {
                let edx = c.x - ex;
                let edy = c.y - ey;
                let enemy_dist_sqr = edx * edx + edy * edy;
                // C++: closer than 60/40 to enemy than to us → skip
                if dist_sqr * 0.4 > enemy_dist_sqr * 0.6 {
                    continue;
                }
            }
            if best.as_ref().map_or(true, |(bd, _)| dist_sqr < *bd) {
                best = Some((dist_sqr, c.id));
            }
        }
        if let Some((_, id)) = best {
            return Some(id);
        }
        // C++: minimumCash /= 2; while (minimumCash > 100)
        cash_floor /= 2;
        if cash_floor <= 100 {
            break;
        }
    }
    None
}

/// C++ `AIPlayer::computeCenterAndRadiusOfBase`.
/// Entries are `(x, y, bounding_circle)`. Radius is hypot of axis-abs + geom*0.4.
/// Returns `(center_set, center_x, center_y, radius)`.
pub fn leftover_compute_center_and_radius_of_base(
    entries: &[(f32, f32, f32)],
) -> (bool, f32, f32, f32) {
    if entries.is_empty() {
        return (false, 0.0, 0.0, 0.0);
    }
    let n = entries.len() as f32;
    let cx = entries.iter().map(|e| e.0).sum::<f32>() / n;
    let cy = entries.iter().map(|e| e.1).sum::<f32>() / n;
    let mut max_rad_sqr = 0.0_f32;
    for &(x, y, bounding) in entries {
        let bldg_radius = bounding * 0.4;
        let mut dx = (x - cx).abs();
        let mut dy = (y - cy).abs();
        dx += bldg_radius;
        dy += bldg_radius;
        let rad_sqr = dx * dx + dy * dy;
        if rad_sqr > max_rad_sqr {
            max_rad_sqr = rad_sqr;
        }
    }
    (true, cx, cy, max_rad_sqr.sqrt())
}

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
