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

use std::cell::RefCell;

use crate::common::global_data;
use crate::common::rts::player_template::PlayerTemplate;
use crate::common::rts::resource_gathering_manager::{ResourceGatheringManager, ResourceWorld};
use crate::common::rts::{
    AcademyStats, Energy, Handicap, MissionStats, Money, NameKeyType, PlayerHandle,
    ProductionPrerequisite, Relationship, SCIENCE_INVALID, ScienceType, ScoreKeeper, TeamID,
    get_science_store,
};
use crate::common::system::{
    Point2D, Snapshotable, Xfer, XferMode, XferVersion, kind_of::KindOfMask,
};
use crate::common::thing::{BuildableStatus, ThingTemplate, get_thing_factory};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Weak};

/// Object ID type used throughout the game engine
pub type ObjectID = u32;

/// Invalid object ID constant
pub const INVALID_OBJECT_ID: ObjectID = 0xFFFFFFFF;

/// Invalid hotkey squad constant (matches C++ NO_HOTKEY_SQUAD)
pub const NO_HOTKEY_SQUAD: i32 = -1;

const MAX_BUILD_LIST_RESOURCE_GATHERERS: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildLimitTemplateInfo {
    name: String,
    max_simultaneous_of_type: u32,
    max_simultaneous_link_key: NameKeyType,
    is_structure: bool,
}

impl BuildLimitTemplateInfo {
    pub fn new(
        name: impl Into<String>,
        max_simultaneous_of_type: u32,
        max_simultaneous_link_key: NameKeyType,
        is_structure: bool,
    ) -> Self {
        Self {
            name: name.into(),
            max_simultaneous_of_type,
            max_simultaneous_link_key,
            is_structure,
        }
    }

