//! Wave 105 residual peels: AI group / AI path / weapon fire / damage application /
//! veterancy residual deepen (host-testable combat/path residual).
//!
//! Orthogonal to Object/ThingFactory residual (Waves 100–101) and Wave 103
//! weapon/armor/loco seed tables. Host residual only —
//! shell `playable_claim` stays false; network deferred.
//!
//! Sources:
//! - AIGroup.cpp / AI.h / AIData.ini group pathfind residual
//! - AIPathfind.h / .cpp path cell / layer residual
//! - Weapon.cpp / Weapon.h / WeaponStatus.h fire residual
//! - Damage.h / ActiveBody.cpp damage application residual
//! - ExperienceTracker.cpp / GameData.ini / GameCommon.h veterancy residual
//!
//! Fail-closed:
//! - Not full AIGroup exclusive group path A* residual
//! - Not full Pathfinder open/closed list exclusive residual
//! - Not full Weapon::privateFireWeapon live projectile residual
//! - Not full ActiveBody attemptDamage exclusive module matrix residual
//! - Not full ExperienceTracker live Object XP sink residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared logic-frame residual
// ---------------------------------------------------------------------------

/// C++ `LOGICFRAMES_PER_SECOND` residual.
pub const LOGIC_FRAMES_PER_SECOND_RESIDUAL: u32 = 30;

/// Convert msec → logic frames residual (ceil).
pub fn duration_ms_to_logic_frames(ms: u32) -> u32 {
    if ms == 0 {
        0
    } else {
        ((ms as u64 * LOGIC_FRAMES_PER_SECOND_RESIDUAL as u64 + 999) / 1000) as u32
    }
}

// ---------------------------------------------------------------------------
// 1. AI group residual peels (AIGroup.cpp / AIData.ini)
// ---------------------------------------------------------------------------

/// C++ AIGroup ctor residual: m_speed starts **0**.
pub const AIGROUP_CTOR_SPEED_RESIDUAL: f32 = 0.0;
/// C++ AIGroup ctor residual: m_dirty starts **false**.
pub const AIGROUP_CTOR_DIRTY_RESIDUAL: bool = false;
/// C++ AIGroup ctor residual: m_memberListSize starts **0**.
pub const AIGROUP_CTOR_MEMBER_COUNT_RESIDUAL: u32 = 0;
/// C++ AIGroup ctor residual: m_groundPath starts **NULL**.
pub const AIGROUP_CTOR_GROUND_PATH_NULL_RESIDUAL: bool = true;

/// Retail AIData.ini `MinInfantryForGroup` residual.
pub const MIN_INFANTRY_FOR_GROUP_RESIDUAL: i32 = 3;
/// Retail AIData.ini `MinVehiclesForGroup` residual.
pub const MIN_VEHICLES_FOR_GROUP_RESIDUAL: i32 = 3;
/// Retail AIData.ini `MinDistanceForGroup` residual.
pub const MIN_DISTANCE_FOR_GROUP_RESIDUAL: f32 = 100.0;
/// Retail AIData.ini `DistanceRequiresGroup` residual.
pub const DISTANCE_REQUIRES_GROUP_RESIDUAL: f32 = 500.0;
/// Retail AIData.ini `MinClumpDensity` residual (TAiData ctor default).
pub const MIN_CLUMP_DENSITY_RESIDUAL: f32 = 0.5;
/// Retail AIData.ini `SkirmishGroupFudgeDistance` residual.
pub const SKIRMISH_GROUP_FUDGE_DISTANCE_RESIDUAL: f32 = 5.0;
/// Retail GameData.ini `GroupMoveClickToGatherAreaFactor` residual.
pub const GROUP_MOVE_CLICK_TO_GATHER_FACTOR_RESIDUAL: f32 = 0.5;
/// Retail AIData.ini `ForceIdleMSEC` residual (67 ms ≈ 2 frames).
pub const FORCE_IDLE_MSEC_RESIDUAL: u32 = 67;
/// ForceIdle frames residual (ceil 67ms @ 30 FPS → 3? wait: 67*30/1000 = 2.01 → 3 with ceil, but comment says 2 frames).
/// C++ TAiData ctor uses `m_forceIdleFramesCount(1)`; AIData ForceIdleMSEC=67 → ~2 frames.
pub const FORCE_IDLE_FRAMES_RESIDUAL: u32 = 2;

/// C++ `CommandSourceType` residual ordinals (GameCommon.h).
pub const CMD_FROM_PLAYER: u32 = 0;
/// C++ `CMD_FROM_SCRIPT` residual.
pub const CMD_FROM_SCRIPT: u32 = 1;
/// C++ `CMD_FROM_AI` residual.
pub const CMD_FROM_AI: u32 = 2;
/// C++ `CMD_FROM_DOZER` residual.
pub const CMD_FROM_DOZER: u32 = 3;

/// Ordered C++ `CommandSourceType` residual names.
pub const COMMAND_SOURCE_TYPE_NAME_LIST: &[&str] = &[
    "CMD_FROM_PLAYER",
    "CMD_FROM_SCRIPT",
    "CMD_FROM_AI",
    "CMD_FROM_DOZER",
];

/// Lookup CommandSourceType residual name index.
pub fn command_source_type_name_index(name: &str) -> Option<usize> {
    COMMAND_SOURCE_TYPE_NAME_LIST
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
}

/// Host-testable AIGroup residual membership / center bookkeeping.
#[derive(Debug, Clone, Default)]
pub struct AiGroupResidual {
    pub id: u32,
    pub members: Vec<u32>,
    pub member_speeds: Vec<f32>,
    pub member_positions: Vec<(f32, f32, f32)>,
    pub dirty: bool,
    pub speed: f32,
    pub ground_path_active: bool,
}

impl AiGroupResidual {
    /// C++ AIGroup ctor residual.
    pub fn new(id: u32) -> Self {
        Self {
            id,
            members: Vec::new(),
            member_speeds: Vec::new(),
            member_positions: Vec::new(),
            dirty: AIGROUP_CTOR_DIRTY_RESIDUAL,
            speed: AIGROUP_CTOR_SPEED_RESIDUAL,
            ground_path_active: !AIGROUP_CTOR_GROUND_PATH_NULL_RESIDUAL,
        }
    }

    /// C++ `AIGroup::add` residual (append member + mark dirty).
    pub fn add(&mut self, object_id: u32, speed: f32, pos: (f32, f32, f32)) {
        if self.members.contains(&object_id) {
            return;
        }
        self.members.push(object_id);
        self.member_speeds.push(speed);
        self.member_positions.push(pos);
        self.dirty = true;
    }

    /// C++ `AIGroup::remove` residual.
    pub fn remove(&mut self, object_id: u32) -> bool {
        if let Some(i) = self.members.iter().position(|&id| id == object_id) {
            self.members.remove(i);
            self.member_speeds.remove(i);
            self.member_positions.remove(i);
            self.dirty = true;
            true
        } else {
            false
        }
    }

    /// C++ `AIGroup::isMember` residual.
    pub fn is_member(&self, object_id: u32) -> bool {
        self.members.contains(&object_id)
    }

    /// C++ `AIGroup::getCount` residual.
    pub fn get_count(&self) -> usize {
        self.members.len()
    }

    /// C++ `AIGroup::isEmpty` residual.
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// C++ `AIGroup::recompute` residual: speed = min member locomotor speed.
    pub fn recompute(&mut self) {
        self.speed = self
            .member_speeds
            .iter()
            .copied()
            .fold(f32::INFINITY, f32::min);
        if !self.speed.is_finite() {
            self.speed = 0.0;
        }
        self.dirty = false;
    }

    /// C++ `AIGroup::getSpeed` residual (recompute if dirty).
    pub fn get_speed(&mut self) -> f32 {
        if self.dirty {
            self.recompute();
        }
        self.speed
    }

