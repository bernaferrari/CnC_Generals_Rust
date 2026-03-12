//! Score Keeper System
//!
//! Maintains accurate counts for various statistics shown on the score screen.
//! This information is also used for the observer screen and post-game analysis.
//!
//! Based on C++ implementation: /GeneralsMD/Code/GameEngine/Source/Common/RTS/ScoreKeeper.cpp

use crate::common::thing::thing_factory::get_thing_factory;
use std::collections::HashMap;

/// Forward declarations
pub struct Object;
pub struct ThingTemplate;

/// Maximum number of players
/// Matches C++ ScoreKeeper.h line 89
pub const MAX_PLAYER_COUNT: usize = 8;

/// KindOf bit flags for object classification
/// Matches C++ KindOf.h enum definitions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KindOfMaskType {
    bits: u64,
}

impl KindOfMaskType {
    pub fn new() -> Self {
        Self { bits: 0 }
    }

    pub fn set(&mut self, kind: KindOf) {
        self.bits |= 1u64 << (kind as u32);
    }

    pub fn is_set(&self, kind: KindOf) -> bool {
        (self.bits & (1u64 << (kind as u32))) != 0
    }

    pub fn matches(&self, other: &KindOfMaskType) -> bool {
        (self.bits & other.bits) == other.bits
    }

    pub fn matches_multi(
        &self,
        valid_mask: &KindOfMaskType,
        invalid_mask: &KindOfMaskType,
    ) -> bool {
        // Must have all bits from valid_mask AND none from invalid_mask
        if invalid_mask.bits != 0 && (self.bits & invalid_mask.bits) != 0 {
            return false;
        }
        if valid_mask.bits != 0 {
            (self.bits & valid_mask.bits) == valid_mask.bits
        } else {
            true
        }
    }
}

impl Default for KindOfMaskType {
    fn default() -> Self {
        Self::new()
    }
}

/// KindOf enumeration for object types
/// Matches C++ KindOf.h definitions
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindOf {
    Structure = 0,
    Score = 1,
    ScoreCreate = 2,
    ScoreDestroy = 3,
    Infantry = 4,
    Vehicle = 5,
}

/// Represents KINDOFMASK_NONE from C++
pub const KINDOFMASK_NONE: KindOfMaskType = KindOfMaskType { bits: 0 };

/// Score keeper for tracking player statistics
/// Matches C++ ScoreKeeper.h lines 44-109
#[derive(Debug, Clone)]
pub struct ScoreKeeper {
    /// Player index this score keeper belongs to
    /// Matches C++ m_myPlayerIdx (line 99)
    player_index: i32,

    /// Current calculated score
    /// Matches C++ m_currentScore (line 97)
    current_score: i32,

    // Financial statistics
    /// Total money harvested, refined, received in crates
    /// Matches C++ m_totalMoneyEarned (line 87)
    total_money_earned: i32,

    /// Total money spent on units, buildings, repairs
    /// Matches C++ m_totalMoneySpent (line 88)
    total_money_spent: i32,

    // Unit statistics
    /// Total units created (built from buildings)
    /// Matches C++ m_totalUnitsBuilt (line 90)
    total_units_built: i32,

    /// Total units lost
    /// Matches C++ m_totalUnitsLost (line 91)
    total_units_lost: i32,

    /// Total enemy units destroyed (per player)
    /// Matches C++ m_totalUnitsDestroyed (line 89)
    total_units_destroyed: [i32; MAX_PLAYER_COUNT],

    // Building statistics
    /// Total buildings constructed
    /// Matches C++ m_totalBuildingsBuilt (line 93)
    total_buildings_built: i32,

    /// Total buildings lost
    /// Matches C++ m_totalBuildingsLost (line 94)
    total_buildings_lost: i32,

    /// Total enemy buildings destroyed (per player)
    /// Matches C++ m_totalBuildingsDestroyed (line 92)
    total_buildings_destroyed: [i32; MAX_PLAYER_COUNT],

    // Special building capture statistics
    /// Tech buildings captured
    /// Matches C++ m_totalTechBuildingsCaptured (line 95)
    total_tech_buildings_captured: i32,

    /// Faction buildings captured
    /// Matches C++ m_totalFactionBuildingsCaptured (line 96)
    total_faction_buildings_captured: i32,

    // Detailed object tracking
    /// Objects built (by template name and count)
    /// Matches C++ m_objectsBuilt (line 103)
    objects_built: HashMap<String, i32>,

    /// Objects lost (by template name and count)
    /// Matches C++ m_objectsLost (line 105)
    objects_lost: HashMap<String, i32>,

    /// Objects captured (by template name and count)
    /// Matches C++ m_objectsCaptured (line 106)
    objects_captured: HashMap<String, i32>,

    /// Objects destroyed (by player, template name, and count)
    /// Matches C++ m_objectsDestroyed (line 104)
    objects_destroyed: [HashMap<String, i32>; MAX_PLAYER_COUNT],

    // Scoring masks (static in C++, instance members here for thread safety)
    /// Mask for buildings that count toward score
    /// Matches C++ scoringBuildingMask (line 63)
    scoring_building_mask: KindOfMaskType,

