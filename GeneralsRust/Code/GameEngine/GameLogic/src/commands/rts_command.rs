////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! RTS Command System - Real-time strategy specific commands
//!
//! This module provides RTS-specific command types and functionality,
//! extending the base command system for strategy game operations.
//! Matches C++ RTSCommand system and ControlBar command processing.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::command::{Command, CommandType, CommandValidation};
use crate::common::{
    AsciiString, Bool, Coord3D, DrawableID, ICoord2D, IRegion2D, Int, ObjectID, PlayerMaskType,
    Real, UnicodeString, UnsignedInt, LOGICFRAMES_PER_SECOND,
};
use crate::object::registry::OBJECT_REGISTRY;

/// RTS-specific command categories for organization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RtsCommandCategory {
    Movement,
    Combat,
    Construction,
    Production,
    SpecialPower,
    Formation,
    Selection,
    Interface,
    Economy,
    Diplomacy,
}

/// Command execution context for RTS operations
#[derive(Debug, Clone)]
pub struct RtsCommandContext {
    /// Current game frame
    pub current_frame: UnsignedInt,

    /// Player issuing the command
    pub player_id: Int,

    /// Currently selected objects
    pub selected_objects: Vec<ObjectID>,

    /// Current cursor mode (normal, force attack, etc.)
    pub cursor_mode: CursorMode,

    /// Modifier keys held (Ctrl, Shift, Alt)
    pub modifier_keys: ModifierKeys,

    /// Whether player is in waypoint mode
    pub waypoint_mode: bool,

    /// Current rally point for production buildings
    pub rally_point: Option<Coord3D>,

    /// Current formation settings
    pub formation_settings: FormationSettings,
}

/// Cursor interaction modes - matches C++ cursor system
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorMode {
    Normal,
    ForceAttack,
    ForceMove,
    PlaceBeacon,
    Waypoint,
    PathBuild,
    SpecialPower,
    Repair,
    Guard,
}

/// Modifier key flags - matches C++ input system
#[derive(Debug, Clone, Copy, Default)]
pub struct ModifierKeys {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl ModifierKeys {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_ctrl(mut self, pressed: bool) -> Self {
        self.ctrl = pressed;
        self
    }

    pub fn with_shift(mut self, pressed: bool) -> Self {
        self.shift = pressed;
        self
    }

    pub fn with_alt(mut self, pressed: bool) -> Self {
        self.alt = pressed;
        self
    }

    pub fn has_any(&self) -> bool {
        self.ctrl || self.shift || self.alt
    }
}

/// Formation settings for group commands
#[derive(Debug, Clone)]
pub struct FormationSettings {
    pub formation_id: Option<UnsignedInt>,
    pub spacing: Real,
    pub orientation: Real,
    pub maintain_formation: bool,
}

impl Default for FormationSettings {
    fn default() -> Self {
        Self {
            formation_id: None,
            spacing: 50.0,
            orientation: 0.0,
            maintain_formation: false,
        }
    }
}

/// RTS Command wrapper - extends base Command with RTS-specific functionality
#[derive(Debug, Clone)]
pub struct RtsCommand {
    /// Base command
    pub base_command: Command,

    /// RTS command category
    pub category: RtsCommandCategory,

    /// Execution context when command was created
    pub context: RtsCommandContext,

    /// Whether this is a queued command (Shift+click)
    pub is_queued: bool,

    /// Whether this replaces existing commands
    pub replaces_existing: bool,

    /// Cost in resources (if applicable)
    pub resource_cost: Option<ResourceCost>,

    /// Prerequisites required (if applicable)
    pub prerequisites: Vec<AsciiString>,

    /// Estimated execution time
    pub estimated_duration: Option<UnsignedInt>,
}

/// Resource cost for commands that require resources
#[derive(Debug, Clone)]
pub struct ResourceCost {
    pub supplies: Int,
    pub power: Int,
    pub command_points: Int,
}

impl RtsCommand {
    /// Create new RTS command from base command
    pub fn new(
        base_command: Command,
        category: RtsCommandCategory,
        context: RtsCommandContext,
    ) -> Self {
        // Determine if command should be queued based on modifiers
        let is_queued = context.modifier_keys.shift && category != RtsCommandCategory::Selection;

        // Determine if command replaces existing commands
        let replaces_existing = !is_queued
            && matches!(
                category,
                RtsCommandCategory::Movement | RtsCommandCategory::Combat
            );

        Self {
            base_command,
            category,
            context,
            is_queued,
            replaces_existing,
            resource_cost: None,
            prerequisites: Vec::new(),
            estimated_duration: None,
        }
    }

