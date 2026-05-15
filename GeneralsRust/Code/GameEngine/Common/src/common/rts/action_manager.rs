//! Action Manager System
//!
//! Manages player actions, unit commands, and game interactions.
//! Ported from C++ ActionManager.cpp/ActionManager.h
//!
//! This is a central place for logical queries about what objects can do
//! in the world and to other objects. The purpose is to assist UI logic
//! and validate network commands.

use std::collections::VecDeque;
use std::sync::Arc;

use super::handles::ObjectHandle;

// ================================================================================================
// ENUMS AND TYPES - Matching C++ definitions
// ================================================================================================

/// Command source type - where the command originated
/// Reference: C++ GameCommon.h lines 193-200
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandSourceType {
    FromPlayer = 0,
    FromScript = 1,
    FromAi = 2,
    FromDozer = 3,           // Special case for dozer attacking mines
    DefaultSwitchWeapon = 4, // Special case for weapon switching
}

/// Result of attack capability checks
/// Reference: C++ WeaponSet.h lines 164-171
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CanAttackResult {
    NotPossible = 0,         // Can't attack at all
    InvalidShot = 1,         // Not a clear shot
    PossibleAfterMoving = 2, // Can attack after moving closer
    Possible = 3,            // Can attack now
}

/// Mode for entering objects/containers
/// Reference: C++ ActionManager.h lines 28-33
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanEnterType {
    CheckCapacity,
    DontCheckCapacity,
    CombatDropInto,
}

// Re-export canonical Relationship from game_common (Enemies=0, Neutral=1, Allies=2)
pub use crate::common::game_common::Relationship;

/// Object shroud status
/// Reference: C++ GameCommon.h lines 140-150
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ObjectShroudStatus {
    Invalid = 0,
    Clear = 1,
    PartialClear = 2,
    Fogged = 3,
    Shrouded = 4,
}

/// Cell shroud status
/// Reference: C++ GameCommon.h lines 129-136
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CellShroudStatus {
    Clear = 0,
    Fogged = 1,
    Shrouded = 2,
}

/// Player type
/// Reference: C++ GameCommon.h lines 119-125
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerType {
    Human = 0,
    Computer = 1,
}

/// Weapon slot types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponSlotType {
    Primary = 0,
    Secondary = 1,
    Tertiary = 2,
}

/// Special power types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialPowerType {
    None = 0,
    InfantryCaptureBuilding,
    BlackLotusDisableVehicleHack,
    BlackLotusStealCashHack,
    BlackLotusCaptureBuilding,
    // Add more as needed...
}

/// Able to attack type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbleToAttackType {
    AttackNewTarget,
    AttackContinueCurrent,
}

// ================================================================================================
// FORWARD DECLARATIONS - Types wrapping real game state
// ================================================================================================

/// 3D coordinate
#[derive(Debug, Clone, Copy)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

// ================================================================================================
// GAME STATE PROVIDER TRAIT
// ================================================================================================

/// Trait providing real game-object state queries for the action system.
///
/// The GameLogic crate implements this trait to wire the Common-layer
/// action validation code to actual game state. In C++, the ActionManager
/// held raw `Object*` pointers and called methods directly; here we
/// use an injected provider to keep the dependency one-directional
/// (GameLogic → Common, not the reverse).
///
/// # Thread safety
///
/// Implementations must be safe to call from the logic thread.
/// The trait is `Send + Sync` so the provider can be stored in a global.
pub trait ObjectDataProvider: Send + Sync {
    /// Returns `true` if the handle refers to a live game object.
    fn is_valid_object(&self, id: ObjectHandle) -> bool;

    /// Relationship between two objects (Enemies / Neutral / Allies).
    fn get_relationship(&self, source: ObjectHandle, target: ObjectHandle) -> Relationship;

    /// Relationship between a source object and a player's default team.
    fn get_team_relationship(&self, source: ObjectHandle, player_id: u32) -> Relationship;

    /// Whether the object is effectively dead (destroyed / pending deletion).
    fn is_effectively_dead(&self, id: ObjectHandle) -> bool;

    /// Whether the object can move (not IMMOBILE kind, not disabled).
    fn is_mobile(&self, id: ObjectHandle) -> bool;

    /// Test an `ObjectStatusTypes` bit on the object.
    fn test_status(&self, id: ObjectHandle, status_bit: u32) -> bool;

    /// Check if the object is of a specific KindOf classification.
    /// The `kind_of` parameter is the bit index matching the C++ KindOf enum order.
    fn is_kind_of(&self, id: ObjectHandle, kind_of: u32) -> bool;

    /// Whether the object is above terrain (airborne).
    fn is_above_terrain(&self, id: ObjectHandle) -> bool;

    /// Current health value.
    fn get_health(&self, id: ObjectHandle) -> f32;