    /// Mask for buildings that count when created
    /// Matches C++ scoringBuildingCreateMask (line 65)
    scoring_building_create_mask: KindOfMaskType,

    /// Mask for buildings that count when destroyed
    /// Matches C++ scoringBuildingDestroyMask (line 64)
    scoring_building_destroy_mask: KindOfMaskType,

    /// Flag to enable/disable scoring (for performance in non-scoring modes)
    scoring_enabled: bool,
}

impl ScoreKeeper {
    /// Create a new ScoreKeeper
    /// Matches C++ ScoreKeeper.cpp lines 53-56
    pub fn new() -> Self {
        let mut keeper = Self {
            player_index: 0,
            current_score: 0,
            total_money_earned: 0,
            total_money_spent: 0,
            total_units_built: 0,
            total_units_lost: 0,
            total_units_destroyed: [0; MAX_PLAYER_COUNT],
            total_buildings_built: 0,
            total_buildings_lost: 0,
            total_buildings_destroyed: [0; MAX_PLAYER_COUNT],
            total_tech_buildings_captured: 0,
            total_faction_buildings_captured: 0,
            objects_built: HashMap::new(),
            objects_lost: HashMap::new(),
            objects_captured: HashMap::new(),
            objects_destroyed: [(); MAX_PLAYER_COUNT].map(|_| HashMap::new()),
            scoring_building_mask: KindOfMaskType::new(),
            scoring_building_create_mask: KindOfMaskType::new(),
            scoring_building_destroy_mask: KindOfMaskType::new(),
            scoring_enabled: true,
        };
        keeper.reset(0);
        keeper
    }

    /// Reset all statistics
    /// Matches C++ ScoreKeeper.cpp lines 67-95
    pub fn reset(&mut self, player_index: i32) {
        // Initialize scoring masks (C++ lines 69-76)
        self.scoring_building_mask.set(KindOf::Structure);
        self.scoring_building_mask.set(KindOf::Score);

        self.scoring_building_create_mask.set(KindOf::Structure);
        self.scoring_building_create_mask.set(KindOf::ScoreCreate);

        self.scoring_building_destroy_mask.set(KindOf::Structure);
        self.scoring_building_destroy_mask.set(KindOf::ScoreDestroy);

        // Reset counters (C++ lines 78-84)
        self.total_money_earned = 0;
        self.total_money_spent = 0;
        self.total_units_lost = 0;
        self.total_units_built = 0;
        self.total_buildings_lost = 0;
        self.total_buildings_built = 0;
        self.total_faction_buildings_captured = 0;
        self.total_tech_buildings_captured = 0;

        self.current_score = 0;

        // Clear maps (C++ lines 86-88)
        self.objects_built.clear();
        self.objects_captured.clear();
        self.objects_lost.clear();

        // Clear per-player tracking (C++ lines 89-93)
        self.total_units_destroyed.fill(0);
        self.total_buildings_destroyed.fill(0);

        for destroyed_map in &mut self.objects_destroyed {
            destroyed_map.clear();
        }

        // Store player index (C++ line 94)
        self.player_index = player_index;
    }

    /// Enable or disable scoring
    /// When disabled, all tracking methods return early for performance
    pub fn set_scoring_enabled(&mut self, enabled: bool) {
        self.scoring_enabled = enabled;
    }

    /// Check if scoring is currently enabled
    pub fn is_scoring_enabled(&self) -> bool {
        self.scoring_enabled
    }

    // Recording methods

    /// Add an object to the built count
    /// Matches C++ ScoreKeeper.cpp lines 97-132
    ///
    /// # Arguments
    /// * `template_name` - Name of the object template
    /// * `kind_of_mask` - KindOf flags for this object
    /// * `under_construction` - Whether object is still under construction (not used for built, included for API consistency)
    pub fn add_object_built(
        &mut self,
        template_name: &str,
        kind_of_mask: &KindOfMaskType,
        _under_construction: bool,
    ) {
        // C++ line 101: Early return if scoring disabled
        if !self.scoring_enabled {
            return;
        }

        let mut add_to_count = false;

        // C++ lines 105-114: Check if this is a scoring building
        if kind_of_mask.matches_multi(&self.scoring_building_mask, &KINDOFMASK_NONE) {
            self.total_buildings_built += 1;
            add_to_count = true;
        } else if kind_of_mask.matches_multi(&self.scoring_building_create_mask, &KINDOFMASK_NONE) {
            self.total_buildings_built += 1;
            add_to_count = true;
        }
        // C++ lines 115-122: Check if this is a scoring unit (infantry or vehicle)
        else if kind_of_mask.is_set(KindOf::Infantry) || kind_of_mask.is_set(KindOf::Vehicle) {
            if kind_of_mask.is_set(KindOf::Score) || kind_of_mask.is_set(KindOf::ScoreCreate) {
                self.total_units_built += 1;
                add_to_count = true;
            }
        }

        // C++ lines 124-131: Update detailed count map
        if add_to_count {
            *self
                .objects_built
                .entry(template_name.to_string())
                .or_insert(0) += 1;
        }
    }

