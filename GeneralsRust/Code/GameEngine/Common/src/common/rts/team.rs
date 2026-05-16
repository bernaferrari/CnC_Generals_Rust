//! Team System
//!
//! Manages teams and alliances between players.
//! Ported from GeneralsMD/Code/GameEngine/Source/Common/RTS/Team.cpp

use std::collections::HashMap;

// Re-export Relationship from game_common for convenience
pub use crate::common::system::game_common::{Relationship, VeterancyLevel};
use crate::common::system::kind_of::KindOfMask;
use crate::common::system::snapshot::Snapshotable;
use crate::common::system::xfer::{Xfer, XferMode, XferVersion};
// Import Coord3D from geometry
use crate::common::system::geometry::Coord3D;

/// Invalid object ID constant (corresponds to C++ INVALID_ID)
pub const INVALID_OBJECT_ID: u32 = 0;

/// Name key type (corresponds to NameKeyType in C++)
pub type NameKeyType = u32;

/// Team ID type (corresponds to TeamID in C++)
pub type TeamID = u32;

/// Invalid team ID constant
pub const TEAM_ID_INVALID: TeamID = 0;

/// Team Prototype ID type (corresponds to TeamPrototypeID in C++)
pub type TeamPrototypeID = u32;

/// Invalid team prototype ID constant
pub const TEAM_PROTOTYPE_ID_INVALID: TeamPrototypeID = 0;

/// Maximum number of generic scripts per team (from C++ Team.h)
pub const MAX_GENERIC_SCRIPTS: usize = 16;

/// Maximum number of unit types in a team template (from C++ Team.h)
pub const MAX_UNIT_TYPES: usize = 7;

// =============================================================================
// AttitudeType - AI behavior modifiers
// =============================================================================

/// AI attitude/behavior modifiers
/// Corresponds to C++ AttitudeType enum in AI.h
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(i32)]
pub enum AttitudeType {
    /// AI is sleeping - not active
    Sleep = -2,
    /// AI is passive - minimal activity
    Passive = -1,
    /// AI is normal - standard behavior
    #[default]
    Normal = 0,
    /// AI is alert - heightened awareness
    Alert = 1,
    /// AI is aggressive - maximum aggression
    Aggressive = 2,
    /// Invalid attitude value
    Invalid = 3,
}

impl AttitudeType {
    /// Convert from i32 value
    pub fn from_i32(value: i32) -> Self {
        match value {
            -2 => AttitudeType::Sleep,
            -1 => AttitudeType::Passive,
            0 => AttitudeType::Normal,
            1 => AttitudeType::Alert,
            2 => AttitudeType::Aggressive,
            _ => AttitudeType::Invalid,
        }
    }

    /// Convert to i32 value
    pub fn to_i32(self) -> i32 {
        self as i32
    }
}

// =============================================================================
// TCreateUnitsInfo - Unit creation info for team templates
// =============================================================================

/// Unit creation information for team templates
/// Corresponds to C++ TCreateUnitsInfo struct in Team.h
#[derive(Debug, Clone, Default)]
pub struct TCreateUnitsInfo {
    /// Minimum number of units to create
    pub min_units: i32,
    /// Maximum number of units to create
    pub max_units: i32,
    /// Name of the thing template (unit type) to create
    pub unit_thing_name: String,
}

impl TCreateUnitsInfo {
    /// Create a new TCreateUnitsInfo with default values
    pub fn new() -> Self {
        Self {
            min_units: 0,
            max_units: 0,
            unit_thing_name: String::new(),
        }
    }

    /// Create a TCreateUnitsInfo with the specified values
    pub fn with_values(min_units: i32, max_units: i32, unit_thing_name: String) -> Self {
        Self {
            min_units,
            max_units,
            unit_thing_name,
        }
    }

    /// Check if this unit info is valid (has max_units > 0)
    pub fn is_valid(&self) -> bool {
        self.max_units > 0 && !self.unit_thing_name.is_empty()
    }
}

// =============================================================================
// TeamTemplateInfo - Template info for team creation
// =============================================================================

/// Team template info for creating reinforcement and AI teams.
///
/// This contains all the configuration data for how a team should be created,
/// including unit counts, scripts, AI behavior, and production settings.
///
/// Corresponds to C++ TeamTemplateInfo class in Team.h
#[derive(Debug, Clone)]
pub struct TeamTemplateInfo {
    // Unit creation configuration
    /// Quantity and type of units to create or build
    pub units_info: [TCreateUnitsInfo; MAX_UNIT_TYPES],
    /// Number of valid entries in units_info
    pub num_units_info: i32,

    // Spawn/location configuration
    /// Spawn location for the team
    pub home_location: Coord3D,
    /// True if home_location is valid
    pub has_home_location: bool,

    // Script callbacks - team lifecycle events
    /// Script executed when team is created
    pub script_on_create: String,
    /// Script executed when team is idle
    pub script_on_idle: String,
    /// Number of frames to wait before considering team idle
    pub initial_idle_frames: i32,
    /// Script executed when enemy is sighted
    pub script_on_enemy_sighted: String,
    /// Script executed when no enemies are visible (all clear)
    pub script_on_all_clear: String,
    /// Script executed each time a unit on this team dies
    pub script_on_unit_destroyed: String,
    /// Script executed when destroyed_threshold of member units are destroyed
    pub script_on_destroyed: String,
    /// OnDestroyed threshold - 1.0 = 100% = all destroyed, 0.5 = 50% destroyed
    pub destroyed_threshold: f32,

    // AI behavior flags
    /// True if other AI teams can recruit from this team
    pub is_ai_recruitable: bool,
    /// True if this is a base defense team
    pub is_base_defense: bool,
    /// True if this is a perimeter base defense team
    pub is_perimeter_defense: bool,
    /// True if team automatically tries to reinforce
    pub automatically_reinforce: bool,
    /// True if transports return to base after unloading
    pub transports_return: bool,
    /// True if the team avoids threats
    pub avoid_threats: bool,
    /// True if the team attacks the same target unit
    pub attack_common_target: bool,

    // Instance limits
    /// Maximum number of instances of a non-singleton team
    pub max_instances: i32,

    // Production priority (mutable for runtime adjustment)
    /// Production priority for AI team building
    pub production_priority: i32,
    /// Amount to increase priority on successful production
    pub production_priority_success_increase: i32,
    /// Amount to decrease priority on failed production
    pub production_priority_failure_decrease: i32,

    // AI attitude
    /// The initial team attitude/behavior
    pub initial_team_attitude: AttitudeType,

    // Reinforcement/transport configuration
    /// Unit type used to transport the team
    pub transport_unit_type: String,
    /// Waypoint where reinforcement team starts
    pub start_reinforce_waypoint: String,
    /// If true, team loads into member transports at start
    pub team_starts_full: bool,
    /// True if transports leave after deploying team
    pub transports_exit: bool,
    /// Veterancy level for created units
    pub veterancy: VeterancyLevel,

    // Production condition scripts
    /// Script containing production conditions
    pub production_condition: String,
    /// If true, execute actions when production condition becomes true
    pub execute_actions: bool,

    // Generic script hooks
    /// Generic script names to potentially run during team lifetime
    pub team_generic_scripts: [String; MAX_GENERIC_SCRIPTS],
}

impl Default for TeamTemplateInfo {
    fn default() -> Self {
        Self {
            units_info: Default::default(),
            num_units_info: 0,
            home_location: Coord3D::default(),
            has_home_location: false,
            script_on_create: String::new(),
            script_on_idle: String::new(),
            initial_idle_frames: 0,
            script_on_enemy_sighted: String::new(),
            script_on_all_clear: String::new(),
            script_on_unit_destroyed: String::new(),
            script_on_destroyed: String::new(),
            destroyed_threshold: 0.0,
            is_ai_recruitable: false,
            is_base_defense: false,
            is_perimeter_defense: false,
            automatically_reinforce: false,
            transports_return: false,
            avoid_threats: false,
            attack_common_target: false,
            max_instances: 0,
            production_priority: 0,
            production_priority_success_increase: 0,
            production_priority_failure_decrease: 0,
            initial_team_attitude: AttitudeType::default(),
            transport_unit_type: String::new(),
            start_reinforce_waypoint: String::new(),
            team_starts_full: false,
            transports_exit: false,
            veterancy: VeterancyLevel::Regular,
            production_condition: String::new(),
            execute_actions: false,
            team_generic_scripts: Default::default(),
        }
    }
}

impl TeamTemplateInfo {
    /// Create a new TeamTemplateInfo with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a TeamTemplateInfo from a Dict (used when loading from map/sides)
    ///
    /// Corresponds to C++ TeamTemplateInfo::TeamTemplateInfo(Dict *d)
    pub fn from_dict(_dict: &crate::common::dict::Dict) -> Self {
        // In a full implementation, this would parse all fields from the dict
        // using the well-known keys (TheKey_teamUnitMinCount1, etc.)
        // For now, return default values
        Self::default()
    }

    /// Get the units info array
    pub fn get_units_info(&self) -> &[TCreateUnitsInfo; MAX_UNIT_TYPES] {
        &self.units_info
    }

    /// Get the number of valid unit info entries
    pub fn get_num_units_info(&self) -> i32 {
        self.num_units_info
    }

    /// Check if this team should execute production condition actions
    pub fn should_execute_actions(&self) -> bool {
        self.execute_actions
    }

    /// Get the generic script name at the given index
    pub fn get_generic_script(&self, index: usize) -> Option<&str> {
        if index < MAX_GENERIC_SCRIPTS {
            Some(&self.team_generic_scripts[index])
        } else {
            None
        }
    }
}

impl Snapshotable for TeamTemplateInfo {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ TeamTemplateInfo::crc() is intentionally empty
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // Version
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;

        // Xfer the production priority (only field persisted in C++)
        xfer.xfer_int(&mut self.production_priority)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Empty in C++
        Ok(())
    }
}

/// Trait for reading sides list data (implemented by GameLogic's SidesList)
/// This allows Common code to access SidesList without depending on GameLogic
pub trait SidesListReader {
    /// Get the number of teams
    fn get_num_teams(&self) -> usize;
    /// Get team info at the given index
    fn get_team_info(&self, index: usize) -> Option<&dyn TeamInfoReader>;
}

/// Trait for reading team info data
pub trait TeamInfoReader {
    /// Get the team name
    fn team_name(&self) -> &str;
    /// Get the team owner
    fn owner(&self) -> &str;
    /// Check if this is a singleton team
    fn is_singleton(&self) -> bool;
    /// Get the dictionary with additional team properties
    fn get_dict(&self) -> Option<&crate::common::dict::Dict>;
}

/// Map of team relationships (corresponds to TeamRelationMap in C++)
///
/// This stores override relationships between this team and other teams.
/// When a relationship is set here, it overrides the default player relationship.
#[derive(Debug, Clone, Default)]
pub struct TeamRelationMap {
    /// Map from TeamID to Relationship
    relations: HashMap<TeamID, Relationship>,
}

impl TeamRelationMap {
    /// Create a new empty relationship map
    pub fn new() -> Self {
        Self {
            relations: HashMap::new(),
        }
    }

    /// Set a relationship override for a team
    pub fn set_relationship(&mut self, team_id: TeamID, relationship: Relationship) {
        if team_id != TEAM_ID_INVALID {
            self.relations.insert(team_id, relationship);
        }
    }

    /// Get a relationship override for a team, if one exists
    pub fn get_relationship(&self, team_id: TeamID) -> Option<Relationship> {
        self.relations.get(&team_id).copied()
    }

    /// Remove a relationship override for a specific team
    /// Returns true if the relationship was removed
    pub fn remove_relationship(&mut self, team_id: TeamID) -> bool {
        if team_id == TEAM_ID_INVALID {
            self.relations.clear();
            return true;
        }
        self.relations.remove(&team_id).is_some()
    }

    /// Check if the map is empty
    pub fn is_empty(&self) -> bool {
        self.relations.is_empty()
    }

