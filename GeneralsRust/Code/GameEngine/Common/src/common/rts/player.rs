//! Player System - Core player class managing all player-specific data and behavior
//!
//! C++ Reference: /GeneralsMD/Code/GameEngine/Source/Common/RTS/Player.cpp
//! C++ Header: /GeneralsMD/Code/GameEngine/Include/Common/Player.h
//!
//! The Player class is one of the most complex in the system, managing:
//! - Resources (money, energy)
//! - Relationships with other players
//! - Sciences and upgrades
//! - Score and statistics
//! - AI behavior
//! - Team management
//! - Radar and battle plans
//! - Build list and production
//! - Squad system (hotkey squads and current selection)
//! - Resource gathering management

use crate::common::rts::{
    AcademyStats, Energy, Handicap, MissionStats, Money, PlayerHandle, ProductionPrerequisite,
    Relationship, ScienceType, ScoreKeeper, TeamID, SCIENCE_INVALID,
};
use crate::common::system::{Snapshotable, Xfer, XferMode, XferVersion};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak};

/// Object ID type used throughout the game engine
pub type ObjectID = u32;

/// Invalid object ID constant
pub const INVALID_OBJECT_ID: ObjectID = 0xFFFFFFFF;

/// Invalid hotkey squad constant (matches C++ NO_HOTKEY_SQUAD)
pub const NO_HOTKEY_SQUAD: i32 = -1;

// =========================================================
// Forward Declarations / Trait Definitions
// These are placeholder traits for AI and related systems
// that are defined in GameLogic but referenced here for type safety
// =========================================================

/// Trait for objects that can be killed for bounty.
/// This allows Player (in Common) to work with Object (in GameLogic)
/// without creating circular dependencies.
///
/// C++ Reference: Player::doBountyForKill takes `const Object* killer, const Object* victim`
pub trait BountyObject {
    /// Get the cost to build this object (used for bounty calculation)
    fn get_build_cost(&self) -> i32;

    /// Check if this object is under construction (no bounty for under-construction)
    fn is_under_construction(&self) -> bool;
}

/// Trait for objects that provide skill points when killed.
/// C++ Reference: Player::addSkillPointsForKill takes `const Object* killer, const Object* victim`
pub trait SkillPointObject {
    /// Get the skill point value for killing this object
    fn get_skill_point_value(&self, killer: &dyn SkillPointObject) -> i32;

    /// Get the veterancy level of this object
    fn get_veterancy_level(&self) -> i32;
}

/// Trait for AI player functionality
/// The actual AIPlayer struct is defined in GameLogic/src/ai/ai_player.rs
/// This trait allows Player to reference AI functionality without direct dependency
pub trait AIPlayerInterface: std::fmt::Debug + Send + Sync {
    /// Update the AI player
    fn update(&mut self) -> Result<(), String>;

    /// Called when a new map is loaded
    fn new_map(&mut self);

    /// Check if this is a skirmish AI
    fn is_skirmish_ai(&self) -> bool;

    /// Get the current enemy target
    fn get_ai_enemy(&self) -> Option<i32>;

    /// Check bridges for pathfinding
    fn check_bridges(&self, _unit_id: ObjectID, _waypoint: i32) -> bool {
        false
    }

    /// Repair a structure
    fn repair_structure(&mut self, _structure_id: ObjectID) {}

    /// Get base center position
    fn get_base_center(&self) -> Option<Coord3D> {
        None
    }

    /// Called when a unit is produced
    fn on_unit_produced(&mut self, _factory_id: ObjectID, _unit_id: ObjectID) {}

    /// Called when a structure is produced
    fn on_structure_produced(&mut self, _factory_id: ObjectID, _structure_id: ObjectID) {}

    /// Set the AI difficulty
    fn set_ai_difficulty(&mut self, _difficulty: GameDifficulty);

    /// Get the AI difficulty
    fn get_ai_difficulty(&self) -> GameDifficulty;
}

/// Game difficulty enumeration
/// C++ Reference: GameDifficulty enum in GameType.h
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GameDifficulty {
    #[default]
    Normal,
    Easy,
    Hard,
    Brutal,
}

/// 3D Coordinate type for positions
#[derive(Debug, Clone, Copy, Default)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Coord3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn origin() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }
}

// =========================================================
// BuildListInfo - Build list entry for AI construction
// C++ Reference: BuildListInfo class in SidesList.h
// =========================================================

/// Build list information for AI construction
/// C++ Reference: BuildListInfo class
#[derive(Debug, Clone)]
pub struct BuildListInfo {
    /// Template name of the building to construct
    template_name: String,
    /// Location to build at
    location: Coord3D,
    /// Angle of the building
    angle: f32,
    /// Object ID if building exists
    object_id: ObjectID,
    /// Number of times to rebuild (0xFFFF_FFFF = unlimited)
    num_rebuilds: u32,
    /// Whether this is a priority build
    priority_build: bool,
    /// Whether currently under construction
    under_construction: bool,
    /// Timestamp when object was created
    object_timestamp: u32,
    /// Next entry in the linked list
    next: Option<Box<BuildListInfo>>,
}

impl BuildListInfo {
    /// Unlimited rebuilds constant
    pub const UNLIMITED_REBUILDS: u32 = 0xFFFF_FFFF;

    /// Create a new build list info entry
    pub fn new(template_name: String, location: Coord3D, angle: f32) -> Self {
        Self {
            template_name,
            location,
            angle,
            object_id: INVALID_OBJECT_ID,
            num_rebuilds: 0,
            priority_build: false,
            under_construction: false,
            object_timestamp: 0,
            next: None,
        }
    }

    /// Get the template name
    pub fn get_template_name(&self) -> &str {
        &self.template_name
    }

    /// Get the location
    pub fn get_location(&self) -> &Coord3D {
        &self.location
    }

    /// Get the angle
    pub fn get_angle(&self) -> f32 {
        self.angle
    }

    /// Get the object ID
    pub fn get_object_id(&self) -> ObjectID {
        self.object_id
    }

    /// Set the object ID
    pub fn set_object_id(&mut self, id: ObjectID) {
        self.object_id = id;
    }

    /// Get number of rebuilds remaining
    pub fn get_num_rebuilds(&self) -> u32 {
        self.num_rebuilds
    }

    /// Set number of rebuilds
    pub fn set_num_rebuilds(&mut self, num: u32) {
        self.num_rebuilds = num;
    }

    /// Mark as priority build
    pub fn mark_priority_build(&mut self) {
        self.priority_build = true;
    }

    /// Check if priority build
    pub fn is_priority_build(&self) -> bool {
        self.priority_build
    }

    /// Set under construction flag
    pub fn set_under_construction(&mut self, under_construction: bool) {
        self.under_construction = under_construction;
    }

    /// Check if under construction
    pub fn is_under_construction(&self) -> bool {
        self.under_construction
    }

    /// Check if buildable (rebuilds remaining)
    pub fn is_buildable(&self) -> bool {
        self.num_rebuilds > 0 || self.num_rebuilds == Self::UNLIMITED_REBUILDS
    }

    /// Decrement rebuild count
    pub fn decrement_num_rebuilds(&mut self) {
        if self.num_rebuilds > 0 && self.num_rebuilds != Self::UNLIMITED_REBUILDS {
            self.num_rebuilds -= 1;
        }
    }

    /// Get next entry
    pub fn get_next(&self) -> Option<&BuildListInfo> {
        self.next.as_deref()
    }

    /// Get mutable next entry
    pub fn get_next_mut(&mut self) -> Option<&mut BuildListInfo> {
        self.next.as_deref_mut()
    }

    /// Set next entry
    pub fn set_next(&mut self, next: Option<Box<BuildListInfo>>) {
        self.next = next;
    }
}

impl Default for BuildListInfo {
    fn default() -> Self {
        Self::new(String::new(), Coord3D::origin(), 0.0)
    }
}

// =========================================================
// Squad - Collection of objects for hotkey groups
// C++ Reference: Squad class in Squad.h
// =========================================================

/// Squad represents a collection of objects for hotkey groups and current selection
/// C++ Reference: Squad class in GameLogic/Squad.h
#[derive(Debug, Clone, Default)]
pub struct Squad {
    /// Object IDs in this squad
    object_ids: Vec<ObjectID>,
}

impl Squad {
    /// Create a new empty squad
    pub fn new() -> Self {
        Self {
            object_ids: Vec::new(),
        }
    }

    /// Add an object to the squad
    pub fn add_object(&mut self, object_id: ObjectID) {
        if !self.object_ids.contains(&object_id) {
            self.object_ids.push(object_id);
        }
    }

    /// Remove an object from the squad
    pub fn remove_object(&mut self, object_id: ObjectID) {
        self.object_ids.retain(|&id| id != object_id);
    }

    /// Clear all objects from the squad
    pub fn clear(&mut self) {
        self.object_ids.clear();
    }

    /// Check if an object is in the squad
    pub fn contains(&self, object_id: ObjectID) -> bool {
        self.object_ids.contains(&object_id)
    }

    /// Get the number of objects in the squad
    pub fn len(&self) -> usize {
        self.object_ids.len()
    }

    /// Check if the squad is empty
    pub fn is_empty(&self) -> bool {
        self.object_ids.is_empty()
    }

    /// Get all object IDs
    pub fn get_object_ids(&self) -> &[ObjectID] {
        &self.object_ids
    }

    /// Get mutable access to object IDs
    pub fn get_object_ids_mut(&mut self) -> &mut Vec<ObjectID> {
        &mut self.object_ids
    }

    /// Clear squad (alias for clear())
    pub fn clear_squad(&mut self) {
        self.clear();
    }

    /// Add object ID
    pub fn add_object_id(&mut self, object_id: ObjectID) {
        self.add_object(object_id);
    }

    /// Check if object is on squad (alias for contains)
    pub fn is_on_squad(&self, object_id: ObjectID) -> bool {
        self.contains(object_id)
    }

    /// Get object IDs for iteration
    pub fn get_live_objects(&self) -> Vec<ObjectID> {
        self.object_ids.clone()
    }
}

// =========================================================
// UpgradeInfo - Upgrade tracking for player
// C++ Reference: Upgrade class in Upgrade.h
// =========================================================

/// Upgrade status enumeration
/// C++ Reference: UpgradeStatusType enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeStatus {
    /// Upgrade is in production
    InProduction,
    /// Upgrade is complete
    Complete,
    /// Upgrade is pending
    Pending,
}

/// Information about an upgrade the player has
#[derive(Debug, Clone)]
pub struct UpgradeInfo {
    /// Name of the upgrade
    name: String,
    /// Status of the upgrade
    status: UpgradeStatus,
    /// Frame when upgrade started
    start_frame: u32,
    /// Frame when upgrade will complete (if in production)
    complete_frame: u32,
}

impl UpgradeInfo {
    /// Create a new upgrade info
    pub fn new(name: String) -> Self {
        Self {
            name,
            status: UpgradeStatus::Pending,
            start_frame: 0,
            complete_frame: 0,
        }
    }

    /// Get the upgrade name
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Get the upgrade status
    pub fn get_status(&self) -> UpgradeStatus {
        self.status
    }

    /// Set the upgrade status
    pub fn set_status(&mut self, status: UpgradeStatus) {
        self.status = status;
    }

    /// Set the start frame
    pub fn set_start_frame(&mut self, frame: u32) {
        self.start_frame = frame;
    }

    /// Set the complete frame
    pub fn set_complete_frame(&mut self, frame: u32) {
        self.complete_frame = frame;
    }

    /// Check if upgrade is in production
    pub fn is_in_production(&self) -> bool {
        self.status == UpgradeStatus::InProduction
    }

    /// Check if upgrade is complete
    pub fn is_complete(&self) -> bool {
        self.status == UpgradeStatus::Complete
    }
}

/// Maximum number of hotkey squads (matches C++ NUM_HOTKEY_SQUADS)
pub const NUM_HOTKEY_SQUADS: usize = 10;

// =========================================================
// PlayerRelationMap - Maps player indices to relationships
// C++ Reference: Player.cpp lines 153-221
// =========================================================

/// Map of player indices to their relationship with this player.
///
/// This struct encapsulates the player-to-player relationship mapping
/// and provides save/load (xfer) and CRC methods for network synchronization.
///
/// C++ Reference: `PlayerRelationMap` class in Player.cpp
#[derive(Debug)]
pub struct PlayerRelationMap {
    /// Internal map from player index to relationship
    /// C++ equivalent: `PlayerRelationMapType m_map` (typedef std::map<Int, Relationship>)
    map: HashMap<i32, Relationship>,
}

impl PlayerRelationMap {
    /// Create a new empty PlayerRelationMap
    /// C++ Reference: PlayerRelationMap::PlayerRelationMap() lines 155-158
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Get the relationship with the specified player.
    /// Returns None if no explicit relationship is set.
    ///
    /// # Arguments
    /// * `player_index` - The index of the player to look up
    ///
    /// # Returns
    /// Some(Relationship) if set, None otherwise
    pub fn get(&self, player_index: i32) -> Option<Relationship> {
        self.map.get(&player_index).copied()
    }

    /// Set the relationship with the specified player.
    /// Creates the entry if it doesn't exist.
    ///
    /// # Arguments
    /// * `player_index` - The index of the player
    /// * `relationship` - The relationship to set
    ///
    /// C++ Reference: Used by Player::setPlayerRelationship() lines 582-588
    pub fn set(&mut self, player_index: i32, relationship: Relationship) {
        self.map.insert(player_index, relationship);
    }

    /// Remove a specific player relationship, or clear all relationships.
    /// Returns true if any relationship was removed.
    ///
    /// # Arguments
    /// * `player_index` - If Some, remove only that relationship. If None, clear all.
    pub fn remove(&mut self, player_index: Option<i32>) -> bool {
        if let Some(idx) = player_index {
            self.map.remove(&idx).is_some()
        } else {
            let had_relations = !self.map.is_empty();
            self.map.clear();
            had_relations
        }
    }

    /// Check if the map is empty
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Get the number of relationships
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Clear all relationships
    /// C++ Reference: Used in Player::initFromDict() and destructor
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Get an iterator over all relationships
    pub fn iter(&self) -> impl Iterator<Item = (&i32, &Relationship)> {
        self.map.iter()
    }

    /// Get an iterator over player indices (keys)
    pub fn keys(&self) -> impl Iterator<Item = &i32> {
        self.map.keys()
    }
}

impl std::ops::Index<i32> for PlayerRelationMap {
    type Output = Relationship;

    fn index(&self, index: i32) -> &Self::Output {
        &self.map[&index]
    }
}

impl<'a> IntoIterator for &'a PlayerRelationMap {
    type Item = (&'a i32, &'a Relationship);
    type IntoIter = std::collections::hash_map::Iter<'a, i32, Relationship>;

    fn into_iter(self) -> Self::IntoIter {
        self.map.iter()
    }
}

impl Default for PlayerRelationMap {
    fn default() -> Self {
        Self::new()
    }
}

impl Snapshotable for PlayerRelationMap {
    /// CRC computation for network synchronization.
    /// C++ Reference: PlayerRelationMap::crc() lines 165-168
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ implementation is empty - relationships are not included in CRC
        // The Player class handles its own CRC separately
        Ok(())
    }

    /// Save/load the player relation map.
    /// C++ Reference: PlayerRelationMap::xfer() lines 170-221
    /// Version History:
    ///   1: Initial version
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;

        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("PlayerRelationMap xfer_version failed: {}", e))?;

        // Player relation count
        let mut relation_count = self.map.len() as u16;
        xfer.xfer_unsigned_short(&mut relation_count)
            .map_err(|e| format!("relation_count xfer failed: {}", e))?;

        match xfer.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                // Go through all player relations and save them
                for (&player_index, &relationship) in &self.map {
                    let mut idx = player_index;
                    let mut rel = relationship as i32; // Relationship is serialized as int

                    // Write player index
                    xfer.xfer_int(&mut idx)
                        .map_err(|e| format!("relation player_idx xfer failed: {}", e))?;

                    // Write relationship (xferUser in C++ serializes as raw bytes, but we use int for portability)
                    xfer.xfer_int(&mut rel)
                        .map_err(|e| format!("relation value xfer failed: {}", e))?;
                }
            }
            XferMode::Load => {
                // Load relationships
                self.map.clear();
                for _ in 0..relation_count {
                    let mut player_index = 0i32;
                    let mut rel_value = 0i32;

                    // Read player index
                    xfer.xfer_int(&mut player_index)
                        .map_err(|e| format!("load relation player_idx failed: {}", e))?;

                    // Read relationship
                    xfer.xfer_int(&mut rel_value)
                        .map_err(|e| format!("load relation value failed: {}", e))?;

                    // Convert int back to Relationship enum
                    let relationship = Relationship::from(rel_value);
                    self.map.insert(player_index, relationship);
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // C++ implementation is empty
        Ok(())
    }
}