    /// Maximum health value.
    fn get_max_health(&self, id: ObjectHandle) -> f32;

    /// Controlling player index (0-based), or `None` if no player controls this object.
    fn get_controlling_player_id(&self, id: ObjectHandle) -> Option<u32>;

    /// Controlling player type (Human / Computer).
    fn get_controlling_player_type(&self, id: ObjectHandle) -> PlayerType;

    /// Shroud status of `target` as seen by `viewer_player_id`.
    fn get_shrouded_status(&self, target: ObjectHandle, viewer_player_id: u32) -> ObjectShroudStatus;

    /// Whether the object has a SupplyTruckAI interface.
    fn has_supply_truck_ai(&self, id: ObjectHandle) -> bool;

    /// Whether the object has a SupplyWarehouseDockUpdate module.
    fn is_supply_warehouse(&self, id: ObjectHandle) -> bool;

    /// Whether the object has a SupplyCenterDockUpdate module.
    fn is_supply_center(&self, id: ObjectHandle) -> bool;

    /// Number of supply boxes remaining in the warehouse.
    fn get_warehouse_boxes(&self, id: ObjectHandle) -> u32;

    /// Number of supply boxes the truck is currently carrying.
    fn get_supply_boxes(&self, id: ObjectHandle) -> u32;

    /// Whether the supply unit is available for supplying (e.g. Chinook not busy).
    fn is_available_for_supplying(&self, id: ObjectHandle) -> bool;

    /// Whether the object has a DockUpdateInterface module.
    fn has_dock_update_interface(&self, id: ObjectHandle) -> bool;

    /// Whether the object has a RailedTransportDockUpdate module.
    fn is_railed_transport_dock(&self, id: ObjectHandle) -> bool;

    /// Whether the object is inside a container (garrisoned, loaded into transport, etc.).
    fn is_contained(&self, id: ObjectHandle) -> bool;

    /// Whether the object is in a surrendered state.
    fn is_surrendered(&self, id: ObjectHandle) -> bool;

    /// Player index the object surrendered to, if any.
    fn get_surrendered_player_index(&self, id: ObjectHandle) -> Option<u32>;

    /// Whether the object has a ContainModule (can hold other objects).
    fn has_contain_module(&self, id: ObjectHandle) -> bool;

    /// The apparent controlling player as seen by `viewer_player_id`.
    /// Used for stealth / disguise logic.
    fn get_apparent_controlling_player(&self, id: ObjectHandle, viewer_player_id: u32) -> Option<u32>;
}

// ================================================================================================
// GLOBAL DATA PROVIDER
// ================================================================================================

/// Global data provider instance (set by GameLogic at startup).
///
/// In C++, ActionManager accessed Object* pointers directly. In Rust,
/// the Common crate cannot depend on GameLogic, so we inject an
/// `ObjectDataProvider` through this global. GameLogic sets it during
/// initialization via `set_object_data_provider()`.
static mut OBJECT_DATA_PROVIDER: Option<&'static dyn ObjectDataProvider> = None;

/// Set the global object data provider. Called once during GameLogic init.
///
/// # Safety
/// Must be called from a single-threaded context before any action manager queries.
pub unsafe fn set_object_data_provider(provider: &'static dyn ObjectDataProvider) {
    OBJECT_DATA_PROVIDER = Some(provider);
}

/// Get a reference to the global object data provider, if one has been installed.
fn get_provider() -> Option<&'static dyn ObjectDataProvider> {
    // SAFETY: read-only access; provider is set once during init before any queries.
    unsafe { OBJECT_DATA_PROVIDER }
}

// ================================================================================================
// OBJECT WRAPPER
// ================================================================================================

/// KindOf bit indices matching the C++ KindOf enum order.
/// Used with `ObjectDataProvider::is_kind_of()`.
/// Reference: C++ Object.h / KindOf.h
pub mod kind_of_bit {
    pub const OBSTACLE: u32 = 0;
    pub const SELECTABLE: u32 = 1;
    pub const IMMOBILE: u32 = 2;
    pub const CAN_ATTACK: u32 = 3;
    pub const STRUCTURE: u32 = 7;
    pub const INFANTRY: u32 = 8;
    pub const VEHICLE: u32 = 9;
    pub const AIRCRAFT: u32 = 10;
    pub const DOZER: u32 = 12;
    pub const HARVESTER: u32 = 13;
    pub const POW_TRUCK: u32 = 17;
    pub const TRANSPORT: u32 = 21;
    pub const BRIDGE: u32 = 22;
    pub const BRIDGE_TOWER: u32 = 24;
    pub const REPAIR_PAD: u32 = 31;
    pub const HEAL_PAD: u32 = 32;
    pub const REBUILD_HOLE: u32 = 37;
    pub const FS_AIRFIELD: u32 = 110;
}