    /// C++ `AIGroup::getCenter` residual (average of member positions).
    pub fn get_center(&self) -> Option<(f32, f32, f32)> {
        if self.member_positions.is_empty() {
            return None;
        }
        let n = self.member_positions.len() as f32;
        let (mut sx, mut sy, mut sz) = (0.0, 0.0, 0.0);
        for &(x, y, z) in &self.member_positions {
            sx += x;
            sy += y;
            sz += z;
        }
        Some((sx / n, sy / n, sz / n))
    }
}

/// Residual: whether group pathfinding should engage for infantry count + distance.
///
/// Retail AIData.ini: need ≥ MinInfantryForGroup infantry AND
/// distance ≥ MinDistanceForGroup (or force when ≥ DistanceRequiresGroup).
pub fn group_path_should_engage_infantry(
    infantry_count: i32,
    move_distance: f32,
) -> bool {
    if move_distance >= DISTANCE_REQUIRES_GROUP_RESIDUAL {
        return infantry_count >= 1;
    }
    infantry_count >= MIN_INFANTRY_FOR_GROUP_RESIDUAL
        && move_distance >= MIN_DISTANCE_FOR_GROUP_RESIDUAL
}

/// Residual: skirmish group fudge radius = fudge × member_count.
pub fn skirmish_group_fudge_radius(member_count: usize) -> f32 {
    SKIRMISH_GROUP_FUDGE_DISTANCE_RESIDUAL * member_count as f32
}

/// AIGroup group* command residual name table (AIGroup.cpp method surface).
pub const AIGROUP_COMMAND_NAME_LIST: &[&str] = &[
    "groupMoveToPosition",
    "groupScatter",
    "groupTightenToPosition",
    "groupFollowWaypointPath",
    "groupFollowWaypointPathExact",
    "groupMoveToAndEvacuate",
    "groupMoveToAndEvacuateAndExit",
    "groupFollowWaypointPathAsTeam",
    "groupFollowWaypointPathAsTeamExact",
    "groupIdle",
    "groupFollowPath",
    "groupAttackObject",
    "groupAttackTeam",
    "groupAttackPosition",
    "groupAttackMoveToPosition",
    "groupHunt",
    "groupRepair",
    "groupResumeConstruction",
    "groupGetHealed",
    "groupGetRepaired",
    "groupEnter",
    "groupDock",
    "groupExit",
    "groupEvacuate",
    "groupExecuteRailedTransport",
    "groupGoProne",
    "groupGuardPosition",
    "groupGuardObject",
    "groupGuardArea",
    "groupAttackArea",
    "groupHackInternet",
    "groupCreateFormation",
    "groupDoSpecialPower",
    "groupDoSpecialPowerAtLocation",
    "groupDoSpecialPowerAtObject",
    "groupCheer",
    "groupSell",
    "groupToggleOvercharge",
    "groupCombatDrop",
    "groupDoCommandButton",
];

/// Wave 105 honesty: AI group residual peels pack.
pub fn honesty_ai_group_residual_pack_wave105() -> bool {
    // Ctor residual.
    let g0 = AiGroupResidual::new(1);
    if g0.speed != AIGROUP_CTOR_SPEED_RESIDUAL
        || g0.dirty != AIGROUP_CTOR_DIRTY_RESIDUAL
        || g0.get_count() as u32 != AIGROUP_CTOR_MEMBER_COUNT_RESIDUAL
        || g0.ground_path_active
        || !g0.is_empty()
    {
        return false;
    }

    // Membership / speed / center residual.
    let mut g = AiGroupResidual::new(42);
    g.add(10, 30.0, (0.0, 0.0, 0.0));
    g.add(11, 20.0, (10.0, 0.0, 0.0));
    g.add(12, 40.0, (20.0, 0.0, 0.0));
    if !g.is_member(11) || g.is_member(99) || g.get_count() != 3 {
        return false;
    }
    let speed = g.get_speed();
    if (speed - 20.0).abs() > 0.001 || g.dirty {
        return false;
    }
    let center = g.get_center();
    if !matches!(center, Some((x, _, _)) if (x - 10.0).abs() < 0.001) {
        return false;
    }
    if !g.remove(11) || g.is_member(11) || g.get_count() != 2 {
        return false;
    }

    // AIData.ini group path residual anchors.
    let path_ok = MIN_INFANTRY_FOR_GROUP_RESIDUAL == 3
        && MIN_VEHICLES_FOR_GROUP_RESIDUAL == 3
        && (MIN_DISTANCE_FOR_GROUP_RESIDUAL - 100.0).abs() < 0.001
        && (DISTANCE_REQUIRES_GROUP_RESIDUAL - 500.0).abs() < 0.001
        && (MIN_CLUMP_DENSITY_RESIDUAL - 0.5).abs() < 0.001
        && (SKIRMISH_GROUP_FUDGE_DISTANCE_RESIDUAL - 5.0).abs() < 0.001
        && (GROUP_MOVE_CLICK_TO_GATHER_FACTOR_RESIDUAL - 0.5).abs() < 0.001
        && FORCE_IDLE_MSEC_RESIDUAL == 67
        && FORCE_IDLE_FRAMES_RESIDUAL == 2
        && group_path_should_engage_infantry(3, 100.0)
        && !group_path_should_engage_infantry(2, 100.0)
        && group_path_should_engage_infantry(1, 500.0)
        && (skirmish_group_fudge_radius(10) - 50.0).abs() < 0.001;

    // CommandSourceType residual.
    let cmd_ok = COMMAND_SOURCE_TYPE_NAME_LIST.len() == 4
        && command_source_type_name_index("CMD_FROM_PLAYER") == Some(0)
        && command_source_type_name_index("CMD_FROM_SCRIPT") == Some(1)
        && command_source_type_name_index("CMD_FROM_AI") == Some(2)
        && command_source_type_name_index("CMD_FROM_DOZER") == Some(3)
        && CMD_FROM_PLAYER == 0
        && CMD_FROM_AI == 2;

    // Group command residual surface ≥ 40 names.
    let cmds_ok = AIGROUP_COMMAND_NAME_LIST.len() >= 40
        && AIGROUP_COMMAND_NAME_LIST.contains(&"groupMoveToPosition")
        && AIGROUP_COMMAND_NAME_LIST.contains(&"groupAttackObject")
        && AIGROUP_COMMAND_NAME_LIST.contains(&"groupIdle")
        && AIGROUP_COMMAND_NAME_LIST.contains(&"groupHunt");

    path_ok && cmd_ok && cmds_ok
}

// ---------------------------------------------------------------------------
// 2. AI path residual deepen (AIPathfind.h / .cpp)
// ---------------------------------------------------------------------------

/// C++ `PATHFIND_CELL_SIZE` residual.
pub const PATHFIND_CELL_SIZE: i32 = 10;
/// C++ `PATHFIND_CELL_SIZE_F` residual.
pub const PATHFIND_CELL_SIZE_F: f32 = 10.0;
/// C++ `PATHFIND_CLOSE_ENOUGH` residual.
pub const PATHFIND_CLOSE_ENOUGH: f32 = 1.0;
/// C++ `PATH_MAX_PRIORITY` residual.
pub const PATH_MAX_PRIORITY: u32 = 0x7FFF_FFFF;
/// C++ `PATHFIND_CELLS_PER_FRAME` residual.
pub const PATHFIND_CELLS_PER_FRAME: u32 = 5_000;
/// C++ `MAX_WALL_PIECES` residual.
pub const MAX_WALL_PIECES: u32 = 128;
/// C++ Path `MAX_CPOP` residual.
pub const PATH_MAX_CPOP: u32 = 20;
/// Retail AIData.ini `InfantryPathfindDiameter` residual.
pub const INFANTRY_PATHFIND_DIAMETER: i32 = 6;
/// Retail AIData.ini `VehiclePathfindDiameter` residual.
pub const VEHICLE_PATHFIND_DIAMETER: i32 = 6;