/// Player type enumeration - matches C++ PlayerType
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerType {
    Human,
    Computer,
}

/// Battle plan status enumeration
/// C++ Reference: BattlePlanStatus enum (referenced in Player.h)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattlePlanStatus {
    Bombardment,
    HoldTheLine,
    SearchAndDestroy,
}

/// Player structure - central hub for player data
///
/// C++ Reference: Player class in Player.h
/// A "Player" consists of an entity controlling a single set of units in a mission.
/// A Player may be human or computer controlled.
///
/// All Players have a "Player Index" associated which allows us to do some shorthand for
/// representing Players (mainly in bitfields).
#[derive(Debug)]
pub struct Player {
    // =========================================================
    // Core Identity Fields (C++ Player.h lines 281-288)
    // =========================================================
    /// Player unique index
    /// C++: m_playerIndex (Player.h line 287)
    index: i32,
    /// Player display name (Unicode in C++)
    /// C++: m_playerDisplayName (Player.h line 282)
    player_display_name: String,
    /// Player internal name (for matching map objects)
    /// C++: m_playerName (Player.h line 285)
    player_name: String,
    /// Side/faction this player is on
    /// C++: m_side (Player.h line 289)
    side: String,
    /// Base side (GLA, USA, or China)
    /// C++: m_baseSide (Player.h line 290)
    base_side: String,
    /// Player type (human/computer)
    /// C++: m_playerType (Player.h line 291)
    player_type: PlayerType,

    // =========================================================
    // Resource Management (C++ Player.h lines 292-298)
    // =========================================================
    /// Money/resource management
    /// C++: m_money (Player.h line 292)
    money: Money,
    /// Energy production/consumption
    /// C++: m_energy (Player.h line 298)
    energy: Energy,

    // =========================================================
    // Statistics and Tracking (C++ Player.h lines 299-305)
    // =========================================================
    /// Mission statistics
    /// C++: m_stats (Player.h line 299)
    mission_stats: MissionStats,
    /// Handicap modifiers
    /// C++: m_handicap (Player.h line 283)
    handicap: Handicap,
    /// Score keeping
    /// C++: m_scoreKeeper (Player.h line 386)
    score_keeper: ScoreKeeper,
    /// Academy statistics for advice
    /// C++: m_academyStats (Player.h line 346)
    academy_stats: AcademyStats,

    // =========================================================
    // Sciences System (C++ Player.h lines 325-334)
    // =========================================================
    /// Sciences currently owned by the player
    /// C++: m_sciences (Player.h line 325)
    sciences: HashSet<ScienceType>,
    /// Sciences that are currently disabled (cannot be used)
    /// C++: m_sciencesDisabled (Player.h line 326)
    sciences_disabled: HashSet<ScienceType>,
    /// Sciences hidden from UI until unlocked
    /// C++: m_sciencesHidden (Player.h line 327)
    sciences_hidden: HashSet<ScienceType>,
    /// Science purchase points available
    /// C++: m_sciencePurchasePoints (Player.h line 332)
    science_purchase_points: i32,
    /// Skill points (for ranking)
    /// C++: m_skillPoints (Player.h line 331)
    skill_points: i32,
    /// Rank level (1...n)
    /// C++: m_rankLevel (Player.h line 330)
    rank_level: i32,
    /// Skill points needed to level up (runtime only, not saved)
    /// C++: m_levelUp (Player.h line 333)
    level_up: i32,
    /// Skill points to level down (runtime only, not saved)
    /// C++: m_levelDown (Player.h line 333)
    level_down: i32,
    /// Skill point modifier (multiplied by skill points before applied)
    /// C++: m_skillPointsModifier (Player.h line 362)
    skill_points_modifier: f32,
    /// General's name (customizable)
    /// C++: m_generalName (Player.h line 334)
    general_name: String,

    // =========================================================
    // Team and Relationship Management (C++ Player.h lines 336-345)
    // =========================================================
    /// Player relationships with other players (keyed by player index)
    /// C++: m_playerRelations (Player.h line 338)
    player_relations: PlayerRelationMap,
    /// Default team for this player
    /// C++: m_defaultTeam (Player.h line 321)
    default_team: Option<TeamID>,
    /// Multiplayer start index
    /// C++: m_mpStartIndex (Player.h line 317)
    mp_start_index: i32,

    // =========================================================
    // Radar System (C++ Player.h lines 299-307)
    // =========================================================
    /// Number of radar-producing facilities
    /// C++: m_radarCount (Player.h line 299)
    radar_count: i32,
    /// Number of disable-proof radars
    /// C++: m_disableProofRadarCount (Player.h line 300)
    disable_proof_radar_count: i32,
    /// Whether radar is disabled
    /// C++: m_radarDisabled (Player.h line 301)
    radar_disabled: bool,

    // =========================================================
    // Battle Plan System (C++ Player.h lines 302-307)
    // =========================================================
    /// Number of bombardment battle plans active
    /// C++: m_bombardBattlePlans (Player.h line 302)
    bombard_battle_plans: i32,
    /// Number of hold-the-line battle plans active
    /// C++: m_holdTheLineBattlePlans (Player.h line 303)
    hold_the_line_battle_plans: i32,
    /// Number of search-and-destroy battle plans active
    /// C++: m_searchAndDestroyBattlePlans (Player.h line 304)
    search_and_destroy_battle_plans: i32,

    // =========================================================
    // Build and Production System (C++ Player.h lines 311-316)
    // =========================================================
    /// Whether player can build units
    /// C++: m_canBuildUnits (Player.h line 355)
    can_build_units: bool,
    /// Whether player can build base buildings
    /// C++: m_canBuildBase (Player.h line 356)
    can_build_base: bool,

    // =========================================================
    // Player State Flags (C++ Player.h lines 358-375)
    // =========================================================
    /// Whether player is dead
    /// C++: m_isPlayerDead (Player.h line 389)
    is_player_dead: bool,
    /// Whether player is an observer
    /// C++: m_observer (Player.h line 358)
    observer: bool,
    /// Whether player preordered
    /// C++: m_isPreorder (Player.h line 360)
    is_preorder: bool,
    /// Whether player should be listed in score screen
    /// C++: m_listInScoreScreen (Player.h line 364)
    list_in_score_screen: bool,
    /// Whether units should hunt
    /// C++: m_unitsShouldHunt (Player.h line 365)
    units_should_hunt: bool,
    /// Logical retaliation mode enabled
    /// C++: m_logicalRetaliationModeEnabled (Player.h line 391)
    logical_retaliation_mode_enabled: bool,

    // =========================================================
    // Bounty System (C++ Player.h line 376)
    // =========================================================
    /// Cash bounty percent (from upgrades)
    /// C++: m_cashBountyPercent (Player.h line 376)
    cash_bounty_percent: f32,

    // =========================================================
    // Attacked Tracking (C++ Player.h lines 378-379)
    // =========================================================
    /// Which players have attacked this player
    /// C++: m_attackedBy[MAX_PLAYER_COUNT] (Player.h line 378)
    attacked_by: Vec<bool>,
    /// Last frame attacked
    /// C++: m_attackedFrame (Player.h line 379)
    attacked_frame: u32,

    // =========================================================
    // AI System Integration (C++ Player.h line 339)
    // =========================================================
    /// AI player reference - weak reference to avoid circular dependencies
    /// C++: m_ai (Player.h line 339)
    /// The actual AIPlayer struct lives in GameLogic, so we use a weak ref
    ai: Option<Weak<dyn AIPlayerInterface>>,
    /// Current difficulty setting (for both human and AI players)
    /// C++: obtained via m_ai->getAIDifficulty() or from scripts
    difficulty: GameDifficulty,

    // =========================================================
    // Build List Management (C++ Player.h line 335)
    // =========================================================
    /// Build list for AI construction
    /// C++: m_pBuildList (Player.h line 335)
    build_list: Option<Box<BuildListInfo>>,

    // =========================================================
    // Resource Gathering Manager (C++ Player.h line 340)
    // =========================================================
    /// Resource gathering manager for supply centers/warehouses
    /// C++: m_resourceGatheringManager (Player.h line 340)
    /// Stores supply center and warehouse IDs for AI/harvester pathfinding
    supply_centers: Vec<ObjectID>,
    supply_warehouses: Vec<ObjectID>,

    // =========================================================
    // Squad System (C++ Player.h lines 382-383)
    // =========================================================
    /// Hotkey squads (0-9 for control groups)
    /// C++: m_squads[NUM_HOTKEY_SQUADS] (Player.h line 382)
    hotkey_squads: [Squad; NUM_HOTKEY_SQUADS],
    /// Current selection squad
    /// C++: m_currentSelection (Player.h line 383)
    current_selection: Squad,

    // =========================================================
    // Upgrade List Management (C++ Player.h line 336)
    // =========================================================
    /// List of upgrades this player has (linked list in C++)
    /// C++: m_upgradeList (Player.h line 336)
    upgrade_list: Vec<UpgradeInfo>,
    /// Bitmask of upgrades in progress
    /// C++: m_upgradesInProgress (Player.h line 348)
    upgrades_in_progress: u64,
    /// Bitmask of completed upgrades
    /// C++: m_upgradesCompleted (Player.h line 349)
    upgrades_completed: u64,

    // =========================================================
    // Team Prototype List (C++ Player.h line 375)
    // =========================================================
    /// List of team prototypes this player owns
    /// C++: m_playerTeamPrototypes (Player.h line 375)
    team_prototypes: Vec<String>,

    // =========================================================
    // Tunnel System (C++ Player.h line 341)
    // =========================================================
    /// Tunnel system tracker
    /// C++: m_tunnelSystem (Player.h line 341)
    tunnel_entrances: Vec<ObjectID>,

    // =========================================================
    // Production Cost Changes (C++ Player.h lines 351-353)
    // =========================================================
    /// Production cost change percentages by thing name
    /// C++: m_productionCostChanges (Player.h line 351)
    production_cost_changes: HashMap<String, f32>,
    /// Production time change percentages by thing name
    /// C++: m_productionTimeChanges (Player.h line 352)
    production_time_changes: HashMap<String, f32>,
    /// KindOf-based production cost change percentages
    /// C++: m_kindOfPercentProductionChangeList (Player.h line 353)
    kind_of_production_cost_changes: Vec<(u64, f32)>,

    // =========================================================
    // Special Power Ready Timers (C++ Player.h lines 392-393)
    // =========================================================
    /// Special power ready timers for shared cooldowns
    /// C++: m_specialPowerReadyTimerList (Player.h line 392)
    special_power_timers: HashMap<u32, u32>, // template_id -> ready_frame
}

impl Player {
    /// Create a new Player with the given index
    ///
    /// C++ Reference: Player::Player() (Player.cpp lines 193-250)
    pub fn new(index: i32) -> Self {
        // C++ lines 195-199: Initialize state flags
        let is_preorder = false;
        let is_player_dead = false;

        // C++ lines 202-204: Allocate relation maps
        let player_relations = PlayerRelationMap::new();

        // C++ lines 225-228: Initialize attacked tracking
        let attacked_by = vec![false; super::player_list::MAX_PLAYER_COUNT as usize];
        let attacked_frame = 0;

        // C++ lines 230-234: Units should hunt
        let units_should_hunt = false;

        let player = Self {
            index,
            player_display_name: String::new(),
            player_name: String::new(),
            side: String::new(),
            base_side: String::new(),
            player_type: PlayerType::Computer,
            money: Money::new(),
            energy: Energy::new(),
            mission_stats: MissionStats::new(),
            handicap: Handicap::new(),
            score_keeper: ScoreKeeper::new(),
            academy_stats: AcademyStats::new(),
            sciences: HashSet::new(),
            sciences_disabled: HashSet::new(),
            sciences_hidden: HashSet::new(),
            science_purchase_points: 0,
            skill_points: 0,
            rank_level: 0,
            level_up: 0,
            level_down: 0,
            skill_points_modifier: 1.0,
            general_name: String::new(),
            player_relations,
            default_team: None,
            mp_start_index: 0,
            radar_count: 0,
            disable_proof_radar_count: 0,
            radar_disabled: false,
            bombard_battle_plans: 0,
            hold_the_line_battle_plans: 0,
            search_and_destroy_battle_plans: 0,
            can_build_units: true,
            can_build_base: true,
            is_player_dead,
            observer: false,
            is_preorder,
            list_in_score_screen: true,
            units_should_hunt,
            logical_retaliation_mode_enabled: false,
            cash_bounty_percent: 0.0,
            attacked_by,
            attacked_frame,
            // AI System
            ai: None,
            difficulty: GameDifficulty::Normal,
            // Build List
            build_list: None,
            // Resource Gathering
            supply_centers: Vec::new(),
            supply_warehouses: Vec::new(),
            // Squad System - initialize with empty squads
            hotkey_squads: Default::default(),
            current_selection: Squad::new(),
            // Upgrade System
            upgrade_list: Vec::new(),
            upgrades_in_progress: 0,
            upgrades_completed: 0,
            // Team prototypes
            team_prototypes: Vec::new(),
            // Tunnel system
            tunnel_entrances: Vec::new(),
            // Production changes
            production_cost_changes: HashMap::new(),
            production_time_changes: HashMap::new(),
            kind_of_production_cost_changes: Vec::new(),

            // Special Power Timers
            special_power_timers: HashMap::new(),
        };
        player
    }

    // =========================================================
    // Accessor Methods
    // =========================================================

    /// Get the player index
    /// C++ Reference: Player::getPlayerIndex() (Player.h line 162)
    pub fn get_player_index(&self) -> i32 {
        self.index
    }

    /// Get a bitmask that is unique to this player
    /// C++ Reference: Player::getPlayerMask() (Player.h line 164)
    pub fn get_player_mask(&self) -> u32 {
        1 << self.index
    }

    /// Get player display name
    /// C++ Reference: Player::getPlayerDisplayName() (Player.h line 118)
    pub fn get_player_display_name(&self) -> &str {
        &self.player_display_name
    }

    /// Get player internal name
    pub fn get_player_name(&self) -> &str {
        &self.player_name
    }

    /// Get player side
    /// C++ Reference: Player::getSide() (Player.h line 121)
    pub fn get_side(&self) -> &str {
        &self.side
    }

    /// Get player base side
    /// C++ Reference: Player::getBaseSide() (Player.h line 122)
    pub fn get_base_side(&self) -> &str {
        &self.base_side
    }

    /// Get player type
    /// C++ Reference: Player::getPlayerType() (Player.h line 138)
    pub fn get_player_type(&self) -> PlayerType {
        self.player_type
    }

    /// Set player type
    /// C++ Reference: Player::setPlayerType() (Player.cpp lines 695-712)
    pub fn set_player_type(&mut self, player_type: PlayerType, _skirmish: bool) {
        self.player_type = player_type;
        // Note: AI player creation would happen here in C++ (lines 706-712)
    }

    /// Get the money object
    /// C++ Reference: Player::getMoney() (Player.h lines 127-128)
    pub fn get_money(&self) -> &Money {
        &self.money
    }

    /// Get mutable reference to money
    pub fn get_money_mut(&mut self) -> &mut Money {
        &mut self.money
    }

    /// Get the energy object
    /// C++ Reference: Player::getEnergy() (Player.h lines 135-136)
    pub fn get_energy(&self) -> &Energy {
        &self.energy
    }

    /// Get mutable reference to energy
    pub fn get_energy_mut(&mut self) -> &mut Energy {
        &mut self.energy
    }