    /// Clear all relationships
    pub fn clear(&mut self) {
        self.relations.clear();
    }

    /// Get the number of relationships
    pub fn len(&self) -> usize {
        self.relations.len()
    }

    /// Iterate over all relationships
    pub fn iter(&self) -> impl Iterator<Item = (TeamID, Relationship)> + '_ {
        self.relations.iter().map(|(&k, &v)| (k, v))
    }
}

impl Snapshotable for TeamRelationMap {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ TeamRelationMap::crc() is intentionally empty
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut relation_count = self.relations.len() as u16;
        xfer.xfer_unsigned_short(&mut relation_count)
            .map_err(|e| e.to_string())?;

        // Team relations
        if xfer.get_xfer_mode() == XferMode::Save {
            // Save all relations
            for (&team_id, &relationship) in self.relations.iter() {
                let mut tid = team_id;
                xfer.xfer_unsigned_int(&mut tid)
                    .map_err(|e| e.to_string())?;
                // Relationship is an enum - convert to u8 for xfer
                let mut rel_byte: u8 = match relationship {
                    Relationship::Enemies => 0u8,
                    Relationship::Neutral => 1u8,
                    Relationship::Allies => 2u8,
                };
                xfer.xfer_unsigned_byte(&mut rel_byte)
                    .map_err(|e| e.to_string())?;
            }
        } else {
            // Load relations
            self.relations.clear();
            for _ in 0..relation_count {
                let mut team_id: TeamID = TEAM_ID_INVALID;
                let mut rel_byte: u8 = 0;
                xfer.xfer_unsigned_int(&mut team_id)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_unsigned_byte(&mut rel_byte)
                    .map_err(|e| e.to_string())?;
                let relationship = match rel_byte {
                    0 => Relationship::Enemies,
                    1 => Relationship::Neutral,
                    2 => Relationship::Allies,
                    _ => Relationship::Neutral,
                };
                self.relations.insert(team_id, relationship);
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Empty in C++
        Ok(())
    }
}

// =============================================================================
// PlayerRelationMap - Maps player indices to relationships
// =============================================================================

/// Map of player indices to relationships for team-level overrides.
///
/// This stores override relationships between this team and other players.
/// When a relationship is set here, it overrides the default team/player relationship.
///
/// Corresponds to C++ PlayerRelationMap class in Player.h
#[derive(Debug, Clone, Default)]
pub struct PlayerRelationMap {
    /// Map from player index to Relationship
    relations: HashMap<i32, Relationship>,
}

impl PlayerRelationMap {
    /// Create a new empty relationship map
    pub fn new() -> Self {
        Self {
            relations: HashMap::new(),
        }
    }

    /// Set a relationship override for a player
    pub fn set_relationship(&mut self, player_index: i32, relationship: Relationship) {
        if player_index != -1 {
            // PLAYER_INDEX_INVALID
            self.relations.insert(player_index, relationship);
        }
    }

    /// Get a relationship override for a player, if one exists
    pub fn get_relationship(&self, player_index: i32) -> Option<Relationship> {
        self.relations.get(&player_index).copied()
    }

    /// Remove a relationship override for a specific player
    /// Returns true if the relationship was removed
    pub fn remove_relationship(&mut self, player_index: i32) -> bool {
        if player_index == -1 {
            self.relations.clear();
            return true;
        }
        self.relations.remove(&player_index).is_some()
    }

    /// Check if the map is empty
    pub fn is_empty(&self) -> bool {
        self.relations.is_empty()
    }

    /// Clear all relationships
    pub fn clear(&mut self) {
        self.relations.clear();
    }

    /// Get the number of relationships
    pub fn len(&self) -> usize {
        self.relations.len()
    }

    /// Iterate over all relationships
    pub fn iter(&self) -> impl Iterator<Item = (i32, Relationship)> + '_ {
        self.relations.iter().map(|(&k, &v)| (k, v))
    }
}

impl Snapshotable for PlayerRelationMap {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // Version
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;

        // Player relation count
        let mut relation_count = self.relations.len() as u16;
        xfer.xfer_unsigned_short(&mut relation_count)
            .map_err(|e| e.to_string())?;

        // Player relations
        if xfer.get_xfer_mode() == XferMode::Save {
            // Save all relations
            for (&player_index, &relationship) in self.relations.iter() {
                let mut idx = player_index;
                xfer.xfer_int(&mut idx).map_err(|e| e.to_string())?;
                // Relationship is an enum - convert to u8 for xfer
                let mut rel_byte: u8 = match relationship {
                    Relationship::Enemies => 0u8,
                    Relationship::Neutral => 1u8,
                    Relationship::Allies => 2u8,
                };
                xfer.xfer_unsigned_byte(&mut rel_byte)
                    .map_err(|e| e.to_string())?;
            }
        } else {
            // Load relations
            self.relations.clear();
            for _ in 0..relation_count {
                let mut player_index: i32 = -1;
                let mut rel_byte: u8 = 0;
                xfer.xfer_int(&mut player_index)
                    .map_err(|e| e.to_string())?;
                xfer.xfer_unsigned_byte(&mut rel_byte)
                    .map_err(|e| e.to_string())?;
                let relationship = match rel_byte {
                    0 => Relationship::Enemies,
                    1 => Relationship::Neutral,
                    2 => Relationship::Allies,
                    _ => Relationship::Neutral,
                };
                self.relations.insert(player_index, relationship);
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Empty in C++
        Ok(())
    }
}

/// Trait for objects that can be team members
/// This allows the Team struct to work with game objects without direct coupling
pub trait TeamMember {
    /// Check if this member is effectively dead
    fn is_effectively_dead(&self) -> bool;

    /// Check if this member is destroyed
    fn is_destroyed(&self) -> bool;

    /// Get the KindOf mask for this member
    fn get_kind_of_mask(&self) -> KindOfMask;

    /// Get the object ID
    fn get_id(&self) -> u32;

    /// Get position (returns None if not available)
    fn get_position(&self) -> Option<Coord3D>;

    /// Check if this member has AI (is recruitable, has AI update interface)
    fn is_ai_recruitable(&self) -> bool;

    /// Check if member is idle
    fn is_idle(&self) -> bool;

    /// Check if member is disabled by being held
    fn is_disabled_held(&self) -> bool;

    /// Check if member has AI update interface (for targetable count)
    fn has_ai_update_interface(&self) -> bool {
        // Default implementation
        false
    }

    /// Get the thing template name for this member
    fn get_template_name(&self) -> Option<&str> {
        // Default implementation
        None
    }

    /// Check if template is equivalent to the given name
    fn is_template_equivalent_to(&self, _template_name: &str) -> bool {
        // Default implementation - override for actual template checking
        false
    }

    /// Get the status bits for under construction check
    fn is_under_construction(&self) -> bool {
        // Default implementation
        false
    }

    /// Heal this member completely
    fn heal_completely(&mut self) {
        // Default implementation - override for actual healing
    }

    /// Check if this member's template is a build facility
    fn is_build_facility(&self) -> bool {
        // Default implementation
        false
    }

    /// Get vision range for enemy detection
    fn get_vision_range(&self) -> f32 {
        // Default implementation
        0.0
    }
}

/// Trait for polygon trigger area detection
pub trait PolygonTrigger {
    /// Get trigger ID
    fn get_id(&self) -> u32;
}

/// Trait for objects that can be contained (transported/garrisoned)
pub trait Containable {
    /// Check if this object contains other objects
    fn get_contain_count(&self) -> u32;

    /// Remove all contained objects
    fn remove_all_contained(&mut self);
}

/// Trait for objects that can be killed/damaged
pub trait Damageable {
    /// Kill the object
    fn kill(&mut self);

    /// Attempt to damage the object
    fn attempt_damage(&mut self, amount: f32, damage_type: i32, death_type: i32);
}

/// Trait for AI groups (groups of units that can be controlled together)
/// Corresponds to C++ AIGroup class
pub trait AIGroup {
    /// Add an object to the group
    fn add(&mut self, object_id: u32);

    /// Remove an object from the group
    fn remove(&mut self, object_id: u32);

    /// Get the number of objects in the group
    fn count(&self) -> usize;

    /// Check if an object is in the group
    fn contains(&self, object_id: u32) -> bool;

    /// Clear all objects from the group
    fn clear(&mut self);
}

/// Trait for script execution callbacks
/// Used by Team to execute scripts without direct dependency on ScriptEngine
pub trait ScriptExecutor {
    /// Run a script by name with this team as context
    fn run_script(&mut self, script_name: &str, team: &Team) -> bool;

    /// Evaluate conditions for a script
    fn evaluate_conditions(&self, script_name: &str, team: &Team) -> bool;
}

/// Trait for getting the player relationship to a team
pub trait PlayerRelationshipProvider {
    /// Get the relationship between a player (by index) and a team
    fn get_player_team_relationship(&self, player_index: i32, team_id: TeamID) -> Relationship;
}

/// Team structure
///
/// Corresponds to C++ Team class in Team.h
#[derive(Debug, Clone)]
pub struct Team {
    /// Team name
    pub name: String,
    /// Team member IDs (Object IDs)
    pub members: Vec<u32>,
    /// Unique team ID
    pub id: TeamID,
    /// Team relation overrides
    team_relations: TeamRelationMap,
    /// Player relation overrides (player index -> relationship)
    player_relations: PlayerRelationMap,
    /// True if a team is complete (false while members are being added)
    active: bool,
    /// True when first activated
    created: bool,

    // === New fields from C++ Team class ===
    /// Name of the current AI state
    state: String,
    /// True if a team member entered or exited a trigger area this frame
    entered_or_exited: bool,
    /// True if we have an on enemy sighted or all clear script
    check_enemy_sighted: bool,
    /// True if we see an enemy
    see_enemy: bool,
    /// Last value of see_enemy
    prev_see_enemy: bool,
    /// True if idle last frame
    was_idle: bool,
    /// Destroyed threshold for onDestroyed script
    destroy_threshold: i32,
    /// Current unit count for onDestroyed tracking
    cur_units: i32,
    /// Current waypoint ID (0 if none)
    current_waypoint_id: u32,
    /// Should check/execute generic scripts
    should_attempt_generic_script: [bool; MAX_GENERIC_SCRIPTS],
    /// If false, recruitability is team proto value. If true, use is_recruitable
    is_recruitability_set: bool,
    /// Team recruitability override
    is_recruitable: bool,
    /// Common attack target object ID
    common_attack_target: u32,
    /// List for post processing during xfer load
    xfer_member_id_list: Vec<u32>,
}

impl Team {
    /// Create a new team with the given name
    pub fn new(name: String) -> Self {
        Self {
            name,
            members: Vec::new(),
            id: TEAM_ID_INVALID,
            team_relations: TeamRelationMap::new(),
            player_relations: PlayerRelationMap::new(),
            active: false,
            created: false,
            state: String::new(),
            entered_or_exited: false,
            check_enemy_sighted: false,
            see_enemy: false,
            prev_see_enemy: false,
            was_idle: false,
            destroy_threshold: 0,
            cur_units: 0,
            current_waypoint_id: 0,
            should_attempt_generic_script: [true; MAX_GENERIC_SCRIPTS],
            is_recruitability_set: false,
            is_recruitable: false,
            common_attack_target: 0, // INVALID_ID
            xfer_member_id_list: Vec::new(),
        }
    }

    /// Create a new team with the given name and ID
    ///
    /// Corresponds to C++ Team::Team(TeamPrototype *proto, TeamID id)
    pub fn with_id(name: String, id: TeamID) -> Self {
        Self {
            name,
            members: Vec::new(),
            id,
            team_relations: TeamRelationMap::new(),
            player_relations: PlayerRelationMap::new(),
            active: false,
            created: false,
            state: String::new(),
            entered_or_exited: false,
            check_enemy_sighted: false,
            see_enemy: false,
            prev_see_enemy: false,
            was_idle: false,
            destroy_threshold: 0,
            cur_units: 0,
            current_waypoint_id: 0,
            should_attempt_generic_script: [true; MAX_GENERIC_SCRIPTS],
            is_recruitability_set: false,
            is_recruitable: false,
            common_attack_target: 0,
            xfer_member_id_list: Vec::new(),
        }
    }

