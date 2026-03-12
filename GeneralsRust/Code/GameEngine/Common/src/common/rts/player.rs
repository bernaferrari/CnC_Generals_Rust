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

use crate::common::rts::{
    AcademyStats, Energy, Handicap, MissionStats, Money, PlayerHandle, Relationship, ScienceType,
    ScoreKeeper, Team, TeamID, TeamPrototype, SCIENCE_INVALID,
};
use crate::common::system::{Snapshotable, Xfer, XferMode, XferVersion};
use std::collections::{HashMap, HashSet};

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

        let mut player = Self {
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
        };

        // C++ lines 236-239: Initialize components with player handle
        let handle = PlayerHandle::new(index.max(0) as u32);
        player.energy.init(handle);
        player.academy_stats.init(handle);
        player.score_keeper.reset(index);

        // C++ line 235: Call init(NULL)
        player.init(None);

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
        self.bombard_battle_plans + self.hold_the_line_battle_plans + self.search_and_destroy_battle_plans
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

    /// Check if player is active (not dead and not observer)
    /// C++ Reference: Player::isPlayerActive() (Player.h line 409)
    pub fn is_player_active(&self) -> bool {
        !self.observer && !self.is_player_dead
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
        if points >= 5000 { 8 }
        else if points >= 4000 { 7 }
        else if points >= 3000 { 6 }
        else if points >= 2000 { 5 }
        else if points >= 1000 { 4 }
        else if points >= 500 { 3 }
        else if points >= 100 { 2 }
        else { 1 }
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
        self.science_purchase_points += delta;
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
            // For now, we store sciences as a simple count + list of science types
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

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // No post-processing needed for basic player data
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
}