    /// Get academy stats
    /// C++ Reference: Player::getAcademyStats() (Player.h lines 417-418)
    pub fn get_academy_stats(&self) -> &AcademyStats {
        &self.academy_stats
    }

    /// Get mutable reference to academy stats
    pub fn get_academy_stats_mut(&mut self) -> &mut AcademyStats {
        &mut self.academy_stats
    }

    /// Get mission stats
    pub fn get_mission_stats(&self) -> &MissionStats {
        &self.mission_stats
    }

    /// Get mutable reference to mission stats
    pub fn get_mission_stats_mut(&mut self) -> &mut MissionStats {
        &mut self.mission_stats
    }

    /// Get handicap
    /// C++ Reference: Player::getHandicap() (Player.h lines 125-126)
    pub fn get_handicap(&self) -> &Handicap {
        &self.handicap
    }

    /// Get score keeper
    /// C++ Reference: Player::getScoreKeeper() (Player.h line 415)
    pub fn get_score_keeper(&self) -> &ScoreKeeper {
        &self.score_keeper
    }

    /// Get mutable reference to score keeper
    pub fn get_score_keeper_mut(&mut self) -> &mut ScoreKeeper {
        &mut self.score_keeper
    }

    /// Get multiplayer start index
    /// C++ Reference: Player::getMpStartIndex() (Player.h line 311)
    pub fn get_mp_start_index(&self) -> i32 {
        self.mp_start_index
    }

    /// Set multiplayer start index
    pub fn set_mp_start_index(&mut self, index: i32) {
        self.mp_start_index = index;
    }

    // =========================================================
    // Initialization Methods (C++ Player.cpp lines 252-437)
    // =========================================================

    /// Initialize player from a player template
    ///
    /// C++ Reference: Player::init() (Player.cpp lines 252-437)
    ///
    /// # Arguments
    /// * `name` - Optional player name to set
    pub fn init(&mut self, name: Option<String>) {
        // C++ lines 257-259: Reset skill point modifier
        self.skill_points_modifier = 1.0;
        self.attacked_frame = 0;

        // C++ lines 261-263: Reset state flags
        self.is_preorder = false;
        self.is_player_dead = false;

        // C++ lines 265-269: Reset radar
        self.radar_count = 0;
        self.disable_proof_radar_count = 0;
        self.radar_disabled = false;

        // C++ lines 271-280: Reset battle plans
        self.bombard_battle_plans = 0;
        self.hold_the_line_battle_plans = 0;
        self.search_and_destroy_battle_plans = 0;

        // C++ lines 285: Initialize energy
        let handle = PlayerHandle::new(self.index.max(0) as u32);
        self.energy.init(handle);

        // C++ line 286: Initialize stats
        self.mission_stats.init();

        // C++ lines 288-291: Initialize handicap
        self.handicap.init();

        // C++ lines 293-310: Initialize squads (simplified - we don't have Squad class yet)

        // C++ lines 318-319: Reset build permissions
        self.can_build_base = true;
        self.can_build_units = true;

        // C++ lines 320-321: Reset observer and bounty
        self.observer = false;
        self.cash_bounty_percent = 0.0;
        self.list_in_score_screen = true;
        self.units_should_hunt = false;

        // C++ lines 333-340: Initialize default values (no player template = neutral player)
        if let Some(name) = name {
            self.player_display_name = name;
        }
        self.player_name.clear();
        self.side.clear();
        self.base_side.clear();
        self.player_type = PlayerType::Computer;

        // C++ line 354: Reset score keeper
        self.score_keeper.reset(self.index);

        // C++ lines 357-358: Reset rank and sciences
        self.reset_rank();
        self.sciences_disabled.clear();
        self.sciences_hidden.clear();

        // C++ lines 369-371: Initialize academy stats
        self.academy_stats.init(handle);

        // C++ line 376: Reset retaliation mode
        self.logical_retaliation_mode_enabled = false;

        // Initialize money
        self.money.init();
        self.money.set_player_index(self.index);
    }

    /// Reset rank to 1
    /// C++ Reference: Player::resetRank() (Player.cpp lines 439-449)
    pub fn reset_rank(&mut self) {
        self.rank_level = 1;
        self.skill_points = 0;
        self.science_purchase_points = 0;

        // C++ lines 445-448: Would calculate level_up and level_down from RankInfo
        // For now, set reasonable defaults
        self.level_up = 100;
        self.level_down = 50;
    }

    /// Reset sciences to just intrinsic ones from player template
    /// C++ Reference: Player::resetSciences() (Player.cpp lines 451-466)
    pub fn reset_sciences(&mut self) {
        self.sciences.clear();

        // C++ lines 456-464: Would grant intrinsic sciences from player template
        // For now, this is a no-op since we don't have PlayerTemplate access
    }

    // =========================================================
    // Update Method (C++ Player.cpp lines 540-590)
    // =========================================================

    /// Update player (called each frame)
    ///
    /// C++ Reference: Player::update() (Player.cpp lines 540-590)
    ///
    /// This method handles:
    /// - AI updates (if computer player)
    /// - Team script updates
    /// - Power sabotage checks
    /// - Academy stats updates
    /// - Retaliation mode sync
    pub fn update(&mut self) {
        // C++ lines 545-546: AI update would happen here

        // C++ lines 548-562: Team script updates would happen here

        // C++ lines 564-569: Check power sabotage expiry
        let current_frame = crate::common::time::frame();
        if self.energy.get_power_sabotaged_till_frame() != 0
            && current_frame > self.energy.get_power_sabotaged_till_frame()
        {
            self.energy.set_power_sabotaged_till_frame(0);
            self.on_power_brown_out_change(!self.energy.has_sufficient_power());
        }

        // C++ line 571: Update academy stats
        self.academy_stats.update();

        // C++ lines 573-590: Retaliation mode sync would happen here
        // (requires access to ThePlayerList and TheGlobalData)
    }

    /// Handle power brownout state change
    /// C++ Reference: Player::onPowerBrownOutChange() (Player.cpp lines 3232-3241)
    fn on_power_brown_out_change(&mut self, brown_out: bool) {
        if brown_out {
            self.disable_radar();
        } else {
            self.enable_radar();
        }
        // C++ lines 3238-3240: Would iterate all objects and call doPowerDisable
    }

    // =========================================================
    // Radar System (C++ Player.h lines 299-301)
    // =========================================================

    /// Add a radar producer
    /// C++ Reference: Player::addRadar() (Player.cpp lines 2414-2422)
    pub fn add_radar(&mut self, disable_proof: bool) {
        self.radar_count += 1;
        if disable_proof {
            self.disable_proof_radar_count += 1;
        }
    }

    /// Remove a radar producer
    /// C++ Reference: Player::removeRadar() (Player.cpp lines 2425-2434)
    pub fn remove_radar(&mut self, disable_proof: bool) {
        if self.radar_count > 0 {
            self.radar_count -= 1;
        }
        if disable_proof && self.disable_proof_radar_count > 0 {
            self.disable_proof_radar_count -= 1;
        }
    }

    /// Disable radar (regardless of count)
    /// C++ Reference: Player::disableRadar() (Player.cpp lines 2437-2440)
    pub fn disable_radar(&mut self) {
        self.radar_disabled = true;
    }

    /// Enable radar (remove restriction)
    /// C++ Reference: Player::enableRadar() (Player.cpp lines 2443-2446)
    pub fn enable_radar(&mut self) {
        self.radar_disabled = false;
    }

    /// Check if player has radar
    /// C++ Reference: Player::hasRadar() (Player.cpp lines 2449-2452)
    pub fn has_radar(&self) -> bool {
        self.radar_count > 0 && !self.radar_disabled
    }

    // =========================================================
    // Battle Plan System (C++ Player.h lines 302-304)
    // =========================================================

    /// Get total number of battle plans active
    /// C++ Reference: Player::getNumBattlePlansActive() (Player.h line 228)
    pub fn get_num_battle_plans_active(&self) -> i32 {
        self.bombard_battle_plans
            + self.hold_the_line_battle_plans
            + self.search_and_destroy_battle_plans
    }

    /// Get count of specific battle plan type
    /// C++ Reference: Player::getBattlePlansActiveSpecific() (Player.cpp lines 2455-2469)
    pub fn get_battle_plans_active_specific(&self, plan_type: BattlePlanStatus) -> i32 {
        match plan_type {
            BattlePlanStatus::Bombardment => self.bombard_battle_plans,
            BattlePlanStatus::HoldTheLine => self.hold_the_line_battle_plans,
            BattlePlanStatus::SearchAndDestroy => self.search_and_destroy_battle_plans,
        }
    }

    /// Change a battle plan count
    /// C++ Reference: Player::changeBattlePlan() (Player.cpp lines 2472-2498)
    pub fn change_battle_plan(&mut self, plan: BattlePlanStatus, delta: i32) {
        match plan {
            BattlePlanStatus::Bombardment => self.bombard_battle_plans += delta,
            BattlePlanStatus::HoldTheLine => self.hold_the_line_battle_plans += delta,
            BattlePlanStatus::SearchAndDestroy => self.search_and_destroy_battle_plans += delta,
        }
    }

    // =========================================================
    // Attacked Tracking (C++ Player.h lines 378-379)
    // =========================================================

    /// Mark that this player was attacked by another player
    /// C++ Reference: Player::setAttackedBy() (Player.cpp lines 3173-3176)
    pub fn set_attacked_by(&mut self, player_index: i32) {
        if player_index >= 0 && (player_index as usize) < self.attacked_by.len() {
            self.attacked_by[player_index as usize] = true;
            self.attacked_frame = crate::common::time::frame();
        }
    }

    /// Check if this player was attacked by another player
    /// C++ Reference: Player::getAttackedBy() (Player.cpp lines 3179-3182)
    pub fn get_attacked_by(&self, player_index: i32) -> bool {
        if player_index >= 0 && (player_index as usize) < self.attacked_by.len() {
            self.attacked_by[player_index as usize]
        } else {
            false
        }
    }

    /// Get the last frame this player was attacked
    /// C++ Reference: Player::getAttackedFrame() (Player.h line 421)
    pub fn get_attacked_frame(&self) -> u32 {
        self.attacked_frame
    }

    /// Get the attacked-by array (for save/load)
    pub fn get_attacked_by_array(&self) -> &[bool] {
        &self.attacked_by
    }

    /// Set the attacked-by array (for load)
    pub fn set_attacked_by_array(&mut self, attacked: Vec<bool>) {
        self.attacked_by = attacked;
    }

    // =========================================================
    // Player State Queries (C++ Player.h lines 398-412)
    // =========================================================

    /// Check if player is dead
    /// C++ Reference: Player::isPlayerDead() (Player.h line 408)
    pub fn is_player_dead(&self) -> bool {
        self.is_player_dead
    }

    /// Set player dead state
    pub fn set_player_dead(&mut self, dead: bool) {
        self.is_player_dead = dead;
    }

    /// Check if player is an observer
    /// C++ Reference: Player::isPlayerObserver() (Player.h line 407)
    pub fn is_player_observer(&self) -> bool {
        self.observer
    }

    /// Set observer mode
    /// C++ Reference: Player::init() sets m_observer (Player.cpp line 320)
    pub fn set_observer(&mut self, observer: bool) {
        self.observer = observer;
        // Observers are considered "dead" for gameplay purposes
        if observer {
            self.is_player_dead = true;
        }
    }

    /// Check if player is active (not dead and not observer)
    /// C++ Reference: Player::isPlayerActive() (Player.h line 409)
    pub fn is_player_active(&self) -> bool {
        !self.observer && !self.is_player_dead
    }

    /// Check if this is a playable side
    /// C++ Reference: Player::isPlayableSide() (Player.cpp lines 3185-3190)
    pub fn is_playable_side(&self) -> bool {
        // Would check player template - simplified
        !self.observer && !self.side.is_empty()
    }

    /// Check if player preordered
    /// C++ Reference: Player::didPlayerPreorder() (Player.h line 411)
    pub fn did_player_preorder(&self) -> bool {
        self.is_preorder
    }

    /// Set preorder status
    pub fn set_preorder(&mut self, preorder: bool) {
        self.is_preorder = preorder;
    }

    /// Check if should be listed in score screen
    /// C++ Reference: Player::getListInScoreScreen() (Player.h line 413)
    pub fn get_list_in_score_screen(&self) -> bool {
        self.list_in_score_screen
    }

    /// Set score screen listing
    pub fn set_list_in_score_screen(&mut self, list: bool) {
        self.list_in_score_screen = list;
    }

    /// Get units should hunt flag
    /// C++ Reference: Player::getUnitsShouldHunt() (Player.h line 376)
    pub fn get_units_should_hunt(&self) -> bool {
        self.units_should_hunt
    }

    /// Set units should hunt
    /// C++ Reference: Player::setUnitsShouldHunt() (Player.cpp lines 3179-3182)
    pub fn set_units_should_hunt(&mut self, should_hunt: bool) {
        self.units_should_hunt = should_hunt;
    }

    /// Get can build units
    /// C++ Reference: Player::getCanBuildUnits() (Player.h line 395)
    pub fn get_can_build_units(&self) -> bool {
        self.can_build_units
    }

    /// Set can build units
    pub fn set_can_build_units(&mut self, can_build: bool) {
        self.can_build_units = can_build;
    }

    /// Get can build base
    /// C++ Reference: Player::getCanBuildBase() (Player.h line 397)
    pub fn get_can_build_base(&self) -> bool {
        self.can_build_base
    }

    /// Set can build base
    pub fn set_can_build_base(&mut self, can_build: bool) {
        self.can_build_base = can_build;
    }

    // =========================================================
    // Kill Player and Related (C++ Player.cpp lines 1597-1650)
    // =========================================================

    /// Kill this player - remove all units and mark as dead
    /// C++ Reference: Player::killPlayer() (Player.cpp lines 1597-1650)
    pub fn kill_player(&mut self) {
        // Mark player as dead first (so OCLs don't spawn useful units)
        self.is_player_dead = true;

        // Clear all team prototypes (would evacuate containers in full impl)
        self.team_prototypes.clear();

        // Clear all hotkey squads
        for squad in &mut self.hotkey_squads {
            squad.clear();
        }

        // Clear current selection
        self.current_selection.clear();

        // Clear build list
        self.build_list = None;

        // Clear supply centers and warehouses
        self.supply_centers.clear();
        self.supply_warehouses.clear();

        // Clear tunnel entrances
        self.tunnel_entrances.clear();

        // Force money to 0
        let all_money = self.money.count_money();
        if all_money > 0 {
            self.money.withdraw(all_money, false);
        }
    }

    /// Transfer all assets from another player to this one
    /// C++ Reference: Player::transferAssetsFromThat() (Player.cpp lines 1666-1701)
    pub fn transfer_assets_from(&mut self, other: &mut Player) {
        // Transfer all money
        let all_money = other.get_money().count_money();
        if all_money > 0 {
            other.get_money_mut().withdraw(all_money, false);
            self.money.deposit(all_money, false);
        }

        // In full implementation, would also transfer all objects
        // to this player's default team
    }

    /// Garrison all units
    /// C++ Reference: Player::garrisonAllUnits() (Player.cpp lines 1704-1751)
    pub fn garrison_all_units(&mut self) {
        // Would iterate all units and find garrisonable structures
        // Simplified: just mark intent
    }

    /// Ungarrison all units
    /// C++ Reference: Player::ungarrisonAllUnits() (Player.cpp lines 1754-1784)
    pub fn ungarrison_all_units(&mut self) {
        // Would iterate all structures and tell them to evacuate
        // Simplified: just mark intent
    }

    /// Set units to idle or resume
    /// C++ Reference: Player::setUnitsShouldIdleOrResume() (Player.cpp lines 1788-1827)
    pub fn set_units_should_idle_or_resume(&mut self, idle: bool) {
        // Would iterate all units and set their idle state
        // Simplified: no-op
        let _ = idle;
    }

    /// Sell everything under the sun
    /// C++ Reference: Player::sellEverythingUnderTheSun() (Player.cpp lines 1832-1839)
    pub fn sell_everything(&mut self) {
        // Would iterate and sell all faction structures
        // Simplified: just clear build list
        self.build_list = None;
    }