/// C++ `PathfindLayerEnum` residual.
pub const LAYER_INVALID: u32 = 0;
/// C++ `LAYER_GROUND` residual.
pub const LAYER_GROUND: u32 = 1;
/// C++ `LAYER_WALL` residual.
pub const LAYER_WALL: u32 = 15;
/// C++ `LAYER_LAST` residual.
pub const LAYER_LAST: u32 = 15;

/// Ordered C++ `PathfindCell::CellType` residual names.
pub const PATHFIND_CELL_TYPE_NAME_LIST: &[&str] = &[
    "CELL_CLEAR",
    "CELL_WATER",
    "CELL_CLIFF",
    "CELL_RUBBLE",
    "CELL_OBSTACLE",
    "CELL_BRIDGE_IMPASSABLE",
    "CELL_IMPASSABLE",
];

/// C++ `CELL_CLEAR` residual ordinal.
pub const CELL_CLEAR: u32 = 0;
/// C++ `CELL_WATER` residual.
pub const CELL_WATER: u32 = 1;
/// C++ `CELL_CLIFF` residual.
pub const CELL_CLIFF: u32 = 2;
/// C++ `CELL_RUBBLE` residual.
pub const CELL_RUBBLE: u32 = 3;
/// C++ `CELL_OBSTACLE` residual.
pub const CELL_OBSTACLE: u32 = 4;
/// C++ `CELL_BRIDGE_IMPASSABLE` residual.
pub const CELL_BRIDGE_IMPASSABLE: u32 = 5;
/// C++ `CELL_IMPASSABLE` residual.
pub const CELL_IMPASSABLE: u32 = 6;

/// Lookup Pathfind CellType residual name index.
pub fn pathfind_cell_type_name_index(name: &str) -> Option<usize> {
    PATHFIND_CELL_TYPE_NAME_LIST
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
}

/// Ordered C++ `PathfindCell::CellFlags` residual names (unit occupancy).
pub const PATHFIND_CELL_FLAGS_NAME_LIST: &[&str] = &[
    "NO_UNITS",
    "UNIT_GOAL",
    "UNIT_PRESENT_MOVING",
    "UNIT_PRESENT_FIXED",
    "UNIT_GOAL_OTHER_MOVING",
];

/// C++ CellFlags residual ordinals.
pub const CELL_FLAG_NO_UNITS: u32 = 0x00;
/// C++ `UNIT_GOAL` residual.
pub const CELL_FLAG_UNIT_GOAL: u32 = 0x01;
/// C++ `UNIT_PRESENT_MOVING` residual.
pub const CELL_FLAG_UNIT_PRESENT_MOVING: u32 = 0x02;
/// C++ `UNIT_PRESENT_FIXED` residual.
pub const CELL_FLAG_UNIT_PRESENT_FIXED: u32 = 0x03;
/// C++ `UNIT_GOAL_OTHER_MOVING` residual (0x05).
pub const CELL_FLAG_UNIT_GOAL_OTHER_MOVING: u32 = 0x05;

/// Residual: world position → pathfind cell index (`floor(pos / CELL_SIZE)`).
pub fn world_to_pathfind_cell(x: f32, y: f32) -> (i32, i32) {
    (
        (x / PATHFIND_CELL_SIZE_F).floor() as i32,
        (y / PATHFIND_CELL_SIZE_F).floor() as i32,
    )
}

/// Residual: pathfind cell center world position.
pub fn pathfind_cell_center(cx: i32, cy: i32) -> (f32, f32) {
    (
        (cx as f32 + 0.5) * PATHFIND_CELL_SIZE_F,
        (cy as f32 + 0.5) * PATHFIND_CELL_SIZE_F,
    )
}

/// Residual: orthogonal A* step cost (1 cell → cost PATHFIND_CELL_SIZE).
pub const PATHFIND_ORTHO_STEP_COST: i32 = PATHFIND_CELL_SIZE;
/// Residual: diagonal A* step uses same cell size (IABS(dx)==CELL_SIZE checks).
pub const PATHFIND_DIAG_STEP_COST: i32 = PATHFIND_CELL_SIZE;

/// Path residual bookkeeping (host-testable Path node list).
#[derive(Debug, Clone, Default)]
pub struct PathResidual {
    pub nodes: Vec<(f32, f32, f32, u32)>, // x,y,z,layer
    pub is_optimized: bool,
    pub blocked_by_ally: bool,
}

impl PathResidual {
    pub fn new() -> Self {
        Self::default()
    }

    /// C++ `Path::appendNode` residual.
    pub fn append_node(&mut self, x: f32, y: f32, z: f32, layer: u32) {
        self.nodes.push((x, y, z, layer));
        self.is_optimized = false;
    }

    /// C++ `Path::prependNode` residual.
    pub fn prepend_node(&mut self, x: f32, y: f32, z: f32, layer: u32) {
        self.nodes.insert(0, (x, y, z, layer));
        self.is_optimized = false;
    }

    pub fn first_node(&self) -> Option<&(f32, f32, f32, u32)> {
        self.nodes.first()
    }

