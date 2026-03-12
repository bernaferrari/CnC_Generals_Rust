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

/// Relationship between objects/players
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relationship {
    Allies,
    Neutral,
    Enemies,
}

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
#[allow(dead_code)]
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
// FORWARD DECLARATIONS - Placeholder types
// ================================================================================================

/// 3D coordinate
#[derive(Debug, Clone, Copy)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Game object (unit, building, etc.)
/// This would be fully implemented in the game logic module
pub struct Object {
    // Placeholder fields
    id: u32,
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
#[allow(dead_code)]
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

/// Extension trait for Object to provide all the query methods
/// These would be implemented on the actual Object struct in the game logic module
impl Object {
    #[inline]
    fn is_null(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn get_relationship(&self, _other: &Object) -> Relationship {
        Relationship::Neutral // Placeholder
    }

    #[inline]
    fn get_team_relationship(&self, _player_id: u32) -> Relationship {
        Relationship::Neutral // Placeholder
    }

    #[inline]
    fn is_effectively_dead(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn is_mobile(&self) -> bool {
        true // Placeholder
    }

    #[inline]
    fn test_status_under_construction(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn test_status_sold(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn is_kind_of_vehicle(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn is_kind_of_aircraft(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn is_kind_of_infantry(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn is_kind_of_pow_truck(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn is_kind_of_structure(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn is_kind_of_dozer(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn is_kind_of_repair_pad(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn is_kind_of_airfield(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn is_kind_of_heal_pad(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn is_kind_of_bridge(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn is_kind_of_bridge_tower(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn is_kind_of_rebuild_hole(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn is_above_terrain(&self) -> bool {
        true // Placeholder
    }

    #[inline]
    fn get_health(&self) -> f32 {
        100.0 // Placeholder
    }

    #[inline]
    fn get_max_health(&self) -> f32 {
        100.0 // Placeholder
    }

    #[inline]
    fn get_controlling_player_id(&self) -> u32 {
        0 // Placeholder
    }

    #[inline]
    fn get_controlling_player_type(&self) -> PlayerType {
        PlayerType::Human // Placeholder
    }

    #[inline]
    fn get_shrouded_status_for_player(&self, _player_index: u32) -> ObjectShroudStatus {
        ObjectShroudStatus::Clear // Placeholder
    }

    #[inline]
    fn has_supply_truck_ai(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn is_supply_warehouse(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn is_supply_center(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn get_warehouse_boxes(&self) -> u32 {
        0 // Placeholder
    }

    #[inline]
    fn get_supply_boxes(&self) -> u32 {
        0 // Placeholder
    }

    #[inline]
    fn is_available_for_supplying(&self) -> bool {
        true // Placeholder
    }

    #[inline]
    fn has_dock_update_interface(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn is_railed_transport_dock(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn is_contained(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn is_surrendered(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn get_surrendered_player_index(&self) -> Option<u32> {
        None // Placeholder
    }

    #[inline]
    fn has_contain_module(&self) -> bool {
        false // Placeholder
    }

    #[inline]
    fn get_apparent_controlling_player(&self, _viewer_player_id: u32) -> Option<u32> {
        None // Placeholder
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
}