    /// Get the team ID
    pub fn get_id(&self) -> TeamID {
        self.id
    }

    /// Set the team ID
    pub fn set_id(&mut self, id: TeamID) {
        self.id = id;
    }

    /// Get relationship to another team
    ///
    /// This checks:
    /// 1. Team-specific relationship override
    /// 2. Player-specific relationship override (based on the other team's controlling player)
    /// 3. Falls back to the default player relationship
    ///
    /// Corresponds to Team::getRelationship() in C++
    pub fn get_relationship(&self, team_id: TeamID) -> Relationship {
        // Check team relation override first
        if !self.team_relations.is_empty() {
            if let Some(rel) = self.team_relations.get_relationship(team_id) {
                return rel;
            }
        }

        // Note: In full implementation, we would also check:
        // - Player relation overrides based on the other team's controlling player
        // - Default player relationship
        // For now, return Neutral as fallback
        Relationship::Neutral
    }

    /// Get relationship to another team with player context.
    ///
    /// This is the full implementation that checks:
    /// 1. Team-specific relationship override
    /// 2. Player-specific relationship override (based on the other team's controlling player)
    /// 3. Falls back to the controlling player's relationship with the other team
    ///
    /// Corresponds to Team::getRelationship(const Team *that) in C++ (Team.cpp lines 1447-1475)
    pub fn get_relationship_with_team(
        &self,
        other_team_id: TeamID,
        other_team_player_index: Option<i32>,
        get_player_relationship: impl Fn(i32, TeamID) -> Relationship,
    ) -> Relationship {
        // Check team relation override first
        if !self.team_relations.is_empty() {
            if let Some(rel) = self.team_relations.get_relationship(other_team_id) {
                return rel;
            }
        }

        // Check player relation override based on the other team's controlling player
        if !self.player_relations.is_empty() {
            if let Some(that_player_index) = other_team_player_index {
                if let Some(rel) = self.player_relations.get_relationship(that_player_index) {
                    return rel;
                }
            }
        }

        // Fall back to our controlling player's relationship with that team
        // In full implementation, this calls getControllingPlayer()->getRelationship(that)
        // For now, use the provided callback
        if let Some(player_idx) = other_team_player_index {
            get_player_relationship(player_idx, other_team_id)
        } else {
            Relationship::Neutral
        }
    }

    /// Set an override relationship for a specific team
    ///
    /// Corresponds to C++ Team::setOverrideTeamRelationship()
    pub fn set_override_team_relationship(&mut self, team_id: TeamID, relationship: Relationship) {
        self.team_relations.set_relationship(team_id, relationship);
    }

    /// Remove an override relationship for a specific team.
    /// If team_id is TEAM_ID_INVALID, removes all team relationships.
    /// Returns true if a relationship was removed.
    ///
    /// Corresponds to C++ Team::removeOverrideTeamRelationship()
    pub fn remove_override_team_relationship(&mut self, team_id: TeamID) -> bool {
        self.team_relations.remove_relationship(team_id)
    }

    /// Set an override relationship for a specific player.
    /// If player_index is -1 (PLAYER_INDEX_INVALID), this is a no-op.
    ///
    /// Corresponds to C++ Team::setOverridePlayerRelationship()
    pub fn set_override_player_relationship(
        &mut self,
        player_index: i32,
        relationship: Relationship,
    ) {
        self.player_relations
            .set_relationship(player_index, relationship);
    }

    /// Remove an override relationship for a specific player.
    /// If player_index is -1, removes all player relationships.
    /// Returns true if a relationship was removed.
    ///
    /// Corresponds to C++ Team::removeOverridePlayerRelationship()
    pub fn remove_override_player_relationship(&mut self, player_index: i32) -> bool {
        self.player_relations.remove_relationship(player_index)
    }