    /// Set objects enabled/disabled by template
    /// C++ Reference: Player::setObjectsEnabled() (Player.cpp lines 1653-1663)
    pub fn set_objects_enabled(&mut self, _template_name: &str, _enable: bool) {
        // Would iterate all objects matching template and enable/disable them
        // Simplified: no-op
    }

    // =========================================================
    // Build Prerequisites and Permissions (C++ Player.cpp lines 1842-2061)
    // =========================================================

    /// Check if allowed to build a thing (basic check)
    /// C++ Reference: Player::allowedToBuild() (Player.cpp lines 1842-1855)
    pub fn allowed_to_build(&self, is_structure: bool) -> bool {
        if !self.can_build_base && is_structure {
            return false;
        }
        if !self.can_build_units && !is_structure {
            return false;
        }
        true
    }

    /// Check if can build a thing (includes prereqs)
    /// C++ Reference: Player::canBuild() (Player.cpp lines 2880-2924)
    ///
    /// This is the simplified interface that doesn't check template-level properties.
    /// For full C++-faithful checking, use `can_build_template`.
    pub fn can_build(&self, _template_name: &str, is_structure: bool) -> bool {
        // Basic check
        if !self.allowed_to_build(is_structure) {
            return false;
        }

        true
    }

    /// Full prerequisite check matching C++ Player::canBuild() behavior.
    ///
    /// C++ Reference: Player::canBuild() (Player.cpp lines 2880-2924)
    ///
    /// Checks:
    /// 1. allowedToBuild()
    /// 2. BuildableStatus != BSTATUS_NO
    /// 3. BuildableStatus != BSTATUS_ONLY_BY_AI (unless player is COMPUTER)
    /// 4. All ProductionPrerequisite entries satisfied (AND logic)
    /// 5. (Debug) ignoresPrereqs override
    /// 6. canBuildMoreOfType
    pub fn can_build_template(
        &self,
        is_structure: bool,
        buildable: i32, // 0=Yes, 1=IgnorePrerequisites, 2=No, 3=OnlyByAI
        prereqs: &[ProductionPrerequisite],
    ) -> bool {
        // C++ line 2885: if (!allowedToBuild(tmplate)) return false;
        if !self.allowed_to_build(is_structure) {
            return false;
        }

        // C++ lines 2888-2895: BuildableStatus checks
        // BuildableStatus: Yes=0, Ignore_Prerequisites=1, No=2, Only_By_AI=3
        if buildable == 2 {
            // BSTATUS_NO
            return false;
        }
        if buildable == 1 {
            // BSTATUS_IGNORE_PREREQUISITES
            return true;
        }
        if buildable == 3 && self.player_type != PlayerType::Computer {
            // BSTATUS_ONLY_BY_AI
            return false;
        }

        // C++ lines 2898-2917: Check all prerequisites (AND logic)
        // All ProductionPrerequisite entries must be satisfied
        let mut prereqs_ok = true;
        for prereq in prereqs {
            if !prereq.is_satisfied(self) {
                prereqs_ok = false;
                break;
            }
        }

        // C++ lines 2909-2912: Debug override
        #[cfg(debug_assertions)]
        if self.ignores_prereqs() {
            prereqs_ok = true;
        }

        if !prereqs_ok {
            return false;
        }

        // C++ lines 2919-2920: canBuildMoreOfType
        // Note: max_simultaneous check requires template info, handled by caller

        true
    }

    /// Check if can afford to build
    /// C++ Reference: Player::canAffordBuild() (Player.cpp lines 2064-2073)
    pub fn can_afford_build(&self, cost: i32) -> bool {
        self.money.count_money() >= cost as u32
    }

    /// Check if can build more of a specific type
    /// C++ Reference: Player::canBuildMoreOfType() (Player.cpp lines 1907-1950)
    pub fn can_build_more_of_type(&self, _template_name: &str, max_simultaneous: u32) -> bool {
        // 0 means unlimited
        if max_simultaneous == 0 {
            return true;
        }
        // Would count existing units and queued units
        // Simplified: assume can build
        true
    }

    // =========================================================
    // AI Build Commands (C++ Player.cpp lines 1858-1960)
    // =========================================================

    /// Build specific team (AI command)
    /// C++ Reference: Player::buildSpecificTeam() (Player.cpp lines 1858-1864)
    pub fn build_specific_team(&mut self, team_name: &str) {
        if let Some(ai) = self.get_ai() {
            let _ = (ai, team_name); // Would call AI build method
        }
    }

    /// Build base defense (AI command)
    /// C++ Reference: Player::buildBaseDefense() (Player.cpp lines 1867-1873)
    pub fn build_base_defense(&mut self, flank: bool) {
        if let Some(ai) = self.get_ai() {
            let _ = (ai, flank); // Would call AI build method
        }
    }

    /// Build base defense structure (AI command)
    /// C++ Reference: Player::buildBaseDefenseStructure() (Player.cpp lines 1876-1882)
    pub fn build_base_defense_structure(&mut self, thing_name: &str, flank: bool) {
        if let Some(ai) = self.get_ai() {
            let _ = (ai, thing_name, flank); // Would call AI build method
        }
    }

    /// Build specific building (AI command)
    /// C++ Reference: Player::buildSpecificBuilding() (Player.cpp lines 1885-1891)
    pub fn build_specific_building(&mut self, thing_name: &str) {
        if let Some(ai) = self.get_ai() {
            let _ = (ai, thing_name); // Would call AI build method
        }
    }

    /// Build by supplies (AI command)
    /// C++ Reference: Player::buildBySupplies() (Player.cpp lines 1894-1900)
    pub fn build_by_supplies(&mut self, minimum_cash: i32, thing_name: &str) {
        if let Some(ai) = self.get_ai() {
            let _ = (ai, minimum_cash, thing_name); // Would call AI build method
        }
    }

    /// Build specific building nearest team (AI command)
    /// C++ Reference: Player::buildSpecificBuildingNearestTeam() (Player.cpp lines 1903-1907)
    pub fn build_specific_building_nearest_team(&mut self, thing_name: &str, _team_id: i32) {
        if let Some(ai) = self.get_ai() {
            let _ = (ai, thing_name); // Would call AI build method
        }
    }

    /// Build upgrade (AI command)
    /// C++ Reference: Player::buildUpgrade() (Player.cpp lines 1910-1916)
    pub fn build_upgrade(&mut self, upgrade_name: &str) {
        if let Some(ai) = self.get_ai() {
            let _ = (ai, upgrade_name); // Would call AI build method
        }
    }

    /// Recruit specific team (AI command)
    /// C++ Reference: Player::recruitSpecificTeam() (Player.cpp lines 1919-1925)
    pub fn recruit_specific_team(&mut self, team_name: &str, recruit_radius: f32) {
        if let Some(ai) = self.get_ai() {
            let _ = (ai, team_name, recruit_radius); // Would call AI recruit method
        }
    }

    /// Calculate closest construction zone location
    /// C++ Reference: Player::calcClosestConstructionZoneLocation() (Player.cpp lines 1929-1939)
    pub fn calc_closest_construction_zone(&self, _template_name: &str) -> Option<Coord3D> {
        // Would query AI for construction zone
        self.get_ai().and_then(|_| None)
    }

    // =========================================================
    // Relationship System (C++ Player.cpp lines 540-590)
    // =========================================================

    /// Get the relationship with another player by their player index.
    /// C++ Reference: Player::getRelationship() for player index lookup
    /// Returns NEUTRAL if no relationship is explicitly set.
    ///
    /// # Arguments
    /// * `player_index` - The index of the other player
    ///
    /// # Returns
    /// The relationship type (Allies, Enemies, or Neutral)
    pub fn get_relationship(&self, player_index: i32) -> Relationship {
        self.player_relations
            .get(player_index)
            .unwrap_or(Relationship::Neutral)
    }

    /// Set the relationship with another player.
    /// C++ Reference: Player::setPlayerRelationship() lines 582-588
    ///
    /// # Arguments
    /// * `player_index` - The index of the other player
    /// * `relationship` - The relationship to set
    pub fn set_player_relationship(&mut self, player_index: i32, relationship: Relationship) {
        self.player_relations.set(player_index, relationship);
    }

    /// Remove all relationships, or a specific player relationship.
    /// Returns true if relationships were removed.
    ///
    /// # Arguments
    /// * `player_index` - If Some, remove only that player's relationship. If None, clear all.
    pub fn remove_player_relationship(&mut self, player_index: Option<i32>) -> bool {
        self.player_relations.remove(player_index)
    }

    /// Get a reference to the player relations map
    pub fn get_player_relations(&self) -> &PlayerRelationMap {
        &self.player_relations
    }

    /// Get a mutable reference to the player relations map
    pub fn get_player_relations_mut(&mut self) -> &mut PlayerRelationMap {
        &mut self.player_relations
    }

    // =========================================================
    // Science System (C++ Player.h lines 325-327)
    // =========================================================

    /// Get skill points
    /// C++ Reference: Player::getSkillPoints() (Player.h line 330)
    pub fn get_skill_points(&self) -> i32 {
        self.skill_points
    }

    /// Add skill points, returns true if player gained/lost levels
    /// C++ Reference: Player::addSkillPoints() (Player.cpp lines 3041-3084)
    pub fn add_skill_points(&mut self, delta: i32) -> bool {
        // C++ line 3045: Apply modifier
        let adjusted_delta = (delta as f32 * self.skill_points_modifier) as i32;

        // C++ lines 3050-3052: Check for no change
        if adjusted_delta == 0 {
            return false;
        }

        // C++ line 3054: Apply the change
        let old_rank = self.rank_level;
        self.skill_points += adjusted_delta;

        // C++ lines 3057-3083: Check for rank up/down
        // This would use RankInfo to determine thresholds
        // Simplified: check if rank should change based on skill points
        let new_rank = self.calculate_rank_from_skill_points();
        if new_rank != old_rank {
            self.rank_level = new_rank;
            true
        } else {
            false
        }
    }

    /// Calculate rank level from current skill points
    /// Simplified version - C++ uses RankInfo system
    fn calculate_rank_from_skill_points(&self) -> i32 {
        // Simplified rank calculation (C++ uses TheRankInfo->getRankLevelForSkillPoints)
        let points = self.skill_points;
        if points >= 5000 {
            8
        } else if points >= 4000 {
            7
        } else if points >= 3000 {
            6
        } else if points >= 2000 {
            5
        } else if points >= 1000 {
            4
        } else if points >= 500 {
            3
        } else if points >= 100 {
            2
        } else {
            1
        }
    }

    /// Get rank level
    /// C++ Reference: Player::getRankLevel() (Player.h line 332)
    pub fn get_rank_level(&self) -> i32 {
        self.rank_level
    }

    /// Set rank level, returns true if changed
    /// C++ Reference: Player::setRankLevel() (Player.cpp lines 3090-3115)
    pub fn set_rank_level(&mut self, level: i32) -> bool {
        if level != self.rank_level && level >= 1 {
            let old_level = self.rank_level;
            self.rank_level = level;

            // C++ lines 3099-3114: Grant rank sciences
            // This would grant sciences associated with this rank
            // Simplified: just update the level

            old_level != self.rank_level
        } else {
            false
        }
    }

    /// Get skill points modifier
    /// C++ Reference: Player::getSkillPointsModifier() (Player.h line 342)
    pub fn get_skill_points_modifier(&self) -> f32 {
        self.skill_points_modifier
    }

    /// Set skill points modifier
    /// C++ Reference: Player::setSkillPointsModifier() (Player.h line 341)
    pub fn set_skill_points_modifier(&mut self, modifier: f32) {
        self.skill_points_modifier = modifier;
    }

    /// Get skill points to level up
    /// C++ Reference: Player::getSkillPointsLevelUp() (Player.h line 333)
    pub fn get_skill_points_level_up(&self) -> i32 {
        self.level_up
    }

    /// Get skill points to level down
    /// C++ Reference: Player::getSkillPointsLevelDown() (Player.h line 334)
    pub fn get_skill_points_level_down(&self) -> i32 {
        self.level_down
    }

    /// Get general name
    /// C++ Reference: Player::getGeneralName() (Player.h line 335)
    pub fn get_general_name(&self) -> &str {
        &self.general_name
    }

    /// Set general name
    /// C++ Reference: Player::setGeneralName() (Player.h line 336)
    pub fn set_general_name(&mut self, name: String) {
        self.general_name = name;
    }

    // =========================================================
    // Science Purchase Points (C++ Player.h lines 337-340)
    // =========================================================

    /// Get science purchase points
    /// C++ Reference: Player::getSciencePurchasePoints() (Player.h line 331)
    pub fn get_science_purchase_points(&self) -> i32 {
        self.science_purchase_points
    }

    /// Add science purchase points
    /// C++ Reference: Player::addSciencePurchasePoints() (Player.h line 339)
    pub fn add_science_purchase_points(&mut self, delta: i32) {
        let old_points = self.science_purchase_points;
        self.science_purchase_points += delta;
        if self.science_purchase_points < 0 {
            self.science_purchase_points = 0;
        }

        // Notify UI if changed (would notify control bar in full impl)
        let _ = old_points; // Just to note we track the change
    }

    /// Add skill points for kill
    /// C++ Reference: Player::addSkillPointsForKill() (Player.cpp lines 2104-2115)
    pub fn add_skill_points_for_kill(&mut self, victim_level: i32, skill_value: i32) -> bool {
        let _ = victim_level; // Would affect calculation based on victim's veterancy
        self.add_skill_points(skill_value)
    }

    /// Add skill points for kill using trait objects.
    /// C++ Reference: Player::addSkillPointsForKill(const Object* killer, const Object* victim)
    ///
    /// # Arguments
    /// * `killer` - The object that made the kill (unused in basic implementation)
    /// * `victim` - The object that was killed
    pub fn add_skill_points_for_kill_obj(
        &mut self,
        killer: &dyn SkillPointObject,
        victim: &dyn SkillPointObject,
    ) -> bool {
        let victim_level = victim.get_veterancy_level();
        let skill_value = victim.get_skill_point_value(killer);
        self.add_skill_points_for_kill(victim_level, skill_value)
    }

    /// Complete rank reset to initial state
    /// C++ Reference: Player::resetRank() (Player.cpp lines 2142-2163)
    pub fn reset_rank_full(&mut self) {
        self.rank_level = 1;
        self.skill_points = 0;
        self.level_up = 100; // Would get from RankInfo
        self.level_down = 0;
        self.sciences.clear();
        self.science_purchase_points = 0; // Would get from player template
        self.general_name = "General".to_string();
        self.reset_sciences();
    }

    /// Get all sciences
    pub fn get_sciences(&self) -> &HashSet<ScienceType> {
        &self.sciences
    }

    /// Get all disabled sciences
    pub fn get_sciences_disabled(&self) -> &HashSet<ScienceType> {
        &self.sciences_disabled
    }

    /// Get all hidden sciences
    pub fn get_sciences_hidden(&self) -> &HashSet<ScienceType> {
        &self.sciences_hidden
    }

    /// Set sciences directly (for save/load)
    pub fn set_sciences(&mut self, sciences: HashSet<ScienceType>) {
        self.sciences = sciences;
    }

    /// Set disabled sciences directly (for save/load)
    pub fn set_sciences_disabled(&mut self, sciences: HashSet<ScienceType>) {
        self.sciences_disabled = sciences;
    }

    /// Set hidden sciences directly (for save/load)
    pub fn set_sciences_hidden(&mut self, sciences: HashSet<ScienceType>) {
        self.sciences_hidden = sciences;
    }

    // =========================================================
    // Bounty System (C++ Player.h lines 373-376)
    // =========================================================

    /// Get cash bounty percent
    /// C++ Reference: Player::getCashBounty() (Player.h line 423)
    pub fn get_cash_bounty_percent(&self) -> f32 {
        self.cash_bounty_percent
    }

    /// Set cash bounty percent
    /// C++ Reference: Player::setCashBounty() (Player.h line 424)
    pub fn set_cash_bounty_percent(&mut self, percent: f32) {
        self.cash_bounty_percent = percent;
    }

