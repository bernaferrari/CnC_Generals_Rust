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

use crate::common::global_data;
use crate::common::ini::get_rank_info_store;
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
use std::sync::{Arc, LazyLock, Mutex, Weak};

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

static PLAYER_OBJECT_WORLD: LazyLock<Mutex<Option<Arc<dyn PlayerObjectWorld>>>> =
    LazyLock::new(|| Mutex::new(None));

pub fn set_player_object_world(world: Arc<dyn PlayerObjectWorld>) {
    if let Ok(mut guard) = PLAYER_OBJECT_WORLD.lock() {
        *guard = Some(world);
    }
}

pub fn clear_player_object_world() {
    if let Ok(mut guard) = PLAYER_OBJECT_WORLD.lock() {
        *guard = None;
    }
}

pub fn get_player_object_world() -> Option<Arc<dyn PlayerObjectWorld>> {
    PLAYER_OBJECT_WORLD.lock().ok().and_then(|g| g.clone())
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
    /// Team relationship overrides (TeamID → Relationship)
    /// C++: m_teamRelations (Player.h line 337)
    team_relations: super::team::TeamRelationMap,
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
    /// Active battle-plan bonus payload (NULL in C++ when no strategy center plan)
    /// C++: m_battlePlanBonuses (Player.h line 723)
    battle_plan_bonuses: Option<BattlePlanBonuses>,

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
    upgrades_in_progress: u128,
    /// Bitmask of completed upgrades
    /// C++: m_upgradesCompleted (Player.h line 349)
    upgrades_completed: u128,

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
    kind_of_production_cost_changes: Vec<KindOfPercentProductionChange>,

    // =========================================================
    // Special Power Ready Timers (C++ Player.h lines 392-393)
    // =========================================================
    /// Special power ready timers for shared cooldowns
    /// C++: m_specialPowerReadyTimerList (Player.h line 392)
    special_power_timers: HashMap<u32, u32>, // template_id -> ready_frame
    /// Object IDs owned by this player (C++ getFirstOwnedObject list).
    owned_objects: Vec<ObjectID>,
    /// True while this slot is the local player.
    is_local: bool,
}