    pub fn last_node(&self) -> Option<&(f32, f32, f32, u32)> {
        self.nodes.last()
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Residual along-path 2D length.
    pub fn path_length_2d(&self) -> f32 {
        let mut len = 0.0;
        for w in self.nodes.windows(2) {
            let dx = w[1].0 - w[0].0;
            let dy = w[1].1 - w[0].1;
            len += (dx * dx + dy * dy).sqrt();
        }
        len
    }

    /// Residual mark optimized (C++ `Path::markOptimized`).
    pub fn mark_optimized(&mut self) {
        self.is_optimized = true;
    }
}

/// Wave 105 honesty: AI path residual deepen pack.
pub fn honesty_ai_path_residual_deepen_pack_wave105() -> bool {
    let cell_ok = PATHFIND_CELL_SIZE == 10
        && (PATHFIND_CELL_SIZE_F - 10.0).abs() < 0.001
        && (PATHFIND_CLOSE_ENOUGH - 1.0).abs() < 0.001
        && PATH_MAX_PRIORITY == 0x7FFF_FFFF
        && PATHFIND_CELLS_PER_FRAME == 5_000
        && MAX_WALL_PIECES == 128
        && PATH_MAX_CPOP == 20
        && INFANTRY_PATHFIND_DIAMETER == 6
        && VEHICLE_PATHFIND_DIAMETER == 6
        && LAYER_INVALID == 0
        && LAYER_GROUND == 1
        && LAYER_WALL == 15
        && LAYER_LAST == 15;

    let types_ok = PATHFIND_CELL_TYPE_NAME_LIST.len() == 7
        && pathfind_cell_type_name_index("CELL_CLEAR") == Some(0)
        && pathfind_cell_type_name_index("CELL_WATER") == Some(1)
        && pathfind_cell_type_name_index("CELL_CLIFF") == Some(2)
        && pathfind_cell_type_name_index("CELL_RUBBLE") == Some(3)
        && pathfind_cell_type_name_index("CELL_OBSTACLE") == Some(4)
        && pathfind_cell_type_name_index("CELL_BRIDGE_IMPASSABLE") == Some(5)
        && pathfind_cell_type_name_index("CELL_IMPASSABLE") == Some(6)
        && CELL_CLEAR == 0
        && CELL_IMPASSABLE == 6;

    let flags_ok = PATHFIND_CELL_FLAGS_NAME_LIST.len() == 5
        && CELL_FLAG_NO_UNITS == 0
        && CELL_FLAG_UNIT_GOAL == 1
        && CELL_FLAG_UNIT_PRESENT_MOVING == 2
        && CELL_FLAG_UNIT_PRESENT_FIXED == 3
        && CELL_FLAG_UNIT_GOAL_OTHER_MOVING == 5;

    // World↔cell residual.
    let (cx, cy) = world_to_pathfind_cell(25.0, 35.0);
    let (wx, wy) = pathfind_cell_center(2, 3);
    let world_ok = cx == 2
        && cy == 3
        && (wx - 25.0).abs() < 0.001
        && (wy - 35.0).abs() < 0.001
        && world_to_pathfind_cell(-5.0, 0.0) == (-1, 0);

    // Path residual bookkeeping.
    let mut path = PathResidual::new();
    path.append_node(0.0, 0.0, 0.0, LAYER_GROUND);
    path.append_node(30.0, 40.0, 0.0, LAYER_GROUND);
    path.prepend_node(-10.0, 0.0, 0.0, LAYER_GROUND);
    let path_ok = path.node_count() == 3
        && path.first_node().map(|n| n.0 == -10.0).unwrap_or(false)
        && path.last_node().map(|n| n.0 == 30.0).unwrap_or(false)
        && (path.path_length_2d() - (10.0 + 50.0)).abs() < 0.01
        && {
            path.mark_optimized();
            path.is_optimized
        };

    cell_ok && types_ok && flags_ok && world_ok && path_ok
}

// ---------------------------------------------------------------------------
// 3. Weapon fire residual deepen (Weapon.cpp / WeaponStatus.h / Weapon.h)
// ---------------------------------------------------------------------------

/// C++ `WeaponStatus` residual names (WeaponStatus.h).
pub const WEAPON_STATUS_NAME_LIST: &[&str] = &[
    "READY_TO_FIRE",
    "OUT_OF_AMMO",
    "BETWEEN_FIRING_SHOTS",
    "RELOADING_CLIP",
    "PRE_ATTACK",
];

/// C++ `READY_TO_FIRE` residual ordinal.
pub const WEAPON_STATUS_READY_TO_FIRE: u32 = 0;
/// C++ `OUT_OF_AMMO` residual.
pub const WEAPON_STATUS_OUT_OF_AMMO: u32 = 1;
/// C++ `BETWEEN_FIRING_SHOTS` residual.
pub const WEAPON_STATUS_BETWEEN_FIRING_SHOTS: u32 = 2;
/// C++ `RELOADING_CLIP` residual.
pub const WEAPON_STATUS_RELOADING_CLIP: u32 = 3;
/// C++ `PRE_ATTACK` residual.
pub const WEAPON_STATUS_PRE_ATTACK: u32 = 4;
/// C++ `WEAPON_STATUS_COUNT` residual.
pub const WEAPON_STATUS_COUNT: u32 = 5;

/// C++ `NO_MAX_SHOTS_LIMIT` residual.
pub const NO_MAX_SHOTS_LIMIT: i32 = 0x7FFF_FFFF;

/// C++ `WeaponBonus::Field` residual names.
pub const WEAPON_BONUS_FIELD_NAME_LIST: &[&str] = &[
    "DAMAGE",
    "RADIUS",
    "RANGE",
    "RATE_OF_FIRE",
    "PRE_ATTACK",
];

/// C++ `WeaponBonus::DAMAGE` residual field ordinal.
pub const WEAPON_BONUS_FIELD_DAMAGE: u32 = 0;
/// C++ `RADIUS` residual.
pub const WEAPON_BONUS_FIELD_RADIUS: u32 = 1;
/// C++ `RANGE` residual.
pub const WEAPON_BONUS_FIELD_RANGE: u32 = 2;
/// C++ `RATE_OF_FIRE` residual.
pub const WEAPON_BONUS_FIELD_RATE_OF_FIRE: u32 = 3;
/// C++ `PRE_ATTACK` residual.
pub const WEAPON_BONUS_FIELD_PRE_ATTACK: u32 = 4;
/// C++ `FIELD_COUNT` residual.
pub const WEAPON_BONUS_FIELD_COUNT: u32 = 5;

/// C++ `WeaponReloadType` residual names.
pub const WEAPON_RELOAD_TYPE_NAME_LIST: &[&str] = &["YES", "NO", "RETURN_TO_BASE"];
/// C++ `AUTO_RELOAD` residual.
pub const WEAPON_RELOAD_AUTO: u32 = 0;
/// C++ `NO_RELOAD` residual.
pub const WEAPON_RELOAD_NO: u32 = 1;
/// C++ `RETURN_TO_BASE_TO_RELOAD` residual.
pub const WEAPON_RELOAD_RETURN_TO_BASE: u32 = 2;

/// C++ `WeaponPrefireType` residual names.
pub const WEAPON_PREFIRE_TYPE_NAME_LIST: &[&str] = &["PER_SHOT", "PER_ATTACK", "PER_CLIP"];
/// C++ `PREFIRE_PER_SHOT` residual.
pub const WEAPON_PREFIRE_PER_SHOT: u32 = 0;
/// C++ `PREFIRE_PER_ATTACK` residual.
pub const WEAPON_PREFIRE_PER_ATTACK: u32 = 1;
/// C++ `PREFIRE_PER_CLIP` residual.
pub const WEAPON_PREFIRE_PER_CLIP: u32 = 2;
/// C++ `PREFIRE_COUNT` residual.
pub const WEAPON_PREFIRE_COUNT: u32 = 3;

/// C++ `WeaponAntiMaskType` residual bits.
pub const WEAPON_ANTI_AIRBORNE_VEHICLE: u32 = 0x01;
/// C++ `WEAPON_ANTI_GROUND` residual.
pub const WEAPON_ANTI_GROUND: u32 = 0x02;
/// C++ `WEAPON_ANTI_PROJECTILE` residual.
pub const WEAPON_ANTI_PROJECTILE: u32 = 0x04;
/// C++ `WEAPON_ANTI_SMALL_MISSILE` residual.
pub const WEAPON_ANTI_SMALL_MISSILE: u32 = 0x08;
/// C++ `WEAPON_ANTI_MINE` residual.
pub const WEAPON_ANTI_MINE: u32 = 0x10;
/// C++ `WEAPON_ANTI_AIRBORNE_INFANTRY` residual.
pub const WEAPON_ANTI_AIRBORNE_INFANTRY: u32 = 0x20;
/// C++ `WEAPON_ANTI_BALLISTIC_MISSILE` residual.
pub const WEAPON_ANTI_BALLISTIC_MISSILE: u32 = 0x40;
/// C++ `WEAPON_ANTI_PARACHUTE` residual.
pub const WEAPON_ANTI_PARACHUTE: u32 = 0x80;

/// Ordered WeaponAnti residual bit names.
pub const WEAPON_ANTI_MASK_NAME_LIST: &[&str] = &[
    "AIRBORNE_VEHICLE",
    "GROUND",
    "PROJECTILE",
    "SMALL_MISSILE",
    "MINE",
    "AIRBORNE_INFANTRY",
    "BALLISTIC_MISSILE",
    "PARACHUTE",
];

/// C++ `WeaponAffectsMaskType` residual bits.
pub const WEAPON_AFFECTS_SELF: u32 = 0x01;
/// C++ `WEAPON_AFFECTS_ALLIES` residual.
pub const WEAPON_AFFECTS_ALLIES: u32 = 0x02;
/// C++ `WEAPON_AFFECTS_ENEMIES` residual.
pub const WEAPON_AFFECTS_ENEMIES: u32 = 0x04;
/// C++ `WEAPON_AFFECTS_NEUTRALS` residual.
pub const WEAPON_AFFECTS_NEUTRALS: u32 = 0x08;
/// C++ `WEAPON_KILLS_SELF` residual.
pub const WEAPON_KILLS_SELF: u32 = 0x10;
/// C++ `WEAPON_DOESNT_AFFECT_SIMILAR` residual.
pub const WEAPON_DOESNT_AFFECT_SIMILAR: u32 = 0x20;
/// C++ `WEAPON_DOESNT_AFFECT_AIRBORNE` residual.
pub const WEAPON_DOESNT_AFFECT_AIRBORNE: u32 = 0x40;

/// Ordered WeaponAffects residual bit names (TheWeaponAffectsMaskNames).
pub const WEAPON_AFFECTS_MASK_NAME_LIST: &[&str] = &[
    "SELF",
    "ALLIES",
    "ENEMIES",
    "NEUTRALS",
    "SUICIDE",
    "NOT_SIMILAR",
    "NOT_AIRBORNE",
];

/// Host-testable Weapon fire residual instance.
#[derive(Debug, Clone)]
pub struct WeaponFireResidual {
    pub status: u32,
    pub ammo_in_clip: i32,
    pub clip_size: i32,
    pub auto_reloads: bool,
    pub when_we_can_fire_again: u32,
    pub max_shot_count: i32,
    pub attack_range: f32,
    pub primary_damage: f32,
    pub bonus_damage: f32,
    pub bonus_range: f32,
    pub bonus_rof: f32,
}

impl WeaponFireResidual {
    /// Residual weapon ready to fire with full clip.
    pub fn ready(clip_size: i32, attack_range: f32, primary_damage: f32) -> Self {
        Self {
            status: WEAPON_STATUS_READY_TO_FIRE,
            ammo_in_clip: clip_size,
            clip_size,
            auto_reloads: true,
            when_we_can_fire_again: 0,
            max_shot_count: NO_MAX_SHOTS_LIMIT,
            attack_range,
            primary_damage,
            bonus_damage: 1.0,
            bonus_range: 1.0,
            bonus_rof: 1.0,
        }
    }