    /// Count the number of buildings (structures) in this team
    ///
    /// Corresponds to Team::countBuildings() in C++
    pub fn count_buildings<M: TeamMember>(&self, get_member: impl Fn(u32) -> Option<M>) -> i32 {
        let mut count = 0;
        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                if member.get_kind_of_mask().contains(KindOfMask::STRUCTURE) {
                    count += 1;
                }
            }
        }
        count
    }

    /// Count objects matching a KindOf mask combination.
    ///
    /// Corresponds to Team::countObjects() in C++
    pub fn count_objects_by_kind<M: TeamMember>(
        &self,
        get_member: impl Fn(u32) -> Option<M>,
        set_mask: KindOfMask,
        clear_mask: KindOfMask,
    ) -> i32 {
        let mut count = 0;
        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                let kind_of = member.get_kind_of_mask();
                // Check that all set_mask bits are set and no clear_mask bits are set
                if kind_of.contains(set_mask) && !kind_of.intersects(clear_mask) {
                    count += 1;
                }
            }
        }
        count
    }

    /// Count objects by thing template names.
    /// Counts how many members match each template name.
    ///
    /// Corresponds to Team::countObjectsByThingTemplate() in C++
    pub fn count_objects_by_thing_template<M: TeamMember>(
        &self,
        get_member: impl Fn(u32) -> Option<M>,
        templates: &[&str],
        ignore_dead: bool,
        ignore_under_construction: bool,
        counts: &mut [i32],
    ) {
        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                if ignore_dead && member.is_effectively_dead() {
                    continue;
                }

                if ignore_under_construction && member.is_under_construction() {
                    continue;
                }

                for (i, template_name) in templates.iter().enumerate() {
                    if i >= counts.len() {
                        break;
                    }

                    let matches_template = member.is_template_equivalent_to(template_name)
                        || member
                            .get_template_name()
                            .is_some_and(|name| name == *template_name);

                    if matches_template {
                        counts[i] += 1;
                        break;
                    }
                }
            }
        }
    }

    /// Check if the team has any buildings
    ///
    /// Corresponds to Team::hasAnyBuildings() in C++
    /// Returns true if any member is a structure that is not dead or destroyed
    pub fn has_any_buildings<M: TeamMember>(&self, get_member: impl Fn(u32) -> Option<M>) -> bool {
        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                // Skip dead or destroyed members
                if member.is_effectively_dead() || member.is_destroyed() {
                    continue;
                }

                if member.get_kind_of_mask().contains(KindOfMask::STRUCTURE) {
                    return true;
                }
            }
        }
        false
    }

    /// Check if the team has any units (non-structure, non-projectile, non-mine)
    ///
    /// Corresponds to Team::hasAnyUnits() in C++
    /// Returns true if any member is a unit (not a structure, projectile, or mine)
    /// that is not dead or destroyed
    pub fn has_any_units<M: TeamMember>(&self, get_member: impl Fn(u32) -> Option<M>) -> bool {
        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                // Skip dead or destroyed members
                if member.is_effectively_dead() || member.is_destroyed() {
                    continue;
                }

                let kind_of = member.get_kind_of_mask();

                // If it's a structure, it's not a unit
                if kind_of.contains(KindOfMask::STRUCTURE) {
                    continue;
                }

                // If it's a projectile, it's not a unit
                if kind_of.contains(KindOfMask::PROJECTILE) {
                    continue;
                }

                // If it's a mine, it's not a unit
                if kind_of.contains(KindOfMask::MINE) {
                    continue;
                }

                return true;
            }
        }
        false
    }

    /// Check if the team has any objects at all
    ///
    /// Corresponds to Team::hasAnyObjects() in C++
    /// Similar to has_any_units but also excludes inert objects
    pub fn has_any_objects<M: TeamMember>(&self, get_member: impl Fn(u32) -> Option<M>) -> bool {
        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                // Skip dead or destroyed members
                if member.is_effectively_dead() || member.is_destroyed() {
                    continue;
                }

                let kind_of = member.get_kind_of_mask();

                // Projectiles don't count
                if kind_of.contains(KindOfMask::PROJECTILE) {
                    continue;
                }

                // Inert objects don't count
                if kind_of.contains(KindOfMask::INERT) {
                    continue;
                }

                // Mines don't count
                if kind_of.contains(KindOfMask::MINE) {
                    continue;
                }

                return true;
            }
        }
        false
    }

    /// Add a member to the team
    pub fn add_member(&mut self, member_id: u32) {
        if !self.members.contains(&member_id) {
            self.members.push(member_id);
        }
    }

    /// Remove a member from the team
    pub fn remove_member(&mut self, member_id: u32) {
        self.members.retain(|&id| id != member_id);
    }

    /// Get the number of members
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// Check if the team is empty
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Set the team as active
    ///
    /// A team is considered created when set active.
    /// Corresponds to C++ Team::setActive()
    pub fn set_active(&mut self) {
        if !self.active {
            self.created = true;
            self.active = true;
        }
    }

    /// Check if the team is active
    ///
    /// Corresponds to C++ Team::isActive()
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Check if the team was just created
    /// Stays true for one logic frame.
    ///
    /// Corresponds to C++ Team::isCreated()
    pub fn is_created(&self) -> bool {
        self.created
    }

    /// Get the team name.
    /// In C++, this returns m_proto->getName().
    /// Since we store the name directly, we just return it.
    ///
    /// Corresponds to C++ Team::getName()
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Get the prototype name (same as get_name for Rust implementation).
    /// In C++, this returns m_proto.
    pub fn get_prototype_name(&self) -> &str {
        &self.name
    }

    /// Get the controlling player index.
    /// In full implementation, this would return m_proto->getControllingPlayer().
    /// For now, returns None since we don't have a prototype reference.
    ///
    /// Corresponds to C++ Team::getControllingPlayer()
    pub fn get_controlling_player(&self) -> Option<i32> {
        // In full implementation, this would return the player from the prototype
        None
    }

    /// Set the controlling player.
    /// In full implementation, this would call m_proto->setControllingPlayer().
    ///
    /// Corresponds to C++ Team::setControllingPlayer()
    pub fn set_controlling_player(&mut self, _player_index: i32) {
        // In full implementation, this would set the player via prototype
        // and notify all members to redo their looking status
    }

    /// Set the attack priority name for this team.
    /// In full implementation, this would forward to m_proto->setAttackPriorityName().
    ///
    /// Corresponds to C++ Team::setAttackPriorityName()
    pub fn set_attack_priority_name(&mut self, _name: &str) {
        // In full implementation, this would set the attack priority on the prototype
    }

    /// Fill an AIGroup with the members of this team.
    /// The AIGroup must be non-null.
    ///
    /// Corresponds to C++ Team::getTeamAsAIGroup()
    pub fn get_team_as_ai_group<A: AIGroup>(&self, group: &mut A, add_to_group: impl Fn(&A, u32)) {
        for &member_id in &self.members {
            add_to_group(group, member_id);
        }
    }

    /// Iterate over team members
    pub fn iter_members(&self) -> impl Iterator<Item = &u32> {
        self.members.iter()
    }

    /// Get the first member in the team (returns None if empty)
    pub fn get_first_member(&self) -> Option<u32> {
        self.members.first().copied()
    }

    /// Check if an object is in this team's member list
    pub fn is_in_member_list(&self, object_id: u32) -> bool {
        self.members.contains(&object_id)
    }

    // ========================================================================
    // NEW METHODS - Team class methods ported from C++ Team.cpp
    // ========================================================================

    /// Get the team's AI state
    ///
    /// Corresponds to C++ Team::getState()
    pub fn get_state(&self) -> &str {
        &self.state
    }

    /// Set the team's AI state
    ///
    /// Corresponds to C++ Team::setState()
    pub fn set_state(&mut self, state: String) {
        self.state = state;
    }

    /// Set the team's AI recruitability
    ///
    /// Corresponds to C++ Team::setRecruitable()
    pub fn set_recruitable(&mut self, recruitable: bool) {
        self.is_recruitability_set = true;
        self.is_recruitable = recruitable;
    }

    /// Check if team is recruitable
    pub fn is_recruitable(&self) -> bool {
        self.is_recruitable
    }

    /// Check if recruitability has been set
    pub fn is_recruitability_set(&self) -> bool {
        self.is_recruitability_set
    }

    /// Note that a team member entered or exited a trigger area
    ///
    /// Corresponds to C++ Team::setEnteredExited()
    pub fn set_entered_exited(&mut self) {
        self.entered_or_exited = true;
    }

    /// Did a team member enter or exit a trigger area
    ///
    /// Corresponds to C++ Team::didEnterOrExit()
    pub fn did_enter_or_exit(&self) -> bool {
        self.entered_or_exited
    }

    /// Set the team's target object
    ///
    /// Corresponds to C++ Team::setTeamTargetObject()
    pub fn set_team_target_object(&mut self, target_id: u32) {
        // In C++, this is only done for computer players with non-easy difficulty
        // and only if the target is not stealthed/undetected
        self.common_attack_target = target_id;
    }

    /// Get the team's target object
    ///
    /// Corresponds to C++ Team::getTeamTargetObject()
    pub fn get_team_target_object(&self) -> u32 {
        self.common_attack_target
    }

    /// Clear the team target
    pub fn clear_team_target(&mut self) {
        self.common_attack_target = 0; // INVALID_ID
    }

    /// Get the current waypoint ID
    ///
    /// Corresponds to C++ Team::getCurrentWaypoint()
    pub fn get_current_waypoint_id(&self) -> u32 {
        self.current_waypoint_id
    }

    /// Set the current waypoint
    ///
    /// Corresponds to C++ Team::setCurrentWaypoint()
    pub fn set_current_waypoint_id(&mut self, waypoint_id: u32) {
        self.current_waypoint_id = waypoint_id;
    }

    /// Get count of targetable units
    ///
    /// Corresponds to C++ Team::getTargetableCount()
    pub fn get_targetable_count<M: TeamMember>(
        &self,
        get_member: impl Fn(u32) -> Option<M>,
    ) -> i32 {
        let mut count = 0;
        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                if member.is_effectively_dead() {
                    continue;
                }

                // In C++, also checks for AI update interface or structure kindOf
                // For now, we count any non-dead member
                count += 1;
            }
        }
        count
    }

    /// Update the team state - called each frame
    ///
    /// Clears m_enteredOrExited, checks & clears m_created.
    /// Corresponds to C++ Team::updateState()
    pub fn update_state<M: TeamMember, F, G>(
        &mut self,
        get_member: F,
        template_info: Option<&TeamTemplateInfo>,
        mut run_script: G,
    ) where
        F: Fn(u32) -> Option<M>,
        G: FnMut(&str, &Team) -> bool,
    {
        // Clear entered/exited flag
        self.entered_or_exited = false;

        if !self.active {
            return;
        }

        // Handle creation scripts
        if self.created {
            self.created = false;

            // Run onCreate script if any
            if let Some(info) = template_info {
                if !info.script_on_create.is_empty() {
                    run_script(&info.script_on_create, self);
                }

                // Set up info for onDestroyed script
                if !info.script_on_destroyed.is_empty() {
                    self.cur_units = 0;
                    for &member_id in &self.members {
                        if get_member(member_id).is_some() {
                            self.cur_units += 1;
                        }
                    }
                    self.destroy_threshold =
                        self.cur_units - (self.cur_units as f32 * info.destroyed_threshold) as i32;
                    if self.destroy_threshold > self.cur_units - 1 {
                        self.destroy_threshold = self.cur_units - 1;
                    }
                    if self.destroy_threshold < 0 {
                        self.destroy_threshold = 0;
                    }
                }
            }
        }

        // Do enemy sighted/on clear checks
        if self.check_enemy_sighted {
            self.prev_see_enemy = self.see_enemy;
            self.see_enemy = false;

            let mut any_alive_in_team = false;

            for &member_id in &self.members {
                if let Some(member) = get_member(member_id) {
                    if member.is_effectively_dead() {
                        continue;
                    }

                    // In full implementation, would check for enemies in vision range
                    any_alive_in_team = true;

                    // For now, we don't have partition manager access
                    // In C++, this checks for enemies in vision range
                }
            }

            if any_alive_in_team && self.prev_see_enemy != self.see_enemy {
                if let Some(info) = template_info {
                    if self.see_enemy {
                        run_script(&info.script_on_enemy_sighted, self);
                    } else {
                        run_script(&info.script_on_all_clear, self);
                    }
                }
            }
        }

        // Do onDestroyed checks
        if let Some(info) = template_info {
            if !info.script_on_destroyed.is_empty() {
                let prev_units = self.cur_units;
                self.cur_units = 0;

                for &member_id in &self.members {
                    if let Some(member) = get_member(member_id) {
                        if member.is_effectively_dead() {
                            continue;
                        }
                        self.cur_units += 1;
                    }
                }

                if self.cur_units != prev_units && self.cur_units <= self.destroy_threshold {
                    run_script(&info.script_on_destroyed, self);
                    self.destroy_threshold = -1; // Don't trigger again
                }
            }

            // Do onIdle checks
            if !info.script_on_idle.is_empty() {
                let mut is_idle = true;
                let mut any_alive_in_team = false;

                for &member_id in &self.members {
                    if let Some(member) = get_member(member_id) {
                        if member.is_effectively_dead() {
                            continue;
                        }
                        any_alive_in_team = true;
                        if !member.is_idle() {
                            is_idle = false;
                        }
                    }
                }

                if any_alive_in_team && is_idle && self.was_idle {
                    run_script(&info.script_on_idle, self);
                }
                self.was_idle = is_idle;
            }
        }
    }

    /// Notify team that an object died
    ///
    /// Corresponds to C++ Team::notifyTeamOfObjectDeath()
    pub fn notify_team_of_object_death<G>(
        &mut self,
        template_info: Option<&TeamTemplateInfo>,
        mut run_script: G,
    ) where
        G: FnMut(&str, &Team) -> bool,
    {
        if let Some(info) = template_info {
            if !info.script_on_unit_destroyed.is_empty() {
                run_script(&info.script_on_unit_destroyed, self);
            }
        }
    }

    /// Check if all members entered a trigger area
    ///
    /// Corresponds to C++ Team::didAllEnter()
    pub fn did_all_enter<M: TeamMember, P: PolygonTrigger>(
        &self,
        trigger: &P,
        _which_to_consider: u32,
        get_member: impl Fn(u32) -> Option<M>,
        did_enter: impl Fn(&M, &P) -> bool,
        is_inside: impl Fn(&M, &P) -> bool,
    ) -> bool {
        // If no units entered or exited, return false
        if !self.entered_or_exited {
            return false;
        }

        let mut any_considered = false;
        let mut entered = false;
        let mut outside = false;

        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                // Check surface type match (simplified - in C++ uses locoSetMatches with which_to_consider)
                // For now, we just check the member

                if member.is_effectively_dead() {
                    continue;
                }

                let kind_of = member.get_kind_of_mask();
                if kind_of.contains(KindOfMask::INERT) {
                    continue;
                }

                if did_enter(&member, trigger) {
                    entered = true;
                } else if !is_inside(&member, trigger) {
                    outside = true;
                }

                any_considered = true;
            }
        }

        any_considered && entered && !outside
    }

    /// Check if any member entered a trigger area
    ///
    /// Corresponds to C++ Team::didPartialEnter()
    pub fn did_partial_enter<M: TeamMember, P: PolygonTrigger>(
        &self,
        trigger: &P,
        _which_to_consider: u32,
        get_member: impl Fn(u32) -> Option<M>,
        did_enter: impl Fn(&M, &P) -> bool,
    ) -> bool {
        if !self.entered_or_exited {
            return false;
        }

        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                if member.is_effectively_dead() {
                    continue;
                }

                let kind_of = member.get_kind_of_mask();
                if kind_of.contains(KindOfMask::INERT) {
                    continue;
                }

                if did_enter(&member, trigger) {
                    return true;
                }
            }
        }

        false
    }

    /// Check if any member exited a trigger area
    ///
    /// Corresponds to C++ Team::didPartialExit()
    pub fn did_partial_exit<M: TeamMember, P: PolygonTrigger>(
        &self,
        trigger: &P,
        _which_to_consider: u32,
        get_member: impl Fn(u32) -> Option<M>,
        did_exit: impl Fn(&M, &P) -> bool,
    ) -> bool {
        if !self.entered_or_exited {
            return false;
        }

        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                if member.is_effectively_dead() {
                    continue;
                }

                let kind_of = member.get_kind_of_mask();
                if kind_of.contains(KindOfMask::INERT) {
                    continue;
                }

                if did_exit(&member, trigger) {
                    return true;
                }
            }
        }

        false
    }

    /// Check if all members exited a trigger area
    ///
    /// Corresponds to C++ Team::didAllExit()
    pub fn did_all_exit<M: TeamMember, P: PolygonTrigger>(
        &self,
        trigger: &P,
        _which_to_consider: u32,
        get_member: impl Fn(u32) -> Option<M>,
        did_exit: impl Fn(&M, &P) -> bool,
        is_inside: impl Fn(&M, &P) -> bool,
    ) -> bool {
        if !self.entered_or_exited {
            return false;
        }

        let mut any_considered = false;
        let mut exited = false;
        let mut inside = false;

        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                if member.is_effectively_dead() {
                    continue;
                }

                let kind_of = member.get_kind_of_mask();
                if kind_of.contains(KindOfMask::INERT) {
                    continue;
                }

                if did_exit(&member, trigger) {
                    exited = true;
                } else if is_inside(&member, trigger) {
                    inside = true;
                }

                any_considered = true;
            }
        }

        any_considered && exited && !inside
    }

    /// Check if all members are inside a trigger area
    ///
    /// Corresponds to C++ Team::allInside()
    pub fn all_inside<M: TeamMember, P: PolygonTrigger>(
        &self,
        trigger: &P,
        _which_to_consider: u32,
        get_member: impl Fn(u32) -> Option<M>,
        is_inside: impl Fn(&M, &P) -> bool,
    ) -> bool {
        // Empty teams are not inside
        if !self.has_any_objects(&get_member) {
            return false;
        }

        let mut any_considered = false;
        let mut any_outside = false;

        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                if member.is_effectively_dead() {
                    continue;
                }

                let kind_of = member.get_kind_of_mask();
                if kind_of.contains(KindOfMask::INERT) {
                    continue;
                }

                if !is_inside(&member, trigger) {
                    any_outside = true;
                }

                any_considered = true;
            }
        }

        any_considered && !any_outside
    }

    /// Check if no members are inside a trigger area
    ///
    /// Corresponds to C++ Team::noneInside()
    pub fn none_inside<M: TeamMember, P: PolygonTrigger>(
        &self,
        trigger: &P,
        _which_to_consider: u32,
        get_member: impl Fn(u32) -> Option<M>,
        is_inside: impl Fn(&M, &P) -> bool,
    ) -> bool {
        let mut any_considered = false;
        let mut any_inside = false;

        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                if member.is_effectively_dead() {
                    continue;
                }

                let kind_of = member.get_kind_of_mask();
                if kind_of.contains(KindOfMask::INERT) {
                    continue;
                }

                if is_inside(&member, trigger) {
                    any_inside = true;
                }

                any_considered = true;
            }
        }

        any_considered && !any_inside
    }

    /// Check if some members are inside and some are outside a trigger area
    ///
    /// Corresponds to C++ Team::someInsideSomeOutside()
    pub fn some_inside_some_outside<M: TeamMember, P: PolygonTrigger>(
        &self,
        trigger: &P,
        _which_to_consider: u32,
        get_member: impl Fn(u32) -> Option<M>,
        is_inside: impl Fn(&M, &P) -> bool,
    ) -> bool {
        let mut any_considered = false;
        let mut any_inside = false;
        let mut any_outside = false;

        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                if member.is_effectively_dead() {
                    continue;
                }

                let kind_of = member.get_kind_of_mask();
                if kind_of.contains(KindOfMask::INERT) {
                    continue;
                }

                if is_inside(&member, trigger) {
                    any_inside = true;
                } else {
                    any_outside = true;
                }

                any_considered = true;
            }
        }

        any_considered && any_inside && any_outside
    }

    /// Get an estimate of the team's position (returns position of first member)
    ///
    /// Corresponds to C++ Team::getEstimateTeamPosition()
    pub fn get_estimate_team_position<M: TeamMember>(
        &self,
        get_member: impl Fn(u32) -> Option<M>,
    ) -> Option<Coord3D> {
        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                if let Some(pos) = member.get_position() {
                    return Some(pos);
                }
            }
        }
        None
    }

    /// Delete team members (destroy objects, not just remove from team)
    ///
    /// Corresponds to C++ Team::deleteTeam()
    pub fn delete_team<M: TeamMember + Damageable, C: Containable>(
        &mut self,
        get_member: impl Fn(u32) -> Option<M>,
        get_containable: impl Fn(u32) -> Option<C>,
        mut destroy_object: impl FnMut(u32),
        is_default_team: bool,
        ignore_dead: bool,
    ) {
        // First, if this is the player's default team, evacuate containers
        // to avoid garrisoned buildings being affected
        if is_default_team {
            let mut containers_to_evacuate: Vec<u32> = Vec::new();

            for &member_id in &self.members {
                if let Some(containable) = get_containable(member_id) {
                    if containable.get_contain_count() > 0 {
                        containers_to_evacuate.push(member_id);
                    }
                }
            }

            // Evacuate containers
            for container_id in containers_to_evacuate {
                if let Some(mut containable) = get_containable(container_id) {
                    containable.remove_all_contained();
                }
            }
        }

        // Collect members to delete (to avoid modifying list while iterating)
        let mut members_to_delete: Vec<u32> = Vec::new();

        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                if ignore_dead && member.is_effectively_dead() {
                    continue;
                }
                members_to_delete.push(member_id);
            }
        }

        // Destroy objects
        for member_id in members_to_delete {
            destroy_object(member_id);
        }
    }

    /// Transfer all units to another team
    ///
    /// Corresponds to C++ Team::transferUnitsTo()
    pub fn transfer_units_to(&mut self, target_team: &mut Team) {
        if self.id == target_team.id {
            return;
        }

        // Transfer all members
        for member_id in self.members.drain(..) {
            target_team.add_member(member_id);
        }
    }

    /// Try to recruit a unit from other teams of this player
    ///
    /// Corresponds to C++ Team::tryToRecruit()
    /// Returns the ID of a recruitable unit if found, or None
    pub fn try_to_recruit<M: TeamMember>(
        &self,
        _template_name: &str,
        team_home: &Coord3D,
        max_dist: f32,
        get_member: impl Fn(u32) -> Option<M>,
        get_member_team: impl Fn(u32) -> Option<TeamID>,
        is_member_template_equivalent: impl Fn(&M, &str) -> bool,
        get_member_template_production_priority: impl Fn(&M) -> i32,
        is_member_team_ai_recruitable: impl Fn(TeamID) -> bool,
        is_member_team_active: impl Fn(TeamID) -> bool,
        this_production_priority: i32,
    ) -> Option<u32> {
        let dist_sqr = max_dist * max_dist;
        let mut recruit: Option<u32> = None;
        let mut best_dist_sqr = dist_sqr;

        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                // Check if template matches
                if !is_member_template_equivalent(&member, _template_name) {
                    continue;
                }

                // Get member's team
                let member_team_id = get_member_team(member_id)?;
                let is_default_team = false; // Would need to check

                // Check if team is active
                if !is_member_team_active(member_team_id) {
                    continue;
                }

                // Check production priority
                let member_priority = get_member_template_production_priority(&member);
                if member_priority >= this_production_priority {
                    continue;
                }

                // Check if recruitable
                let mut team_is_recruitable = is_default_team;
                if is_member_team_ai_recruitable(member_team_id) {
                    team_is_recruitable = true;
                }

                if !team_is_recruitable {
                    continue;
                }

                // Check if individual member is recruitable
                if !member.is_ai_recruitable() {
                    continue;
                }

                // Check if disabled by being held
                if member.is_disabled_held() {
                    continue;
                }

                // Check distance
                if let Some(pos) = member.get_position() {
                    let dx = team_home.x - pos.x;
                    let dy = team_home.y - pos.y;
                    let this_dist_sqr = dx * dx + dy * dy;

                    if is_default_team && recruit.is_none() {
                        recruit = Some(member_id);
                        best_dist_sqr = this_dist_sqr;
                    }

                    if this_dist_sqr <= best_dist_sqr {
                        best_dist_sqr = this_dist_sqr;
                        recruit = Some(member_id);
                    }
                }
            }
        }

        recruit
    }

    /// Evacuate team - make all containers dump their contents
    ///
    /// Corresponds to C++ Team::evacuateTeam()
    pub fn evacuate_team<M: TeamMember, C: Containable>(
        &mut self,
        get_member: impl Fn(u32) -> Option<M>,
        get_containable: impl Fn(u32) -> Option<C>,
    ) {
        let mut objects_to_process: Vec<u32> = Vec::new();

        // Find all containers with occupants
        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                if member.is_destroyed() || member.is_effectively_dead() {
                    continue;
                }

                if let Some(containable) = get_containable(member_id) {
                    if containable.get_contain_count() > 0 {
                        objects_to_process.push(member_id);
                    }
                }
            }
        }

        // Evacuate all containers
        for obj_id in objects_to_process {
            if let Some(mut containable) = get_containable(obj_id) {
                containable.remove_all_contained();
            }
        }
    }

    /// Kill team - evacuate and then kill all members
    ///
    /// Corresponds to C++ Team::killTeam()
    pub fn kill_team<M: TeamMember, C: Containable, D: Damageable>(
        &mut self,
        get_member: impl Fn(u32) -> Option<M>,
        get_containable: impl Fn(u32) -> Option<C>,
        get_damageable: impl Fn(u32) -> Option<D>,
        is_tech_building: impl Fn(u32) -> bool,
        get_neutral_default_team_id: impl Fn() -> TeamID,
    ) {
        // First evacuate
        self.evacuate_team(&get_member, &get_containable);

        // Collect objects to kill
        let mut objects_to_process: Vec<u32> = Vec::new();

        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                if member.is_destroyed() || member.is_effectively_dead() {
                    continue;
                }

                // Object's team could have changed after evacuation
                objects_to_process.push(member_id);
            }
        }

        // Kill objects (or transfer tech buildings to neutral)
        for obj_id in objects_to_process {
            if is_tech_building(obj_id) {
                // Tech buildings get transferred to neutral player's default team
                // In full implementation, would set the team
                let _ = get_neutral_default_team_id();
            } else if let Some(mut damageable) = get_damageable(obj_id) {
                damageable.kill();
            }
        }
    }

    /// Damage all team members (C++ Team::damageTeamMembers, Team.cpp:2451)
    pub fn damage_team_members<M: TeamMember + Damageable>(
        &mut self,
        get_member: impl Fn(u32) -> Option<M>,
        amount: f32,
    ) -> bool {
        for &member_id in &self.members {
            if let Some(mut member) = get_member(member_id) {
                if member.is_effectively_dead() || member.is_destroyed() {
                    continue;
                }

                if amount < 0.0 {
                    member.kill();
                } else {
                    // DAMAGE_UNRESISTABLE=11, DEATH_NORMAL=0 (C++ Damage.h)
                    member.attempt_damage(amount, 11, 0);
                }
            }
        }
        false
    }

    /// Heal all objects in the team
    ///
    /// Corresponds to C++ Team::healAllObjects()
    pub fn heal_all_objects<M: TeamMember>(
        &mut self,
        get_member: impl Fn(u32) -> Option<M>,
        heal_member: impl Fn(&mut M),
    ) {
        for &member_id in &self.members {
            if let Some(mut member) = get_member(member_id) {
                heal_member(&mut member);
            }
        }
    }

    /// Check if team is idle
    ///
    /// Corresponds to C++ Team::isIdle()
    pub fn is_idle<M: TeamMember>(&self, get_member: impl Fn(u32) -> Option<M>) -> bool {
        let mut idle = true;

        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                if member.is_effectively_dead() {
                    continue;
                }

                if !member.is_idle() {
                    idle = false;
                    break;
                }
            }
        }

        idle
    }

    /// Update generic scripts
    ///
    /// Corresponds to C++ Team::updateGenericScripts()
    ///
    /// The callback evaluates conditions and executes actions for an available script.
    /// It returns true when that script should no longer be attempted, matching the
    /// C++ one-shot-success path.
    pub fn update_generic_scripts<F>(
        &mut self,
        get_script: impl Fn(usize) -> Option<String>,
        mut evaluate_and_execute: F,
    ) where
        F: FnMut(&str, &mut Team) -> bool,
    {
        for i in 0..MAX_GENERIC_SCRIPTS {
            if self.should_attempt_generic_script[i] {
                if let Some(script_name) = get_script(i) {
                    if evaluate_and_execute(&script_name, self) {
                        self.should_attempt_generic_script[i] = false;
                    }
                } else {
                    // No script, mark as not to attempt
                    self.should_attempt_generic_script[i] = false;
                }
            }
        }
    }

    /// Check if team has any build facility
    ///
    /// Corresponds to C++ Team::hasAnyBuildFacility()
    pub fn has_any_build_facility<M: TeamMember>(
        &self,
        get_member: impl Fn(u32) -> Option<M>,
        is_build_facility: impl Fn(&M) -> bool,
    ) -> bool {
        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                if is_build_facility(&member) {
                    return true;
                }
            }
        }
        false
    }

    /// Check if any objects are in a trigger area.
    /// A convenience routine to quickly check if any objects are in a trigger area.
    ///
    /// Corresponds to C++ Team::unitsEntered()
    pub fn units_entered<M: TeamMember, P: PolygonTrigger>(
        &self,
        trigger: &P,
        get_member: impl Fn(u32) -> Option<M>,
        is_inside: impl Fn(&M, &P) -> bool,
    ) -> bool {
        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                if member.is_effectively_dead() {
                    continue;
                }

                let kind_of = member.get_kind_of_mask();
                if kind_of.contains(KindOfMask::INERT) {
                    continue;
                }

                if is_inside(&member, trigger) {
                    return true;
                }
            }
        }
        false
    }

    /// Move team to destination.
    /// Note: In full implementation, this would give a "team move" command, not individual move orders.
    ///
    /// Corresponds to C++ Team::moveTeamTo()
    pub fn move_team_to<M: TeamMember>(
        &mut self,
        get_member: impl Fn(u32) -> Option<M>,
        _destination: Coord3D,
    ) {
        // In full implementation, this would send move commands to all units
        for &member_id in &self.members {
            if let Some(member) = get_member(member_id) {
                if member.is_effectively_dead() || member.is_destroyed() {
                    continue;
                }
                // Would send move command to member here
            }
        }
    }
}