/// ObjectStatus bit indices matching the C++ ObjectStatusTypes enum order.
/// Used with `ObjectDataProvider::test_status()`.
/// Reference: C++ ObjectStatusTypes.h
pub mod status_bit {
    pub const DESTROYED: u32 = 1;
    pub const UNDER_CONSTRUCTION: u32 = 3;
    pub const SOLD: u32 = 19;
}

/// Game object (unit, building, etc.) used by the action validation system.
///
/// In C++ the ActionManager held raw `Object*` pointers and called methods
/// directly. Here the `Object` wraps an `ObjectHandle` and delegates every
/// query through the globally-injected `ObjectDataProvider`, which the
/// GameLogic crate implements against the real game-object system.
pub struct Object {
    handle: ObjectHandle,
}

impl Object {
    /// Create an Object wrapper from a raw handle.
    pub fn from_handle(handle: ObjectHandle) -> Self {
        Self { handle }
    }

    /// Create an Object wrapper from a raw u32 ID.
    pub fn from_id(id: u32) -> Self {
        Self {
            handle: ObjectHandle::new(id),
        }
    }

    /// The underlying handle value.
    pub fn handle(&self) -> ObjectHandle {
        self.handle
    }

    /// The raw ID value.
    pub fn id(&self) -> u32 {
        self.handle.value()
    }

    /// Helper: execute a query against the data provider, returning a fallback
    /// value when no provider is registered (e.g. during unit tests or early init).
    #[inline]
    fn with_provider<F, T>(&self, f: F, fallback: T) -> T
    where
        F: FnOnce(&'static dyn ObjectDataProvider) -> T,
    {
        match get_provider() {
            Some(p) => f(p),
            None => fallback,
        }
    }
}

/// Player in the game
/// This would be fully implemented in the player module
pub struct Player {
    // Placeholder fields
    player_index: u32,
}

/// Special power template
pub struct SpecialPowerTemplate {
    // Placeholder
}

// ================================================================================================
// ACTION QUEUE SYSTEM
// ================================================================================================

/// Action types that can be queued
/// Reference: C++ would have these as part of command system
#[derive(Debug, Clone)]
pub enum ActionType {
    Move {
        target_pos: Coord3D,
    },
    Attack {
        target_id: u32,
    },
    AttackMove {
        target_pos: Coord3D,
    },
    Guard {
        target_pos: Option<Coord3D>,
        target_id: Option<u32>,
    },
    Stop,
    Build {
        building_type: u32,
        position: Coord3D,
    },
    Repair {
        target_id: u32,
    },
    Enter {
        container_id: u32,
    },
    Garrison {
        building_id: u32,
    },
    TransferSupplies {
        target_id: u32,
    },
    SpecialPower {
        power_type: SpecialPowerType,
        target_pos: Option<Coord3D>,
        target_id: Option<u32>,
    },
}

/// Queued action with metadata
#[derive(Debug, Clone)]
struct QueuedAction {
    action: ActionType,
    player_index: u32,
    timestamp: u32, // Frame number when queued
}

/// Action executor hook for integrating with game logic
pub trait ActionExecutor: Send + Sync {
    fn execute(&self, player_index: u32, action: ActionType, options: u32) -> bool;
}

// ================================================================================================
// ACTION MANAGER
// ================================================================================================

/// ActionManager - central hub for action validation and execution
///
/// Reference: C++ ActionManager.h lines 37-86
/// C++ ActionManager.cpp lines 108-2070
///
/// This is a convenient place to wrap up logical queries about what objects
/// can do in the world and to other objects. This assists UI and validates
/// network commands.
pub struct ActionManager {
    /// Queue of pending actions
    action_queue: VecDeque<QueuedAction>,

    /// Current frame number
    current_frame: u32,

    /// Maximum queue size
    max_queue_size: usize,

    /// Optional action executor for real game logic integration
    action_executor: Option<Arc<dyn ActionExecutor>>,
}

impl ActionManager {
    /// Create a new ActionManager
    /// Reference: C++ ActionManager.cpp lines 108-111
    pub fn new() -> Self {
        Self {
            action_queue: VecDeque::new(),
            current_frame: 0,
            max_queue_size: 1000,
            action_executor: None,
        }
    }

    /// Initialize the action manager subsystem
    /// Reference: C++ ActionManager.h line 45
    pub fn init(&mut self) {
        // Initialization logic would go here
        self.action_queue.clear();
        self.current_frame = 0;
    }

    /// Reset the action manager
    /// Reference: C++ ActionManager.h line 46
    pub fn reset(&mut self) {
        self.action_queue.clear();
        self.current_frame = 0;
    }