    pub fn from_thing_template(template: &ThingTemplate) -> Self {
        Self::new(
            template.get_name().as_str(),
            template.get_max_simultaneous_of_type() as u32,
            template.get_max_simultaneous_link_key(),
            template.is_kind_of_mask(KindOfMask::STRUCTURE.bits() as u64),
        )
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn max_simultaneous_of_type(&self) -> u32 {
        self.max_simultaneous_of_type
    }

    pub fn max_simultaneous_link_key(&self) -> NameKeyType {
        self.max_simultaneous_link_key
    }

    pub fn is_structure(&self) -> bool {
        self.is_structure
    }

    fn matches_template(&self, other: &BuildLimitTemplateInfo) -> bool {
        self.name == other.name
            || (self.max_simultaneous_link_key != 0
                && self.max_simultaneous_link_key == other.max_simultaneous_link_key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildLimitObjectInfo {
    template: BuildLimitTemplateInfo,
    effectively_dead: bool,
    queued_units: Vec<BuildLimitTemplateInfo>,
}

impl BuildLimitObjectInfo {
    pub fn new(template: BuildLimitTemplateInfo) -> Self {
        Self {
            template,
            effectively_dead: false,
            queued_units: Vec::new(),
        }
    }

    pub fn effectively_dead(mut self, effectively_dead: bool) -> Self {
        self.effectively_dead = effectively_dead;
        self
    }

    pub fn with_queued_units(mut self, queued_units: Vec<BuildLimitTemplateInfo>) -> Self {
        self.queued_units = queued_units;
        self
    }

    pub fn template(&self) -> &BuildLimitTemplateInfo {
        &self.template
    }

    pub fn is_effectively_dead(&self) -> bool {
        self.effectively_dead
    }

    pub fn queued_units(&self) -> &[BuildLimitTemplateInfo] {
        &self.queued_units
    }
}

pub trait BuildLimitWorld {
    fn build_limit_objects_for_player(&self, player_index: i32) -> Vec<BuildLimitObjectInfo>;
}

/// Snapshot of a live object the Common Player can act on.
#[derive(Debug, Clone, Default)]
pub struct PlayerObjectSnapshot {
    pub id: ObjectID,
    pub template_name: String,
    pub kind_of: u128,
    pub is_dead: bool,
    pub has_ai: bool,
    pub is_idle: bool,
    pub is_structure: bool,
    pub is_powered: bool,
    pub is_faction_structure: bool,
    pub is_beacon: bool,
    pub has_contain: bool,
    pub is_disguiser: bool,
    pub contain_player_mask: u32,
    pub position: Coord3D,
}

impl PlayerObjectSnapshot {
    pub fn is_kind(&self, mask: KindOfMask) -> bool {
        (self.kind_of & mask.bits()) != 0 || self.kind_matches_bit(mask)
    }

    fn kind_matches_bit(&self, mask: KindOfMask) -> bool {
        if mask.contains(KindOfMask::POWERED) {
            return self.is_powered;
        }
        if mask.contains(KindOfMask::STRUCTURE) {
            return self.is_structure;
        }
        false
    }
}

/// GameLogic-facing world so Player can iterate real objects.
pub trait PlayerObjectWorld: Send + Sync + std::fmt::Debug {
    fn snapshot(&self, id: ObjectID) -> Option<PlayerObjectSnapshot>;
    fn object_ids_for_player(&self, player_index: i32) -> Vec<ObjectID>;
    fn all_object_ids(&self) -> Vec<ObjectID> {
        Vec::new()
    }

    fn set_disabled_underpowered(&self, id: ObjectID, disable: bool);
    fn set_script_disabled(&self, id: ObjectID, disabled: bool);
    fn set_team(&self, id: ObjectID, team_id: TeamID);
    fn get_neutral_default_team(&self) -> Option<TeamID> {
        None
    }
    fn evacuate_container(&self, id: ObjectID);
    fn kill_object(&self, id: ObjectID);
    fn sell_object(&self, id: ObjectID);
    fn ai_enter(&self, unit_id: ObjectID, building_id: ObjectID);
    fn ai_evacuate(&self, id: ObjectID);
    fn ai_move_to(&self, id: ObjectID, pos: Coord3D);
    fn ai_force_want_supplies(&self, id: ObjectID) {
        let _ = (id,);
    }
    fn can_enter(&self, unit_id: ObjectID, building_id: ObjectID) -> bool {
        let _ = (unit_id, building_id);
        false
    }
    fn recalc_contain_and_radar(&self, id: ObjectID) {
        let _ = id;
    }
    fn refresh_disguise_for_local(&self, id: ObjectID, local_player_index: i32) {
        let _ = (id, local_player_index);
    }
    fn mark_ui_dirty(&self) {}
    fn set_observer_control_bar(&self) {}
    fn set_player_control_bar(&self, _player_index: i32) {}
    fn set_team_color(&self, _r: i32, _g: i32, _b: i32) {}
    fn is_single_player_game(&self) -> bool {
        false
    }
    fn is_shell_game(&self) -> bool {
        false
    }
}

pub fn set_player_object_world(world: Arc<dyn PlayerObjectWorld>) {
    *crate::common::common_services::common_services()
        .player_object_world
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = Some(world);
}

pub fn clear_player_object_world() {
    *crate::common::common_services::common_services()
        .player_object_world
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = None;
}

pub fn get_player_object_world() -> Option<Arc<dyn PlayerObjectWorld>> {
    crate::common::common_services::common_services()
        .player_object_world
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
}

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
    /// C++ ThingTemplate::calcCostToBuild(victim->getControllingPlayer()).
    /// No controlling player → 0.
    fn calc_cost_to_build(&self) -> i32;

    /// Raw INI sticker fallback. Bounty uses [`Self::calc_cost_to_build`].
    fn get_build_cost(&self) -> i32 {
        self.calc_cost_to_build()
    }

    /// Check if this object is under construction (no bounty for under-construction)
    fn is_under_construction(&self) -> bool;

    /// C++ `killer->getPosition()` for the bounty floater. Default is the origin.
    fn bounty_anchor(&self) -> (f32, f32, f32) {
        (0.0, 0.0, 0.0)
    }
}

/// Trait for objects that provide skill points when killed.
/// C++ Reference: Player::addSkillPointsForKill takes `const Object* killer, const Object* victim`
pub trait SkillPointObject {
    /// Get the skill point value for killing this object
    fn get_skill_point_value(&self, killer: &dyn SkillPointObject) -> i32;

    /// Get the veterancy level of this object
    fn get_veterancy_level(&self) -> i32;

    /// C++ `Object::testStatus(OBJECT_STATUS_UNDER_CONSTRUCTION)`.
    fn is_under_construction(&self) -> bool {
        false
    }
}

/// Trait for AI player functionality
/// The actual AIPlayer struct is defined in GameLogic/src/ai/ai_player.rs
/// This trait allows Player to reference AI functionality without direct dependency
pub trait AIPlayerInterface: std::fmt::Debug + Send + Sync {
    /// Update the AI player
    fn update(&self) -> Result<(), String> {
        Ok(())
    }

    /// Called when a new map is loaded
    fn new_map(&self) {}

    /// Check if this is a skirmish AI
    fn is_skirmish_ai(&self) -> bool;

    /// Get the current enemy target
    fn get_ai_enemy(&self) -> Option<i32>;

    /// Check bridges for pathfinding
    fn check_bridges(&self, _unit_id: ObjectID, _waypoint: i32) -> bool {
        false
    }

    /// Repair a structure
    fn repair_structure(&self, _structure_id: ObjectID) {}

    /// Get base center position
    fn get_base_center(&self) -> Option<Coord3D> {
        None
    }

    /// Called when a unit is produced
    fn on_unit_produced(&self, _factory_id: ObjectID, _unit_id: ObjectID) {}

    /// Called when a structure is produced
    fn on_structure_produced(&self, _factory_id: ObjectID, _structure_id: ObjectID) {}

    /// Set the AI difficulty
    fn set_ai_difficulty(&self, _difficulty: GameDifficulty) {}

    /// Get the AI difficulty
    fn get_ai_difficulty(&self) -> GameDifficulty;

    fn build_specific_ai_team(&self, _team_name: &str, _priority: bool) {}
    fn build_ai_base_defense(&self, _flank: bool) {}
    fn build_ai_base_defense_structure(&self, _thing_name: &str, _flank: bool) {}
    fn build_specific_ai_building(&self, _thing_name: &str) {}
    fn build_by_supplies(&self, _minimum_cash: i32, _thing_name: &str) {}
    fn build_specific_building_nearest_team(&self, _thing_name: &str, _team_id: i32) {}
    fn build_upgrade(&self, _upgrade_name: &str) {}
    fn recruit_specific_ai_team(&self, _team_name: &str, _recruit_radius: f32) {}
    fn calc_closest_construction_zone(&self, _template_name: &str) -> Option<Coord3D> {
        None
    }
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
pub use crate::common::system::geometry::Coord3D;

// =========================================================
// BuildListInfo - Build list entry for AI construction
// C++ Reference: BuildListInfo class in SidesList.h
// =========================================================

/// Build list information for AI construction
/// C++ Reference: BuildListInfo class
#[derive(Debug, Clone)]
pub struct BuildListInfo {
    /// Name of the building entry in WorldBuilder/build lists
    building_name: String,
    /// Template name of the building to construct
    template_name: String,
    /// Location to build at
    location: Coord3D,
    /// Offset to natural rally point
    rally_point_offset: Point2D,
    /// Angle of the building
    angle: f32,
    /// Whether the structure exists at map start
    initially_built: bool,
    /// Object ID if building exists
    object_id: ObjectID,
    /// Number of times to rebuild (0xFFFF_FFFF = unlimited)
    num_rebuilds: u32,
    /// Script attached to this build-list entry
    script: String,
    /// Initial health percent
    health: i32,
    /// Whether this structure can emit low-power/attack warnings
    whiner: bool,
    /// Whether this structure cannot be sold
    unsellable: bool,
    /// Whether this structure can be repaired
    repairable: bool,
    /// Whether AI is allowed to build without script enabling it
    automatically_build: bool,
    /// Whether this is a priority build
    priority_build: bool,
    /// Whether currently under construction
    under_construction: bool,
    /// Timestamp when object was created
    object_timestamp: u32,
    /// Gatherers assigned to this supply building entry
    resource_gatherers: [ObjectID; MAX_BUILD_LIST_RESOURCE_GATHERERS],
    /// Whether this entry represents a supply building
    supply_building: bool,
    /// Desired gatherer count
    desired_gatherers: i32,
    /// Current gatherer count
    current_gatherers: i32,
    /// Next entry in the linked list
    next: Option<Box<BuildListInfo>>,
}

impl BuildListInfo {
    /// Unlimited rebuilds constant
    pub const UNLIMITED_REBUILDS: u32 = 0xFFFF_FFFF;

    /// Create a new build list info entry
    pub fn new(template_name: String, location: Coord3D, angle: f32) -> Self {
        Self {
            building_name: String::new(),
            template_name,
            location,
            rally_point_offset: Point2D::new(0.0, 0.0),
            angle,
            initially_built: false,
            object_id: INVALID_OBJECT_ID,
            num_rebuilds: 0,
            script: String::new(),
            health: 100,
            whiner: true,
            unsellable: false,
            repairable: true,
            automatically_build: true,
            priority_build: false,
            under_construction: false,
            object_timestamp: 0,
            resource_gatherers: [INVALID_OBJECT_ID; MAX_BUILD_LIST_RESOURCE_GATHERERS],
            supply_building: false,
            desired_gatherers: 0,
            current_gatherers: 0,
            next: None,
        }
    }

    /// Get the build-list entry name
    pub fn get_building_name(&self) -> &str {
        &self.building_name
    }

    /// Set the build-list entry name
    pub fn set_building_name(&mut self, name: String) {
        self.building_name = name;
    }

    /// Get the template name
    pub fn get_template_name(&self) -> &str {
        &self.template_name
    }

    /// Get the location
    pub fn get_location(&self) -> &Coord3D {
        &self.location
    }

    /// Get the rally-point offset
    pub fn get_rally_offset(&self) -> &Point2D {
        &self.rally_point_offset
    }

    /// Set the rally-point offset
    pub fn set_rally_offset(&mut self, offset: Point2D) {
        self.rally_point_offset = offset;
    }

    /// Get the angle
    pub fn get_angle(&self) -> f32 {
        self.angle
    }

    /// Whether this entry is initially built
    pub fn is_initially_built(&self) -> bool {
        self.initially_built
    }

    /// Set whether this entry is initially built
    pub fn set_initially_built(&mut self, built: bool) {
        self.initially_built = built;
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

    /// Whether AI automatically builds this entry
    pub fn is_automatic_build(&self) -> bool {
        self.automatically_build
    }

    /// Set whether AI automatically builds this entry
    pub fn set_automatic_build(&mut self, automatic: bool) {
        self.automatically_build = automatic;
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

    /// Increment rebuild count
    pub fn increment_num_rebuilds(&mut self) {
        if self.num_rebuilds != Self::UNLIMITED_REBUILDS {
            self.num_rebuilds += 1;
        }
    }

    /// Get frame when the object was built
    pub fn get_object_timestamp(&self) -> u32 {
        self.object_timestamp
    }

    /// Set frame when the object was built
    pub fn set_object_timestamp(&mut self, frame: u32) {
        self.object_timestamp = frame;
    }

    pub fn get_script(&self) -> &str {
        &self.script
    }

    pub fn set_script(&mut self, script: String) {
        self.script = script;
    }

    pub fn get_health(&self) -> i32 {
        self.health
    }

    pub fn set_health(&mut self, health: i32) {
        self.health = health;
    }

    pub fn get_whiner(&self) -> bool {
        self.whiner
    }

    pub fn set_whiner(&mut self, whiner: bool) {
        self.whiner = whiner;
    }

    pub fn get_unsellable(&self) -> bool {
        self.unsellable
    }

    pub fn set_unsellable(&mut self, unsellable: bool) {
        self.unsellable = unsellable;
    }

    pub fn get_repairable(&self) -> bool {
        self.repairable
    }

    pub fn set_repairable(&mut self, repairable: bool) {
        self.repairable = repairable;
    }

    pub fn is_supply_building(&self) -> bool {
        self.supply_building
    }

    pub fn set_supply_building(&mut self, supply: bool) {
        self.supply_building = supply;
    }

    pub fn get_gatherer_id(&self, index: usize) -> ObjectID {
        self.resource_gatherers
            .get(index)
            .copied()
            .unwrap_or(INVALID_OBJECT_ID)
    }

    pub fn set_gatherer_id(&mut self, index: usize, id: ObjectID) {
        if let Some(gatherer) = self.resource_gatherers.get_mut(index) {
            *gatherer = id;
        }
    }

    pub fn get_desired_gatherers(&self) -> i32 {
        self.desired_gatherers
    }

    pub fn set_desired_gatherers(&mut self, desired: i32) {
        self.desired_gatherers = desired;
    }

    pub fn get_current_gatherers(&self) -> i32 {
        self.current_gatherers
    }

    pub fn set_current_gatherers(&mut self, current: i32) {
        self.current_gatherers = current;
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

/// C++ `KindOfPercentProductionChange` entry (Player.h).
/// Xfer format: `m_kindOf.xfer()` (BitFlags name list) + percent + ref (Player.cpp:4346-4352).
#[derive(Debug, Clone, PartialEq)]
struct KindOfPercentProductionChange {
    kind_of: KindOfMask,
    percent: f32,
    refs: u32,
}

/// C++ `BattlePlanBonuses` (BattlePlanUpdate.h:85-96).
/// Player::xfer writes armor, sight, bombardment, holdTheLine, searchAndDestroy, valid/invalid KindOf
/// (Player.cpp:4497-4503) — not the struct declaration order.
#[derive(Debug, Clone, PartialEq)]
struct BattlePlanBonuses {
    armor_scalar: f32,
    sight_range_scalar: f32,
    bombardment: i32,
    hold_the_line: i32,
    search_and_destroy: i32,
    valid_kind_of: KindOfMask,
    invalid_kind_of: KindOfMask,
}

impl Default for BattlePlanBonuses {
    fn default() -> Self {
        Self {
            armor_scalar: 1.0,
            sight_range_scalar: 1.0,
            bombardment: 0,
            hold_the_line: 0,
            search_and_destroy: 0,
            valid_kind_of: KindOfMask::empty(),
            invalid_kind_of: KindOfMask::empty(),
        }
    }
}

/// C++ `BitFlags<NUMBITS>::xfer` (BitFlagsIO.h:134-207): version + name list (save/load)
/// or raw user bytes (CRC). Used by KindOf production entries and BattlePlanBonuses KindOf.
fn xfer_kind_of_mask(xfer: &mut dyn Xfer, mask: &mut KindOfMask) -> Result<(), String> {
    const CURRENT_VERSION: XferVersion = 1;
    let mut version = CURRENT_VERSION;
    xfer.xfer_version(&mut version, CURRENT_VERSION)
        .map_err(|e| format!("KindOfMask version xfer failed: {}", e))?;

    match xfer.get_xfer_mode() {
        XferMode::Save => {
            let names = mask.to_string_list();
            let mut count = names.len() as i32;
            xfer.xfer_int(&mut count)
                .map_err(|e| format!("KindOfMask count xfer failed: {}", e))?;
            for mut name in names {
                xfer.xfer_ascii_string(&mut name)
                    .map_err(|e| format!("KindOfMask name xfer failed: {}", e))?;
            }
            Ok(())
        }
        XferMode::Load => {
            *mask = KindOfMask::empty();
            let mut count = 0i32;
            xfer.xfer_int(&mut count)
                .map_err(|e| format!("KindOfMask count load failed: {}", e))?;
            for _ in 0..count {
                let mut name = String::new();
                xfer.xfer_ascii_string(&mut name)
                    .map_err(|e| format!("KindOfMask name load failed: {}", e))?;
                let bit = KindOfMask::from_string(&name)
                    .ok_or_else(|| format!("KindOfMask invalid bit name '{}'", name))?;
                *mask |= bit;
            }
            Ok(())
        }
        XferMode::Crc => {
            // C++ uses xferUser(this, sizeof(this)); we CRC the live 128-bit mask.
            let mut bits = mask.bits();
            xfer.xfer_u128(&mut bits)
                .map_err(|e| format!("KindOfMask crc failed: {}", e))?;
            Ok(())
        }
        _ => Err(format!(
            "KindOfMask xfer - unknown mode {:?}",
            xfer.get_xfer_mode()
        )),
    }
}

/// C++ `Upgrade::xfer` (Upgrade.cpp:63-74): version byte + status enum ONLY.
fn xfer_upgrade_instance(xfer: &mut dyn Xfer, status: &mut UpgradeStatus) -> Result<(), String> {
    const CURRENT_VERSION: XferVersion = 1;
    let mut version = CURRENT_VERSION;
    xfer.xfer_version(&mut version, CURRENT_VERSION)
        .map_err(|e| format!("Upgrade::xfer version failed: {}", e))?;

    let mut status_byte = match *status {
        UpgradeStatus::Pending => 0u8, // UPGRADE_STATUS_INVALID
        UpgradeStatus::InProduction => 1u8,
        UpgradeStatus::Complete => 2u8,
    };
    xfer.xfer_unsigned_byte(&mut status_byte)
        .map_err(|e| format!("Upgrade::xfer status failed: {}", e))?;
    *status = match status_byte {
        1 => UpgradeStatus::InProduction,
        2 => UpgradeStatus::Complete,
        _ => UpgradeStatus::Pending,
    };
    Ok(())
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
    /// C++ Reference: PlayerRelationMap::crc() lines 165-168 - intentionally empty
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
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