impl Snapshotable for Team {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ Team::crc() is intentionally empty
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // Version
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;

        // Xfer id - sanity check
        let mut team_id = self.id;
        xfer.xfer_unsigned_int(&mut team_id)
            .map_err(|e| e.to_string())?;
        if team_id != self.id {
            return Err(format!(
                "Team::xfer - TeamID mismatch. Xfered '{}' but should be '{}'",
                team_id, self.id
            ));
        }

        // Member list count and data
        let mut member_count = self.members.len() as u16;
        xfer.xfer_unsigned_short(&mut member_count)
            .map_err(|e| e.to_string())?;

        if xfer.get_xfer_mode() == XferMode::Save {
            // Save all member info
            for member_id in &self.members {
                let mut id = *member_id;
                xfer.xfer_unsigned_int(&mut id).map_err(|e| e.to_string())?;
            }
        } else {
            // Load all members - store in xfer list for post-processing
            self.xfer_member_id_list.clear();
            for _ in 0..member_count {
                let mut member_id: u32 = 0;
                xfer.xfer_unsigned_int(&mut member_id)
                    .map_err(|e| e.to_string())?;
                self.xfer_member_id_list.push(member_id);
            }
        }

        // State
        xfer.xfer_ascii_string(&mut self.state)
            .map_err(|e| e.to_string())?;