    /// Update the action manager (called per frame)
    /// Reference: C++ ActionManager.h line 47
    pub fn update(&mut self) {
        self.current_frame += 1;

        // Process queued actions
        // In the real implementation, this would execute actions from the queue
        // based on timing, dependencies, and game state

        // For now, just a basic structure
        while let Some(action) = self.action_queue.front() {
            // Check if action is ready to execute
            if action.timestamp <= self.current_frame {
                // Execute the action
                let executed_action = self.action_queue.pop_front();
                if let Some(executed_action) = executed_action {
                    if let Some(executor) = &self.action_executor {
                        executor.execute(executed_action.player_index, executed_action.action, 0);
                    } else {
                        log::warn!(
                            "Dropping action for player {} without executor",
                            executed_action.player_index
                        );
                    }
                }
            } else {
                break;
            }
        }
    }

    // ============================================================================================
    // SINGLE UNIT TO UNIT CHECKS
    // ============================================================================================

    /// Check if object can get repaired at repair destination
    /// Reference: C++ ActionManager.cpp lines 122-182
    pub fn can_get_repaired_at(
        &self,
        obj: &Object,
        repair_dest: &Object,
        command_source: CommandSourceType,
    ) -> bool {
        // Line 126-127: Sanity check
        if obj.is_null() || repair_dest.is_null() {
            return false;
        }

        // Line 129: Check relationship
        let relationship = obj.get_relationship(repair_dest);

        // Line 132-133: Only available to allies
        if relationship != Relationship::Allies {
            return false;
        }

        // Line 136-137: Dead objects cannot be repaired
        if obj.is_effectively_dead() {
            return false;
        }

        // Line 140-141: Must be mobile to get repaired
        if !obj.is_mobile() {
            return false;
        }

        // Line 144-146: Nothing under construction
        if obj.test_status_under_construction() || repair_dest.test_status_under_construction() {
            return false;
        }

        // Line 149-150: Can't repair at something being sold
        if repair_dest.test_status_sold() {
            return false;
        }

        // Line 153-154: Only vehicles can get repaired
        if !obj.is_kind_of_vehicle() {
            return false;
        }

        // Line 157-168: Aircraft require airfield, other vehicles require repair pad
        if obj.is_kind_of_aircraft() {
            if !obj.is_above_terrain() || !repair_dest.is_kind_of_airfield() {
                return false;
            }
        } else {
            if !repair_dest.is_kind_of_repair_pad() {
                return false;
            }
        }

        // Line 171-173: Can't repair if at full health
        if obj.get_health() == obj.get_max_health() {
            return false;
        }

        // Line 176-177: Check shroud status
        if is_object_shrouded_for_action(obj, repair_dest, command_source) {
            return false;
        }

        // Line 180: All checks passed
        true
    }

    /// Check if supplies can be transferred at location
    /// Reference: C++ ActionManager.cpp lines 187-261
    pub fn can_transfer_supplies_at(&self, obj: &Object, transfer_dest: &Object) -> bool {
        // Line 191-192: Sanity check
        if obj.is_null() || transfer_dest.is_null() {
            return false;
        }

        // Line 194-197: Can't transfer to dead objects
        if transfer_dest.is_effectively_dead() {
            return false;
        }

        // Line 200-202: Nothing under construction
        if obj.test_status_under_construction() || transfer_dest.test_status_under_construction() {
            return false;
        }

        // Line 205-206: Can't transfer at something being sold
        if transfer_dest.test_status_sold() {
            return false;
        }

        // Line 209-215: Must have supply truck AI interface
        if !obj.has_supply_truck_ai() {
            return false;
        }

        // Line 218-222: Warehouse checks - must have boxes and not be enemy
        if transfer_dest.is_supply_warehouse() {
            if transfer_dest.get_warehouse_boxes() == 0
                || transfer_dest.get_relationship(obj) == Relationship::Enemies
            {
                return false;
            }
        }

        // Line 226-230: Supply center checks - must have boxes and same player
        if transfer_dest.is_supply_center() {
            if obj.get_supply_boxes() == 0
                || transfer_dest.get_controlling_player_id() != obj.get_controlling_player_id()
            {
                return false;
            }
        }

        // Line 233-234: Must be warehouse or center
        if !transfer_dest.is_supply_warehouse() && !transfer_dest.is_supply_center() {
            return false;
        }

        // Line 239-240: Unit must be available for supplying
        if !obj.is_available_for_supplying() {
            return false;
        }

        // Line 248-256: Shroud check - INTENTIONALLY DIFFERENT from most commands
        // Fogged is okay for player, shrouded is not, anything is okay for AI
        if obj.get_controlling_player_type() == PlayerType::Human {
            if transfer_dest.get_shrouded_status_for_player(obj.get_controlling_player_id())
                == ObjectShroudStatus::Shrouded
            {
                return false;
            }
        }

        // Line 259: All checks passed
        true
    }