    /// Get the base command type
    pub fn get_command_type(&self) -> CommandType {
        self.base_command.get_type()
    }

    /// Get command category
    pub fn get_category(&self) -> RtsCommandCategory {
        self.category
    }

    /// Check if this command affects the given object
    pub fn affects_object(&self, object_id: ObjectID) -> bool {
        // Check if object is in the command arguments
        for i in 0..self.base_command.get_argument_count() {
            if let Some(arg) = self.base_command.get_argument(i as Int) {
                match arg {
                    crate::commands::command::CommandArgumentType::ObjectID(id) => {
                        if *id == object_id {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
        }
        false
    }

    /// Get all object IDs affected by this command
    pub fn get_affected_objects(&self) -> Vec<ObjectID> {
        let mut objects = Vec::new();

        for i in 0..self.base_command.get_argument_count() {
            if let Some(arg) = self.base_command.get_argument(i as Int) {
                match arg {
                    crate::commands::command::CommandArgumentType::ObjectID(id) => {
                        objects.push(*id);
                    }
                    _ => {}
                }
            }
        }

        objects
    }

    /// Set resource cost for this command
    pub fn set_resource_cost(&mut self, cost: ResourceCost) {
        self.resource_cost = Some(cost);
    }

    /// Add prerequisite for this command
    pub fn add_prerequisite(&mut self, prerequisite: AsciiString) {
        self.prerequisites.push(prerequisite);
    }

    /// Set estimated execution duration in frames
    pub fn set_estimated_duration(&mut self, frames: UnsignedInt) {
        self.estimated_duration = Some(frames);
    }
}

/// RTS Command Factory - creates properly formatted RTS commands
pub struct RtsCommandFactory {
    /// Default context for command creation
    default_context: RtsCommandContext,
}

impl RtsCommandFactory {
    pub fn new() -> Self {
        Self {
            default_context: RtsCommandContext {
                current_frame: 0,
                player_id: 0,
                selected_objects: Vec::new(),
                cursor_mode: CursorMode::Normal,
                modifier_keys: ModifierKeys::new(),
                waypoint_mode: false,
                rally_point: None,
                formation_settings: FormationSettings::default(),
            },
        }
    }

    /// Update the default context
    pub fn update_context(&mut self, context: RtsCommandContext) {
        self.default_context = context;
    }

    /// Create movement command - matches C++ move command creation
    pub fn create_move_command(
        &self,
        position: Coord3D,
        objects: Option<Vec<ObjectID>>,
    ) -> RtsCommand {
        let objects_to_move =
            objects.unwrap_or_else(|| self.default_context.selected_objects.clone());

        let mut base_command = Command::new(CommandType::DoMoveTo);
        base_command.set_player_index(self.default_context.player_id);
        base_command.append_location_argument(position);

        // Add all objects to move
        for object_id in &objects_to_move {
            base_command.append_object_id_argument(*object_id);
        }

        let mut rts_command = RtsCommand::new(
            base_command,
            RtsCommandCategory::Movement,
            self.default_context.clone(),
        );
        rts_command.estimated_duration = Some(self.estimate_move_time(&objects_to_move, position));

        rts_command
    }

    /// Create attack command - matches C++ attack command creation
    pub fn create_attack_command(
        &self,
        target: ObjectID,
        attackers: Option<Vec<ObjectID>>,
    ) -> RtsCommand {
        let attacking_objects =
            attackers.unwrap_or_else(|| self.default_context.selected_objects.clone());

        let mut base_command = Command::new(CommandType::DoAttackObject);
        base_command.set_player_index(self.default_context.player_id);
        base_command.append_object_id_argument(target);

        // Add all attacking objects
        for object_id in &attacking_objects {
            base_command.append_object_id_argument(*object_id);
        }

        RtsCommand::new(
            base_command,
            RtsCommandCategory::Combat,
            self.default_context.clone(),
        )
    }

    /// Create force attack ground command
    pub fn create_force_attack_ground_command(
        &self,
        position: Coord3D,
        attackers: Option<Vec<ObjectID>>,
    ) -> RtsCommand {
        let attacking_objects =
            attackers.unwrap_or_else(|| self.default_context.selected_objects.clone());

        let mut base_command = Command::new(CommandType::DoForceAttackGround);
        base_command.set_player_index(self.default_context.player_id);
        base_command.append_location_argument(position);

        for object_id in &attacking_objects {
            base_command.append_object_id_argument(*object_id);
        }

        RtsCommand::new(
            base_command,
            RtsCommandCategory::Combat,
            self.default_context.clone(),
        )
    }

    /// Create build structure command - matches C++ construction system
    pub fn create_build_command(
        &self,
        builder: ObjectID,
        building_type: AsciiString,
        position: Coord3D,
    ) -> RtsCommand {
        let mut base_command = Command::new(CommandType::DozerConstruct);
        base_command.set_player_index(self.default_context.player_id);
        base_command.append_object_id_argument(builder);
        base_command.append_location_argument(position);

        let mut rts_command = RtsCommand::new(
            base_command,
            RtsCommandCategory::Construction,
            self.default_context.clone(),
        );

        let derived_cost = crate::template::find_template(&building_type)
            .map(|template| template.get_build_cost())
            .unwrap_or(1000);
        let derived_time = crate::template::find_template(&building_type)
            .map(|template| template.calc_time_to_build(None))
            .filter(|frames| *frames > 0)
            .map(|frames| frames as UnsignedInt)
            .unwrap_or(900);

        // Set typical construction cost and duration
        rts_command.set_resource_cost(ResourceCost {
            supplies: derived_cost,
            power: 0,
            command_points: 0,
        });
        rts_command.set_estimated_duration(derived_time);

        rts_command
    }

    /// Create unit production command
    pub fn create_produce_unit_command(
        &self,
        factory: ObjectID,
        unit_type: AsciiString,
        count: Int,
    ) -> RtsCommand {
        let mut base_command = Command::new(CommandType::QueueUnitCreate);
        base_command.set_player_index(self.default_context.player_id);
        base_command.append_object_id_argument(factory);
        base_command.append_integer_argument(count);

        let mut rts_command = RtsCommand::new(
            base_command,
            RtsCommandCategory::Production,
            self.default_context.clone(),
        );

        let derived_unit_cost = crate::template::find_template(&unit_type)
            .map(|template| template.get_build_cost())
            .unwrap_or(600);
        let count_clamped = count.max(1) as UnsignedInt;
        let derived_unit_time = crate::template::find_template(&unit_type)
            .map(|template| template.calc_time_to_build(None))
            .filter(|frames| *frames > 0)
            .map(|frames| frames as UnsignedInt)
            .unwrap_or(15 * 30);
        let derived_total_time = derived_unit_time.saturating_mul(count_clamped);

        // Set typical unit cost
        rts_command.set_resource_cost(ResourceCost {
            supplies: derived_unit_cost.saturating_mul(count),
            power: 0,
            command_points: 1 * count,
        });
        rts_command.set_estimated_duration(derived_total_time);

        rts_command
    }

    /// Create special power command
    pub fn create_special_power_command(
        &self,
        power_type: AsciiString,
        target_position: Option<Coord3D>,
        target_object: Option<ObjectID>,
    ) -> RtsCommand {
        use crate::common::INVALID_OBJECT_ID;
        use crate::object::special_power_template::get_special_power_store;
        use crate::object_creation_list::nuggets::INVALID_ANGLE;

        let mut base_command = if target_position.is_some() {
            Command::new(CommandType::DoSpecialPowerAtLocation)
        } else if target_object.is_some() {
            Command::new(CommandType::DoSpecialPowerAtObject)
        } else {
            Command::new(CommandType::DoSpecialPower)
        };

        base_command.set_player_index(self.default_context.player_id);

        let power_id = get_special_power_store()
            .and_then(|store| {
                store
                    .find_special_power_template(power_type.as_str())
                    .map(|t| t.get_id())
            })
            .unwrap_or(0);
        let options = 0;
        let source_id = INVALID_OBJECT_ID;

        base_command.append_integer_argument(power_id as Int);

        if let Some(pos) = target_position {
            base_command.append_location_argument(pos);
            base_command.append_real_argument(INVALID_ANGLE);
            base_command.append_object_id_argument(INVALID_OBJECT_ID);
            base_command.append_integer_argument(options);
            base_command.append_object_id_argument(source_id);
        }

        if let Some(obj) = target_object {
            base_command.append_object_id_argument(obj);
            base_command.append_integer_argument(options);
            base_command.append_object_id_argument(source_id);
        }

        if target_position.is_none() && target_object.is_none() {
            base_command.append_integer_argument(options);
            base_command.append_object_id_argument(source_id);
        }

        RtsCommand::new(
            base_command,
            RtsCommandCategory::SpecialPower,
            self.default_context.clone(),
        )
    }

    /// Create area selection command
    pub fn create_area_selection_command(&self, selection_region: IRegion2D) -> RtsCommand {
        let mut base_command = Command::new(CommandType::AreaSelection);
        base_command.set_player_index(self.default_context.player_id);
        base_command.append_pixel_region_argument(selection_region);

        RtsCommand::new(
            base_command,
            RtsCommandCategory::Selection,
            self.default_context.clone(),
        )
    }

    /// Create stop command
    pub fn create_stop_command(&self, objects: Option<Vec<ObjectID>>) -> RtsCommand {
        let objects_to_stop =
            objects.unwrap_or_else(|| self.default_context.selected_objects.clone());

        let mut base_command = Command::new(CommandType::DoStop);
        base_command.set_player_index(self.default_context.player_id);

        for object_id in &objects_to_stop {
            base_command.append_object_id_argument(*object_id);
        }

        RtsCommand::new(
            base_command,
            RtsCommandCategory::Movement,
            self.default_context.clone(),
        )
    }

    /// Create scatter command
    pub fn create_scatter_command(&self, objects: Option<Vec<ObjectID>>) -> RtsCommand {
        let objects_to_scatter =
            objects.unwrap_or_else(|| self.default_context.selected_objects.clone());

        let mut base_command = Command::new(CommandType::DoScatter);
        base_command.set_player_index(self.default_context.player_id);

        for object_id in &objects_to_scatter {
            base_command.append_object_id_argument(*object_id);
        }

        RtsCommand::new(
            base_command,
            RtsCommandCategory::Movement,
            self.default_context.clone(),
        )
    }

    /// Create guard command
    pub fn create_guard_command(
        &self,
        guard_position: Option<Coord3D>,
        guard_object: Option<ObjectID>,
        guards: Option<Vec<ObjectID>>,
    ) -> RtsCommand {
        let guarding_objects =
            guards.unwrap_or_else(|| self.default_context.selected_objects.clone());

        let mut base_command = if guard_object.is_some() {
            Command::new(CommandType::DoGuardObject)
        } else {
            Command::new(CommandType::DoGuardPosition)
        };

        base_command.set_player_index(self.default_context.player_id);

        if let Some(pos) = guard_position {
            base_command.append_location_argument(pos);
        }

        if let Some(obj) = guard_object {
            base_command.append_object_id_argument(obj);
        }

        for object_id in &guarding_objects {
            base_command.append_object_id_argument(*object_id);
        }

        RtsCommand::new(
            base_command,
            RtsCommandCategory::Combat,
            self.default_context.clone(),
        )
    }

    /// Create rally point command
    pub fn create_set_rally_point_command(
        &self,
        factory: ObjectID,
        rally_position: Coord3D,
    ) -> RtsCommand {
        let mut base_command = Command::new(CommandType::SetRallyPoint);
        base_command.set_player_index(self.default_context.player_id);
        base_command.append_object_id_argument(factory);
        base_command.append_location_argument(rally_position);

        RtsCommand::new(
            base_command,
            RtsCommandCategory::Production,
            self.default_context.clone(),
        )
    }

    /// Create sell structure command
    pub fn create_sell_command(&self, structure: ObjectID) -> RtsCommand {
        let mut base_command = Command::new(CommandType::Sell);
        base_command.set_player_index(self.default_context.player_id);
        base_command.append_object_id_argument(structure);

        RtsCommand::new(
            base_command,
            RtsCommandCategory::Economy,
            self.default_context.clone(),
        )
    }

    /// Estimate movement time for planning
    fn estimate_move_time(&self, objects: &[ObjectID], destination: Coord3D) -> UnsignedInt {
        let mut max_distance = 0.0f32;
        for &object_id in objects {
            let Some(dist) = OBJECT_REGISTRY.with_object(object_id, |obj_guard| {
                let pos = obj_guard.get_position();
                let dx = pos.x - destination.x;
                let dy = pos.y - destination.y;
                let dz = pos.z - destination.z;
                (dx * dx + dy * dy + dz * dz).sqrt()
            }) else {
                continue;
            };
            if dist > max_distance {
                max_distance = dist;
            }
        }

        let default_speed = 30.0f32; // World units per second (fallback).
        if max_distance <= f32::EPSILON || default_speed <= f32::EPSILON {
            return 0;
        }
        let seconds = max_distance / default_speed;
        (seconds * LOGICFRAMES_PER_SECOND as f32).ceil() as UnsignedInt
    }
}

impl Default for RtsCommandFactory {
    fn default() -> Self {
        Self::new()
    }
}

/// RTS Command Validator - validates RTS-specific constraints
pub struct RtsCommandValidator {
    /// Object ownership lookup
    object_owners: HashMap<ObjectID, Int>,

    /// Available resources per player
    player_resources: HashMap<Int, PlayerResources>,

    /// Object capabilities lookup
    object_capabilities: HashMap<ObjectID, ObjectCapabilities>,
}

/// Player resource state
#[derive(Debug, Clone)]
pub struct PlayerResources {
    pub supplies: Int,
    pub power_available: Int,
    pub power_used: Int,
    pub command_points_available: Int,
    pub command_points_used: Int,
}

/// Object capabilities and state
#[derive(Debug, Clone)]
pub struct ObjectCapabilities {
    pub can_move: bool,
    pub can_attack: bool,
    pub can_build: bool,
    pub can_produce: bool,
    pub can_repair: bool,
    pub is_alive: bool,
    pub is_controllable: bool,
}

impl RtsCommandValidator {
    pub fn new() -> Self {
        Self {
            object_owners: HashMap::new(),
            player_resources: HashMap::new(),
            object_capabilities: HashMap::new(),
        }
    }

    /// Validate RTS command with game state checks
    pub fn validate_rts_command(&self, command: &RtsCommand) -> CommandValidation {
        let mut player_id = command.base_command.get_player_index();
        if player_id == 0 {
            if let Some(object_id) = command.get_affected_objects().first().copied() {
                if let Some(owner) = self.object_owners.get(&object_id) {
                    player_id = *owner;
                }
            }
        }

        // Check player ownership of objects
        if !self.validate_object_ownership(player_id, command) {
            return CommandValidation::NotAllowed;
        }

        // Check resource requirements
        if let Some(cost) = &command.resource_cost {
            if !self.validate_resource_cost(player_id, cost) {
                return CommandValidation::InsufficientResources;
            }
        }

        // Check object capabilities
        if !self.validate_object_capabilities(command) {
            return CommandValidation::InvalidTarget;
        }

        // Category-specific validation
        match command.category {
            RtsCommandCategory::Movement => self.validate_movement_command(command),
            RtsCommandCategory::Combat => self.validate_combat_command(command),
            RtsCommandCategory::Construction => self.validate_construction_command(command),
            RtsCommandCategory::Production => self.validate_production_command(command),
            _ => CommandValidation::Valid,
        }
    }

    /// Update object ownership
    pub fn update_object_owner(&mut self, object_id: ObjectID, owner: Int) {
        self.object_owners.insert(object_id, owner);
    }

    /// Update player resources
    pub fn update_player_resources(&mut self, player_id: Int, resources: PlayerResources) {
        self.player_resources.insert(player_id, resources);
    }

    /// Update object capabilities
    pub fn update_object_capabilities(
        &mut self,
        object_id: ObjectID,
        capabilities: ObjectCapabilities,
    ) {
        self.object_capabilities.insert(object_id, capabilities);
    }

    /// Validate that player owns all objects in command
    fn validate_object_ownership(&self, player_id: Int, command: &RtsCommand) -> bool {
        let objects = command.get_affected_objects();
        for object_id in objects {
            if let Some(owner) = self.object_owners.get(&object_id) {
                if *owner != player_id {
                    return false;
                }
            } else {
                // Unknown object - probably invalid
                return false;
            }
        }
        true
    }

    /// Validate resource cost against player resources
    fn validate_resource_cost(&self, player_id: Int, cost: &ResourceCost) -> bool {
        if let Some(resources) = self.player_resources.get(&player_id) {
            resources.supplies >= cost.supplies
                && (resources.power_available - resources.power_used) >= cost.power
                && (resources.command_points_available - resources.command_points_used)
                    >= cost.command_points
        } else {
            false
        }
    }

    /// Validate object capabilities for command
    fn validate_object_capabilities(&self, command: &RtsCommand) -> bool {
        let objects = command.get_affected_objects();

        for object_id in objects {
            if let Some(capabilities) = self.object_capabilities.get(&object_id) {
                if !capabilities.is_alive || !capabilities.is_controllable {
                    return false;
                }

                // Check specific capabilities based on command type
                match command.get_command_type() {
                    CommandType::DoMoveTo
                    | CommandType::DoAttackMoveTo
                    | CommandType::DoForceMoveTo => {
                        if !capabilities.can_move {
                            return false;
                        }
                    }
                    CommandType::DoAttackObject
                    | CommandType::DoForceAttackObject
                    | CommandType::DoForceAttackGround => {
                        if !capabilities.can_attack {
                            return false;
                        }
                    }
                    CommandType::DozerConstruct => {
                        if !capabilities.can_build {
                            return false;
                        }
                    }
                    CommandType::QueueUnitCreate => {
                        if !capabilities.can_produce {
                            return false;
                        }
                    }
                    _ => {}
                }
            } else {
                return false;
            }
        }

        true
    }

    /// Validate movement command specifics
    fn validate_movement_command(&self, _command: &RtsCommand) -> CommandValidation {
        // Additional movement validation would go here:
        // - Check if destination is reachable
        // - Check for terrain constraints
        // - Check for no-go zones
        CommandValidation::Valid
    }

    /// Validate combat command specifics
    fn validate_combat_command(&self, _command: &RtsCommand) -> CommandValidation {
        // Additional combat validation would go here:
        // - Check if target is attackable
        // - Check weapon range
        // - Check line of sight
        CommandValidation::Valid
    }

    /// Validate construction command specifics
    fn validate_construction_command(&self, _command: &RtsCommand) -> CommandValidation {
        // Additional construction validation would go here:
        // - Check build prerequisites
        // - Check terrain suitability
        // - Check clearance requirements
        CommandValidation::Valid
    }

    /// Validate production command specifics
    fn validate_production_command(&self, _command: &RtsCommand) -> CommandValidation {
        // Additional production validation would go here:
        // - Check unit prerequisites
        // - Check factory availability
        // - Check population limits
        CommandValidation::Valid
    }
}

impl Default for RtsCommandValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::command::command_builder;

    #[test]
    fn test_rts_command_creation() {
        let factory = RtsCommandFactory::new();
        let position = [100.0, 200.0, 0.0];
        let objects = vec![1, 2, 3];

        let move_command = factory.create_move_command(position.into(), Some(objects));

        assert_eq!(move_command.get_category(), RtsCommandCategory::Movement);
        assert_eq!(move_command.get_command_type(), CommandType::DoMoveTo);
        assert_eq!(move_command.get_affected_objects().len(), 3);
    }

    #[test]
    fn test_modifier_keys() {
        let keys = ModifierKeys::new().with_ctrl(true).with_shift(false);

        assert!(keys.ctrl);
        assert!(!keys.shift);
        assert!(keys.has_any());
    }

    #[test]
    fn test_command_queuing() {
        let mut context = RtsCommandContext {
            current_frame: 100,
            player_id: 1,
            selected_objects: vec![1, 2],
            cursor_mode: CursorMode::Normal,
            modifier_keys: ModifierKeys::new().with_shift(true),
            waypoint_mode: false,
            rally_point: None,
            formation_settings: FormationSettings::default(),
        };

        let factory = RtsCommandFactory::new();
        // Update factory context
        let mut factory = factory;
        factory.update_context(context);

        let move_command = factory.create_move_command([50.0, 50.0, 0.0].into(), None);

        // Should be queued due to Shift key
        assert!(move_command.is_queued);
    }

    #[test]
    fn test_resource_validation() {
        let mut validator = RtsCommandValidator::new();

        // Set up player resources
        validator.update_player_resources(
            1,
            PlayerResources {
                supplies: 2000,
                power_available: 10,
                power_used: 5,
                command_points_available: 20,
                command_points_used: 15,
            },
        );

        // Set up object ownership
        validator.update_object_owner(100, 1);
        validator.update_object_capabilities(
            100,
            ObjectCapabilities {
                can_move: true,
                can_attack: true,
                can_build: true,
                can_produce: false,
                can_repair: false,
                is_alive: true,
                is_controllable: true,
            },
        );

        let factory = RtsCommandFactory::new();
        let mut build_command = factory.create_build_command(
            100,
            "Barracks".to_string().into(),
            Coord3D::new(0.0, 0.0, 0.0),
        );

        // Should pass validation with sufficient resources
        assert_eq!(
            validator.validate_rts_command(&build_command),
            CommandValidation::Valid
        );

        // Set cost too high
        build_command.set_resource_cost(ResourceCost {
            supplies: 5000, // More than player has
            power: 0,
            command_points: 0,
        });

        assert_eq!(
            validator.validate_rts_command(&build_command),
            CommandValidation::InsufficientResources
        );
    }
}