    /// Do bounty for kill - awards cash when player kills an enemy
    /// C++ Reference: Player::doBountyForKill() (Player.cpp lines 1963-1989)
    pub fn do_bounty_for_kill(&mut self, killer_cost: i32) -> i32 {
        // Calculate bounty based on victim's cost and our cash bounty percent
        let bounty = ((killer_cost as f32) * self.cash_bounty_percent).ceil() as i32;

        if bounty > 0 {
            if let Ok(amount) = u32::try_from(bounty) {
                self.money.deposit(amount, false);
            }
            self.score_keeper.add_money_earned(bounty);
        }

        bounty
    }

    /// Do bounty for kill using trait objects.
    /// C++ Reference: Player::doBountyForKill(const Object* killer, const Object* victim)
    ///
    /// # Arguments
    /// * `_killer` - The object that made the kill (unused in basic implementation)
    /// * `victim` - The object that was killed
    ///
    /// Returns the bounty amount awarded.
    pub fn do_bounty_for_kill_obj(
        &mut self,
        _killer: &dyn BountyObject,
        victim: &dyn BountyObject,
    ) -> i32 {
        // C++ lines 1968-1970: Don't award bounty for under-construction objects
        if victim.is_under_construction() {
            return 0;
        }

        // C++ line 1972: Get victim's build cost for bounty calculation
        let killer_cost = victim.get_build_cost();

        self.do_bounty_for_kill(killer_cost)
    }

    // =========================================================
    // CRC for networking (C++ Player.cpp lines 3939-3960)
    // =========================================================

    /// Compute CRC for network synchronization.
    /// C++ Reference: Player::crc(Xfer* xfer) - used for network game state validation
    /// This method computes a simple CRC hash of the player's critical state
    /// for network synchronization purposes.
    pub fn crc(&self) -> u32 {
        // Simple CRC computation based on key player state
        // This mirrors the C++ approach of xfer'ing key values for CRC
        let mut result: u32 = 0;

        // Hash player index
        result = result.wrapping_add(self.index as u32);

        // Hash skill points
        result = result.wrapping_add(self.skill_points as u32);

        // Hash science purchase points
        result = result.wrapping_add(self.science_purchase_points as u32);

        // Hash rank level
        result = result.wrapping_add(self.rank_level as u32);

        // Hash cash bounty (convert to bits for deterministic hashing)
        result = result.wrapping_add(self.cash_bounty_percent.to_bits());

        // Hash relationships using PlayerRelationMap (deterministic order)
        let mut indices: Vec<_> = self.player_relations.iter().map(|(k, _)| *k).collect();
        indices.sort();
        for idx in indices {
            result = result.wrapping_add(idx as u32);
            if let Some(rel) = self.player_relations.get(idx) {
                result = result.wrapping_add(rel.clone() as i32 as u32);
            }
        }

        // Hash sciences count (for state consistency)
        result = result.wrapping_add(self.sciences.len() as u32);
        result = result.wrapping_add(self.sciences_disabled.len() as u32);
        result = result.wrapping_add(self.sciences_hidden.len() as u32);

        result
    }

    /// Check whether this player already owns the specified science
    pub fn has_science(&self, science: ScienceType) -> bool {
        science != SCIENCE_INVALID && self.sciences.contains(&science)
    }

    /// Grant a science to the player
    pub fn grant_science(&mut self, science: ScienceType) {
        if science == SCIENCE_INVALID {
            return;
        }
        self.sciences_disabled.remove(&science);
        self.sciences_hidden.remove(&science);
        self.sciences.insert(science);
    }

    /// Disable a science (remains known but unusable)
    pub fn disable_science(&mut self, science: ScienceType) {
        if science == SCIENCE_INVALID {
            return;
        }
        self.sciences.remove(&science);
        self.sciences_hidden.remove(&science);
        self.sciences_disabled.insert(science);
    }

    /// Hide a science (used by UI gating, retains knowledge state)
    pub fn hide_science(&mut self, science: ScienceType) {
        if science == SCIENCE_INVALID {
            return;
        }
        self.sciences_disabled.remove(&science);
        self.sciences_hidden.insert(science);
    }

    /// Check if a science is disabled
    pub fn is_science_disabled(&self, science: ScienceType) -> bool {
        self.sciences_disabled.contains(&science)
    }

    /// Check if a science is hidden
    pub fn is_science_hidden(&self, science: ScienceType) -> bool {
        self.sciences_hidden.contains(&science)
    }

    /// Set science availability
    /// C++ Reference: Player::setScienceAvailability() (Player.cpp lines 2273-2307)
    pub fn set_science_availability(&mut self, science: ScienceType, available: bool) {
        if available {
            // Remove from disabled and hidden lists
            self.sciences_disabled.remove(&science);
            self.sciences_hidden.remove(&science);
        } else {
            // Add to disabled list
            self.sciences_disabled.insert(science);
        }
    }

    /// Check if has prerequisites for science
    /// C++ Reference: Player::hasPrereqsForScience() (Player.cpp lines 1992-1995)
    pub fn has_prereqs_for_science(&self, science: ScienceType) -> bool {
        if science == SCIENCE_INVALID {
            return false;
        }
        // In full implementation, would check TheScienceStore
        // Simplified: always true
        true
    }

    /// Check if capable of purchasing science
    /// C++ Reference: Player::isCapableOfPurchasingScience() (Player.cpp lines 2226-2254)
    pub fn is_capable_of_purchasing_science(&self, science: ScienceType) -> bool {
        if science == SCIENCE_INVALID {
            return false;
        }

        // Already have it?
        if self.has_science(science) {
            return false;
        }

        // Is it disabled or hidden?
        if self.is_science_disabled(science) || self.is_science_hidden(science) {
            return false;
        }

        // Has prereqs?
        if !self.has_prereqs_for_science(science) {
            return false;
        }

        // Check cost (simplified: assume cost of 1)
        let cost = 1; // Would query TheScienceStore->getSciencePurchaseCost()
        if cost == 0 || cost > self.science_purchase_points {
            return false;
        }

        true
    }

    /// Attempt to purchase a science
    /// C++ Reference: Player::attemptToPurchaseScience() (Player.cpp lines 2204-2223)
    pub fn attempt_to_purchase_science(&mut self, science: ScienceType) -> bool {
        if !self.is_capable_of_purchasing_science(science) {
            return false;
        }

        // Deduct cost (simplified: 1 point)
        let cost = 1;
        self.science_purchase_points -= cost;
        if self.science_purchase_points < 0 {
            self.science_purchase_points = 0;
        }

        // Add the science
        self.grant_science(science);

        // Record in academy stats
        self.academy_stats.record_generals_points_spent(cost);

        true
    }

    /// Grant a science (bypassing purchase system)
    /// C++ Reference: Player::grantScience() (Player.cpp lines 2195-2201)
    pub fn grant_science_with_check(&mut self, science: ScienceType) -> bool {
        // In full implementation, would check TheScienceStore->isScienceGrantable()
        self.grant_science(science);
        true
    }

    /// Reset sciences to default state
    /// C++ Reference: Player::resetSciences() (Player.cpp lines 2118-2140)
    pub fn reset_sciences_full(&mut self) {
        self.sciences.clear();
        self.sciences_disabled.clear();
        self.sciences_hidden.clear();

        // In full implementation, would grant intrinsic sciences from player template
        // and rank sciences from RankInfo
    }

    // =========================================================
    // AI System Integration (C++ Player.cpp lines 695-712)
    // =========================================================

    /// Set the AI player reference
    /// C++ Reference: Player::setPlayerType() creates and assigns m_ai
    pub fn set_ai(&mut self, ai: Option<Arc<dyn AIPlayerInterface>>) {
        self.ai = ai.map(|arc| Arc::downgrade(&arc));
    }

    /// Get the AI player reference
    /// Returns None if player is human or AI has been destroyed
    pub fn get_ai(&self) -> Option<Arc<dyn AIPlayerInterface>> {
        self.ai.as_ref().and_then(|weak| weak.upgrade())
    }

    /// Check if this player has an AI controller
    /// C++ Reference: m_ai != NULL checks throughout Player.cpp
    pub fn has_ai(&self) -> bool {
        self.ai
            .as_ref()
            .map_or(false, |weak| weak.strong_count() > 0)
    }

    /// Get player difficulty
    /// C++ Reference: Player::getPlayerDifficulty() (Player.cpp lines 1500-1505)
    pub fn get_player_difficulty(&self) -> GameDifficulty {
        self.difficulty
    }

    /// Set player difficulty
    pub fn set_player_difficulty(&mut self, difficulty: GameDifficulty) {
        self.difficulty = difficulty;
        // Also update AI if present
        if let Some(ai) = self.get_ai() {
            // AI would be updated via write access - skipped here for simplicity
            let _ = ai;
        }
    }

    /// Check if this is a skirmish AI player
    /// C++ Reference: Player::isSkirmishAIPlayer() (Player.cpp lines 1491-1494)
    pub fn is_skirmish_ai_player(&self) -> bool {
        self.get_ai().map_or(false, |ai| ai.is_skirmish_ai())
    }

    /// Get current enemy for AI
    /// C++ Reference: Player::getCurrentEnemy() (Player.cpp lines 1486-1489)
    pub fn get_current_enemy(&self) -> Option<i32> {
        self.get_ai().and_then(|ai| ai.get_ai_enemy())
    }

    // =========================================================
    // Build List Management (C++ Player.cpp lines 592-636)
    // =========================================================

    /// Set the build list
    /// C++ Reference: Player::setBuildList() (Player.cpp lines 592-598)
    pub fn set_build_list(&mut self, build_list: Option<Box<BuildListInfo>>) {
        self.build_list = build_list;
    }

    /// Get the build list
    /// C++ Reference: Player::getBuildList() (Player.h line 316)
    pub fn get_build_list(&self) -> Option<&BuildListInfo> {
        self.build_list.as_deref()
    }

    /// Get mutable build list
    pub fn get_build_list_mut(&mut self) -> Option<&mut BuildListInfo> {
        self.build_list.as_deref_mut()
    }

    /// Add an object to the build list
    /// C++ Reference: Player::addToBuildList() (Player.cpp lines 601-610)
    pub fn add_to_build_list(
        &mut self,
        object_id: ObjectID,
        template_name: String,
        location: Coord3D,
        angle: f32,
    ) {
        let mut new_info = Box::new(BuildListInfo::new(template_name, location, angle));
        new_info.set_object_id(object_id);
        new_info.set_num_rebuilds(0); // Can't rebuild
        new_info.set_next(self.build_list.take());
        self.build_list = Some(new_info);
    }

    /// Add to priority build list
    /// C++ Reference: Player::addToPriorityBuildList() (Player.cpp lines 613-623)
    pub fn add_to_priority_build_list(
        &mut self,
        template_name: String,
        location: Coord3D,
        angle: f32,
    ) {
        let mut new_info = Box::new(BuildListInfo::new(template_name, location, angle));
        new_info.mark_priority_build();
        new_info.set_num_rebuilds(1); // Build once
        new_info.set_next(self.build_list.take());
        self.build_list = Some(new_info);
    }

    // =========================================================
    // Resource Gathering Manager (C++ ResourceGatheringManager.h)
    // =========================================================

    /// Add a supply center
    /// C++ Reference: ResourceGatheringManager::addSupplyCenter()
    pub fn add_supply_center(&mut self, center_id: ObjectID) {
        if !self.supply_centers.contains(&center_id) {
            self.supply_centers.push(center_id);
        }
    }

    /// Remove a supply center
    /// C++ Reference: ResourceGatheringManager::removeSupplyCenter()
    pub fn remove_supply_center(&mut self, center_id: ObjectID) {
        self.supply_centers.retain(|&id| id != center_id);
    }

    /// Add a supply warehouse
    /// C++ Reference: ResourceGatheringManager::addSupplyWarehouse()
    pub fn add_supply_warehouse(&mut self, warehouse_id: ObjectID) {
        if !self.supply_warehouses.contains(&warehouse_id) {
            self.supply_warehouses.push(warehouse_id);
        }
    }

    /// Remove a supply warehouse
    /// C++ Reference: ResourceGatheringManager::removeSupplyWarehouse()
    pub fn remove_supply_warehouse(&mut self, warehouse_id: ObjectID) {
        self.supply_warehouses.retain(|&id| id != warehouse_id);
    }

    /// Get all supply centers
    pub fn get_supply_centers(&self) -> &[ObjectID] {
        &self.supply_centers
    }

    /// Get all supply warehouses
    pub fn get_supply_warehouses(&self) -> &[ObjectID] {
        &self.supply_warehouses
    }

    /// Find best supply warehouse for a query object
    /// C++ Reference: ResourceGatheringManager::findBestSupplyWarehouse()
    pub fn find_best_supply_warehouse(&self, _query_object_id: ObjectID) -> Option<ObjectID> {
        // Simplified: return first available warehouse
        // Full implementation would check distances and validity
        self.supply_warehouses.first().copied()
    }

    /// Find best supply center for a query object
    /// C++ Reference: ResourceGatheringManager::findBestSupplyCenter()
    pub fn find_best_supply_center(&self, _query_object_id: ObjectID) -> Option<ObjectID> {
        // Simplified: return first available center
        self.supply_centers.first().copied()
    }

    // =========================================================
    // Squad System - Hotkey Squads (C++ Player.h line 382)
    // =========================================================

    /// Get a hotkey squad by number
    /// C++ Reference: Player::getHotkeySquad() (Player.h line 429)
    pub fn get_hotkey_squad(&mut self, squad_number: i32) -> Option<&mut Squad> {
        if squad_number >= 0 && (squad_number as usize) < NUM_HOTKEY_SQUADS {
            Some(&mut self.hotkey_squads[squad_number as usize])
        } else {
            None
        }
    }

    /// Get hotkey squad (const access)
    pub fn get_hotkey_squad_const(&self, squad_number: i32) -> Option<&Squad> {
        if squad_number >= 0 && (squad_number as usize) < NUM_HOTKEY_SQUADS {
            Some(&self.hotkey_squads[squad_number as usize])
        } else {
            None
        }
    }

    /// Get the squad number for an object, or NO_HOTKEY_SQUAD if not in any
    /// C++ Reference: Player::getSquadNumberForObject() (Player.cpp)
    pub fn get_squad_number_for_object(&self, object_id: ObjectID) -> i32 {
        for (i, squad) in self.hotkey_squads.iter().enumerate() {
            if squad.contains(object_id) {
                return i as i32;
            }
        }
        NO_HOTKEY_SQUAD
    }

    /// Remove an object from all hotkey squads
    /// C++ Reference: Player::removeObjectFromHotkeySquad() (Player.cpp)
    pub fn remove_object_from_hotkey_squad(&mut self, object_id: ObjectID) {
        for squad in &mut self.hotkey_squads {
            squad.remove_object(object_id);
        }
    }

    /// Clear a specific hotkey squad
    pub fn clear_hotkey_squad(&mut self, squad_number: i32) {
        if let Some(squad) = self.get_hotkey_squad(squad_number) {
            squad.clear();
        }
    }

    // =========================================================
    // Current Selection Tracking (C++ Player.h line 383)
    // =========================================================

    /// Get the current selection squad
    /// C++ Reference: m_currentSelection usage throughout Player.cpp
    pub fn get_current_selection(&self) -> &Squad {
        &self.current_selection
    }

    /// Get mutable current selection
    pub fn get_current_selection_mut(&mut self) -> &mut Squad {
        &mut self.current_selection
    }

    /// Clear current selection
    pub fn clear_current_selection(&mut self) {
        self.current_selection.clear();
    }

    /// Add object to current selection
    pub fn add_to_current_selection(&mut self, object_id: ObjectID) {
        self.current_selection.add_object(object_id);
    }

    /// Remove object from current selection
    pub fn remove_from_current_selection(&mut self, object_id: ObjectID) {
        self.current_selection.remove_object(object_id);
    }

    /// Check if object is in current selection
    pub fn is_in_current_selection(&self, object_id: ObjectID) -> bool {
        self.current_selection.contains(object_id)
    }

    /// Get current selection size
    pub fn get_current_selection_size(&self) -> usize {
        self.current_selection.len()
    }

    // =========================================================
    // Upgrade List Management (C++ Player.h line 336)
    // =========================================================