    /// Remove an object from the built count (for cancelled/destroyed during construction)
    /// Matches C++ ScoreKeeper.cpp lines 160-194
    ///
    /// # Arguments
    /// * `template_name` - Name of the object template
    /// * `kind_of_mask` - KindOf flags for this object
    pub fn remove_object_built(&mut self, template_name: &str, kind_of_mask: &KindOfMaskType) {
        // C++ line 162: Early return if scoring disabled
        if !self.scoring_enabled {
            return;
        }

        let mut remove_from_count = false;

        // C++ lines 167-176: Check if this is a scoring building
        if kind_of_mask.matches_multi(&self.scoring_building_mask, &KINDOFMASK_NONE) {
            self.total_buildings_built -= 1;
            remove_from_count = true;
        } else if kind_of_mask.matches_multi(&self.scoring_building_create_mask, &KINDOFMASK_NONE) {
            self.total_buildings_built -= 1;
            remove_from_count = true;
        }
        // C++ lines 177-184: Check if this is a scoring unit
        else if kind_of_mask.is_set(KindOf::Infantry) || kind_of_mask.is_set(KindOf::Vehicle) {
            if kind_of_mask.is_set(KindOf::Score) || kind_of_mask.is_set(KindOf::ScoreCreate) {
                self.total_units_built -= 1;
                remove_from_count = true;
            }
        }

        // C++ lines 186-193: Update detailed count map
        if remove_from_count {
            if let Some(count) = self.objects_built.get_mut(template_name) {
                *count -= 1;
            }
        }
    }

    /// Add a destroyed object to this player's kill count
    /// Matches C++ ScoreKeeper.cpp lines 228-271
    ///
    /// # Arguments
    /// * `template_name` - Name of the object template
    /// * `kind_of_mask` - KindOf flags for this object
    /// * `owner_player_index` - Index of the player who owned this object
    /// * `under_construction` - Whether the object was under construction (doesn't count if true)
    pub fn add_object_destroyed(
        &mut self,
        template_name: &str,
        kind_of_mask: &KindOfMaskType,
        owner_player_index: usize,
        under_construction: bool,
    ) {
        // C++ line 231: Early return if scoring disabled
        if !self.scoring_enabled {
            return;
        }

        if owner_player_index >= MAX_PLAYER_COUNT {
            return;
        }

        let mut add_to_count = false;

        // C++ lines 238-250: Check if this is a scoring building
        if kind_of_mask.matches_multi(&self.scoring_building_mask, &KINDOFMASK_NONE) {
            // C++ line 240: Don't count buildings under construction
            if !under_construction {
                self.total_buildings_destroyed[owner_player_index] += 1;
                add_to_count = true;
            }
        } else if kind_of_mask.matches_multi(&self.scoring_building_destroy_mask, &KINDOFMASK_NONE)
        {
            // C++ line 247: Don't count buildings under construction
            if !under_construction {
                self.total_buildings_destroyed[owner_player_index] += 1;
                add_to_count = true;
            }
        }
        // C++ lines 252-261: Check if this is a scoring unit
        else if kind_of_mask.is_set(KindOf::Infantry) || kind_of_mask.is_set(KindOf::Vehicle) {
            if kind_of_mask.is_set(KindOf::Score) || kind_of_mask.is_set(KindOf::ScoreDestroy) {
                // C++ line 256: Don't count units under construction
                if !under_construction {
                    self.total_units_destroyed[owner_player_index] += 1;
                    add_to_count = true;
                }
            }
        }

        // C++ lines 263-270: Update detailed count map
        if add_to_count {
            *self.objects_destroyed[owner_player_index]
                .entry(template_name.to_string())
                .or_insert(0) += 1;
        }
    }

    /// Add an object to the lost count (this player's losses)
    /// Matches C++ ScoreKeeper.cpp lines 273-313
    ///
    /// # Arguments
    /// * `template_name` - Name of the object template
    /// * `kind_of_mask` - KindOf flags for this object
    /// * `under_construction` - Whether the object was under construction (doesn't count if true)
    pub fn add_object_lost(
        &mut self,
        template_name: &str,
        kind_of_mask: &KindOfMaskType,
        under_construction: bool,
    ) {
        // C++ line 275: Early return if scoring disabled
        if !self.scoring_enabled {
            return;
        }

        let mut add_to_count = false;

        // C++ lines 280-292: Check if this is a scoring building
        if kind_of_mask.matches_multi(&self.scoring_building_mask, &KINDOFMASK_NONE) {
            // C++ line 282: Don't count buildings under construction
            if !under_construction {
                self.total_buildings_lost += 1;
                add_to_count = true;
            }
        } else if kind_of_mask.matches_multi(&self.scoring_building_destroy_mask, &KINDOFMASK_NONE)
        {
            // C++ line 289: Don't count buildings under construction
            if !under_construction {
                self.total_buildings_lost += 1;
                add_to_count = true;
            }
        }
        // C++ lines 294-303: Check if this is a scoring unit
        else if kind_of_mask.is_set(KindOf::Infantry) || kind_of_mask.is_set(KindOf::Vehicle) {
            if kind_of_mask.is_set(KindOf::Score) || kind_of_mask.is_set(KindOf::ScoreDestroy) {
                // C++ line 298: Don't count units under construction
                if !under_construction {
                    self.total_units_lost += 1;
                    add_to_count = true;
                }
            }
        }

        // C++ lines 305-312: Update detailed count map
        if add_to_count {
            *self
                .objects_lost
                .entry(template_name.to_string())
                .or_insert(0) += 1;
        }
    }