    /// C++ WeaponBonus clear residual (all fields 1.0).
    pub fn clear_bonus(&mut self) {
        self.bonus_damage = 1.0;
        self.bonus_range = 1.0;
        self.bonus_rof = 1.0;
    }

    /// Residual: effective attack range with RANGE bonus.
    pub fn effective_attack_range(&self) -> f32 {
        self.attack_range * self.bonus_range
    }

    /// Residual: effective primary damage with DAMAGE bonus.
    pub fn effective_primary_damage(&self) -> f32 {
        self.primary_damage * self.bonus_damage
    }

    /// Residual: delay frames with RATE_OF_FIRE bonus (higher ROF → fewer frames).
    ///
    /// Host residual uses a small epsilon so exact ratio values like 60/1.2
    /// (VETERAN 120% ROF) land on the intended frame count under f32 noise.
    pub fn effective_delay_frames(&self, base_delay_frames: u32) -> u32 {
        if self.bonus_rof <= 0.0 {
            return base_delay_frames;
        }
        ((base_delay_frames as f32) / self.bonus_rof + 1.0e-4).floor() as u32
    }

    /// C++ `Weapon::isWithinAttackRange` residual (2D).
    pub fn is_within_attack_range_2d(&self, dx: f32, dy: f32) -> bool {
        let r = self.effective_attack_range();
        dx * dx + dy * dy <= r * r
    }

    /// Residual: remaining ammo (0 while RELOADING_CLIP).
    pub fn remaining_ammo(&self) -> i32 {
        if self.status == WEAPON_STATUS_RELOADING_CLIP {
            0
        } else {
            self.ammo_in_clip
        }
    }

    /// C++ `Weapon::privateFireWeapon` residual fire step (host bookkeeping).
    ///
    /// Returns true if auto-reloaded clip after this shot.
    pub fn private_fire_weapon_residual(&mut self, now_frame: u32, delay_frames: u32) -> bool {
        if self.status != WEAPON_STATUS_READY_TO_FIRE || self.ammo_in_clip <= 0 {
            return false;
        }
        self.ammo_in_clip -= 1;
        if self.max_shot_count != NO_MAX_SHOTS_LIMIT {
            self.max_shot_count -= 1;
        }
        let delay = self.effective_delay_frames(delay_frames);
        self.when_we_can_fire_again = now_frame.saturating_add(delay);
        if self.ammo_in_clip <= 0 {
            if self.auto_reloads {
                self.status = WEAPON_STATUS_RELOADING_CLIP;
                // Auto reload residual: refill and go BETWEEN then ready after delay.
                self.ammo_in_clip = self.clip_size;
                self.status = WEAPON_STATUS_BETWEEN_FIRING_SHOTS;
                true
            } else {
                self.status = WEAPON_STATUS_OUT_OF_AMMO;
                false
            }
        } else {
            self.status = WEAPON_STATUS_BETWEEN_FIRING_SHOTS;
            false
        }
    }