#[path = "player_ai_build.rs"]
mod player_ai_build;
#[path = "player_ai_supply_squads.rs"]
mod player_ai_supply_squads;
#[path = "player_lifecycle.rs"]
mod player_lifecycle;
#[path = "player_production.rs"]
mod player_production;
#[path = "player_relationships_science.rs"]
mod player_relationships_science;
#[path = "player_snapshot.rs"]
mod player_snapshot;
#[path = "player_state.rs"]
mod player_state;
#[path = "player_upgrades_objects.rs"]
mod player_upgrades_objects;

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
// Tests
// =========================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TestResourceWorld {
        existing: HashSet<ObjectID>,
        distances: HashMap<(ObjectID, ObjectID), f32>,
        preferred: Option<ObjectID>,
        blocked: HashSet<ObjectID>,
        scan_distance: Option<f32>,
        query_has_ai: bool,
    }

    impl TestResourceWorld {
        fn new(query_id: ObjectID) -> Self {
            let mut world = Self {
                query_has_ai: true,
                ..Default::default()
            };
            world.existing.insert(query_id);
            world
        }

        fn add_destination(&mut self, query_id: ObjectID, dest_id: ObjectID, distance_sq: f32) {
            self.existing.insert(dest_id);
            self.distances.insert((query_id, dest_id), distance_sq);
        }
    }

    impl ResourceWorld for TestResourceWorld {
        fn object_exists(&self, id: ObjectID) -> bool {
            self.existing.contains(&id)
        }

        fn has_ai(&self, _id: ObjectID) -> bool {
            self.query_has_ai
        }

        fn can_transfer_supplies_at(&self, _query_id: ObjectID, dest_id: ObjectID) -> bool {
            !self.blocked.contains(&dest_id)
        }

        fn is_clear_to_approach(&self, dest_id: ObjectID, _query_id: ObjectID) -> bool {
            !self.blocked.contains(&dest_id)
        }

        fn distance_squared(&self, query_id: ObjectID, dest_id: ObjectID) -> Option<f32> {
            self.distances.get(&(query_id, dest_id)).copied()
        }

        fn preferred_dock(&self, _query_id: ObjectID) -> Option<ObjectID> {
            self.preferred
        }

        fn warehouse_scan_distance(&self, _query_id: ObjectID) -> Option<f32> {
            self.scan_distance
        }
    }

    #[derive(Default)]
    struct TestBuildLimitWorld {
        objects_by_player: HashMap<i32, Vec<BuildLimitObjectInfo>>,
    }

    impl TestBuildLimitWorld {
        fn add_object(&mut self, player_index: i32, object: BuildLimitObjectInfo) {
            self.objects_by_player
                .entry(player_index)
                .or_default()
                .push(object);
        }
    }

    impl BuildLimitWorld for TestBuildLimitWorld {
        fn build_limit_objects_for_player(&self, player_index: i32) -> Vec<BuildLimitObjectInfo> {
            self.objects_by_player
                .get(&player_index)
                .cloned()
                .unwrap_or_default()
        }
    }

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
    fn test_skill_points_do_not_downgrade_rank() {
        let mut player = Player::new(0);
        player.reset_rank();

        assert!(player.add_skill_points(500));
        assert_eq!(player.get_rank_level(), 3);

        assert!(!player.add_skill_points(-450));
        assert_eq!(player.get_skill_points(), 50);
        assert_eq!(player.get_rank_level(), 3);
    }

    #[test]
    fn test_set_rank_level_clamps_to_one() {
        let mut player = Player::new(0);
        player.set_rank_level(3);

        assert!(player.set_rank_level(0));
        assert_eq!(player.get_rank_level(), 1);
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

        let mut info = BuildListInfo::new("AmericaBarracks".to_string(), location, 0.0);
        info.set_num_rebuilds(1);
        info.increment_num_rebuilds();
        assert_eq!(info.get_num_rebuilds(), 2);
        info.decrement_num_rebuilds();
        assert_eq!(info.get_num_rebuilds(), 1);
        info.set_num_rebuilds(BuildListInfo::UNLIMITED_REBUILDS);
        info.increment_num_rebuilds();
        assert_eq!(info.get_num_rebuilds(), BuildListInfo::UNLIMITED_REBUILDS);
        info.decrement_num_rebuilds();
        assert_eq!(info.get_num_rebuilds(), BuildListInfo::UNLIMITED_REBUILDS);
        info.set_object_timestamp(1234);
        assert_eq!(info.get_object_timestamp(), 1234);
        info.set_building_name("MainBase".to_string());
        info.set_rally_offset(Point2D::new(3.0, 4.0));
        info.set_initially_built(true);
        info.set_script("BuildScript".to_string());
        info.set_health(80);
        info.set_whiner(false);
        info.set_unsellable(true);
        info.set_repairable(false);
        info.set_automatic_build(false);
        info.set_supply_building(true);
        info.set_gatherer_id(0, 77);
        info.set_desired_gatherers(4);
        info.set_current_gatherers(2);
        assert_eq!(info.get_building_name(), "MainBase");
        assert_eq!(*info.get_rally_offset(), Point2D::new(3.0, 4.0));
        assert!(info.is_initially_built());
        assert_eq!(info.get_script(), "BuildScript");
        assert_eq!(info.get_health(), 80);
        assert!(!info.get_whiner());
        assert!(info.get_unsellable());
        assert!(!info.get_repairable());
        assert!(!info.is_automatic_build());
        assert!(info.is_supply_building());
        assert_eq!(info.get_gatherer_id(0), 77);
        assert_eq!(
            info.get_gatherer_id(MAX_BUILD_LIST_RESOURCE_GATHERERS),
            INVALID_OBJECT_ID
        );
        assert_eq!(info.get_desired_gatherers(), 4);
        assert_eq!(info.get_current_gatherers(), 2);

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
        player.add_supply_center(1); // C++ appends duplicates
        assert_eq!(player.get_supply_centers().len(), 3);

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
    fn player_supply_world_lookup_matches_resource_manager_selection() {
        let query_id = 99;
        let mut player = Player::new(0);
        player.add_supply_warehouse(10);
        player.add_supply_warehouse(11);
        player.add_supply_warehouse(12);
        player.add_supply_warehouse(13);
        player.add_supply_center(20);
        player.add_supply_center(21);
        player.add_supply_center(22);

        let mut world = TestResourceWorld::new(query_id);
        world.add_destination(query_id, 10, 90.0);
        world.add_destination(query_id, 11, 40.0);
        world.add_destination(query_id, 20, 250.0);
        world.add_destination(query_id, 21, 30.0);
        world.blocked.insert(11);

        assert_eq!(
            player.find_best_supply_warehouse_with_world(query_id, &world),
            Some(10)
        );
        assert_eq!(player.get_supply_warehouses(), &[10, 11]);

        assert_eq!(
            player.find_best_supply_center_with_world(query_id, &world),
            Some(21)
        );
        assert_eq!(player.get_supply_centers(), &[20, 21]);
    }

    #[test]
    fn player_supply_world_lookup_honors_preferred_and_scan_distance() {
        let query_id = 7;
        let mut player = Player::new(0);
        player.add_supply_warehouse(100);
        player.add_supply_warehouse(101);

        let mut world = TestResourceWorld::new(query_id);
        world.add_destination(query_id, 100, 9.0);
        world.add_destination(query_id, 101, 400.0);
        world.preferred = Some(101);
        world.scan_distance = Some(4.0);

        assert_eq!(
            player.find_best_supply_warehouse_with_world(query_id, &world),
            Some(101)
        );

        world.preferred = None;
        assert_eq!(
            player.find_best_supply_warehouse_with_world(query_id, &world),
            Some(100)
        );
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
    fn process_create_team_evicts_from_other_squads() {
        // C++ Player::processCreateTeamGameMessage (Player.cpp:3637-3647)
        let mut player = Player::new(0);
        player.process_create_team_game_message(0, &[1, 2]);
        player.process_create_team_game_message(1, &[2, 3]);
        assert!(player.get_hotkey_squad_const(0).unwrap().contains(1));
        assert!(!player.get_hotkey_squad_const(0).unwrap().contains(2));
        assert!(player.get_hotkey_squad_const(1).unwrap().contains(2));
        assert!(player.get_hotkey_squad_const(1).unwrap().contains(3));
        assert_eq!(player.get_squad_number_for_object(2), 1);

        player.process_select_team_game_message(1);
        assert!(player.is_in_current_selection(2));
        assert!(player.is_in_current_selection(3));
        player.process_add_team_game_message(0);
        assert!(player.is_in_current_selection(1));
        assert_eq!(player.get_current_selection_size(), 3);
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
    fn player_build_limit_counts_live_objects_and_linked_templates() {
        let player = Player::new(2);
        let target = BuildLimitTemplateInfo::new("AmericaParticleCannonUplink", 2, 777, true);
        let same_link = BuildLimitTemplateInfo::new("ChinaNuclearMissileLauncher", 1, 777, true);
        let dead_same_name = BuildLimitObjectInfo::new(BuildLimitTemplateInfo::new(
            "AmericaParticleCannonUplink",
            1,
            0,
            true,
        ))
        .effectively_dead(true);

        let mut world = TestBuildLimitWorld::default();
        world.add_object(2, BuildLimitObjectInfo::new(target.clone()));
        world.add_object(2, BuildLimitObjectInfo::new(same_link));
        world.add_object(2, dead_same_name);
        world.add_object(3, BuildLimitObjectInfo::new(target.clone()));

        assert!(!player.can_build_more_of_template_with_world(&target, &world));
    }

    #[test]
    fn player_build_limit_counts_queued_units_only_for_non_structures() {
        let player = Player::new(1);
        let tank = BuildLimitTemplateInfo::new("AmericaTank", 2, 900, false);
        let linked_tank = BuildLimitTemplateInfo::new("AmericaTankVariant", 1, 900, false);
        let factory = BuildLimitTemplateInfo::new("AmericaWarFactory", 0, 0, true);

        let mut world = TestBuildLimitWorld::default();
        world.add_object(
            1,
            BuildLimitObjectInfo::new(factory).with_queued_units(vec![tank.clone(), linked_tank]),
        );

        assert!(!player.can_build_more_of_template_with_world(&tank, &world));

        let structure = BuildLimitTemplateInfo::new("AmericaStrategyCenter", 2, 1200, true);
        let linked_structure =
            BuildLimitTemplateInfo::new("AmericaStrategyCenterAlt", 1, 1200, true);
        let mut structure_world = TestBuildLimitWorld::default();
        structure_world.add_object(
            1,
            BuildLimitObjectInfo::new(BuildLimitTemplateInfo::new("AmericaDozer", 0, 0, false))
                .with_queued_units(vec![structure.clone(), linked_structure]),
        );

        assert!(player.can_build_more_of_template_with_world(&structure, &structure_world));
    }

    #[test]
    fn player_build_limit_zero_max_is_unlimited() {
        let player = Player::new(0);
        let unlimited = BuildLimitTemplateInfo::new("AmericaRanger", 0, 0, false);
        let mut world = TestBuildLimitWorld::default();
        world.add_object(0, BuildLimitObjectInfo::new(unlimited.clone()));

        assert!(player.can_build_more_of_template_with_world(&unlimited, &world));
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
        assert_eq!(player.get_completed_upgrade_mask(), 0b101000);

        // C++ UpgradeMaskType has 128 bits; high upgrade indices must not truncate.
        player.set_upgrade_completed(127);
        assert_eq!(
            player.get_completed_upgrade_mask(),
            (1u128 << 127) | 0b101000
        );

        // Out-of-range indices are ignored.
        player.set_upgrade_completed(128);
        assert_eq!(
            player.get_completed_upgrade_mask(),
            (1u128 << 127) | 0b101000
        );
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

        // Default percent change is 0.0; callers apply it as 1 + percent.
        assert!((player.get_production_cost_change("SomeUnit") - 0.0).abs() < f32::EPSILON);
        assert!((player.get_production_time_change("SomeUnit") - 0.0).abs() < f32::EPSILON);

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

    /// C++ Player.cpp:4014-4018 + Upgrade.cpp:63-74 — upgrade payload is name + version + status only.
    #[test]
    fn player_xfer_upgrade_payload_is_name_plus_upgrade_xfer() {
        use crate::common::system::xfer_load::XferLoad;
        use crate::common::system::xfer_save::XferSave;
        use std::io::Cursor;

        let mut saved = Player::new(0);
        saved.add_upgrade(
            "AmericaAdvancedTraining".to_string(),
            UpgradeStatus::Complete,
        );
        if let Some(upgrade) = saved.find_upgrade_mut("AmericaAdvancedTraining") {
            upgrade.set_start_frame(42);
            upgrade.set_complete_frame(99);
        }

        let mut encoded = Vec::new();
        {
            let mut xfer = XferSave::new(Cursor::new(&mut encoded), 1);
            saved.xfer(&mut xfer).expect("save player");
        }

        let mut loaded = Player::new(0);
        {
            let mut xfer = XferLoad::new(Cursor::new(&encoded), 1);
            loaded.xfer(&mut xfer).expect("load player");
        }

        let upgrade = loaded
            .find_upgrade("AmericaAdvancedTraining")
            .expect("upgrade restored");
        assert_eq!(upgrade.get_status(), UpgradeStatus::Complete);
        // Runtime-only frames are not in Upgrade::xfer — must not come back from the stream.
        assert_eq!(upgrade.start_frame, 0);
        assert_eq!(upgrade.complete_frame, 0);
    }

    /// C++ Player.cpp:4340-4360 / BitFlagsIO.h:134-163 — KindOf list is version + name list, not a u32 mask.
    #[test]
    fn player_xfer_kindof_production_uses_bitflags_name_list() {
        use crate::common::system::xfer_save::XferSave;
        use std::io::Cursor;

        let mut player = Player::new(0);
        player.add_kind_of_production_cost_change(KindOfMask::VEHICLE.bits() as u64, -0.10);

        let mut encoded = Vec::new();
        {
            let mut xfer = XferSave::new(Cursor::new(&mut encoded), 1);
            player.xfer(&mut xfer).expect("save player");
        }

        let as_text = String::from_utf8_lossy(&encoded);
        assert!(
            as_text.contains("VEHICLE"),
            "KindOf production xfer must persist bit names (BitFlagsIO.h:158), got {:?}",
            encoded
        );
    }

    /// C++ Player.cpp:4290 — team relations are a live TeamID→Relationship map, not an empty stub.
    #[test]
    fn player_xfer_round_trips_team_relations_and_battle_plan_bonuses() {
        use crate::common::system::xfer_load::XferLoad;
        use crate::common::system::xfer_save::XferSave;
        use std::io::Cursor;

        let mut saved = Player::new(0);
        saved.set_team_relationship(7, Relationship::Allies);
        saved.set_battle_plan_bonuses_for_test(BattlePlanBonuses {
            armor_scalar: 1.25,
            sight_range_scalar: 1.5,
            bombardment: 1,
            hold_the_line: 0,
            search_and_destroy: 2,
            valid_kind_of: KindOfMask::VEHICLE,
            invalid_kind_of: KindOfMask::AIRCRAFT,
        });

        let mut encoded = Vec::new();
        {
            let mut xfer = XferSave::new(Cursor::new(&mut encoded), 1);
            saved.xfer(&mut xfer).expect("save player");
        }

        let mut loaded = Player::new(0);
        {
            let mut xfer = XferLoad::new(Cursor::new(&encoded), 1);
            loaded.xfer(&mut xfer).expect("load player");
        }

        assert_eq!(loaded.get_team_relationship(7), Some(Relationship::Allies));
        let bonuses = loaded
            .battle_plan_bonuses
            .as_ref()
            .expect("battle plan bonuses restored");
        assert!((bonuses.armor_scalar - 1.25).abs() < f32::EPSILON);
        assert!((bonuses.sight_range_scalar - 1.5).abs() < f32::EPSILON);
        assert_eq!(bonuses.bombardment, 1);
        assert_eq!(bonuses.search_and_destroy, 2);
        assert_eq!(bonuses.valid_kind_of, KindOfMask::VEHICLE);
        assert_eq!(bonuses.invalid_kind_of, KindOfMask::AIRCRAFT);
    }

    struct TestBountyVictim {
        cost: i32,
        under_construction: bool,
    }

    impl BountyObject for TestBountyVictim {
        fn calc_cost_to_build(&self) -> i32 {
            self.cost
        }
        fn is_under_construction(&self) -> bool {
            self.under_construction
        }
    }

    #[test]
    fn do_bounty_for_kill_obj_uses_calc_cost_to_build_and_add_money_earned() {
        let mut player = Player::new(0);
        player.set_cash_bounty_percent(0.20);
        let victim = TestBountyVictim {
            cost: 1000,
            under_construction: false,
        };
        let bounty = player.do_bounty_for_kill_obj(&victim, &victim);
        assert_eq!(bounty, 200);
        assert_eq!(player.get_score_keeper().get_total_money_earned(), 200);
        assert_eq!(player.get_money().count_money(), 200);

        let unfinished = TestBountyVictim {
            cost: 1000,
            under_construction: true,
        };
        assert_eq!(player.do_bounty_for_kill_obj(&unfinished, &unfinished), 0);
        assert_eq!(player.get_score_keeper().get_total_money_earned(), 200);
    }
}