    /// Add a captured building
    /// Matches C++ ScoreKeeper.cpp lines 196-224
    ///
    /// # Arguments
    /// * `template_name` - Name of the building template
    /// * `kind_of_mask` - KindOf flags for this building
    pub fn add_object_captured(&mut self, template_name: &str, kind_of_mask: &KindOfMaskType) {
        // C++ line 198: Early return if scoring disabled
        if !self.scoring_enabled {
            return;
        }

        let mut add_to_count = false;

        // C++ lines 203-214: Only structures can be captured
        if kind_of_mask.is_set(KindOf::Structure) {
            if kind_of_mask.is_set(KindOf::Score) {
                // C++ line 207: Faction building (has SCORE flag)
                self.total_faction_buildings_captured += 1;
            } else {
                // C++ line 211: Tech building (structure but no SCORE flag)
                self.total_tech_buildings_captured += 1;
            }
            add_to_count = true;
        }

        // C++ lines 216-223: Update detailed count map
        if add_to_count {
            *self
                .objects_captured
                .entry(template_name.to_string())
                .or_insert(0) += 1;
        }
    }

    /// Add money to earned total
    /// Matches C++ ScoreKeeper.h line 115 (inline implementation)
    pub fn add_money_earned(&mut self, amount: i32) {
        self.total_money_earned += amount;
    }

    /// Add money to spent total
    /// Matches C++ ScoreKeeper.h line 114 (inline implementation)
    pub fn add_money_spent(&mut self, amount: i32) {
        self.total_money_spent += amount;
    }

    // Query methods

    /// Get current score
    /// Matches C++ ScoreKeeper.h line 63 (via calculateScore)
    pub fn get_current_score(&self) -> i32 {
        self.current_score
    }

    /// Get total units built (simple count)
    /// Matches C++ ScoreKeeper.h line 66
    pub fn get_total_units_built(&self) -> i32 {
        self.total_units_built
    }

    /// Get total units built matching specified masks
    /// Matches C++ ScoreKeeper.cpp lines 134-145
    /// Used for battle honor calculation
    ///
    /// # Arguments
    /// * `valid_mask` - Objects must have ALL these KindOf bits set
    /// * `invalid_mask` - Objects must have NONE of these KindOf bits set
    pub fn get_total_units_built_filtered(
        &self,
        valid_mask: &KindOfMaskType,
        invalid_mask: &KindOfMaskType,
    ) -> i32 {
        if let Ok(factory_guard) = get_thing_factory() {
            if let Some(factory) = factory_guard.as_ref() {
                let mut total = 0;
                for (template_name, count) in &self.objects_built {
                    if let Some(template) = factory.find_template(template_name, false) {
                        let mask = KindOfMaskType {
                            bits: template.get_kindof_mask(),
                        };
                        if mask.matches_multi(valid_mask, invalid_mask) {
                            total += *count;
                        }
                    }
                }
                return total;
            }
        }

        self.total_units_built
    }

    /// Get total objects built matching a specific template
    /// Matches C++ ScoreKeeper.cpp lines 147-157
    ///
    /// # Arguments
    /// * `template_name` - Name of the template to match
    ///
    /// Note: C++ uses isEquivalentTo() which allows for template variants
    /// We use exact string matching for now
    pub fn get_total_objects_built(&self, template_name: &str) -> i32 {
        self.objects_built.get(template_name).copied().unwrap_or(0)
    }

    /// Get total units lost
    /// Matches C++ ScoreKeeper.h line 67
    pub fn get_total_units_lost(&self) -> i32 {
        self.total_units_lost
    }

    /// Get total enemy units destroyed (all players combined)
    /// Matches C++ ScoreKeeper.cpp lines 356-370
    pub fn get_total_units_destroyed(&self) -> i32 {
        // C++ lines 358-368: Sum across all players
        // C++ comment lines 361-362: Design change, include own units destroyed
        self.total_units_destroyed.iter().sum()
    }

    /// Get total buildings built
    /// Matches C++ ScoreKeeper.h line 69
    pub fn get_total_buildings_built(&self) -> i32 {
        self.total_buildings_built
    }

    /// Get total buildings lost
    /// Matches C++ ScoreKeeper.h line 70
    pub fn get_total_buildings_lost(&self) -> i32 {
        self.total_buildings_lost
    }

    /// Get total enemy buildings destroyed (all players combined)
    /// Matches C++ ScoreKeeper.cpp lines 338-355
    pub fn get_total_buildings_destroyed(&self) -> i32 {
        // C++ lines 340-353: Sum across all players
        // C++ comment lines 343-345: Design change, include own buildings destroyed
        self.total_buildings_destroyed.iter().sum()
    }