        // Entered or exited
        xfer.xfer_bool(&mut self.entered_or_exited)
            .map_err(|e| e.to_string())?;

        // Active status
        xfer.xfer_bool(&mut self.active)
            .map_err(|e| e.to_string())?;

        // Created flag
        xfer.xfer_bool(&mut self.created)
            .map_err(|e| e.to_string())?;

        // Check enemy sighted
        xfer.xfer_bool(&mut self.check_enemy_sighted)
            .map_err(|e| e.to_string())?;

        // See enemy
        xfer.xfer_bool(&mut self.see_enemy)
            .map_err(|e| e.to_string())?;

        // Previous see enemy
        xfer.xfer_bool(&mut self.prev_see_enemy)
            .map_err(|e| e.to_string())?;

        // Was idle
        xfer.xfer_bool(&mut self.was_idle)
            .map_err(|e| e.to_string())?;

        // Destroy threshold
        xfer.xfer_int(&mut self.destroy_threshold)
            .map_err(|e| e.to_string())?;

        // Current units
        xfer.xfer_int(&mut self.cur_units)
            .map_err(|e| e.to_string())?;

        // Waypoint
        xfer.xfer_unsigned_int(&mut self.current_waypoint_id)
            .map_err(|e| e.to_string())?;

        // Should attempt generic scripts
        let mut script_count = MAX_GENERIC_SCRIPTS as u16;
        xfer.xfer_unsigned_short(&mut script_count)
            .map_err(|e| e.to_string())?;
        if script_count as usize != MAX_GENERIC_SCRIPTS {
            return Err(format!(
                "Team::xfer - The number of allowable Generic scripts has changed. Expected {}, got {}",
                MAX_GENERIC_SCRIPTS, script_count
            ));
        }

        for i in 0..MAX_GENERIC_SCRIPTS {
            xfer.xfer_bool(&mut self.should_attempt_generic_script[i])
                .map_err(|e| e.to_string())?;
        }

        // Recruitability set
        xfer.xfer_bool(&mut self.is_recruitability_set)
            .map_err(|e| e.to_string())?;

        // Is recruitable
        xfer.xfer_bool(&mut self.is_recruitable)
            .map_err(|e| e.to_string())?;

        // Common attack target
        xfer.xfer_unsigned_int(&mut self.common_attack_target)
            .map_err(|e| e.to_string())?;

        // Team relations (C++ Team.cpp line 2685)
        self.team_relations.xfer(xfer)?;

        // Player relations (C++ Team.cpp line 2687)
        self.player_relations.xfer(xfer)?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Now that all objects have been loaded, populate the member list
        // with real object pointers (in C++, objects set their team during their own xfer)
        // For Rust, we just copy the xfer list to the members list
        //
        // Corresponds to C++ Team::loadPostProcess() (Team.cpp lines 2693-2729)
        self.members = self.xfer_member_id_list.clone();
        self.xfer_member_id_list.clear();

        // Post-process team relations
        self.team_relations.load_post_process()?;

        // Post-process player relations
        self.player_relations.load_post_process()?;

        Ok(())
    }
}

// =============================================================================
// TeamPrototype
// =============================================================================

// TeamPrototype flags (corresponds to C++ TeamPrototype::TeamPrototypeFlags)
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TeamPrototypeFlags: u32 {
        /// If set, this prototype should only produce one team
        const TEAM_SINGLETON = 0x01;
    }
}

/// Forward declaration holder for player reference
/// In full implementation, this would be a reference to Player
pub type PlayerRef = Option<usize>; // Placeholder for Player pointer

/// TeamPrototype - holds information that is invariant between multiple instances of a given Team
///
/// Note that TeamPrototype is used to hold information that is invariant between
/// multiple instances of a given Team (e.g., alliance info).
///
/// However, a TeamPrototype doesn't contain any build-list style info; that is handled
/// by the BuildList stuff.
///
/// Corresponds to C++ TeamPrototype class in Team.h
#[derive(Debug, Clone)]
pub struct TeamPrototype {
    /// Unique prototype ID
    id: TeamPrototypeID,
    /// Name of the team(s) produced
    name: String,
    /// Misc team flags
    flags: TeamPrototypeFlags,
    /// Team template info (configuration data for team creation)
    team_template: TeamTemplateInfo,
    /// Team instances list
    team_instances: Vec<Team>,
    /// The Player that currently controls the team-proto
    owning_player: PlayerRef,
    /// Attack priority name for this team
    attack_priority_name: String,
    /// Flag set to true if we don't have a production condition
    production_condition_always_false: bool,
    /// Whether we've retrieved generic scripts yet
    _retrieved_generic_scripts: bool,
}

impl TeamPrototype {
    /// Create a new TeamPrototype
    ///
    /// Corresponds to C++ TeamPrototype constructor.
    /// Note: In C++, this takes a TeamFactory pointer which is used to register
    /// the prototype with the factory. In Rust, registration is handled separately
    /// by the TeamFactory::init_team method.
    pub fn new(
        _factory: Option<()>, // Placeholder - registration handled by TeamFactory
        name: String,
        owner_player: PlayerRef,
        is_singleton: bool,
        dict: Option<&crate::common::dict::Dict>,
        id: TeamPrototypeID,
    ) -> Self {
        let mut flags = TeamPrototypeFlags::empty();
        if is_singleton {
            flags |= TeamPrototypeFlags::TEAM_SINGLETON;
        }

        // Create template info from dict if provided
        let team_template = if let Some(d) = dict {
            TeamTemplateInfo::from_dict(d)
        } else {
            TeamTemplateInfo::new()
        };

        Self {
            owning_player: owner_player,
            id,
            name,
            flags,
            team_template,
            team_instances: Vec::new(),
            attack_priority_name: String::new(),
            production_condition_always_false: false,
            _retrieved_generic_scripts: false,
        }
    }

    /// Get the prototype ID
    pub fn get_id(&self) -> TeamPrototypeID {
        self.id
    }

    /// Get the prototype name
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Check if this is a singleton team
    pub fn get_is_singleton(&self) -> bool {
        self.flags.contains(TeamPrototypeFlags::TEAM_SINGLETON)
    }

    /// Get the team template info
    ///
    /// Corresponds to C++ TeamPrototype::getTemplateInfo()
    pub fn get_template_info(&self) -> &TeamTemplateInfo {
        &self.team_template
    }

    /// Get mutable team template info
    pub fn get_template_info_mut(&mut self) -> &mut TeamTemplateInfo {
        &mut self.team_template
    }

    /// Get the first team in the instance list
    pub fn get_first_team_instance(&self) -> Option<&Team> {
        self.team_instances.first()
    }

    /// Get mutable first team in the instance list
    pub fn get_first_team_instance_mut(&mut self) -> Option<&mut Team> {
        self.team_instances.first_mut()
    }

    /// Add a team instance
    pub fn add_team_instance(&mut self, team: Team) {
        self.team_instances.push(team);
    }

    /// Find a team by ID in this prototype's instances
    ///
    /// Corresponds to C++ TeamPrototype::findTeamByID()
    pub fn find_team_by_id(&self, team_id: TeamID) -> Option<&Team> {
        self.team_instances.iter().find(|t| t.id == team_id)
    }

    /// Find a mutable team by ID
    pub fn find_team_by_id_mut(&mut self, team_id: TeamID) -> Option<&mut Team> {
        self.team_instances.iter_mut().find(|t| t.id == team_id)
    }

    /// Get the controlling player
    pub fn get_controlling_player(&self) -> PlayerRef {
        self.owning_player
    }

    /// Set the controlling player
    pub fn set_controlling_player(&mut self, player: PlayerRef) {
        self.owning_player = player;
    }

    /// Count team instances
    pub fn count_team_instances(&self) -> usize {
        self.team_instances.len()
    }

    /// Remove a team that is about to be deleted
    ///
    /// Corresponds to C++ TeamPrototype::teamAboutToBeDeleted()
    pub fn team_about_to_be_deleted(&mut self, team_id: TeamID) {
        self.team_instances.retain(|t| t.id != team_id);
    }

    /// Iterate over team instances
    pub fn iter_team_instances(&self) -> impl Iterator<Item = &Team> {
        self.team_instances.iter()
    }

    /// Iterate mutably over team instances
    pub fn iter_team_instances_mut(&mut self) -> impl Iterator<Item = &mut Team> {
        self.team_instances.iter_mut()
    }

    /// Set the attack priority name
    pub fn set_attack_priority_name(&mut self, name: String) {
        self.attack_priority_name = name;
    }

    /// Get the attack priority name
    pub fn get_attack_priority_name(&self) -> &str {
        &self.attack_priority_name
    }

    /// Make a team more likely to be selected by the AI for building due to success
    ///
    /// Corresponds to C++ TeamPrototype::increaseAIPriorityForSuccess()
    pub fn increase_ai_priority_for_success(&mut self) {
        self.team_template.production_priority +=
            self.team_template.production_priority_success_increase;
    }

    /// Make a team less likely to be selected by the AI for building due to failure
    ///
    /// Corresponds to C++ TeamPrototype::decreaseAIPriorityForFailure()
    pub fn decrease_ai_priority_for_failure(&mut self) {
        self.team_template.production_priority -=
            self.team_template.production_priority_failure_decrease;
    }

    /// Evaluate the team's production condition (C++ TeamPrototype::evaluateProductionCondition, Team.cpp:1104)
    pub fn evaluate_production_condition(&mut self) -> bool {
        if self.production_condition_always_false {
            return false;
        }

        // C++ defers to script engine; until wired up we follow the
        // "no script found" path (C++ line 1157-1159): mark always-false.
        if !self.team_template.production_condition.is_empty() {
            self.production_condition_always_false = true;
            return false;
        }

        self.production_condition_always_false = true;
        false
    }

    /// Update state for all team instances
    ///
    /// Corresponds to C++ TeamPrototype::updateState()
    pub fn update_state(&mut self) {
        // Update each team instance
        for team in &mut self.team_instances {
            // In full implementation, this would call team.update_state()
            // and handle removing empty non-singleton teams
            let _ = team;
        }
    }

    // ========================================================================
    // Counting and checking methods for TeamPrototype
    // These aggregate across all team instances
    // ========================================================================

    /// Count buildings across all team instances.
    ///
    /// Corresponds to C++ TeamPrototype::countBuildings()
    pub fn count_buildings<M: TeamMember>(&self, get_member: impl Fn(u32) -> Option<M>) -> i32 {
        let mut count = 0;
        for team in &self.team_instances {
            count += team.count_buildings(&get_member);
        }
        count
    }

    /// Count objects by KindOf mask across all team instances.
    ///
    /// Corresponds to C++ TeamPrototype::countObjects()
    pub fn count_objects<M: TeamMember>(
        &self,
        get_member: impl Fn(u32) -> Option<M>,
        set_mask: KindOfMask,
        clear_mask: KindOfMask,
    ) -> i32 {
        let mut count = 0;
        for team in &self.team_instances {
            for &member_id in &team.members {
                if let Some(member) = get_member(member_id) {
                    let kind_of = member.get_kind_of_mask();
                    if kind_of.matches(set_mask, clear_mask) {
                        count += 1;
                    }
                }
            }
        }
        count
    }

    /// Count objects by thing template across all team instances.
    ///
    /// Corresponds to C++ TeamPrototype::countObjectsByThingTemplate()
    pub fn count_objects_by_thing_template<M: TeamMember>(
        &self,
        get_member: impl Fn(u32) -> Option<M>,
        templates: &[&str],
        ignore_dead: bool,
        ignore_under_construction: bool,
    ) -> Vec<i32> {
        let mut counts = vec![0i32; templates.len()];
        for team in &self.team_instances {
            team.count_objects_by_thing_template(
                &get_member,
                templates,
                ignore_dead,
                ignore_under_construction,
                &mut counts,
            );
        }
        counts
    }