    /// Add an upgrade to the player's list
    /// C++ Reference: Player::addUpgrade() (Player.cpp)
    pub fn add_upgrade(&mut self, upgrade_name: String, status: UpgradeStatus) {
        // Check if already exists
        if let Some(existing) = self
            .upgrade_list
            .iter_mut()
            .find(|u| u.get_name() == upgrade_name)
        {
            existing.set_status(status);
        } else {
            let mut upgrade = UpgradeInfo::new(upgrade_name);
            upgrade.set_status(status);
            self.upgrade_list.push(upgrade);
        }
    }

    /// Remove an upgrade from the player's list
    /// C++ Reference: Player::removeUpgrade() (Player.cpp)
    pub fn remove_upgrade(&mut self, upgrade_name: &str) {
        self.upgrade_list.retain(|u| u.get_name() != upgrade_name);
    }

    /// Find an upgrade by name
    /// C++ Reference: Player::findUpgrade() (Player.h line 163)
    pub fn find_upgrade(&self, upgrade_name: &str) -> Option<&UpgradeInfo> {
        self.upgrade_list
            .iter()
            .find(|u| u.get_name() == upgrade_name)
    }

    /// Find mutable upgrade by name
    pub fn find_upgrade_mut(&mut self, upgrade_name: &str) -> Option<&mut UpgradeInfo> {
        self.upgrade_list
            .iter_mut()
            .find(|u| u.get_name() == upgrade_name)
    }

    /// Check if player has an upgrade complete
    /// C++ Reference: Player::hasUpgradeComplete() (Player.h line 157)
    pub fn has_upgrade_complete(&self, upgrade_name: &str) -> bool {
        self.upgrade_list
            .iter()
            .any(|u| u.get_name() == upgrade_name && u.is_complete())
    }

    /// Check if player has an upgrade in production
    /// C++ Reference: Player::hasUpgradeInProduction() (Player.h line 160)
    pub fn has_upgrade_in_production(&self, upgrade_name: &str) -> bool {
        self.upgrade_list
            .iter()
            .any(|u| u.get_name() == upgrade_name && u.is_in_production())
    }

    /// Get completed upgrade mask
    /// C++ Reference: Player::getCompletedUpgradeMask() (Player.h line 159)
    pub fn get_completed_upgrade_mask(&self) -> u64 {
        self.upgrades_completed
    }

    /// Set upgrade in progress bit
    pub fn set_upgrade_in_progress(&mut self, bit: u32) {
        if bit < 64 {
            self.upgrades_in_progress |= 1 << bit;
        }
    }

    /// Clear upgrade in progress bit
    pub fn clear_upgrade_in_progress(&mut self, bit: u32) {
        if bit < 64 {
            self.upgrades_in_progress &= !(1 << bit);
        }
    }

    /// Set upgrade completed bit
    pub fn set_upgrade_completed(&mut self, bit: u32) {
        if bit < 64 {
            self.upgrades_completed |= 1 << bit;
            // Clear from in-progress when completed
            self.upgrades_in_progress &= !(1 << bit);
        }
    }

    /// Clear upgrade completed bit
    pub fn clear_upgrade_completed(&mut self, bit: u32) {
        if bit < 64 {
            self.upgrades_completed &= !(1 << bit);
        }
    }

    // =========================================================
    // Team Prototype List (C++ Player.h line 375)
    // =========================================================

    /// Add a team prototype to the player's list
    /// C++ Reference: Player::addTeamToList() (Player.cpp lines 974-982)
    pub fn add_team_prototype(&mut self, team_name: String) {
        if !self.team_prototypes.contains(&team_name) {
            self.team_prototypes.push(team_name);
        }
    }

    /// Remove a team prototype from the player's list
    /// C++ Reference: Player::removeTeamFromList() (Player.cpp lines 985-995)
    pub fn remove_team_prototype(&mut self, team_name: &str) {
        self.team_prototypes.retain(|name| name != team_name);
    }

    /// Get all team prototypes
    pub fn get_team_prototypes(&self) -> &[String] {
        &self.team_prototypes
    }

    // =========================================================
    // Tunnel System (C++ Player.h line 341)
    // =========================================================

    /// Add a tunnel entrance
    pub fn add_tunnel_entrance(&mut self, entrance_id: ObjectID) {
        if !self.tunnel_entrances.contains(&entrance_id) {
            self.tunnel_entrances.push(entrance_id);
        }
    }

    /// Remove a tunnel entrance
    pub fn remove_tunnel_entrance(&mut self, entrance_id: ObjectID) {
        self.tunnel_entrances.retain(|&id| id != entrance_id);
    }

    /// Get all tunnel entrances
    pub fn get_tunnel_entrances(&self) -> &[ObjectID] {
        &self.tunnel_entrances
    }

    // =========================================================
    // Production Cost/Time Changes (C++ Player.h lines 351-353)
    // =========================================================

    /// Set production cost change for a thing
    /// C++ Reference: Player production cost modifiers
    pub fn set_production_cost_change(&mut self, thing_name: String, percent: f32) {
        self.production_cost_changes.insert(thing_name, percent);
    }

    /// Get production cost change for a thing
    /// C++ Reference: Player::getProductionCostChangePercent() (Player.cpp)
    pub fn get_production_cost_change(&self, thing_name: &str) -> f32 {
        self.production_cost_changes
            .get(thing_name)
            .copied()
            .unwrap_or(1.0)
    }

    /// Set production time change for a thing
    pub fn set_production_time_change(&mut self, thing_name: String, percent: f32) {
        self.production_time_changes.insert(thing_name, percent);
    }

    /// Get production time change for a thing
    /// C++ Reference: Player::getProductionTimeChangePercent() (Player.cpp)
    pub fn get_production_time_change(&self, thing_name: &str) -> f32 {
        self.production_time_changes
            .get(thing_name)
            .copied()
            .unwrap_or(1.0)
    }

    /// Get production cost change based on KindOf mask.
    /// C++ Reference: Player::getProductionCostChangeBasedOnKindOf (Player.cpp lines 3842-3859)
    ///
    /// Iterates the KindOf-based production cost changes. For each entry whose
    /// KindOf mask overlaps with the provided `kindof`, the modifier is applied
    /// multiplicatively: `result *= (1 + percent)`.
    pub fn get_production_cost_change_based_on_kind_of(&self, kindof: u64) -> f32 {
        let mut result = 1.0f32;
        for (mask, percent) in &self.kind_of_production_cost_changes {
            if (kindof & mask) != 0 {
                result *= 1.0 + percent;
            }
        }
        result
    }

    /// Add a KindOf-based production cost change entry.
    pub fn add_kind_of_production_cost_change(&mut self, kindof: u64, percent: f32) {
        self.kind_of_production_cost_changes.push((kindof, percent));
    }

    // =========================================================
    // Special Power Timers (C++ Player.h line 392)
    // =========================================================

    /// Set special power ready frame
    pub fn set_special_power_ready_frame(&mut self, template_id: u32, ready_frame: u32) {
        self.special_power_timers.insert(template_id, ready_frame);
    }

    /// Get special power ready frame
    pub fn get_special_power_ready_frame(&self, template_id: u32) -> Option<u32> {
        self.special_power_timers.get(&template_id).copied()
    }

    /// Remove special power timer
    pub fn remove_special_power_timer(&mut self, template_id: u32) {
        self.special_power_timers.remove(&template_id);
    }

    // =========================================================
    // Vision Spied (C++ Player.cpp lines 3138-3152)
    // =========================================================

    /// Set units vision spied status
    /// C++ Reference: Player::setUnitsVisionSpied() (Player.cpp lines 3138-3152)
    pub fn set_units_vision_spied(&mut self, _setting: bool, _by_whom: i32) {
        // Would iterate all objects and set their vision spied status
        // Simplified: no-op
    }

    // =========================================================
    // Retaliation Mode (C++ Player.cpp lines 573-590)
    // =========================================================

    /// Get logical retaliation mode enabled
    /// C++ Reference: Player::isLogicalRetaliationModeEnabled() (Player.h line 391)
    pub fn is_logical_retaliation_mode_enabled(&self) -> bool {
        self.logical_retaliation_mode_enabled
    }

    /// Set logical retaliation mode enabled
    /// C++ Reference: Player::setLogicalRetaliationModeEnabled()
    pub fn set_logical_retaliation_mode_enabled(&mut self, enabled: bool) {
        self.logical_retaliation_mode_enabled = enabled;
    }

    // =========================================================
    // Default Team (C++ Player.h line 321)
    // =========================================================

    /// Get default team
    /// C++ Reference: Player::getDefaultTeam() (Player.h line 322)
    pub fn get_default_team(&self) -> Option<TeamID> {
        self.default_team
    }

    /// Set default team
    /// C++ Reference: Player::setDefaultTeam() (Player.cpp lines 715-725)
    pub fn set_default_team(&mut self, team_id: TeamID) {
        self.default_team = Some(team_id);
    }

    // =========================================================
    // Side Information (C++ Player.h lines 289-290)
    // =========================================================

    /// Set player side
    pub fn set_side(&mut self, side: String) {
        self.side = side;
    }

    /// Set player base side
    pub fn set_base_side(&mut self, base_side: String) {
        self.base_side = base_side;
    }

    /// Set player display name
    pub fn set_player_display_name(&mut self, name: String) {
        self.player_display_name = name;
    }

    /// Set player name
    pub fn set_player_name(&mut self, name: String) {
        self.player_name = name;
    }

    // =========================================================
    // Debug/Cheat Methods (C++ #if _DEBUG sections)
    // =========================================================

    /// Check if ignores prereqs (debug only)
    /// C++ Reference: Player::ignoresPrereqs() (Player.cpp)
    #[cfg(debug_assertions)]
    pub fn ignores_prereqs(&self) -> bool {
        // Would return m_DEMO_ignorePrereqs in debug builds
        false
    }

    /// Check if free build (debug only)
    /// C++ Reference: Player::isFreeBuild() (Player.cpp)
    #[cfg(debug_assertions)]
    pub fn is_free_build(&self) -> bool {
        // Would return m_DEMO_freeBuild in debug builds
        false
    }

    /// Check if instant build (debug only)
    /// C++ Reference: Player::isInstantBuild() (Player.cpp)
    #[cfg(debug_assertions)]
    pub fn is_instant_build(&self) -> bool {
        // Would return m_DEMO_instantBuild in debug builds
        false
    }

    // =========================================================
    // Skillset (C++ Player.cpp line 1928)
    // =========================================================

    /// Set AI skillset (friend function for AI)
    /// C++ Reference: Player::friend_setSkillset() (Player.cpp line 1928)
    pub fn set_skillset(&mut self, skillset: i32) {
        if let Some(ai) = self.get_ai() {
            let _ = (ai, skillset); // Would call ai.selectSkillset()
        }
    }

    // =========================================================
    // Score Methods (C++ ScoreKeeper integration)
    // =========================================================

    /// Add object built to score
    pub fn score_add_object_built(&mut self, cost: i32) {
        self.score_keeper.add_money_spent(cost);
    }

    /// Get score keeper reference
    pub fn get_score_keeper_mut_ref(&mut self) -> &mut ScoreKeeper {
        &mut self.score_keeper
    }

    // =========================================================
    // Supply Box Value (C++ Player.cpp lines 1928-1933)
    // =========================================================

    /// Get supply box value
    /// C++ Reference: Player::getSupplyBoxValue() (Player.cpp lines 1928-1933)
    pub fn get_supply_box_value(&self) -> u32 {
        // Would return TheGlobalData->m_baseValuePerSupplyBox
        // Simplified: return a default value
        100
    }

    // =========================================================
    // New Map (C++ Player.cpp lines 592-595)
    // =========================================================

    /// Called when a new map is loaded
    /// C++ Reference: Player::newMap() (Player.cpp lines 592-595)
    pub fn new_map(&mut self) {
        if let Some(ai) = self.get_ai() {
            let _ = ai; // Would call ai.new_map()
        }
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new(0)
    }
}

impl super::science::ScienceAccess for Player {
    fn has_science(&self, science: ScienceType) -> bool {
        Player::has_science(self, science)
    }
}

// =========================================================
// Snapshotable Implementation (save/load and CRC)
// C++ Reference: Player.cpp lines 3936-4526
// =========================================================

impl Snapshotable for Player {
    /// CRC computation for network synchronization.
    /// C++ Reference: Player::crc() lines 3939-3960
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // Xfer skill points and science purchase points for CRC
        let mut skill_points = self.skill_points;
        let mut science_purchase_points = self.science_purchase_points;

        xfer.xfer_int(&mut skill_points)
            .map_err(|e| format!("CRC skill_points failed: {}", e))?;
        xfer.xfer_int(&mut science_purchase_points)
            .map_err(|e| format!("CRC science_purchase_points failed: {}", e))?;

        // Xfer cash bounty percent
        let mut cash_bounty = self.cash_bounty_percent;
        xfer.xfer_real(&mut cash_bounty)
            .map_err(|e| format!("CRC cash_bounty_percent failed: {}", e))?;