    /// Get total tech buildings captured
    /// Matches C++ ScoreKeeper.h line 71
    pub fn get_total_tech_buildings_captured(&self) -> i32 {
        self.total_tech_buildings_captured
    }

    /// Get total faction buildings captured
    /// Matches C++ ScoreKeeper.h line 72
    pub fn get_total_faction_buildings_captured(&self) -> i32 {
        self.total_faction_buildings_captured
    }

    /// Get total money earned
    /// Matches C++ ScoreKeeper.h line 63
    pub fn get_total_money_earned(&self) -> i32 {
        self.total_money_earned
    }

    /// Get total money spent
    /// Matches C++ ScoreKeeper.h line 64
    pub fn get_total_money_spent(&self) -> i32 {
        self.total_money_spent
    }

    /// Calculate the current score based on statistics
    /// Matches C++ ScoreKeeper.cpp lines 315-332
    ///
    /// Score formula (C++ lines 318-326):
    /// - Units built: +100 points each
    /// - Money earned: +1 point per dollar
    /// - Buildings built: +100 points each
    /// - Enemy units destroyed: +100 points each
    /// - Enemy buildings destroyed: +100 points each
    pub fn calculate_score(&mut self) -> i32 {
        let mut score = 0;

        // C++ line 318: Units built
        score += self.total_units_built * 100;

        // C++ line 319: Money earned
        score += self.total_money_earned;

        // C++ line 320: Buildings built
        score += self.total_buildings_built * 100;

        // C++ lines 321-327: Enemy units and buildings destroyed
        for i in 0..MAX_PLAYER_COUNT {
            // C++ lines 323-324: Skip own player index
            if i == self.player_index as usize {
                continue;
            }
            // C++ line 325: Enemy units destroyed
            score += self.total_units_destroyed[i] * 100;
            // C++ line 326: Enemy buildings destroyed
            score += self.total_buildings_destroyed[i] * 100;
        }

        // C++ line 329: Store calculated score
        self.current_score = score;

        // C++ line 330: Return score
        self.current_score
    }

    /// Get objects destroyed for a specific player
    /// Returns detailed map of what objects this player destroyed from the specified enemy
    pub fn get_objects_destroyed_for_player(
        &self,
        player_index: usize,
    ) -> Option<&HashMap<String, i32>> {
        if player_index < MAX_PLAYER_COUNT {
            Some(&self.objects_destroyed[player_index])
        } else {
            None
        }
    }

    /// Get objects lost map
    /// Returns detailed map of what objects this player lost
    pub fn get_objects_lost_map(&self) -> &HashMap<String, i32> {
        &self.objects_lost
    }

    /// Get objects captured map
    /// Returns detailed map of what buildings this player captured
    pub fn get_objects_captured_map(&self) -> &HashMap<String, i32> {
        &self.objects_captured
    }

    /// Get objects built map
    /// Returns detailed map of what objects this player built
    pub fn get_objects_built_map(&self) -> &HashMap<String, i32> {
        &self.objects_built
    }
}

impl Default for ScoreKeeper {
    fn default() -> Self {
        Self::new()
    }
}

impl ScoreKeeper {
    /// Serialize for save games
    /// Matches C++ ScoreKeeper.cpp xfer() implementation (lines 465-536, version 1)
    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // Version
        data.extend_from_slice(&1u32.to_le_bytes());

        // Money earned and spent
        data.extend_from_slice(&self.total_money_earned.to_le_bytes());
        data.extend_from_slice(&self.total_money_spent.to_le_bytes());

        // Units destroyed array
        for i in 0..MAX_PLAYER_COUNT {
            data.extend_from_slice(&self.total_units_destroyed[i].to_le_bytes());
        }

        // Units built and lost
        data.extend_from_slice(&self.total_units_built.to_le_bytes());
        data.extend_from_slice(&self.total_units_lost.to_le_bytes());

        // Buildings destroyed array
        for i in 0..MAX_PLAYER_COUNT {
            data.extend_from_slice(&self.total_buildings_destroyed[i].to_le_bytes());
        }

        // Buildings built and lost
        data.extend_from_slice(&self.total_buildings_built.to_le_bytes());
        data.extend_from_slice(&self.total_buildings_lost.to_le_bytes());

        // Tech and faction buildings captured
        data.extend_from_slice(&self.total_tech_buildings_captured.to_le_bytes());
        data.extend_from_slice(&self.total_faction_buildings_captured.to_le_bytes());

        // Current score
        data.extend_from_slice(&self.current_score.to_le_bytes());

        // Player index
        data.extend_from_slice(&self.player_index.to_le_bytes());

        // Objects built map
        Self::serialize_map(&mut data, &self.objects_built);

        // Objects destroyed array of maps
        data.extend_from_slice(&(MAX_PLAYER_COUNT as u16).to_le_bytes());
        for i in 0..MAX_PLAYER_COUNT {
            Self::serialize_map(&mut data, &self.objects_destroyed[i]);
        }

        // Objects lost map
        Self::serialize_map(&mut data, &self.objects_lost);

        // Objects captured map
        Self::serialize_map(&mut data, &self.objects_captured);