    /// Check if object can dock at destination
    /// Reference: C++ ActionManager.cpp lines 266-303
    pub fn can_dock_at(
        &self,
        obj: &Object,
        dock_dest: &Object,
        _command_source: CommandSourceType,
    ) -> bool {
        // Line 276-277: Must have dock update interface
        if !dock_dest.has_dock_update_interface() {
            return false;
        }

        // Line 286-287: Supply transfer is valid docking
        if self.can_transfer_supplies_at(obj, dock_dest) {
            return true;
        }

        // Line 290-298: Railed transport docking
        if dock_dest.is_railed_transport_dock() {
            if obj.is_kind_of_vehicle() || obj.is_kind_of_infantry() {
                return true;
            }
        }

        // Line 301: Cannot dock
        false
    }

    /// Check if object can get healed at destination
    /// Reference: C++ ActionManager.cpp lines 307-355
    pub fn can_get_healed_at(
        &self,
        obj: &Object,
        heal_dest: &Object,
        command_source: CommandSourceType,
    ) -> bool {
        // Line 311-312: Sanity check
        if obj.is_null() || heal_dest.is_null() {
            return false;
        }

        // Line 314-318: Only allies
        if obj.get_relationship(heal_dest) != Relationship::Allies {
            return false;
        }

        // Line 321-322: Can't heal dead objects
        if heal_dest.is_effectively_dead() {
            return false;
        }

        // Line 325-327: Nothing under construction
        if obj.test_status_under_construction() || heal_dest.test_status_under_construction() {
            return false;
        }

        // Line 330-331: Can't heal at something being sold
        if heal_dest.test_status_sold() {
            return false;
        }

        // Line 334-335: Only infantry can be healed
        if !obj.is_kind_of_infantry() {
            return false;
        }

        // Line 338-339: Must be heal pad
        if !heal_dest.is_kind_of_heal_pad() {
            return false;
        }

        // Line 342-343: Check shroud
        if is_object_shrouded_for_action(obj, heal_dest, command_source) {
            return false;
        }

        // Line 345-350: No point healing if full health
        if obj.get_health() == obj.get_max_health() {
            return false;
        }

        // Line 353: All checks passed
        true
    }

    /// Check if object can repair another object
    /// Reference: C++ ActionManager.cpp lines 359-424
    pub fn can_repair_object(
        &self,
        obj: &Object,
        object_to_repair: &Object,
        command_source: CommandSourceType,
    ) -> bool {
        // Line 363-364: Sanity check
        if obj.is_null() || object_to_repair.is_null() {
            return false;
        }

        // Line 366-372: Can only repair non-enemies
        if obj.get_relationship(object_to_repair) == Relationship::Enemies {
            return false;
        }

        // Line 379-382: Can't repair dead things
        if object_to_repair.is_effectively_dead() {
            return false;
        }

        // Line 385-386: Can't repair bridges (feature was cut)
        if object_to_repair.is_kind_of_bridge() || object_to_repair.is_kind_of_bridge_tower() {
            return false;
        }

        // Line 389-391: Nothing under construction
        if obj.test_status_under_construction() || object_to_repair.test_status_under_construction()
        {
            return false;
        }

        // Line 394-395: Can't repair rebuild holes
        if object_to_repair.is_kind_of_rebuild_hole() {
            return false;
        }

        // Line 398-399: Only dozers can repair
        if !obj.is_kind_of_dozer() {
            return false;
        }

        // Line 402-403: Dozers only repair buildings
        if !object_to_repair.is_kind_of_structure() {
            return false;
        }

        // Line 409-410: Can't repair if at full health
        if object_to_repair.get_health() == object_to_repair.get_max_health() {
            return false;
        }

        // Line 413-414: Check shroud
        if is_object_shrouded_for_action(obj, object_to_repair, command_source) {
            return false;
        }

        // Line 416-420: Can't repair while in transport
        if obj.is_contained() {
            return false;
        }

        // Line 422: All checks passed
        true
    }

    /// Can `obj` pick up the surrendered `prisoner` (C++ ActionManager::canPickUpPrisoner).
    pub fn can_pick_up_prisoner(
        &self,
        obj: &Object,
        prisoner: &Object,
        _command_source: CommandSourceType,
    ) -> bool {
        if obj.is_null() || prisoner.is_null() {
            return false;
        }

        if !obj.is_kind_of_pow_truck() {
            return false;
        }

        if !prisoner.is_kind_of_infantry() {
            return false;
        }

        if prisoner.is_contained() {
            return false;
        }

        if !prisoner.is_surrendered() {
            return false;
        }

        if let Some(surrendered_to_player) = prisoner.get_surrendered_player_index() {
            if obj.get_controlling_player_id() != surrendered_to_player {
                return false;
            }
        }

        if obj.get_relationship(prisoner) != Relationship::Enemies {
            return false;
        }

        true
    }