    /// Check if any team instance has buildings.
    ///
    /// Corresponds to C++ TeamPrototype::hasAnyBuildings()
    pub fn has_any_buildings<M: TeamMember>(&self, get_member: impl Fn(u32) -> Option<M>) -> bool {
        for team in &self.team_instances {
            if team.has_any_buildings(&get_member) {
                return true;
            }
        }
        false
    }

    /// Check if any team instance has buildings with specific KindOf.
    ///
    /// Corresponds to C++ TeamPrototype::hasAnyBuildings(KindOfMaskType)
    pub fn has_any_buildings_of_kind<M: TeamMember>(
        &self,
        get_member: impl Fn(u32) -> Option<M>,
        kind_of: KindOfMask,
    ) -> bool {
        for team in &self.team_instances {
            for &member_id in &team.members {
                if let Some(member) = get_member(member_id) {
                    if member.is_effectively_dead() || member.is_destroyed() {
                        continue;
                    }
                    let member_kind = member.get_kind_of_mask();
                    if member_kind.contains(KindOfMask::STRUCTURE)
                        && member_kind.contains_all(kind_of)
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Check if any team instance has units.
    ///
    /// Corresponds to C++ TeamPrototype::hasAnyUnits()
    pub fn has_any_units<M: TeamMember>(&self, get_member: impl Fn(u32) -> Option<M>) -> bool {
        for team in &self.team_instances {
            if team.has_any_units(&get_member) {
                return true;
            }
        }
        false
    }

    /// Check if any team instance has objects.
    ///
    /// Corresponds to C++ TeamPrototype::hasAnyObjects()
    pub fn has_any_objects<M: TeamMember>(&self, get_member: impl Fn(u32) -> Option<M>) -> bool {
        for team in &self.team_instances {
            if team.has_any_objects(&get_member) {
                return true;
            }
        }
        false
    }

    /// Check if any team instance has a build facility.
    ///
    /// Corresponds to C++ TeamPrototype::hasAnyBuildFacility()
    pub fn has_any_build_facility<M: TeamMember>(
        &self,
        get_member: impl Fn(u32) -> Option<M>,
        is_build_facility: impl Fn(&M) -> bool,
    ) -> bool {
        for team in &self.team_instances {
            if team.has_any_build_facility(&get_member, &is_build_facility) {
                return true;
            }
        }
        false
    }

    /// Heal all objects in all team instances.
    ///
    /// Corresponds to C++ TeamPrototype::healAllObjects()
    pub fn heal_all_objects<M: TeamMember>(&mut self, _get_member: impl Fn(u32) -> Option<M>) {
        for team in &mut self.team_instances {
            // In full implementation, this would heal each member
            let _ = team;
        }
    }

    /// Damage all members across all team instances.
    ///
    /// Corresponds to C++ TeamPrototype::damageTeamMembers()
    pub fn damage_team_members<M: TeamMember + Damageable>(
        &mut self,
        get_member: impl Fn(u32) -> Option<M>,
        amount: f32,
    ) {
        for team in &mut self.team_instances {
            let _ = team.damage_team_members(&get_member, amount);
        }
    }

    /// Move all team instances to a destination.
    ///
    /// Corresponds to C++ TeamPrototype::moveTeamTo()
    pub fn move_team_to(&mut self, _destination: Coord3D) {
        // In full implementation, would send move commands to all units
        for team in &mut self.team_instances {
            let _ = team;
        }
    }

    /// Iterate over all objects in all team instances.
    ///
    /// Corresponds to C++ TeamPrototype::iterateObjects()
    pub fn iterate_objects<M, F>(&self, get_member: impl Fn(u32) -> Option<M>, mut func: F)
    where
        M: TeamMember,
        F: FnMut(&M),
    {
        for team in &self.team_instances {
            for &member_id in &team.members {
                if let Some(member) = get_member(member_id) {
                    func(&member);
                }
            }
        }
    }
}

impl Snapshotable for TeamPrototype {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ TeamPrototype::crc() is intentionally empty
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // Version info - C++ uses version 2
        const CURRENT_VERSION: XferVersion = 2;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;

        // Owning player index (saved as player index, resolved to player pointer on load)
        // In this implementation, we store the player index as a simple i32
        let mut owning_player_index: i32 = self.owning_player.map(|p| p as i32).unwrap_or(-1);
        xfer.xfer_int(&mut owning_player_index)
            .map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.owning_player = if owning_player_index >= 0 {
                Some(owning_player_index as usize)
            } else {
                None
            };
        }

        // Version 2+: Attack priority name
        if version >= 2 {
            xfer.xfer_ascii_string(&mut self.attack_priority_name)
                .map_err(|e| e.to_string())?;
        }

        // Production condition always false flag
        xfer.xfer_bool(&mut self.production_condition_always_false)
            .map_err(|e| e.to_string())?;

        // Team template information (xfer the production priority)
        self.team_template.xfer(xfer)?;

        // Xfer team instance count
        let mut instance_count = self.team_instances.len() as u16;
        xfer.xfer_unsigned_short(&mut instance_count)
            .map_err(|e| e.to_string())?;

        // Xfer each team instance
        if xfer.get_xfer_mode() == XferMode::Save {
            for team in &mut self.team_instances {
                // Write team ID
                let mut team_id = team.id;
                xfer.xfer_unsigned_int(&mut team_id)
                    .map_err(|e| e.to_string())?;
                // Xfer team data
                team.xfer(xfer)?;
            }
        } else {
            // Loading
            self.team_instances.clear();
            self.team_instances.reserve(instance_count as usize);
            for _ in 0..instance_count {
                // Read team ID
                let mut team_id: TeamID = TEAM_ID_INVALID;
                xfer.xfer_unsigned_int(&mut team_id)
                    .map_err(|e| e.to_string())?;

                // Create team with ID
                let mut team = Team::with_id(self.name.clone(), team_id);
                team.xfer(xfer)?;
                self.team_instances.push(team);
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Post-process each team instance
        for team in &mut self.team_instances {
            team.load_post_process()?;
        }
        // Post-process team template
        self.team_template.load_post_process()?;
        Ok(())
    }
}

// =============================================================================
// TeamFactory
// =============================================================================

/// Team factory for creating and managing teams and team prototypes
///
/// Corresponds to C++ TeamFactory class in Team.h
#[derive(Debug)]
pub struct TeamFactory {
    /// Map from NameKeyType to TeamPrototype
    prototypes: HashMap<NameKeyType, TeamPrototype>,
    /// Unique team prototype ID counter
    unique_team_prototype_id: TeamPrototypeID,
    /// Unique team ID counter
    unique_team_id: TeamID,
}

impl TeamFactory {
    /// Create a new TeamFactory
    ///
    /// Corresponds to C++ TeamFactory::TeamFactory()
    pub fn new() -> Self {
        Self {
            prototypes: HashMap::new(),
            unique_team_prototype_id: TEAM_PROTOTYPE_ID_INVALID,
            unique_team_id: TEAM_ID_INVALID,
        }
    }

    /// Initialize the factory
    ///
    /// Corresponds to C++ TeamFactory::init()
    pub fn init(&mut self) {
        self.clear();
    }

    /// Reset the factory
    ///
    /// Corresponds to C++ TeamFactory::reset()
    pub fn reset(&mut self) {
        self.unique_team_prototype_id = TEAM_PROTOTYPE_ID_INVALID;
        self.unique_team_id = TEAM_ID_INVALID;
        self.clear();
    }

    /// Update the factory (called each frame)
    ///
    /// Corresponds to C++ TeamFactory::update()
    pub fn update(&mut self) {
        // Empty in C++ - no per-frame work
    }

    /// Clear all prototypes and teams
    ///
    /// Corresponds to C++ TeamFactory::clear()
    pub fn clear(&mut self) {
        self.prototypes.clear();
    }

    /// Initialize teams from a sides list
    ///
    /// Corresponds to C++ TeamFactory::initFromSides()
    /// Note: In full implementation, this would process a SidesList from GameLogic
    /// SidesList is in GameLogic, not Common, so we use a generic trait here
    pub fn init_from_sides<S: SidesListReader>(&mut self, sides: &S) {
        self.clear();

        // Iterate through all teams in the sides list
        for i in 0..sides.get_num_teams() {
            if let Some(info) = sides.get_team_info(i) {
                self.init_team(
                    info.team_name(),
                    info.owner(),
                    info.is_singleton(),
                    info.get_dict(),
                );
            }
        }
    }

    /// Initialize a single team
    ///
    /// Corresponds to C++ TeamFactory::initTeam()
    pub fn init_team(
        &mut self,
        name: &str,
        _owner: &str,
        is_singleton: bool,
        dict: Option<&crate::common::dict::Dict>,
    ) {
        // Check if team already exists
        if self.find_team_prototype(name).is_some() {
            // In C++: DEBUG_ASSERTCRASH - team already exists
            return;
        }

        // Increment prototype ID
        self.unique_team_prototype_id = self.unique_team_prototype_id.wrapping_add(1);

        // Create new prototype - registration is handled separately by this factory
        let prototype = TeamPrototype::new(
            None, // No factory reference needed - we handle registration
            name.to_string(),
            None, // owner would be resolved to Player*
            is_singleton,
            dict,
            self.unique_team_prototype_id,
        );

        // Add to map
        let name_key = self.name_to_key(name);
        self.prototypes.insert(name_key, prototype);

        // If singleton, create the singleton team
        if is_singleton {
            let _ = self.create_inactive_team(name);
        }
    }

    /// Convert name to name key (simple hash for now)
    fn name_to_key(&self, name: &str) -> NameKeyType {
        // Simple hash function - in full implementation would use NAMEKEY() macro
        let mut hash: u32 = 0;
        for byte in name.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
        }
        hash
    }

    /// Add a team prototype to the list
    ///
    /// Corresponds to C++ TeamFactory::addTeamPrototypeToList()
    pub fn add_team_prototype_to_list(&mut self, team: TeamPrototype) {
        let name_key = self.name_to_key(team.get_name());
        if self.prototypes.contains_key(&name_key) {
            // Already present - skip
            return;
        }
        self.prototypes.insert(name_key, team);
    }

    /// Remove a team prototype from the list
    ///
    /// Corresponds to C++ TeamFactory::removeTeamPrototypeFromList()
    pub fn remove_team_prototype_from_list(&mut self, name: &str) {
        let name_key = self.name_to_key(name);
        self.prototypes.remove(&name_key);
    }

    /// Find a team prototype by name
    ///
    /// Corresponds to C++ TeamFactory::findTeamPrototype()
    pub fn find_team_prototype(&self, name: &str) -> Option<&TeamPrototype> {
        let name_key = self.name_to_key(name);
        self.prototypes.get(&name_key)
    }

    /// Find a mutable team prototype by name
    pub fn find_team_prototype_mut(&mut self, name: &str) -> Option<&mut TeamPrototype> {
        let name_key = self.name_to_key(name);
        self.prototypes.get_mut(&name_key)
    }

    /// Find a team prototype by ID
    ///
    /// Corresponds to C++ TeamFactory::findTeamPrototypeByID()
    pub fn find_team_prototype_by_id(&self, id: TeamPrototypeID) -> Option<&TeamPrototype> {
        self.prototypes.values().find(|p| p.get_id() == id)
    }

    /// Find a mutable team prototype by ID
    pub fn find_team_prototype_by_id_mut(
        &mut self,
        id: TeamPrototypeID,
    ) -> Option<&mut TeamPrototype> {
        self.prototypes.values_mut().find(|p| p.get_id() == id)
    }

    /// Find a team by ID across all prototypes
    ///
    /// Corresponds to C++ TeamFactory::findTeamByID()
    pub fn find_team_by_id(&self, team_id: TeamID) -> Option<&Team> {
        if team_id == TEAM_ID_INVALID {
            return None;
        }

        for prototype in self.prototypes.values() {
            if let Some(team) = prototype.find_team_by_id(team_id) {
                return Some(team);
            }
        }
        None
    }

    /// Find a mutable team by ID
    pub fn find_team_by_id_mut(&mut self, team_id: TeamID) -> Option<&mut Team> {
        if team_id == TEAM_ID_INVALID {
            return None;
        }

        for prototype in self.prototypes.values_mut() {
            if let Some(team) = prototype.find_team_by_id_mut(team_id) {
                return Some(team);
            }
        }
        None
    }

    /// Create an inactive team (suitable for adding members as they are built)
    ///
    /// Call team.set_active() when all members are added.
    /// Corresponds to C++ TeamFactory::createInactiveTeam()
    pub fn create_inactive_team(&mut self, name: &str) -> Option<&mut Team> {
        let name_key = self.name_to_key(name);

        // Get the prototype
        let prototype = self.prototypes.get(&name_key)?;

        // For singleton teams, return existing team if present
        if prototype.get_is_singleton() {
            if let Some(team) = prototype.get_first_team_instance() {
                let existing_id = team.id;
                // Return the existing team
                return self.find_team_by_id_mut(existing_id);
            }
        }

        // Increment team ID
        self.unique_team_id = self.unique_team_id.wrapping_add(1);
        let new_team_id = self.unique_team_id;

        // Create new team
        let new_team = Team::with_id(name.to_string(), new_team_id);

        // Add to prototype's instance list
        let prototype = self.prototypes.get_mut(&name_key)?;
        prototype.add_team_instance(new_team);

        // Return the newly created team
        prototype.get_first_team_instance_mut()
    }

    /// Create an active team
    ///
    /// Corresponds to C++ TeamFactory::createTeam()
    pub fn create_team(&mut self, name: &str) -> Option<&mut Team> {
        let team = self.create_inactive_team(name)?;
        team.set_active();
        Some(team)
    }

    /// Create a team on a specific prototype
    ///
    /// Corresponds to C++ TeamFactory::createTeamOnPrototype()
    pub fn create_team_on_prototype(&mut self, prototype_id: TeamPrototypeID) -> Option<&mut Team> {
        let is_singleton = self
            .find_team_prototype_by_id(prototype_id)?
            .get_is_singleton();

        // For singleton, return existing team if present
        if is_singleton {
            let existing_id = self
                .find_team_prototype_by_id(prototype_id)?
                .get_first_team_instance()?
                .id;
            return self.find_team_by_id_mut(existing_id);
        }

        // Increment team ID
        self.unique_team_id = self.unique_team_id.wrapping_add(1);
        let new_team_id = self.unique_team_id;

        // Get prototype name for the new team
        let name = self
            .find_team_prototype_by_id(prototype_id)?
            .get_name()
            .to_string();

        // Create new team (must be mutable to call set_active)
        let mut new_team = Team::with_id(name, new_team_id);
        new_team.set_active();

        // Add to prototype
        let prototype = self.find_team_prototype_by_id_mut(prototype_id)?;
        prototype.add_team_instance(new_team);

        prototype.get_first_team_instance_mut()
    }

    /// Find a team by name
    ///
    /// Corresponds to C++ TeamFactory::findTeam()
    pub fn find_team(&mut self, name: &str) -> Option<&mut Team> {
        let name_key = self.name_to_key(name);
        let prototype = self.prototypes.get(&name_key)?;
        let is_singleton = prototype.get_is_singleton();

        if let Some(team) = prototype.get_first_team_instance() {
            let existing_id = team.id;
            return self.find_team_by_id_mut(existing_id);
        }

        // If not singleton and no team exists, create one
        if !is_singleton {
            return self.create_inactive_team(name);
        }

        None
    }

    /// Notify that a team is about to be deleted
    ///
    /// Corresponds to C++ TeamFactory::teamAboutToBeDeleted()
    pub fn team_about_to_be_deleted(&mut self, team_id: TeamID) {
        for prototype in self.prototypes.values_mut() {
            prototype.team_about_to_be_deleted(team_id);
        }
    }

    /// Get the count of prototypes
    pub fn prototype_count(&self) -> usize {
        self.prototypes.len()
    }

    /// Iterate over all prototypes
    pub fn iter_prototypes(&self) -> impl Iterator<Item = &TeamPrototype> {
        self.prototypes.values()
    }

    /// Iterate mutably over all prototypes
    pub fn iter_prototypes_mut(&mut self) -> impl Iterator<Item = &mut TeamPrototype> {
        self.prototypes.values_mut()
    }
}

impl Snapshotable for TeamFactory {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ TeamFactory::crc() is intentionally empty
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // Version
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;

        // Unique team ID counter
        xfer.xfer_unsigned_int(&mut self.unique_team_id)
            .map_err(|e| e.to_string())?;

        // Prototype count
        let mut prototype_count = self.prototypes.len() as u16;
        xfer.xfer_unsigned_short(&mut prototype_count)
            .map_err(|e| e.to_string())?;

        // Verify count matches (prototypes cannot change during runtime)
        if prototype_count as usize != self.prototypes.len() {
            return Err(format!(
                "TeamFactory::xfer - Prototype count mismatch: {} should be {}",
                prototype_count,
                self.prototypes.len()
            ));
        }

        // Xfer each prototype
        if xfer.get_xfer_mode() == XferMode::Save {
            for prototype in self.prototypes.values_mut() {
                let proto_id = prototype.get_id();
                let mut proto_id_copy = proto_id;
                xfer.xfer_unsigned_int(&mut proto_id_copy)
                    .map_err(|e| e.to_string())?;
                prototype.xfer(xfer)?;
            }
        } else {
            // Loading
            for _ in 0..prototype_count {
                let mut prototype_id: TeamPrototypeID = TEAM_PROTOTYPE_ID_INVALID;
                xfer.xfer_unsigned_int(&mut prototype_id)
                    .map_err(|e| e.to_string())?;

                let prototype = self
                    .find_team_prototype_by_id_mut(prototype_id)
                    .ok_or_else(|| {
                        format!(
                            "TeamFactory::xfer - Unable to find team prototype by id {}",
                            prototype_id
                        )
                    })?;

                prototype.xfer(xfer)?;
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Set next unique IDs to just over highest in use
        self.unique_team_id = 0;
        self.unique_team_prototype_id = 0;

        for prototype in self.prototypes.values_mut() {
            // Check prototype ID
            if prototype.get_id() >= self.unique_team_prototype_id {
                self.unique_team_prototype_id = prototype.get_id() + 1;
            }

            // Check team instance IDs
            for team in prototype.iter_team_instances() {
                if team.id >= self.unique_team_id {
                    self.unique_team_id = team.id + 1;
                }
            }

            // Call post-process on prototype
            prototype.load_post_process()?;
        }

        Ok(())
    }
}

impl Default for TeamFactory {
    fn default() -> Self {
        Self::new()
    }
}

// Global team factory singleton (corresponds to C++ TheTeamFactory)
// In full implementation, this would be managed by the game engine
lazy_static::lazy_static! {
    static ref THE_TEAM_FACTORY: std::sync::RwLock<TeamFactory> = std::sync::RwLock::new(TeamFactory::new());
}

/// Get the global team factory
pub fn the_team_factory() -> &'static std::sync::RwLock<TeamFactory> {
    &THE_TEAM_FACTORY
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct TestTeamMember {
        id: u32,
        template_name: &'static str,
        equivalent_names: &'static [&'static str],
        dead: bool,
        under_construction: bool,
    }

    impl TeamMember for TestTeamMember {
        fn is_effectively_dead(&self) -> bool {
            self.dead
        }

        fn is_destroyed(&self) -> bool {
            false
        }

        fn get_kind_of_mask(&self) -> KindOfMask {
            KindOfMask::empty()
        }

        fn get_id(&self) -> u32 {
            self.id
        }

        fn get_position(&self) -> Option<Coord3D> {
            None
        }

        fn is_ai_recruitable(&self) -> bool {
            true
        }

        fn is_idle(&self) -> bool {
            true
        }

        fn is_disabled_held(&self) -> bool {
            false
        }

        fn get_template_name(&self) -> Option<&str> {
            Some(self.template_name)
        }

        fn is_template_equivalent_to(&self, template_name: &str) -> bool {
            self.template_name == template_name || self.equivalent_names.contains(&template_name)
        }

        fn is_under_construction(&self) -> bool {
            self.under_construction
        }
    }

    #[test]
    fn count_objects_by_thing_template_matches_templates_and_filters_members() {
        let mut team = Team::with_id("CountTeam".to_string(), 1);
        team.add_member(10);
        team.add_member(20);
        team.add_member(30);
        team.add_member(40);

        let members = HashMap::from([
            (
                10,
                TestTeamMember {
                    id: 10,
                    template_name: "AmericaTankCrusader",
                    equivalent_names: &["AmericaTankCrusader_Var1"],
                    dead: false,
                    under_construction: false,
                },
            ),
            (
                20,
                TestTeamMember {
                    id: 20,
                    template_name: "AmericaInfantryRanger",
                    equivalent_names: &[],
                    dead: true,
                    under_construction: false,
                },
            ),
            (
                30,
                TestTeamMember {
                    id: 30,
                    template_name: "AmericaTankCrusader",
                    equivalent_names: &[],
                    dead: false,
                    under_construction: true,
                },
            ),
            (
                40,
                TestTeamMember {
                    id: 40,
                    template_name: "AmericaVehicleDozer",
                    equivalent_names: &[],
                    dead: false,
                    under_construction: false,
                },
            ),
        ]);

        let mut counts = [0, 0, 0];
        team.count_objects_by_thing_template(
            |id| members.get(&id).cloned(),
            &[
                "AmericaTankCrusader_Var1",
                "AmericaInfantryRanger",
                "AmericaVehicleDozer",
            ],
            true,
            true,
            &mut counts,
        );

        assert_eq!(counts, [1, 0, 1]);
    }

    #[test]
    fn count_objects_by_thing_template_keeps_cxx_first_match_behavior() {
        let mut team = Team::with_id("FirstMatchTeam".to_string(), 2);
        team.add_member(10);

        let members = HashMap::from([(
            10,
            TestTeamMember {
                id: 10,
                template_name: "SharedTemplate",
                equivalent_names: &["AliasTemplate"],
                dead: false,
                under_construction: false,
            },
        )]);

        let mut counts = [0, 0];
        team.count_objects_by_thing_template(
            |id| members.get(&id).cloned(),
            &["SharedTemplate", "AliasTemplate"],
            false,
            false,
            &mut counts,
        );

        assert_eq!(counts, [1, 0]);
    }

    #[test]
    fn update_generic_scripts_runs_available_scripts_and_keeps_repeating_slots() {
        let mut team = Team::with_id("GenericScriptTeam".to_string(), 3);
        let mut ran_scripts = Vec::new();

        team.update_generic_scripts(
            |index| (index == 0).then(|| "GenericScript0".to_string()),
            |script_name, team| {
                ran_scripts.push((script_name.to_string(), team.id));
                false
            },
        );

        assert_eq!(ran_scripts, vec![("GenericScript0".to_string(), 3)]);
        assert!(team.should_attempt_generic_script[0]);
        assert!(!team.should_attempt_generic_script[1]);
    }

    #[test]
    fn update_generic_scripts_disables_missing_and_completed_one_shot_slots() {
        let mut team = Team::with_id("OneShotGenericScriptTeam".to_string(), 4);
        let mut ran_scripts = Vec::new();

        team.update_generic_scripts(
            |index| match index {
                0 => Some("OneShotScript".to_string()),
                2 => Some("RepeatingScript".to_string()),
                _ => None,
            },
            |script_name, _team| {
                ran_scripts.push(script_name.to_string());
                script_name == "OneShotScript"
            },
        );

        assert_eq!(
            ran_scripts,
            vec!["OneShotScript".to_string(), "RepeatingScript".to_string()]
        );
        assert!(!team.should_attempt_generic_script[0]);
        assert!(!team.should_attempt_generic_script[1]);
        assert!(team.should_attempt_generic_script[2]);
    }
}