        data
    }

    fn serialize_map(data: &mut Vec<u8>, map: &HashMap<String, i32>) {
        // Map size
        data.extend_from_slice(&(map.len() as u16).to_le_bytes());

        // Map entries
        for (name, count) in map {
            // String length and data
            let name_bytes = name.as_bytes();
            data.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            data.extend_from_slice(name_bytes);

            // Count
            data.extend_from_slice(&count.to_le_bytes());
        }
    }

    /// Deserialize from save game data
    /// Matches C++ ScoreKeeper.cpp xfer() implementation
    pub fn deserialize(data: &[u8]) -> Result<Self, &'static str> {
        let mut offset = 0;

        // Version
        if offset + 4 > data.len() {
            return Err("Truncated data");
        }
        let version = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if version != 1 {
            return Err("Unsupported version");
        }
        offset += 4;

        let mut keeper = Self::new();

        // Money earned and spent
        if offset + 8 > data.len() {
            return Err("Truncated data");
        }
        keeper.total_money_earned = i32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;
        keeper.total_money_spent = i32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        // Units destroyed array
        for i in 0..MAX_PLAYER_COUNT {
            if offset + 4 > data.len() {
                return Err("Truncated data");
            }
            keeper.total_units_destroyed[i] = i32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;
        }

        // Units built and lost
        if offset + 8 > data.len() {
            return Err("Truncated data");
        }
        keeper.total_units_built = i32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;
        keeper.total_units_lost = i32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        // Buildings destroyed array
        for i in 0..MAX_PLAYER_COUNT {
            if offset + 4 > data.len() {
                return Err("Truncated data");
            }
            keeper.total_buildings_destroyed[i] = i32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            offset += 4;
        }

        // Buildings built and lost
        if offset + 8 > data.len() {
            return Err("Truncated data");
        }
        keeper.total_buildings_built = i32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;
        keeper.total_buildings_lost = i32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        // Tech and faction buildings captured
        if offset + 8 > data.len() {
            return Err("Truncated data");
        }
        keeper.total_tech_buildings_captured = i32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;
        keeper.total_faction_buildings_captured = i32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        // Current score
        if offset + 4 > data.len() {
            return Err("Truncated data");
        }
        keeper.current_score = i32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        // Player index
        if offset + 4 > data.len() {
            return Err("Truncated data");
        }
        keeper.player_index = i32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        // Objects built map
        keeper.objects_built = Self::deserialize_map(data, &mut offset)?;

        // Objects destroyed array of maps
        if offset + 2 > data.len() {
            return Err("Truncated data");
        }
        let array_size = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
        offset += 2;
        if array_size != MAX_PLAYER_COUNT {
            return Err("Objects destroyed array size mismatch");
        }

        for i in 0..MAX_PLAYER_COUNT {
            keeper.objects_destroyed[i] = Self::deserialize_map(data, &mut offset)?;
        }

        // Objects lost map
        keeper.objects_lost = Self::deserialize_map(data, &mut offset)?;

        // Objects captured map
        keeper.objects_captured = Self::deserialize_map(data, &mut offset)?;

        Ok(keeper)
    }

    fn deserialize_map(
        data: &[u8],
        offset: &mut usize,
    ) -> Result<HashMap<String, i32>, &'static str> {
        if *offset + 2 > data.len() {
            return Err("Truncated map data");
        }

        let map_size = u16::from_le_bytes([data[*offset], data[*offset + 1]]) as usize;
        *offset += 2;

        let mut map = HashMap::new();

        for _ in 0..map_size {
            // String length
            if *offset + 2 > data.len() {
                return Err("Truncated map entry");
            }
            let str_len = u16::from_le_bytes([data[*offset], data[*offset + 1]]) as usize;
            *offset += 2;

            // String data
            if *offset + str_len > data.len() {
                return Err("Truncated string data");
            }
            let name = String::from_utf8(data[*offset..*offset + str_len].to_vec())
                .map_err(|_| "Invalid UTF-8")?;
            *offset += str_len;

            // Count
            if *offset + 4 > data.len() {
                return Err("Truncated count data");
            }
            let count = i32::from_le_bytes([
                data[*offset],
                data[*offset + 1],
                data[*offset + 2],
                data[*offset + 3],
            ]);
            *offset += 4;

            map.insert(name, count);
        }

        Ok(map)
    }

    /// Calculate CRC for network synchronization
    /// Matches C++ ScoreKeeper.cpp crc() pattern (currently empty in C++, but we add basic CRC)
    pub fn calculate_crc(&self) -> u32 {
        let mut crc = 0u32;

        crc = crc.wrapping_add(self.total_money_earned as u32);
        crc = crc.wrapping_add(self.total_money_spent as u32);
        crc = crc.wrapping_add(self.total_units_built as u32);
        crc = crc.wrapping_add(self.total_units_lost as u32);
        crc = crc.wrapping_add(self.total_buildings_built as u32);
        crc = crc.wrapping_add(self.total_buildings_lost as u32);
        crc = crc.wrapping_add(self.current_score as u32);

        for &count in &self.total_units_destroyed {
            crc = crc.wrapping_add(count as u32);
        }

        for &count in &self.total_buildings_destroyed {
            crc = crc.wrapping_add(count as u32);
        }

        crc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a scoring unit mask (infantry with SCORE flag)
    fn create_scoring_unit_mask() -> KindOfMaskType {
        let mut mask = KindOfMaskType::new();
        mask.set(KindOf::Infantry);
        mask.set(KindOf::Score);
        mask
    }

    /// Helper to create a scoring building mask (structure with SCORE flag)
    fn create_scoring_building_mask() -> KindOfMaskType {
        let mut mask = KindOfMaskType::new();
        mask.set(KindOf::Structure);
        mask.set(KindOf::Score);
        mask
    }

    /// Helper to create a tech building mask (structure without SCORE flag)
    fn create_tech_building_mask() -> KindOfMaskType {
        let mut mask = KindOfMaskType::new();
        mask.set(KindOf::Structure);
        mask
    }

    #[test]
    fn test_score_keeper_creation() {
        // Matches C++ ScoreKeeper.cpp lines 53-56
        let keeper = ScoreKeeper::new();

        assert_eq!(keeper.get_current_score(), 0);
        assert_eq!(keeper.get_total_units_built(), 0);
        assert_eq!(keeper.get_total_money_earned(), 0);
        assert!(keeper.is_scoring_enabled());
    }

    #[test]
    fn test_reset() {
        // Matches C++ ScoreKeeper.cpp lines 67-95
        let mut keeper = ScoreKeeper::new();

        // Add some data
        let unit_mask = create_scoring_unit_mask();
        keeper.add_object_built("Tank", &unit_mask, false);
        keeper.add_money_earned(1000);

        // Reset
        keeper.reset(5);

        assert_eq!(keeper.player_index, 5);
        assert_eq!(keeper.get_total_units_built(), 0);
        assert_eq!(keeper.get_total_money_earned(), 0);
    }

    #[test]
    fn test_object_built_with_masks() {
        // Matches C++ ScoreKeeper.cpp lines 97-132
        let mut keeper = ScoreKeeper::new();

        let unit_mask = create_scoring_unit_mask();
        let building_mask = create_scoring_building_mask();

        keeper.add_object_built("Tank", &unit_mask, false);
        keeper.add_object_built("Barracks", &building_mask, false);

        assert_eq!(keeper.get_total_units_built(), 1);
        assert_eq!(keeper.get_total_buildings_built(), 1);
        assert_eq!(keeper.get_total_objects_built("Tank"), 1);
        assert_eq!(keeper.get_total_objects_built("Barracks"), 1);
    }

    #[test]
    fn test_remove_object_built() {
        // Matches C++ ScoreKeeper.cpp lines 160-194
        let mut keeper = ScoreKeeper::new();

        let unit_mask = create_scoring_unit_mask();

        keeper.add_object_built("Tank", &unit_mask, false);
        keeper.add_object_built("Tank", &unit_mask, false);
        assert_eq!(keeper.get_total_units_built(), 2);

        keeper.remove_object_built("Tank", &unit_mask);
        assert_eq!(keeper.get_total_units_built(), 1);
    }

    #[test]
    fn test_object_destroyed_with_under_construction() {
        // Matches C++ ScoreKeeper.cpp lines 228-271 (specifically line 240, 256)
        let mut keeper = ScoreKeeper::new();

        let unit_mask = create_scoring_unit_mask();

        // Under construction units don't count
        keeper.add_object_destroyed("Tank", &unit_mask, 1, true);
        assert_eq!(keeper.get_total_units_destroyed(), 0);

        // Completed units do count
        keeper.add_object_destroyed("Tank", &unit_mask, 1, false);
        assert_eq!(keeper.get_total_units_destroyed(), 1);
    }

    #[test]
    fn test_object_lost_with_under_construction() {
        // Matches C++ ScoreKeeper.cpp lines 273-313 (specifically line 282, 298)
        let mut keeper = ScoreKeeper::new();

        let building_mask = create_scoring_building_mask();

        // Under construction buildings don't count
        keeper.add_object_lost("Barracks", &building_mask, true);
        assert_eq!(keeper.get_total_buildings_lost(), 0);

        // Completed buildings do count
        keeper.add_object_lost("Barracks", &building_mask, false);
        assert_eq!(keeper.get_total_buildings_lost(), 1);
    }

    #[test]
    fn test_building_capture() {
        // Matches C++ ScoreKeeper.cpp lines 196-224
        let mut keeper = ScoreKeeper::new();

        let faction_building = create_scoring_building_mask(); // Has SCORE flag
        let tech_building = create_tech_building_mask(); // No SCORE flag

        keeper.add_object_captured("CommandCenter", &faction_building);
        keeper.add_object_captured("OilRefinery", &tech_building);

        assert_eq!(keeper.get_total_faction_buildings_captured(), 1);
        assert_eq!(keeper.get_total_tech_buildings_captured(), 1);
    }

    #[test]
    fn test_score_calculation_exact() {
        // Matches C++ ScoreKeeper.cpp lines 315-332
        let mut keeper = ScoreKeeper::new();
        keeper.reset(0); // Player 0

        let unit_mask = create_scoring_unit_mask();
        let building_mask = create_scoring_building_mask();

        // C++ line 318: units built * 100
        keeper.add_object_built("Tank", &unit_mask, false);

        // C++ line 319: money earned (1:1)
        keeper.add_money_earned(1000);

        // C++ line 320: buildings built * 100
        keeper.add_object_built("Barracks", &building_mask, false);

        // C++ line 325: enemy units destroyed * 100 (player 1)
        keeper.add_object_destroyed("EnemyTank", &unit_mask, 1, false);

        // C++ line 326: enemy buildings destroyed * 100 (player 1)
        keeper.add_object_destroyed("EnemyBarracks", &building_mask, 1, false);

        let score = keeper.calculate_score();

        // Score = 1*100 + 1000 + 1*100 + 1*100 + 1*100 = 1400
        assert_eq!(score, 1400);
        assert_eq!(keeper.get_current_score(), 1400);
    }

    #[test]
    fn test_score_calculation_excludes_own_player() {
        // Matches C++ ScoreKeeper.cpp lines 323-324
        let mut keeper = ScoreKeeper::new();
        keeper.reset(0); // Player 0

        let unit_mask = create_scoring_unit_mask();

        // Destroying own units shouldn't count toward score
        keeper.add_object_destroyed("OwnTank", &unit_mask, 0, false);

        // Destroying enemy units should count
        keeper.add_object_destroyed("EnemyTank", &unit_mask, 1, false);

        let score = keeper.calculate_score();

        // Only enemy kill counts: 1 * 100 = 100
        assert_eq!(score, 100);
    }

    #[test]
    fn test_scoring_disabled() {
        // Matches C++ ScoreKeeper.cpp lines 101, 162, 198, 231, 275
        let mut keeper = ScoreKeeper::new();
        keeper.set_scoring_enabled(false);

        let unit_mask = create_scoring_unit_mask();

        keeper.add_object_built("Tank", &unit_mask, false);
        keeper.add_object_destroyed("EnemyTank", &unit_mask, 1, false);
        keeper.add_object_lost("Tank", &unit_mask, false);

        // Nothing should be tracked
        assert_eq!(keeper.get_total_units_built(), 0);
        assert_eq!(keeper.get_total_units_destroyed(), 0);
        assert_eq!(keeper.get_total_units_lost(), 0);
    }

    #[test]
    fn test_serialization_round_trip() {
        // Matches C++ ScoreKeeper.cpp xfer() implementation lines 465-536
        let mut keeper = ScoreKeeper::new();
        keeper.reset(3);

        let unit_mask = create_scoring_unit_mask();
        let building_mask = create_scoring_building_mask();

        keeper.add_object_built("Tank", &unit_mask, false);
        keeper.add_object_built("Barracks", &building_mask, false);
        keeper.add_money_earned(5000);
        keeper.add_money_spent(2000);
        keeper.add_object_destroyed("EnemyTank", &unit_mask, 1, false);
        keeper.calculate_score();

        let serialized = keeper.serialize();
        let deserialized = ScoreKeeper::deserialize(&serialized).expect("Deserialization failed");

        assert_eq!(
            keeper.get_total_units_built(),
            deserialized.get_total_units_built()
        );
        assert_eq!(
            keeper.get_total_buildings_built(),
            deserialized.get_total_buildings_built()
        );
        assert_eq!(
            keeper.get_total_money_earned(),
            deserialized.get_total_money_earned()
        );
        assert_eq!(
            keeper.get_total_money_spent(),
            deserialized.get_total_money_spent()
        );
        assert_eq!(keeper.get_current_score(), deserialized.get_current_score());
        assert_eq!(keeper.player_index, deserialized.player_index);
    }

    #[test]
    fn test_kindof_mask_operations() {
        let mut mask = KindOfMaskType::new();

        assert!(!mask.is_set(KindOf::Infantry));

        mask.set(KindOf::Infantry);
        assert!(mask.is_set(KindOf::Infantry));
        assert!(!mask.is_set(KindOf::Vehicle));

        mask.set(KindOf::Score);
        assert!(mask.is_set(KindOf::Infantry));
        assert!(mask.is_set(KindOf::Score));
    }

    #[test]
    fn test_kindof_mask_multi_matching() {
        let mut test_mask = KindOfMaskType::new();
        test_mask.set(KindOf::Structure);
        test_mask.set(KindOf::Score);

        let mut valid_mask = KindOfMaskType::new();
        valid_mask.set(KindOf::Structure);
        valid_mask.set(KindOf::Score);

        // Should match when all valid bits are present
        assert!(test_mask.matches_multi(&valid_mask, &KINDOFMASK_NONE));

        let mut invalid_mask = KindOfMaskType::new();
        invalid_mask.set(KindOf::Infantry);

        // Should still match (has Structure+Score, doesn't have Infantry)
        assert!(test_mask.matches_multi(&valid_mask, &invalid_mask));

        // Should NOT match if invalid bit is present
        test_mask.set(KindOf::Infantry);
        assert!(!test_mask.matches_multi(&valid_mask, &invalid_mask));
    }
}