    /// Queue an action for later execution
    /// Reference: C++ would queue actions in command system
    pub fn queue_action(&mut self, action: ActionType, player_index: u32) {
        if self.action_queue.len() >= self.max_queue_size {
            // Queue is full, remove oldest action
            self.action_queue.pop_front();
        }

        let queued = QueuedAction {
            action,
            player_index,
            timestamp: self.current_frame,
        };

        self.action_queue.push_back(queued);
    }

    /// Execute a player action immediately
    /// This is a simplified version - the real implementation would be much more complex
    pub fn execute_action(&mut self, player: &Player, action: ActionType, options: u32) -> bool {
        if let Some(executor) = &self.action_executor {
            executor.execute(player.player_index, action, options)
        } else {
            log::warn!(
                "execute_action called without executor for player {}",
                player.player_index
            );
            false
        }
    }

    /// Register a game logic executor for action execution
    pub fn set_action_executor(&mut self, executor: Arc<dyn ActionExecutor>) {
        self.action_executor = Some(executor);
    }

    /// Get number of queued actions
    pub fn get_queue_size(&self) -> usize {
        self.action_queue.len()
    }

    /// Clear all queued actions
    pub fn clear_queue(&mut self) {
        self.action_queue.clear();
    }
}

impl Default for ActionManager {
    fn default() -> Self {
        Self::new()
    }
}

// ================================================================================================
// HELPER FUNCTIONS
// ================================================================================================

/// Check if target is shrouded for action
/// Reference: C++ ActionManager.cpp lines 76-102
fn is_object_shrouded_for_action(
    source: &Object,
    target: &Object,
    command_source: CommandSourceType,
) -> bool {
    // Line 84-89: Target is only shrouded if:
    // - Source player is human
    // - Command is not from script
    // - Target is fogged or worse

    if source.is_null() || target.is_null() {
        return false;
    }

    // Line 92-94: Check if human player and not from script
    if source.get_controlling_player_type() == PlayerType::Human
        && command_source != CommandSourceType::FromScript
    {
        // Line 94: Check if target is fogged or shrouded
        let shroud_status =
            target.get_shrouded_status_for_player(source.get_controlling_player_id());
        if shroud_status >= ObjectShroudStatus::Fogged {
            return true;
        }
    }

    // Line 101: Not shrouded
    false
}

/// Check if object appears to contain friendlies (stealth trick)
/// Reference: C++ ActionManager.cpp lines 56-72
#[allow(dead_code)] // C++ parity: duplicated in GameLogic/src/action_manager.rs which is the active copy
fn appears_to_contain_friendlies(obj: &Object, other_object: &Object) -> bool {
    // Line 60-70: Check if container has stealth units tricking player
    if other_object.has_contain_module() {
        let apparent_player =
            other_object.get_apparent_controlling_player(obj.get_controlling_player_id());

        if apparent_player.is_some() {
            let relationship = obj.get_team_relationship(apparent_player.unwrap());
            if relationship != Relationship::Enemies {
                return true;
            }
        }
    }

    // Line 71: Does not appear to contain friendlies
    false
}

// ================================================================================================
// OBJECT TRAIT EXTENSIONS
// ================================================================================================

// ================================================================================================
// OBJECT QUERY METHODS - Delegated to ObjectDataProvider
// ================================================================================================

impl Object {
    #[inline]
    fn is_null(&self) -> bool {
        self.with_provider(|p| !p.is_valid_object(self.handle), true)
    }

    #[inline]
    fn get_relationship(&self, other: &Object) -> Relationship {
        self.with_provider(
            |p| p.get_relationship(self.handle, other.handle),
            Relationship::Neutral,
        )
    }

    #[inline]
    fn get_team_relationship(&self, player_id: u32) -> Relationship {
        self.with_provider(
            |p| p.get_team_relationship(self.handle, player_id),
            Relationship::Neutral,
        )
    }

    #[inline]
    fn is_effectively_dead(&self) -> bool {
        self.with_provider(|p| p.is_effectively_dead(self.handle), true)
    }

    #[inline]
    fn is_mobile(&self) -> bool {
        self.with_provider(|p| p.is_mobile(self.handle), false)
    }

    #[inline]
    fn test_status_under_construction(&self) -> bool {
        self.with_provider(
            |p| p.test_status(self.handle, status_bit::UNDER_CONSTRUCTION),
            false,
        )
    }

    #[inline]
    fn test_status_sold(&self) -> bool {
        self.with_provider(
            |p| p.test_status(self.handle, status_bit::SOLD),
            false,
        )
    }

    #[inline]
    fn is_kind_of_vehicle(&self) -> bool {
        self.with_provider(|p| p.is_kind_of(self.handle, kind_of_bit::VEHICLE), false)
    }

    #[inline]
    fn is_kind_of_aircraft(&self) -> bool {
        self.with_provider(|p| p.is_kind_of(self.handle, kind_of_bit::AIRCRAFT), false)
    }