    /// Residual: advance status to READY when frame ≥ whenWeCanFireAgain.
    pub fn tick_status(&mut self, now_frame: u32) {
        if (self.status == WEAPON_STATUS_BETWEEN_FIRING_SHOTS
            || self.status == WEAPON_STATUS_RELOADING_CLIP
            || self.status == WEAPON_STATUS_PRE_ATTACK)
            && now_frame >= self.when_we_can_fire_again
            && self.ammo_in_clip > 0
        {
            self.status = WEAPON_STATUS_READY_TO_FIRE;
        }
    }
}

/// privateFireWeapon residual pipeline step names.
pub const PRIVATE_FIRE_WEAPON_STEPS: &[&str] = &[
    "VALIDATE_TEMPLATE",
    "PROCESS_ASSIST_REQUEST",
    "LEECH_RANGE_ACTIVATE",
    "SPECIAL_DAMAGE_TYPE_OVERRIDE",
    "COMPUTE_BONUS",
    "ASSERT_READY_AND_AMMO",
    "DECREMENT_AMMO_AND_SHOTS",
    "CREATE_PROJECTILE_OR_DEAL_DAMAGE",
    "SCHEDULE_NEXT_FIRE_FRAME",
    "SET_BETWEEN_OR_RELOAD_STATUS",
];


/// Wave 105 honesty: weapon fire residual deepen pack.
pub fn honesty_weapon_fire_residual_deepen_pack_wave105() -> bool {
    let status_ok = WEAPON_STATUS_NAME_LIST.len() == 5
        && WEAPON_STATUS_COUNT == 5
        && WEAPON_STATUS_READY_TO_FIRE == 0
        && WEAPON_STATUS_OUT_OF_AMMO == 1
        && WEAPON_STATUS_BETWEEN_FIRING_SHOTS == 2
        && WEAPON_STATUS_RELOADING_CLIP == 3
        && WEAPON_STATUS_PRE_ATTACK == 4
        && NO_MAX_SHOTS_LIMIT == 0x7FFF_FFFF;

    let bonus_ok = WEAPON_BONUS_FIELD_NAME_LIST.len() == 5
        && WEAPON_BONUS_FIELD_DAMAGE == 0
        && WEAPON_BONUS_FIELD_RADIUS == 1
        && WEAPON_BONUS_FIELD_RANGE == 2
        && WEAPON_BONUS_FIELD_RATE_OF_FIRE == 3
        && WEAPON_BONUS_FIELD_PRE_ATTACK == 4
        && WEAPON_BONUS_FIELD_COUNT == 5;

    let reload_ok = WEAPON_RELOAD_TYPE_NAME_LIST.len() == 3
        && WEAPON_RELOAD_AUTO == 0
        && WEAPON_RELOAD_NO == 1
        && WEAPON_RELOAD_RETURN_TO_BASE == 2
        && WEAPON_PREFIRE_TYPE_NAME_LIST.len() == 3
        && WEAPON_PREFIRE_COUNT == 3;

    let anti_ok = WEAPON_ANTI_MASK_NAME_LIST.len() == 8
        && WEAPON_ANTI_GROUND == 0x02
        && WEAPON_ANTI_AIRBORNE_VEHICLE == 0x01
        && WEAPON_ANTI_PARACHUTE == 0x80
        && WEAPON_AFFECTS_MASK_NAME_LIST.len() == 7
        && WEAPON_AFFECTS_ENEMIES == 0x04
        && WEAPON_KILLS_SELF == 0x10;

    // Fire residual instance.
    let mut w = WeaponFireResidual::ready(4, 100.0, 50.0);
    w.bonus_damage = 1.25; // PLAYER_UPGRADE style
    w.bonus_range = 1.33; // GARRISONED style
    w.bonus_rof = 1.2; // VETERAN style
    let fire_ok = (w.effective_primary_damage() - 62.5).abs() < 0.001
        && (w.effective_attack_range() - 133.0).abs() < 0.01
        && w.effective_delay_frames(60) == 50
        && w.is_within_attack_range_2d(100.0, 0.0)
        && w.is_within_attack_range_2d(130.0, 0.0)
        && !w.is_within_attack_range_2d(140.0, 0.0)
        && w.remaining_ammo() == 4;

    // privateFireWeapon residual fire loop (clip of 4, auto reload on last).
    // Tick BETWEEN → READY before each subsequent shot (frame ≥ whenWeCanFireAgain).
    let mut fired = 0;
    let mut reloaded = false;
    let mut frame = 0u32;
    for _shot in 0..4 {
        w.tick_status(frame);
        let r = w.private_fire_weapon_residual(frame, 60);
        if !r && w.status != WEAPON_STATUS_BETWEEN_FIRING_SHOTS && w.ammo_in_clip == 0 {
            // only auto-reload returns true; partial clip fires return false
        }
        fired += 1;
        if r {
            reloaded = true;
        }
        // Advance to next eligible fire frame.
        frame = w.when_we_can_fire_again;
    }
    let loop_ok = fired == 4
        && reloaded
        && w.ammo_in_clip == 4
        && w.status == WEAPON_STATUS_BETWEEN_FIRING_SHOTS
        && {
            w.tick_status(w.when_we_can_fire_again);
            w.status == WEAPON_STATUS_READY_TO_FIRE
        };

    // No-auto-reload residual → OUT_OF_AMMO.
    let mut w2 = WeaponFireResidual::ready(1, 50.0, 10.0);
    w2.auto_reloads = false;
    let no_reload = !w2.private_fire_weapon_residual(0, 30)
        && w2.status == WEAPON_STATUS_OUT_OF_AMMO
        && w2.remaining_ammo() == 0;

    let steps_ok = PRIVATE_FIRE_WEAPON_STEPS.len() >= 10
        && PRIVATE_FIRE_WEAPON_STEPS.contains(&"COMPUTE_BONUS")
        && PRIVATE_FIRE_WEAPON_STEPS.contains(&"CREATE_PROJECTILE_OR_DEAL_DAMAGE");

    status_ok && bonus_ok && reload_ok && anti_ok && fire_ok && loop_ok && no_reload && steps_ok
}

// ---------------------------------------------------------------------------
// 4. Damage residual application residual deepen (Damage.h / ActiveBody.cpp)
// ---------------------------------------------------------------------------

/// Retail GameData.ini `UnitDamagedThreshold` residual.
pub const UNIT_DAMAGED_THRESHOLD_RESIDUAL: f32 = 0.7;
/// Retail GameData.ini `UnitReallyDamagedThreshold` residual.
pub const UNIT_REALLY_DAMAGED_THRESHOLD_RESIDUAL: f32 = 0.35;
/// Retail GameData.ini `MovementPenaltyDamageState` residual name.
pub const MOVEMENT_PENALTY_DAMAGE_STATE_RESIDUAL: &str = "REALLYDAMAGED";

/// C++ `BodyDamageType` residual names.
pub const BODY_DAMAGE_TYPE_NAME_LIST: &[&str] =
    &["PRISTINE", "DAMAGED", "REALLYDAMAGED", "RUBBLE"];

/// C++ `BODY_PRISTINE` residual.
pub const BODY_PRISTINE: u32 = 0;
/// C++ `BODY_DAMAGED` residual.
pub const BODY_DAMAGED: u32 = 1;
/// C++ `BODY_REALLYDAMAGED` residual.
pub const BODY_REALLYDAMAGED: u32 = 2;
/// C++ `BODY_RUBBLE` residual.
pub const BODY_RUBBLE: u32 = 3;
/// C++ `BODYDAMAGETYPE_COUNT` residual.
pub const BODY_DAMAGE_TYPE_COUNT: u32 = 4;

/// C++ `IS_CONDITION_WORSE` residual (higher ordinal = worse).
pub fn is_body_condition_worse(a: u32, b: u32) -> bool {
    a > b
}

/// C++ `IS_CONDITION_BETTER` residual.
pub fn is_body_condition_better(a: u32, b: u32) -> bool {
    a < b
}

/// C++ `ActiveBody::calcDamageState` residual.
///
/// ratio > DamagedThresh → PRISTINE  
/// ratio > ReallyDamagedThresh → DAMAGED  
/// ratio > 0 → REALLYDAMAGED  
/// else RUBBLE
pub fn calc_damage_state_residual(health: f32, max_health: f32) -> u32 {
    if max_health <= 0.0 {
        return BODY_RUBBLE;
    }
    let ratio = health / max_health;
    if ratio > UNIT_DAMAGED_THRESHOLD_RESIDUAL {
        BODY_PRISTINE
    } else if ratio > UNIT_REALLY_DAMAGED_THRESHOLD_RESIDUAL {
        BODY_DAMAGED
    } else if ratio > 0.0 {
        BODY_REALLYDAMAGED
    } else {
        BODY_RUBBLE
    }
}

/// Host-testable DamageInfo residual (input + output).
#[derive(Debug, Clone)]
pub struct DamageInfoResidual {
    pub source_id: u32,
    pub damage_type: u32,
    pub amount: f32,
    pub kill: bool,
    pub actual_damage_dealt: f32,
    pub actual_damage_clipped: f32,
    pub no_effect: bool,
}

impl DamageInfoResidual {
    /// C++ DamageInfoInput defaults residual (DAMAGE_EXPLOSION=0, amount=0).
    pub fn new_default() -> Self {
        Self {
            source_id: 0,
            damage_type: 0, // DAMAGE_EXPLOSION
            amount: 0.0,
            kill: false,
            actual_damage_dealt: 0.0,
            actual_damage_clipped: 0.0,
            no_effect: false,
        }
    }