        Ok(())
    }

    /// Save/load player state.
    /// C++ Reference: Player::xfer() lines 3975-4526
    /// Version History:
    ///   1: Initial version
    ///   2: Skill point modifier
    ///   3: Score screen exclusion flag
    ///   4: Special power ready timer list
    ///   5: ???
    ///   6: m_unitsShouldHunt flag
    ///   7: Preorder flag
    ///   8: Disabled/hidden sciences
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 8;
        let mut version = CURRENT_VERSION;

        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("xfer_version failed: {}", e))?;

        // Money - use Money's own xfer_save/xfer_load methods
        match xfer.get_xfer_mode() {
            XferMode::Save => {
                let money_data = self.money.xfer_save();
                let mut len = money_data.len() as u16;
                xfer.xfer_unsigned_short(&mut len)
                    .map_err(|e| format!("money len xfer failed: {}", e))?;
                for byte in &money_data {
                    let mut b = *byte as i8;
                    xfer.xfer_byte(&mut b)
                        .map_err(|e| format!("money data xfer failed: {}", e))?;
                }
            }
            XferMode::Load => {
                let mut len = 0u16;
                xfer.xfer_unsigned_short(&mut len)
                    .map_err(|e| format!("money len load failed: {}", e))?;
                let mut money_data = vec![0u8; len as usize];
                for byte in &mut money_data {
                    let mut b = 0i8;
                    xfer.xfer_byte(&mut b)
                        .map_err(|e| format!("money data load failed: {}", e))?;
                    *byte = b as u8;
                }
                self.money
                    .xfer_load(&money_data)
                    .map_err(|e| e.to_string())?;
            }
            XferMode::Crc => {
                // For CRC, just hash the money amount
                let amount = self.money.count_money();
                let mut amount_mut = amount;
                xfer.xfer_unsigned_int(&mut amount_mut)
                    .map_err(|e| format!("money crc failed: {}", e))?;
            }
            _ => {}
        }

        // Player relations - delegate to PlayerRelationMap::xfer()
        // C++ lines 4297-4335: PlayerRelationMap::xfer
        self.player_relations
            .xfer(xfer)
            .map_err(|e| format!("player_relations xfer failed: {}", e))?;

        // Rank level
        xfer.xfer_int(&mut self.rank_level)
            .map_err(|e| format!("rank_level xfer failed: {}", e))?;

        // Skill points
        xfer.xfer_int(&mut self.skill_points)
            .map_err(|e| format!("skill_points xfer failed: {}", e))?;

        // Science purchase points
        xfer.xfer_int(&mut self.science_purchase_points)
            .map_err(|e| format!("science_purchase_points xfer failed: {}", e))?;

        // Player dead state
        let mut is_dead = self.is_player_dead;
        xfer.xfer_bool(&mut is_dead)
            .map_err(|e| format!("is_player_dead xfer failed: {}", e))?;
        if matches!(xfer.get_xfer_mode(), XferMode::Load) {
            self.is_player_dead = is_dead;
        }

        // Observer flag
        let mut observer = self.observer;
        xfer.xfer_bool(&mut observer)
            .map_err(|e| format!("observer xfer failed: {}", e))?;
        if matches!(xfer.get_xfer_mode(), XferMode::Load) {
            self.observer = observer;
        }

        // Cash bounty percent
        xfer.xfer_real(&mut self.cash_bounty_percent)
            .map_err(|e| format!("cash_bounty_percent xfer failed: {}", e))?;

        // Score keeper - xfer the player index this keeper tracks
        // The score keeper itself will be re-initialized on load
        let mut score_player_idx = self.index;
        xfer.xfer_int(&mut score_player_idx)
            .map_err(|e| format!("score_keeper player_idx xfer failed: {}", e))?;
        if matches!(xfer.get_xfer_mode(), XferMode::Load) {
            self.score_keeper.reset(score_player_idx);
        }

        // Version 8+: Disabled and hidden sciences
        if version >= 8 {
            let mut disabled_count = self.sciences_disabled.len() as u16;
            let mut hidden_count = self.sciences_hidden.len() as u16;

            xfer.xfer_unsigned_short(&mut disabled_count)
                .map_err(|e| format!("disabled_count xfer failed: {}", e))?;
            xfer.xfer_unsigned_short(&mut hidden_count)
                .map_err(|e| format!("hidden_count xfer failed: {}", e))?;

            match xfer.get_xfer_mode() {
                XferMode::Save | XferMode::Crc => {
                    for &science in &self.sciences_disabled {
                        let mut sci = science;
                        xfer.xfer_int(&mut sci)
                            .map_err(|e| format!("disabled science xfer failed: {}", e))?;
                    }
                    for &science in &self.sciences_hidden {
                        let mut sci = science;
                        xfer.xfer_int(&mut sci)
                            .map_err(|e| format!("hidden science xfer failed: {}", e))?;
                    }
                }
                XferMode::Load => {
                    self.sciences_disabled.clear();
                    self.sciences_hidden.clear();
                    for _ in 0..disabled_count {
                        let mut science = 0i32;
                        xfer.xfer_int(&mut science)
                            .map_err(|e| format!("load disabled science failed: {}", e))?;
                        self.sciences_disabled.insert(science);
                    }
                    for _ in 0..hidden_count {
                        let mut science = 0i32;
                        xfer.xfer_int(&mut science)
                            .map_err(|e| format!("load hidden science failed: {}", e))?;
                        self.sciences_hidden.insert(science);
                    }
                }
                _ => {}
            }
        }

        // Upgrade list count
        let mut upgrade_count = self.upgrade_list.len() as u16;
        xfer.xfer_unsigned_short(&mut upgrade_count)
            .map_err(|e| format!("upgrade_count xfer failed: {}", e))?;

        match xfer.get_xfer_mode() {
            XferMode::Save => {
                for upgrade in &self.upgrade_list {
                    let mut name_bytes: Vec<i8> =
                        upgrade.get_name().bytes().map(|b| b as i8).collect();
                    let mut name_len = name_bytes.len() as u16;
                    xfer.xfer_unsigned_short(&mut name_len)
                        .map_err(|e| format!("upgrade name len failed: {}", e))?;
                    for byte in &mut name_bytes {
                        xfer.xfer_byte(byte)
                            .map_err(|e| format!("upgrade name byte failed: {}", e))?;
                    }
                    let mut status = upgrade.get_status() as i32;
                    xfer.xfer_int(&mut status)
                        .map_err(|e| format!("upgrade status failed: {}", e))?;
                }
            }
            XferMode::Load => {
                self.upgrade_list.clear();
                for _ in 0..upgrade_count {
                    let mut name_len = 0u16;
                    xfer.xfer_unsigned_short(&mut name_len)
                        .map_err(|e| format!("load upgrade name len failed: {}", e))?;
                    let mut name_bytes = vec![0i8; name_len as usize];
                    for byte in &mut name_bytes {
                        xfer.xfer_byte(byte)
                            .map_err(|e| format!("load upgrade name byte failed: {}", e))?;
                    }
                    let name: String = name_bytes.iter().map(|&b| b as u8 as char).collect();
                    let mut status = 0i32;
                    xfer.xfer_int(&mut status)
                        .map_err(|e| format!("load upgrade status failed: {}", e))?;
                    let status = match status {
                        0 => UpgradeStatus::Pending,
                        1 => UpgradeStatus::InProduction,
                        _ => UpgradeStatus::Complete,
                    };
                    let mut upgrade = UpgradeInfo::new(name);
                    upgrade.set_status(status);
                    self.upgrade_list.push(upgrade);
                }
            }
            _ => {}
        }

        // Radar info
        xfer.xfer_int(&mut self.radar_count)
            .map_err(|e| format!("radar_count xfer failed: {}", e))?;
        xfer.xfer_int(&mut self.disable_proof_radar_count)
            .map_err(|e| format!("disable_proof_radar_count failed: {}", e))?;
        xfer.xfer_bool(&mut self.radar_disabled)
            .map_err(|e| format!("radar_disabled xfer failed: {}", e))?;

        // Upgrades in progress and completed (store as two u32s each since u64 may not be directly supported)
        let mut upgrades_in_progress_lo = (self.upgrades_in_progress & 0xFFFFFFFF) as u32;
        let mut upgrades_in_progress_hi = ((self.upgrades_in_progress >> 32) & 0xFFFFFFFF) as u32;
        let mut upgrades_completed_lo = (self.upgrades_completed & 0xFFFFFFFF) as u32;
        let mut upgrades_completed_hi = ((self.upgrades_completed >> 32) & 0xFFFFFFFF) as u32;
        xfer.xfer_unsigned_int(&mut upgrades_in_progress_lo)
            .map_err(|e| format!("upgrades_in_progress_lo failed: {}", e))?;
        xfer.xfer_unsigned_int(&mut upgrades_in_progress_hi)
            .map_err(|e| format!("upgrades_in_progress_hi failed: {}", e))?;
        xfer.xfer_unsigned_int(&mut upgrades_completed_lo)
            .map_err(|e| format!("upgrades_completed_lo failed: {}", e))?;
        xfer.xfer_unsigned_int(&mut upgrades_completed_hi)
            .map_err(|e| format!("upgrades_completed_hi failed: {}", e))?;
        if matches!(xfer.get_xfer_mode(), XferMode::Load) {
            self.upgrades_in_progress =
                ((upgrades_in_progress_hi as u64) << 32) | (upgrades_in_progress_lo as u64);
            self.upgrades_completed =
                ((upgrades_completed_hi as u64) << 32) | (upgrades_completed_lo as u64);
        }

        // Team prototypes
        let mut prototype_count = self.team_prototypes.len() as u16;
        xfer.xfer_unsigned_short(&mut prototype_count)
            .map_err(|e| format!("prototype_count failed: {}", e))?;
        match xfer.get_xfer_mode() {
            XferMode::Save => {
                for prototype_name in &self.team_prototypes {
                    let mut name_bytes: Vec<i8> = prototype_name.bytes().map(|b| b as i8).collect();
                    let mut name_len = name_bytes.len() as u16;
                    xfer.xfer_unsigned_short(&mut name_len)
                        .map_err(|e| format!("prototype name len failed: {}", e))?;
                    for byte in &mut name_bytes {
                        xfer.xfer_byte(byte)
                            .map_err(|e| format!("prototype name byte failed: {}", e))?;
                    }
                }
            }
            XferMode::Load => {
                self.team_prototypes.clear();
                for _ in 0..prototype_count {
                    let mut name_len = 0u16;
                    xfer.xfer_unsigned_short(&mut name_len)
                        .map_err(|e| format!("load prototype name len failed: {}", e))?;
                    let mut name_bytes = vec![0i8; name_len as usize];
                    for byte in &mut name_bytes {
                        xfer.xfer_byte(byte)
                            .map_err(|e| format!("load prototype name byte failed: {}", e))?;
                    }
                    let name: String = name_bytes.iter().map(|&b| b as u8 as char).collect();
                    self.team_prototypes.push(name);
                }
            }
            _ => {}
        }

        // Build list count
        let mut build_list_count = 0u16;
        let mut current: Option<&BuildListInfo> = self.build_list.as_deref();
        while let Some(info) = current {
            build_list_count += 1;
            current = info.get_next();
        }
        xfer.xfer_unsigned_short(&mut build_list_count)
            .map_err(|e| format!("build_list_count failed: {}", e))?;

        match xfer.get_xfer_mode() {
            XferMode::Save => {
                current = self.build_list.as_deref();
                while let Some(info) = current {
                    let mut template_bytes: Vec<i8> =
                        info.get_template_name().bytes().map(|b| b as i8).collect();
                    let mut len = template_bytes.len() as u16;
                    xfer.xfer_unsigned_short(&mut len)
                        .map_err(|e| format!("build template len failed: {}", e))?;
                    for byte in &mut template_bytes {
                        xfer.xfer_byte(byte)
                            .map_err(|e| format!("build template byte failed: {}", e))?;
                    }
                    let mut x = info.get_location().x;
                    let mut y = info.get_location().y;
                    let mut z = info.get_location().z;
                    let mut angle = info.get_angle();
                    let mut object_id = info.get_object_id();
                    let mut num_rebuilds = info.get_num_rebuilds();
                    let mut priority = info.is_priority_build() as i32;
                    xfer.xfer_real(&mut x)
                        .map_err(|e| format!("x failed: {}", e))?;
                    xfer.xfer_real(&mut y)
                        .map_err(|e| format!("y failed: {}", e))?;
                    xfer.xfer_real(&mut z)
                        .map_err(|e| format!("z failed: {}", e))?;
                    xfer.xfer_real(&mut angle)
                        .map_err(|e| format!("angle failed: {}", e))?;
                    xfer.xfer_unsigned_int(&mut object_id)
                        .map_err(|e| format!("object_id failed: {}", e))?;
                    xfer.xfer_unsigned_int(&mut num_rebuilds)
                        .map_err(|e| format!("num_rebuilds failed: {}", e))?;
                    xfer.xfer_int(&mut priority)
                        .map_err(|e| format!("priority failed: {}", e))?;
                    current = info.get_next();
                }
            }
            XferMode::Load => {
                self.build_list = None;
                for _ in 0..build_list_count {
                    let mut len = 0u16;
                    xfer.xfer_unsigned_short(&mut len)
                        .map_err(|e| format!("load build template len failed: {}", e))?;
                    let mut template_bytes = vec![0i8; len as usize];
                    for byte in &mut template_bytes {
                        xfer.xfer_byte(byte)
                            .map_err(|e| format!("load build template byte failed: {}", e))?;
                    }
                    let template_name: String =
                        template_bytes.iter().map(|&b| b as u8 as char).collect();
                    let mut x = 0.0f32;
                    let mut y = 0.0f32;
                    let mut z = 0.0f32;
                    let mut angle = 0.0f32;
                    let mut object_id = 0u32;
                    let mut num_rebuilds = 0u32;
                    let mut priority = 0i32;
                    xfer.xfer_real(&mut x)
                        .map_err(|e| format!("load x failed: {}", e))?;
                    xfer.xfer_real(&mut y)
                        .map_err(|e| format!("load y failed: {}", e))?;
                    xfer.xfer_real(&mut z)
                        .map_err(|e| format!("load z failed: {}", e))?;
                    xfer.xfer_real(&mut angle)
                        .map_err(|e| format!("load angle failed: {}", e))?;
                    xfer.xfer_unsigned_int(&mut object_id)
                        .map_err(|e| format!("load object_id failed: {}", e))?;
                    xfer.xfer_unsigned_int(&mut num_rebuilds)
                        .map_err(|e| format!("load num_rebuilds failed: {}", e))?;
                    xfer.xfer_int(&mut priority)
                        .map_err(|e| format!("load priority failed: {}", e))?;
                    let mut info = Box::new(BuildListInfo::new(
                        template_name,
                        Coord3D::new(x, y, z),
                        angle,
                    ));
                    info.set_object_id(object_id);
                    info.set_num_rebuilds(num_rebuilds);
                    if priority != 0 {
                        info.mark_priority_build();
                    }
                    info.set_next(self.build_list.take());
                    self.build_list = Some(info);
                }
            }
            _ => {}
        }

        // AI present flag
        let mut ai_present = self.ai.is_some();
        xfer.xfer_bool(&mut ai_present)
            .map_err(|e| format!("ai_present xfer failed: {}", e))?;

        // Resource manager and tunnel present flags
        let mut resource_manager_present =
            !self.supply_centers.is_empty() || !self.supply_warehouses.is_empty();
        let mut tunnel_present = !self.tunnel_entrances.is_empty();
        xfer.xfer_bool(&mut resource_manager_present)
            .map_err(|e| format!("resource_manager_present failed: {}", e))?;
        xfer.xfer_bool(&mut tunnel_present)
            .map_err(|e| format!("tunnel_present xfer failed: {}", e))?;

        // Default team
        let mut default_team_id = self.default_team.unwrap_or(0) as i32;
        xfer.xfer_int(&mut default_team_id)
            .map_err(|e| format!("default_team_id xfer failed: {}", e))?;
        if matches!(xfer.get_xfer_mode(), XferMode::Load) {
            self.default_team = if default_team_id != 0 {
                Some(default_team_id as u32)
            } else {
                None
            };
        }

        // Sciences
        let mut science_count = self.sciences.len() as u16;
        xfer.xfer_unsigned_short(&mut science_count)
            .map_err(|e| format!("science_count xfer failed: {}", e))?;
        match xfer.get_xfer_mode() {
            XferMode::Save => {
                for &science in &self.sciences {
                    let mut sci = science;
                    xfer.xfer_int(&mut sci)
                        .map_err(|e| format!("science xfer failed: {}", e))?;
                }
            }
            XferMode::Load => {
                self.sciences.clear();
                for _ in 0..science_count {
                    let mut science = 0i32;
                    xfer.xfer_int(&mut science)
                        .map_err(|e| format!("load science failed: {}", e))?;
                    self.sciences.insert(science);
                }
            }
            _ => {}
        }

        // Level up/down
        xfer.xfer_int(&mut self.level_up)
            .map_err(|e| format!("level_up xfer failed: {}", e))?;
        xfer.xfer_int(&mut self.level_down)
            .map_err(|e| format!("level_down xfer failed: {}", e))?;

        // General name
        let mut general_name_bytes: Vec<i8> = self.general_name.bytes().map(|b| b as i8).collect();
        let mut general_name_len = general_name_bytes.len() as u16;
        xfer.xfer_unsigned_short(&mut general_name_len)
            .map_err(|e| format!("general_name_len failed: {}", e))?;
        match xfer.get_xfer_mode() {
            XferMode::Save => {
                for byte in &mut general_name_bytes {
                    xfer.xfer_byte(byte)
                        .map_err(|e| format!("general_name byte failed: {}", e))?;
                }
            }
            XferMode::Load => {
                let mut name_bytes = vec![0i8; general_name_len as usize];
                for byte in &mut name_bytes {
                    xfer.xfer_byte(byte)
                        .map_err(|e| format!("load general_name byte failed: {}", e))?;
                }
                self.general_name = name_bytes.iter().map(|&b| b as u8 as char).collect();
            }
            _ => {}
        }

        // Can build flags
        xfer.xfer_bool(&mut self.can_build_units)
            .map_err(|e| format!("can_build_units xfer failed: {}", e))?;
        xfer.xfer_bool(&mut self.can_build_base)
            .map_err(|e| format!("can_build_base xfer failed: {}", e))?;

        // Version 2+: Skill point modifier
        if version >= 2 {
            xfer.xfer_real(&mut self.skill_points_modifier)
                .map_err(|e| format!("skill_points_modifier xfer failed: {}", e))?;
        }

        // Version 3+: List in score screen
        if version >= 3 {
            xfer.xfer_bool(&mut self.list_in_score_screen)
                .map_err(|e| format!("list_in_score_screen xfer failed: {}", e))?;
        }

        // Attacked by array
        for i in 0..self.attacked_by.len() {
            let mut attacked = self.attacked_by[i];
            xfer.xfer_bool(&mut attacked)
                .map_err(|e| format!("attacked_by[{}] xfer failed: {}", i, e))?;
            if matches!(xfer.get_xfer_mode(), XferMode::Load) {
                self.attacked_by[i] = attacked;
            }
        }

        // Version 4+: Special power timers
        if version >= 4 {
            let mut timer_count = self.special_power_timers.len() as u16;
            xfer.xfer_unsigned_short(&mut timer_count)
                .map_err(|e| format!("timer_count xfer failed: {}", e))?;
            match xfer.get_xfer_mode() {
                XferMode::Save => {
                    for (&template_id, &ready_frame) in &self.special_power_timers {
                        let mut tid = template_id;
                        let mut rf = ready_frame;
                        xfer.xfer_unsigned_int(&mut tid)
                            .map_err(|e| format!("timer template_id failed: {}", e))?;
                        xfer.xfer_unsigned_int(&mut rf)
                            .map_err(|e| format!("timer ready_frame failed: {}", e))?;
                    }
                }
                XferMode::Load => {
                    self.special_power_timers.clear();
                    for _ in 0..timer_count {
                        let mut template_id = 0u32;
                        let mut ready_frame = 0u32;
                        xfer.xfer_unsigned_int(&mut template_id)
                            .map_err(|e| format!("load timer template_id failed: {}", e))?;
                        xfer.xfer_unsigned_int(&mut ready_frame)
                            .map_err(|e| format!("load timer ready_frame failed: {}", e))?;
                        self.special_power_timers.insert(template_id, ready_frame);
                    }
                }
                _ => {}
            }
        }

        // Squads
        let squad_count = NUM_HOTKEY_SQUADS as u16;
        let mut squad_count_xfer = squad_count;
        xfer.xfer_unsigned_short(&mut squad_count_xfer)
            .map_err(|e| format!("squad_count xfer failed: {}", e))?;

        for i in 0..NUM_HOTKEY_SQUADS {
            let mut obj_count = self.hotkey_squads[i].len() as u16;
            xfer.xfer_unsigned_short(&mut obj_count)
                .map_err(|e| format!("squad[{}] obj_count failed: {}", i, e))?;
            match xfer.get_xfer_mode() {
                XferMode::Save => {
                    for &obj_id in self.hotkey_squads[i].get_object_ids() {
                        let mut id = obj_id;
                        xfer.xfer_unsigned_int(&mut id)
                            .map_err(|e| format!("squad[{}] obj_id failed: {}", i, e))?;
                    }
                }
                XferMode::Load => {
                    self.hotkey_squads[i].clear();
                    for _ in 0..obj_count {
                        let mut obj_id = 0u32;
                        xfer.xfer_unsigned_int(&mut obj_id)
                            .map_err(|e| format!("load squad[{}] obj_id failed: {}", i, e))?;
                        self.hotkey_squads[i].add_object(obj_id);
                    }
                }
                _ => {}
            }
        }

        // Current selection
        let mut selection_present = true;
        xfer.xfer_bool(&mut selection_present)
            .map_err(|e| format!("selection_present xfer failed: {}", e))?;
        if selection_present {
            let mut obj_count = self.current_selection.len() as u16;
            xfer.xfer_unsigned_short(&mut obj_count)
                .map_err(|e| format!("current_selection obj_count failed: {}", e))?;
            match xfer.get_xfer_mode() {
                XferMode::Save => {
                    for &obj_id in self.current_selection.get_object_ids() {
                        let mut id = obj_id;
                        xfer.xfer_unsigned_int(&mut id)
                            .map_err(|e| format!("current_selection obj_id failed: {}", e))?;
                    }
                }
                XferMode::Load => {
                    self.current_selection.clear();
                    for _ in 0..obj_count {
                        let mut obj_id = 0u32;
                        xfer.xfer_unsigned_int(&mut obj_id)
                            .map_err(|e| format!("load current_selection obj_id failed: {}", e))?;
                        self.current_selection.add_object(obj_id);
                    }
                }
                _ => {}
            }
        }

        // Battle plan counts
        xfer.xfer_int(&mut self.bombard_battle_plans)
            .map_err(|e| format!("bombard_battle_plans xfer failed: {}", e))?;
        xfer.xfer_int(&mut self.hold_the_line_battle_plans)
            .map_err(|e| format!("hold_the_line_battle_plans xfer failed: {}", e))?;
        xfer.xfer_int(&mut self.search_and_destroy_battle_plans)
            .map_err(|e| format!("search_and_destroy_battle_plans xfer failed: {}", e))?;

        // Version 6+: Units should hunt
        if version >= 6 {
            xfer.xfer_bool(&mut self.units_should_hunt)
                .map_err(|e| format!("units_should_hunt xfer failed: {}", e))?;
        }

        // Version 7+: Preorder
        if version >= 7 {
            xfer.xfer_bool(&mut self.is_preorder)
                .map_err(|e| format!("is_preorder xfer failed: {}", e))?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

// =========================================================
// Tests
// =========================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_creation() {
        let player = Player::new(3);
        assert_eq!(player.get_player_index(), 3);
        assert!(!player.is_player_dead());
        assert!(player.is_player_active());
    }

    #[test]
    fn test_relationship_system() {
        let mut player = Player::new(0);

        // Default relationship should be Neutral
        assert_eq!(player.get_relationship(1), Relationship::Neutral);

        // Set relationship to Allies
        player.set_player_relationship(1, Relationship::Allies);
        assert_eq!(player.get_relationship(1), Relationship::Allies);

        // Set relationship to Enemies
        player.set_player_relationship(2, Relationship::Enemies);
        assert_eq!(player.get_relationship(2), Relationship::Enemies);

        // Unset relationship should still be Neutral
        assert_eq!(player.get_relationship(3), Relationship::Neutral);

        // Remove relationship
        assert!(player.remove_player_relationship(Some(1)));
        assert_eq!(player.get_relationship(1), Relationship::Neutral);

        // Clear all relationships
        player.set_player_relationship(4, Relationship::Allies);
        player.set_player_relationship(5, Relationship::Enemies);
        assert!(player.remove_player_relationship(None));
        assert_eq!(player.get_relationship(4), Relationship::Neutral);
        assert_eq!(player.get_relationship(5), Relationship::Neutral);
    }

    #[test]
    fn test_player_state() {
        let mut player = Player::new(0);

        // Test dead state
        player.set_player_dead(true);
        assert!(player.is_player_dead());
        assert!(!player.is_player_active());

        // Test skill points
        player.add_skill_points(100);
        assert_eq!(player.get_skill_points(), 100);

        // Test rank
        player.set_rank_level(3);
        assert_eq!(player.get_rank_level(), 3);

        // Test cash bounty
        player.set_cash_bounty_percent(0.25);
        assert!((player.get_cash_bounty_percent() - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn test_science_system() {
        let mut player = Player::new(0);

        // Grant science
        player.grant_science(1);
        assert!(player.has_science(1));
        assert!(!player.is_science_disabled(1));

        // Disable science
        player.disable_science(1);
        assert!(!player.has_science(1));
        assert!(player.is_science_disabled(1));

        // Hide science
        player.hide_science(2);
        assert!(player.is_science_hidden(2));

        // Invalid science should be ignored
        player.grant_science(SCIENCE_INVALID);
        assert!(!player.has_science(SCIENCE_INVALID));
    }

    // =========================================================
    // New Tests for AI, Build List, Squads, Upgrades
    // =========================================================

    #[test]
    fn test_build_list_management() {
        let mut player = Player::new(0);

        // Initially no build list
        assert!(player.get_build_list().is_none());

        // Add to build list
        let location = Coord3D::new(100.0, 200.0, 0.0);
        player.add_to_build_list(1, "AmericaCommandCenter".to_string(), location, 0.5);

        // Verify build list exists
        assert!(player.get_build_list().is_some());
        let build_info = player.get_build_list().unwrap();
        assert_eq!(build_info.get_template_name(), "AmericaCommandCenter");
        assert_eq!(build_info.get_object_id(), 1);
        assert!(!build_info.is_priority_build());

        // Add priority build
        let location2 = Coord3D::new(150.0, 250.0, 0.0);
        player.add_to_priority_build_list("AmericaPowerPlant".to_string(), location2, 0.0);

        let build_info2 = player.get_build_list().unwrap();
        assert_eq!(build_info2.get_template_name(), "AmericaPowerPlant");
        assert!(build_info2.is_priority_build());

        // Clear build list
        player.set_build_list(None);
        assert!(player.get_build_list().is_none());
    }

    #[test]
    fn test_resource_gathering_manager() {
        let mut player = Player::new(0);

        // Initially no supply infrastructure
        assert!(player.get_supply_centers().is_empty());
        assert!(player.get_supply_warehouses().is_empty());

        // Add supply centers
        player.add_supply_center(1);
        player.add_supply_center(2);
        player.add_supply_center(1); // Duplicate - should not be added
        assert_eq!(player.get_supply_centers().len(), 2);

        // Add supply warehouses
        player.add_supply_warehouse(10);
        player.add_supply_warehouse(11);
        assert_eq!(player.get_supply_warehouses().len(), 2);

        // Remove supply center
        player.remove_supply_center(1);
        assert_eq!(player.get_supply_centers().len(), 1);
        assert_eq!(player.get_supply_centers()[0], 2);

        // Find best supply warehouse (simplified - returns first)
        let best = player.find_best_supply_warehouse(99);
        assert!(best.is_some());
        assert_eq!(best.unwrap(), 10);
    }

    #[test]
    fn test_hotkey_squads() {
        let mut player = Player::new(0);

        // All squads start empty
        for i in 0..NUM_HOTKEY_SQUADS {
            assert!(player.get_hotkey_squad_const(i as i32).unwrap().is_empty());
        }

        // Add objects to squad 0
        {
            let squad = player.get_hotkey_squad(0).unwrap();
            squad.add_object(1);
            squad.add_object(2);
            squad.add_object(3);
        }

        assert_eq!(player.get_hotkey_squad_const(0).unwrap().len(), 3);
        assert!(player.get_hotkey_squad_const(0).unwrap().contains(2));

        // Check squad number for object
        assert_eq!(player.get_squad_number_for_object(2), 0);
        assert_eq!(player.get_squad_number_for_object(99), NO_HOTKEY_SQUAD);

        // Remove object from all squads
        player.remove_object_from_hotkey_squad(2);
        assert_eq!(player.get_hotkey_squad_const(0).unwrap().len(), 2);
        assert!(!player.get_hotkey_squad_const(0).unwrap().contains(2));

        // Clear specific squad
        player.clear_hotkey_squad(0);
        assert!(player.get_hotkey_squad_const(0).unwrap().is_empty());

        // Invalid squad number returns None
        assert!(player.get_hotkey_squad(-1).is_none());
        assert!(player.get_hotkey_squad(NUM_HOTKEY_SQUADS as i32).is_none());
    }

    #[test]
    fn test_current_selection() {
        let mut player = Player::new(0);

        // Initially empty
        assert!(player.get_current_selection().is_empty());
        assert_eq!(player.get_current_selection_size(), 0);

        // Add to selection
        player.add_to_current_selection(1);
        player.add_to_current_selection(2);
        player.add_to_current_selection(1); // Duplicate - should not be added twice
        assert_eq!(player.get_current_selection_size(), 2);
        assert!(player.is_in_current_selection(1));
        assert!(player.is_in_current_selection(2));
        assert!(!player.is_in_current_selection(3));

        // Remove from selection
        player.remove_from_current_selection(1);
        assert_eq!(player.get_current_selection_size(), 1);
        assert!(!player.is_in_current_selection(1));

        // Clear selection
        player.add_to_current_selection(5);
        player.add_to_current_selection(6);
        player.clear_current_selection();
        assert!(player.get_current_selection().is_empty());
    }

    #[test]
    fn test_upgrade_system() {
        let mut player = Player::new(0);

        // Initially no upgrades
        assert!(!player.has_upgrade_complete("Upgrade1"));
        assert!(!player.has_upgrade_in_production("Upgrade1"));

        // Add upgrade in production
        player.add_upgrade("Upgrade1".to_string(), UpgradeStatus::InProduction);
        assert!(player.has_upgrade_in_production("Upgrade1"));
        assert!(!player.has_upgrade_complete("Upgrade1"));

        // Mark upgrade as complete
        if let Some(upgrade) = player.find_upgrade_mut("Upgrade1") {
            upgrade.set_status(UpgradeStatus::Complete);
        }
        assert!(player.has_upgrade_complete("Upgrade1"));
        assert!(!player.has_upgrade_in_production("Upgrade1"));

        // Add another upgrade
        player.add_upgrade("Upgrade2".to_string(), UpgradeStatus::Complete);
        assert!(player.has_upgrade_complete("Upgrade2"));

        // Remove upgrade
        player.remove_upgrade("Upgrade1");
        assert!(!player.has_upgrade_complete("Upgrade1"));
    }

    #[test]
    fn test_upgrade_bitmask() {
        let mut player = Player::new(0);

        // Initially no bits set
        assert_eq!(player.get_completed_upgrade_mask(), 0);

        // Set upgrade bits
        player.set_upgrade_completed(0);
        assert_eq!(player.get_completed_upgrade_mask(), 0b1);

        player.set_upgrade_completed(3);
        assert_eq!(player.get_completed_upgrade_mask(), 0b1001);

        // Clear upgrade bit
        player.clear_upgrade_completed(0);
        assert_eq!(player.get_completed_upgrade_mask(), 0b1000);

        // Set in-progress bit
        player.set_upgrade_in_progress(5);
        player.set_upgrade_completed(5); // Should also clear in-progress
        assert_eq!(player.get_completed_upgrade_mask(), 0b11000);
    }

    #[test]
    fn test_team_prototypes() {
        let mut player = Player::new(0);

        // Initially empty
        assert!(player.get_team_prototypes().is_empty());

        // Add team prototypes
        player.add_team_prototype("teamPlayer0".to_string());
        player.add_team_prototype("teamPlayer0attack".to_string());
        player.add_team_prototype("teamPlayer0".to_string()); // Duplicate
        assert_eq!(player.get_team_prototypes().len(), 2);

        // Remove team prototype
        player.remove_team_prototype("teamPlayer0");
        assert_eq!(player.get_team_prototypes().len(), 1);
    }

    #[test]
    fn test_tunnel_system() {
        let mut player = Player::new(0);

        // Initially empty
        assert!(player.get_tunnel_entrances().is_empty());

        // Add tunnel entrances
        player.add_tunnel_entrance(1);
        player.add_tunnel_entrance(2);
        assert_eq!(player.get_tunnel_entrances().len(), 2);

        // Remove tunnel entrance
        player.remove_tunnel_entrance(1);
        assert_eq!(player.get_tunnel_entrances().len(), 1);
    }

    #[test]
    fn test_production_changes() {
        let mut player = Player::new(0);

        // Default cost is 1.0 (100%)
        assert!((player.get_production_cost_change("SomeUnit") - 1.0).abs() < f32::EPSILON);

        // Set production cost change (90% = 0.9)
        player.set_production_cost_change("SomeUnit".to_string(), 0.9);
        assert!((player.get_production_cost_change("SomeUnit") - 0.9).abs() < f32::EPSILON);

        // Set production time change (80% = 0.8)
        player.set_production_time_change("SomeUnit".to_string(), 0.8);
        assert!((player.get_production_time_change("SomeUnit") - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_special_power_timers() {
        let mut player = Player::new(0);

        // Initially no timer
        assert!(player.get_special_power_ready_frame(1).is_none());

        // Set timer
        player.set_special_power_ready_frame(1, 1000);
        assert_eq!(player.get_special_power_ready_frame(1), Some(1000));

        // Update timer
        player.set_special_power_ready_frame(1, 2000);
        assert_eq!(player.get_special_power_ready_frame(1), Some(2000));

        // Remove timer
        player.remove_special_power_timer(1);
        assert!(player.get_special_power_ready_frame(1).is_none());
    }

    #[test]
    fn test_difficulty_setting() {
        let mut player = Player::new(0);

        // Default difficulty is Normal
        assert_eq!(player.get_player_difficulty(), GameDifficulty::Normal);

        // Change difficulty
        player.set_player_difficulty(GameDifficulty::Hard);
        assert_eq!(player.get_player_difficulty(), GameDifficulty::Hard);

        player.set_player_difficulty(GameDifficulty::Easy);
        assert_eq!(player.get_player_difficulty(), GameDifficulty::Easy);

        // No AI by default
        assert!(!player.has_ai());
        assert!(!player.is_skirmish_ai_player());
    }
}