    #[inline]
    fn is_kind_of_infantry(&self) -> bool {
        self.with_provider(|p| p.is_kind_of(self.handle, kind_of_bit::INFANTRY), false)
    }

    #[inline]
    fn is_kind_of_pow_truck(&self) -> bool {
        self.with_provider(|p| p.is_kind_of(self.handle, kind_of_bit::POW_TRUCK), false)
    }

    #[inline]
    fn is_kind_of_structure(&self) -> bool {
        self.with_provider(|p| p.is_kind_of(self.handle, kind_of_bit::STRUCTURE), false)
    }

    #[inline]
    fn is_kind_of_dozer(&self) -> bool {
        self.with_provider(|p| p.is_kind_of(self.handle, kind_of_bit::DOZER), false)
    }

    #[inline]
    fn is_kind_of_repair_pad(&self) -> bool {
        self.with_provider(|p| p.is_kind_of(self.handle, kind_of_bit::REPAIR_PAD), false)
    }

    #[inline]
    fn is_kind_of_airfield(&self) -> bool {
        self.with_provider(|p| p.is_kind_of(self.handle, kind_of_bit::FS_AIRFIELD), false)
    }

    #[inline]
    fn is_kind_of_heal_pad(&self) -> bool {
        self.with_provider(|p| p.is_kind_of(self.handle, kind_of_bit::HEAL_PAD), false)
    }

    #[inline]
    fn is_kind_of_bridge(&self) -> bool {
        self.with_provider(|p| p.is_kind_of(self.handle, kind_of_bit::BRIDGE), false)
    }

    #[inline]
    fn is_kind_of_bridge_tower(&self) -> bool {
        self.with_provider(|p| p.is_kind_of(self.handle, kind_of_bit::BRIDGE_TOWER), false)
    }

    #[inline]
    fn is_kind_of_rebuild_hole(&self) -> bool {
        self.with_provider(|p| p.is_kind_of(self.handle, kind_of_bit::REBUILD_HOLE), false)
    }

    #[inline]
    fn is_above_terrain(&self) -> bool {
        self.with_provider(|p| p.is_above_terrain(self.handle), false)
    }

    #[inline]
    fn get_health(&self) -> f32 {
        self.with_provider(|p| p.get_health(self.handle), 100.0)
    }

    #[inline]
    fn get_max_health(&self) -> f32 {
        self.with_provider(|p| p.get_max_health(self.handle), 100.0)
    }

    #[inline]
    fn get_controlling_player_id(&self) -> u32 {
        self.with_provider(
            |p| p.get_controlling_player_id(self.handle).unwrap_or(0),
            0,
        )
    }

    #[inline]
    fn get_controlling_player_type(&self) -> PlayerType {
        self.with_provider(
            |p| p.get_controlling_player_type(self.handle),
            PlayerType::Human,
        )
    }

    #[inline]
    fn get_shrouded_status_for_player(&self, player_index: u32) -> ObjectShroudStatus {
        self.with_provider(
            |p| p.get_shrouded_status(self.handle, player_index),
            ObjectShroudStatus::Clear,
        )
    }

    #[inline]
    fn has_supply_truck_ai(&self) -> bool {
        self.with_provider(|p| p.has_supply_truck_ai(self.handle), false)
    }

    #[inline]
    fn is_supply_warehouse(&self) -> bool {
        self.with_provider(|p| p.is_supply_warehouse(self.handle), false)
    }

    #[inline]
    fn is_supply_center(&self) -> bool {
        self.with_provider(|p| p.is_supply_center(self.handle), false)
    }

    #[inline]
    fn get_warehouse_boxes(&self) -> u32 {
        self.with_provider(|p| p.get_warehouse_boxes(self.handle), 0)
    }

    #[inline]
    fn get_supply_boxes(&self) -> u32 {
        self.with_provider(|p| p.get_supply_boxes(self.handle), 0)
    }

    #[inline]
    fn is_available_for_supplying(&self) -> bool {
        self.with_provider(|p| p.is_available_for_supplying(self.handle), false)
    }

    #[inline]
    fn has_dock_update_interface(&self) -> bool {
        self.with_provider(|p| p.has_dock_update_interface(self.handle), false)
    }

    #[inline]
    fn is_railed_transport_dock(&self) -> bool {
        self.with_provider(|p| p.is_railed_transport_dock(self.handle), false)
    }

    #[inline]
    fn is_contained(&self) -> bool {
        self.with_provider(|p| p.is_contained(self.handle), false)
    }

    #[inline]
    fn is_surrendered(&self) -> bool {
        self.with_provider(|p| p.is_surrendered(self.handle), false)
    }

    #[inline]
    fn get_surrendered_player_index(&self) -> Option<u32> {
        self.with_provider(
            |p| p.get_surrendered_player_index(self.handle),
            None,
        )
    }