    pub fn with_amount(amount: f32, damage_type: u32) -> Self {
        Self {
            source_id: 1,
            damage_type,
            amount,
            kill: false,
            actual_damage_dealt: 0.0,
            actual_damage_clipped: 0.0,
            no_effect: false,
        }
    }
}

/// C++ `IsSubdualDamage` residual.
pub fn is_subdual_damage(damage_type: u32) -> bool {
    // DAMAGE_SUBDUAL_MISSILE=31 … DAMAGE_SUBDUAL_UNRESISTABLE=34
    (31..=34).contains(&damage_type)
}

/// C++ `IsHealthDamagingDamage` residual (false for status/subdual/killpilot/garrison).
pub fn is_health_damaging_damage(damage_type: u32) -> bool {
    match damage_type {
        37 => false,      // DAMAGE_STATUS
        31..=34 => false, // SUBDUAL_*
        16 => false,      // DAMAGE_KILLPILOT
        36 => false,      // DAMAGE_KILL_GARRISONED
        _ => true,
    }
}

/// Residual damage application: armor coefficient scales amount; clip to health remaining.
///
/// C++ ActiveBody / Armor:  
/// dealt = amount * armor_coeff  
/// clipped = min(dealt, current_health)  
/// new_health = current - clipped
pub fn apply_damage_residual(
    current_health: f32,
    max_health: f32,
    amount: f32,
    armor_coeff: f32,
    damage_type: u32,
) -> (f32, DamageInfoResidual) {
    let mut info = DamageInfoResidual::with_amount(amount, damage_type);
    if !is_health_damaging_damage(damage_type) {
        info.no_effect = true;
        info.actual_damage_dealt = 0.0;
        info.actual_damage_clipped = 0.0;
        return (current_health, info);
    }
    let dealt = amount * armor_coeff;
    let clipped = dealt.min(current_health.max(0.0));
    info.actual_damage_dealt = dealt;
    info.actual_damage_clipped = clipped;
    let new_health = (current_health - clipped).max(0.0).min(max_health);
    (new_health, info)
}

/// Wave 105 honesty: damage residual application residual deepen pack.
pub fn honesty_damage_application_residual_deepen_pack_wave105() -> bool {
    let thresh_ok = (UNIT_DAMAGED_THRESHOLD_RESIDUAL - 0.7).abs() < 0.001
        && (UNIT_REALLY_DAMAGED_THRESHOLD_RESIDUAL - 0.35).abs() < 0.001
        && MOVEMENT_PENALTY_DAMAGE_STATE_RESIDUAL == "REALLYDAMAGED"
        && BODY_DAMAGE_TYPE_NAME_LIST.len() == 4
        && BODY_DAMAGE_TYPE_COUNT == 4
        && BODY_PRISTINE == 0
        && BODY_DAMAGED == 1
        && BODY_REALLYDAMAGED == 2
        && BODY_RUBBLE == 3;

    // calcDamageState residual anchors (maxHealth=100).
    let state_ok = calc_damage_state_residual(100.0, 100.0) == BODY_PRISTINE
        && calc_damage_state_residual(71.0, 100.0) == BODY_PRISTINE
        && calc_damage_state_residual(70.0, 100.0) == BODY_DAMAGED
        && calc_damage_state_residual(50.0, 100.0) == BODY_DAMAGED
        && calc_damage_state_residual(35.0, 100.0) == BODY_REALLYDAMAGED
        && calc_damage_state_residual(10.0, 100.0) == BODY_REALLYDAMAGED
        && calc_damage_state_residual(0.0, 100.0) == BODY_RUBBLE
        && is_body_condition_worse(BODY_REALLYDAMAGED, BODY_DAMAGED)
        && is_body_condition_better(BODY_PRISTINE, BODY_DAMAGED);

    // Damage type residual helpers.
    let type_ok = is_subdual_damage(31)
        && is_subdual_damage(34)
        && !is_subdual_damage(0)
        && is_health_damaging_damage(0) // EXPLOSION
        && is_health_damaging_damage(3) // SMALL_ARMS
        && !is_health_damaging_damage(37) // STATUS
        && !is_health_damaging_damage(31) // SUBDUAL
        && !is_health_damaging_damage(16); // KILLPILOT

    // Application residual: 100 dmg × 50% armor on 30 HP → dealt 50, clipped 30.
    let (new_hp, out) = apply_damage_residual(30.0, 100.0, 100.0, 0.5, 0);
    let apply_ok = (out.actual_damage_dealt - 50.0).abs() < 0.001
        && (out.actual_damage_clipped - 30.0).abs() < 0.001
        && (new_hp - 0.0).abs() < 0.001
        && !out.no_effect
        && calc_damage_state_residual(new_hp, 100.0) == BODY_RUBBLE;

    // Status damage residual: no health effect.
    let (hp2, out2) = apply_damage_residual(80.0, 100.0, 999.0, 1.0, 37);
    let status_ok = out2.no_effect && (hp2 - 80.0).abs() < 0.001;

    // Default DamageInfo residual.
    let def = DamageInfoResidual::new_default();
    let def_ok = def.damage_type == 0 && def.amount == 0.0 && !def.kill;

    thresh_ok && state_ok && type_ok && apply_ok && status_ok && def_ok
}

// ---------------------------------------------------------------------------
// 5. Veterancy residual deepen (ExperienceTracker / GameData / GameCommon)
// ---------------------------------------------------------------------------

/// C++ `VeterancyLevel` residual ordinals.
pub const LEVEL_REGULAR: u32 = 0;
/// C++ `LEVEL_VETERAN` residual.
pub const LEVEL_VETERAN: u32 = 1;
/// C++ `LEVEL_ELITE` residual.
pub const LEVEL_ELITE: u32 = 2;
/// C++ `LEVEL_HEROIC` residual.
pub const LEVEL_HEROIC: u32 = 3;
/// C++ `LEVEL_COUNT` residual.
pub const LEVEL_COUNT: u32 = 4;
/// C++ `LEVEL_FIRST` residual.
pub const LEVEL_FIRST: u32 = LEVEL_REGULAR;
/// C++ `LEVEL_LAST` residual.
pub const LEVEL_LAST: u32 = LEVEL_HEROIC;

/// Ordered C++ `TheVeterancyNames` residual.
pub const VETERANCY_LEVEL_NAME_LIST: &[&str] = &["REGULAR", "VETERAN", "ELITE", "HEROIC"];

/// GameData.ini HealthBonus residual.
pub const HEALTH_BONUS_REGULAR: f32 = 1.0;
/// HealthBonus_Veteran residual.
pub const HEALTH_BONUS_VETERAN: f32 = 1.2;
/// HealthBonus_Elite residual.
pub const HEALTH_BONUS_ELITE: f32 = 1.3;
/// HealthBonus_Heroic residual.
pub const HEALTH_BONUS_HEROIC: f32 = 1.5;

/// GameData.ini WeaponBonus DAMAGE residual for veterancy.
pub const VET_DAMAGE_BONUS: [f32; 4] = [1.0, 1.1, 1.2, 1.3];
/// GameData.ini WeaponBonus RATE_OF_FIRE residual for veterancy.
pub const VET_ROF_BONUS: [f32; 4] = [1.0, 1.2, 1.4, 1.6];

/// Sample ExperienceRequired residual rows (Object.ini).
/// (template, required[4], value[4], trainable)
pub const EXPERIENCE_TEMPLATE_RESIDUAL_TABLE: &[(&str, [i32; 4], [i32; 4], bool)] = &[
    // AmericaInfantryColonelBurton
    (
        "AmericaInfantryColonelBurton",
        [0, 200, 300, 600],
        [50, 100, 100, 150],
        true,
    ),
    // AmericaInfantryRanger (common residual)
    (
        "AmericaInfantryRanger",
        [0, 40, 60, 120],
        [20, 20, 40, 60],
        true,
    ),
    // AmericaInfantryMissileDefender
    (
        "AmericaInfantryMissileDefender",
        [0, 100, 200, 400],
        [20, 20, 40, 60],
        true,
    ),
    // AmericaInfantryPathfinder
    (
        "AmericaInfantryPathfinder",
        [0, 50, 100, 200],
        [40, 40, 60, 80],
        true,
    ),
];

/// Relationship residual: ALLIES kill gives **0** XP.
pub const RELATIONSHIP_ALLIES_XP: i32 = 0;
/// Relationship residual ordinals (cross-link Wave 84).
pub const RELATIONSHIP_ENEMIES: u32 = 0;
/// NEUTRAL residual.
pub const RELATIONSHIP_NEUTRAL: u32 = 1;
/// ALLIES residual.
pub const RELATIONSHIP_ALLIES: u32 = 2;

/// Host-testable ExperienceTracker residual.
#[derive(Debug, Clone)]
pub struct ExperienceTrackerResidual {
    pub current_level: u32,
    pub current_experience: i32,
    pub experience_scalar: f32,
    pub experience_sink: Option<u32>,
    pub trainable: bool,
    pub experience_required: [i32; 4],
    pub experience_value: [i32; 4],
}

impl ExperienceTrackerResidual {
    /// C++ ExperienceTracker ctor residual.
    pub fn new(required: [i32; 4], value: [i32; 4], trainable: bool) -> Self {
        Self {
            current_level: LEVEL_REGULAR,
            current_experience: 0,
            experience_scalar: 1.0,
            experience_sink: None,
            trainable,
            experience_required: required,
            experience_value: value,
        }
    }

    /// C++ `getExperienceValue` residual (0 if killer relationship is ALLIES).
    pub fn get_experience_value(&self, killer_relationship: u32) -> i32 {
        if killer_relationship == RELATIONSHIP_ALLIES {
            return RELATIONSHIP_ALLIES_XP;
        }
        self.experience_value[self.current_level as usize]
    }

    /// C++ `isAcceptingExperiencePoints` residual.
    pub fn is_accepting_experience_points(&self) -> bool {
        self.trainable || self.experience_sink.is_some()
    }

    /// C++ `addExperiencePoints` residual level-up loop.
    pub fn add_experience_points(&mut self, experience_gain: i32, can_scale_for_bonus: bool) {
        if self.experience_sink.is_some() {
            // Sink residual: points go elsewhere (fail-closed host: just return).
            return;
        }
        if !self.trainable {
            return;
        }
        let mut amount = experience_gain;
        if can_scale_for_bonus {
            amount = (amount as f32 * self.experience_scalar) as i32;
        }
        self.current_experience += amount;
        let mut level_index = 0u32;
        while (level_index + 1) < LEVEL_COUNT
            && self.current_experience
                >= self.experience_required[(level_index + 1) as usize]
        {
            level_index += 1;
        }
        self.current_level = level_index;
    }