    #[inline]
    fn has_contain_module(&self) -> bool {
        self.with_provider(|p| p.has_contain_module(self.handle), false)
    }

    #[inline]
    fn get_apparent_controlling_player(&self, viewer_player_id: u32) -> Option<u32> {
        self.with_provider(
            |p| p.get_apparent_controlling_player(self.handle, viewer_player_id),
            None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_manager_creation() {
        let manager = ActionManager::new();
        assert_eq!(manager.get_queue_size(), 0);
    }

    #[test]
    fn test_queue_action() {
        let mut manager = ActionManager::new();

        let action = ActionType::Move {
            target_pos: Coord3D {
                x: 100.0,
                y: 100.0,
                z: 0.0,
            },
        };

        manager.queue_action(action, 0);
        assert_eq!(manager.get_queue_size(), 1);
    }

    #[test]
    fn test_clear_queue() {
        let mut manager = ActionManager::new();

        let action = ActionType::Stop;
        manager.queue_action(action.clone(), 0);
        manager.queue_action(action, 0);

        assert_eq!(manager.get_queue_size(), 2);

        manager.clear_queue();
        assert_eq!(manager.get_queue_size(), 0);
    }

    #[test]
    fn test_update_processes_actions() {
        let mut manager = ActionManager::new();

        let action = ActionType::Stop;
        manager.queue_action(action, 0);

        // Action is queued at frame 0, should execute on update
        manager.update();

        // Action should have been processed
        assert_eq!(manager.get_queue_size(), 0);
    }

    #[test]
    fn test_command_source_type_values() {
        assert_eq!(CommandSourceType::FromPlayer as u32, 0);
        assert_eq!(CommandSourceType::FromScript as u32, 1);
        assert_eq!(CommandSourceType::FromAi as u32, 2);
    }

    #[test]
    fn test_can_attack_result_ordering() {
        assert!(CanAttackResult::Possible > CanAttackResult::NotPossible);
        assert!(CanAttackResult::PossibleAfterMoving > CanAttackResult::InvalidShot);
        assert!(CanAttackResult::Possible > CanAttackResult::PossibleAfterMoving);
    }

    #[test]
    fn test_object_from_handle() {
        let obj = Object::from_id(42);
        assert_eq!(obj.id(), 42);
        assert!(obj.handle().is_valid());
    }

    #[test]
    fn test_object_null_without_provider() {
        // Without a data provider registered, is_null returns true (fallback)
        // since the provider can't validate the handle.
        let obj = Object::from_id(1);
        assert!(obj.is_null());
    }

    #[test]
    fn test_object_fallback_defaults_without_provider() {
        // Without a provider, all methods return safe fallback values.
        let obj = Object::from_id(99);
        assert_eq!(obj.get_relationship(&obj), Relationship::Neutral);
        assert!(obj.is_effectively_dead());
        assert!(!obj.is_mobile());
        assert!(!obj.test_status_under_construction());
        assert!(!obj.test_status_sold());
        assert!(!obj.is_kind_of_vehicle());
        assert!(!obj.is_kind_of_aircraft());
        assert!(!obj.is_kind_of_infantry());
        assert!(!obj.is_kind_of_pow_truck());
        assert!(!obj.is_kind_of_structure());
        assert!(!obj.is_kind_of_dozer());
        assert!(!obj.is_kind_of_repair_pad());
        assert!(!obj.is_kind_of_airfield());
        assert!(!obj.is_kind_of_heal_pad());
        assert!(!obj.is_kind_of_bridge());
        assert!(!obj.is_kind_of_bridge_tower());
        assert!(!obj.is_kind_of_rebuild_hole());
        assert!(!obj.is_above_terrain());
        assert_eq!(obj.get_health(), 100.0);
        assert_eq!(obj.get_max_health(), 100.0);
        assert_eq!(obj.get_controlling_player_id(), 0);
        assert_eq!(obj.get_controlling_player_type(), PlayerType::Human);
        assert_eq!(
            obj.get_shrouded_status_for_player(0),
            ObjectShroudStatus::Clear
        );
        assert!(!obj.has_supply_truck_ai());
        assert!(!obj.is_supply_warehouse());
        assert!(!obj.is_supply_center());
        assert_eq!(obj.get_warehouse_boxes(), 0);
        assert_eq!(obj.get_supply_boxes(), 0);
        assert!(!obj.is_available_for_supplying());
        assert!(!obj.has_dock_update_interface());
        assert!(!obj.is_railed_transport_dock());
        assert!(!obj.is_contained());
        assert!(!obj.is_surrendered());
        assert_eq!(obj.get_surrendered_player_index(), None);
        assert!(!obj.has_contain_module());
        assert_eq!(obj.get_apparent_controlling_player(0), None);
    }
}