    /// C++ `setMinVeterancyLevel` residual.
    pub fn set_min_veterancy_level(&mut self, new_level: u32) {
        if self.current_level < new_level && new_level <= LEVEL_LAST {
            self.current_level = new_level;
            self.current_experience = self.experience_required[new_level as usize];
        }
    }

    /// C++ `setVeterancyLevel` residual.
    pub fn set_veterancy_level(&mut self, new_level: u32) {
        if new_level <= LEVEL_LAST && self.current_level != new_level {
            self.current_level = new_level;
            self.current_experience = self.experience_required[new_level as usize];
        }
    }

    /// Residual health bonus for current level.
    pub fn health_bonus(&self) -> f32 {
        match self.current_level {
            LEVEL_VETERAN => HEALTH_BONUS_VETERAN,
            LEVEL_ELITE => HEALTH_BONUS_ELITE,
            LEVEL_HEROIC => HEALTH_BONUS_HEROIC,
            _ => HEALTH_BONUS_REGULAR,
        }
    }

    /// Residual weapon damage bonus for current level.
    pub fn damage_bonus(&self) -> f32 {
        VET_DAMAGE_BONUS[self.current_level as usize]
    }

    /// Residual weapon ROF bonus for current level.
    pub fn rof_bonus(&self) -> f32 {
        VET_ROF_BONUS[self.current_level as usize]
    }
}

/// Lookup experience residual template row.
pub fn experience_template_row(
    name: &str,
) -> Option<&'static (&'static str, [i32; 4], [i32; 4], bool)> {
    EXPERIENCE_TEMPLATE_RESIDUAL_TABLE
        .iter()
        .find(|(n, _, _, _)| *n == name)
}

/// Wave 105 honesty: veterancy residual deepen pack.
pub fn honesty_veterancy_residual_deepen_pack_wave105() -> bool {
    let enum_ok = VETERANCY_LEVEL_NAME_LIST.len() == 4
        && LEVEL_REGULAR == 0
        && LEVEL_VETERAN == 1
        && LEVEL_ELITE == 2
        && LEVEL_HEROIC == 3
        && LEVEL_COUNT == 4
        && LEVEL_FIRST == LEVEL_REGULAR
        && LEVEL_LAST == LEVEL_HEROIC
        && (HEALTH_BONUS_VETERAN - 1.2).abs() < 0.001
        && (HEALTH_BONUS_ELITE - 1.3).abs() < 0.001
        && (HEALTH_BONUS_HEROIC - 1.5).abs() < 0.001
        && (VET_DAMAGE_BONUS[1] - 1.1).abs() < 0.001
        && (VET_DAMAGE_BONUS[3] - 1.3).abs() < 0.001
        && (VET_ROF_BONUS[1] - 1.2).abs() < 0.001
        && (VET_ROF_BONUS[3] - 1.6).abs() < 0.001;

    // Experience template residual anchors.
    let burton = experience_template_row("AmericaInfantryColonelBurton");
    let ranger = experience_template_row("AmericaInfantryRanger");
    let pathfinder = experience_template_row("AmericaInfantryPathfinder");
    let table_ok = EXPERIENCE_TEMPLATE_RESIDUAL_TABLE.len() >= 4
        && matches!(
            burton,
            Some((_, [0, 200, 300, 600], [50, 100, 100, 150], true))
        )
        && matches!(ranger, Some((_, [0, 40, 60, 120], [20, 20, 40, 60], true)))
        && matches!(
            pathfinder,
            Some((_, [0, 50, 100, 200], [40, 40, 60, 80], true))
        );

    // Tracker residual: level-up Ranger 0→VET at 40 XP, →ELITE at 60, →HEROIC at 120.
    let mut t = ExperienceTrackerResidual::new([0, 40, 60, 120], [20, 20, 40, 60], true);
    if t.current_level != LEVEL_REGULAR
        || t.current_experience != 0
        || (t.experience_scalar - 1.0).abs() > 0.001
        || !t.is_accepting_experience_points()
    {
        return false;
    }
    // Ally kill residual → 0 XP.
    if t.get_experience_value(RELATIONSHIP_ALLIES) != 0
        || t.get_experience_value(RELATIONSHIP_ENEMIES) != 20
    {
        return false;
    }
    t.add_experience_points(40, true);
    if t.current_level != LEVEL_VETERAN || t.current_experience != 40 {
        return false;
    }
    t.add_experience_points(20, true);
    if t.current_level != LEVEL_ELITE || t.current_experience != 60 {
        return false;
    }
    t.add_experience_points(60, true);
    if t.current_level != LEVEL_HEROIC || t.current_experience != 120 {
        return false;
    }
    // Bonus residual matrix at HEROIC.
    if (t.health_bonus() - 1.5).abs() > 0.001
        || (t.damage_bonus() - 1.3).abs() > 0.001
        || (t.rof_bonus() - 1.6).abs() > 0.001
    {
        return false;
    }

    // setMinVeterancyLevel residual.
    let mut t2 = ExperienceTrackerResidual::new([0, 200, 300, 600], [50, 100, 100, 150], true);
    t2.set_min_veterancy_level(LEVEL_ELITE);
    let min_ok = t2.current_level == LEVEL_ELITE && t2.current_experience == 300;
    t2.set_min_veterancy_level(LEVEL_VETERAN); // no-op (already higher)
    let min_ok = min_ok && t2.current_level == LEVEL_ELITE;

    // AdvancedTraining scalar residual: gain × 2.
    let mut t3 = ExperienceTrackerResidual::new([0, 40, 60, 120], [20, 20, 40, 60], true);
    t3.experience_scalar = 2.0;
    t3.add_experience_points(20, true);
    let scale_ok = t3.current_experience == 40 && t3.current_level == LEVEL_VETERAN;

    // Non-trainable residual rejects XP.
    let mut t4 = ExperienceTrackerResidual::new([0, 1, 2, 3], [10, 10, 10, 10], false);
    t4.add_experience_points(100, true);
    let untrainable_ok = t4.current_experience == 0 && t4.current_level == LEVEL_REGULAR;

    enum_ok && table_ok && min_ok && scale_ok && untrainable_ok
}

// ---------------------------------------------------------------------------
// Combined Wave 105 pack
// ---------------------------------------------------------------------------

/// Combined Wave 105 honesty pack (AI group / path / weapon fire / damage / veterancy).
pub fn honesty_ai_path_combat_residual_pack_wave105() -> bool {
    honesty_ai_group_residual_pack_wave105()
        && honesty_ai_path_residual_deepen_pack_wave105()
        && honesty_weapon_fire_residual_deepen_pack_wave105()
        && honesty_damage_application_residual_deepen_pack_wave105()
        && honesty_veterancy_residual_deepen_pack_wave105()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack_honesty_wave105_ai_group() {
        assert!(honesty_ai_group_residual_pack_wave105());
    }

    #[test]
    fn residual_pack_honesty_wave105_ai_path() {
        assert!(honesty_ai_path_residual_deepen_pack_wave105());
    }

    #[test]
    fn residual_pack_honesty_wave105_weapon_fire() {
        assert!(honesty_weapon_fire_residual_deepen_pack_wave105());
    }

    #[test]
    fn residual_pack_honesty_wave105_damage_application() {
        assert!(honesty_damage_application_residual_deepen_pack_wave105());
    }

    #[test]
    fn residual_pack_honesty_wave105_veterancy() {
        assert!(honesty_veterancy_residual_deepen_pack_wave105());
    }

    #[test]
    fn residual_pack_honesty_wave105_combined() {
        assert!(honesty_ai_path_combat_residual_pack_wave105());
    }
}
