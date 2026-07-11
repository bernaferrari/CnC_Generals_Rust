use crate::game_logic::{AIState, BuildingType, GameLogic, KindOf, Object, ObjectId, Team};
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::f32::consts::TAU;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

const DOUBLE_CLICK_THRESHOLD: Duration = Duration::from_millis(250);

fn screen_to_world(screen: Vec2, viewport_size: Vec2, world_min: Vec3, world_max: Vec3) -> Vec3 {
    let viewport_width = viewport_size.x.max(1.0);
    let viewport_height = viewport_size.y.max(1.0);
    let normalized_x = (screen.x / viewport_width).clamp(0.0, 1.0);
    let normalized_y = (screen.y / viewport_height).clamp(0.0, 1.0);
    let world_width = (world_max.x - world_min.x).max(1.0);
    let world_height = (world_max.z - world_min.z).max(1.0);

    Vec3::new(
        world_min.x + normalized_x * world_width,
        0.0,
        world_min.z + normalized_y * world_height,
    )
}

/// All possible command types that can be issued in the game
/// Based on MSG_* types from MessageStream.h starting at MSG_BEGIN_NETWORK_MESSAGES = 1000
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommandType {
    // Selection commands
    CreateSelectedGroup {
        create_new: bool,
        units: Vec<ObjectId>,
    },
    DestroySelectedGroup {
        team_id: u32,
    },
    RemoveFromSelectedGroup {
        units: Vec<ObjectId>,
    },

    // Movement commands
    Move {
        destination: Vec3,
    }, // Basic move command
    MoveTo {
        destination: Vec3,
        waypoints: Vec<Vec3>,
    },
    AttackMoveTo {
        destination: Vec3,
    },
    ForceMoveTo {
        destination: Vec3,
    },
    AddWaypoint {
        destination: Vec3,
    },

    // Combat commands
    Attack {
        target_id: ObjectId,
    }, // Basic attack command
    AttackObject {
        target_id: ObjectId,
    },
    ForceAttackObject {
        target_id: ObjectId,
    },
    ForceAttackGround {
        location: Vec3,
    },
    Stop,
    Guard {
        target: GuardTarget,
    },
    Scatter,
    Deploy,
    Gather {
        target_id: ObjectId,
    },

    // Building and construction
    Build {
        template_name: String,
        location: Vec3,
    }, // Basic build command
    DozerConstruct {
        template_name: String,
        location: Vec3,
    },
    DozerConstructLine {
        template_name: String,
        start: Vec3,
        end: Vec3,
    },
    DozerCancelConstruct {
        object_id: ObjectId,
    },
    ResumeConstruction {
        target_id: ObjectId,
    },
    Sell {
        object_id: ObjectId,
    },

    // Unit production
    QueueUnitCreate {
        template_name: String,
        quantity: u32,
    },
    CancelUnitCreate {
        template_name: String,
    },

    // Special abilities
    DoSpecialPower {
        power_type: SpecialPowerType,
        target: PowerTarget,
    },
    DoWeapon {
        weapon_slot: WeaponSlot,
        target: WeaponTarget,
    },

    // Transport and container
    Enter {
        target_id: ObjectId,
    },
    Exit,
    Evacuate,
    Dock {
        target_id: ObjectId,
    },
    CombatDrop {
        target: DropTarget,
    },

    // Utility commands
    Repair {
        target_id: ObjectId,
    },
    GetRepaired {
        target_id: ObjectId,
    },
    GetHealed {
        target_id: ObjectId,
    },
    SetRallyPoint {
        location: Vec3,
    },

    // Economy and resources
    PurchaseScience {
        science_name: String,
    },
    QueueUpgrade {
        upgrade_name: String,
    },
    CancelUpgrade {
        upgrade_name: String,
    },

    // Special unit abilities
    Hijack {
        target_id: ObjectId,
    },
    Sabotage {
        target_id: ObjectId,
    },
    ConvertToCarbomb {
        target_id: ObjectId,
    },
    CaptureBuilding {
        target_id: ObjectId,
    },
    SnipeVehicle {
        target_id: ObjectId,
    },
    SwitchWeapons,
    ToggleOvercharge,

    // Formation and group commands
    CreateFormation,
    Cheer,

    // Network/multiplayer commands
    PlaceBeacon {
        location: Vec3,
        text: String,
    },
    RemoveBeacon,
    ViewLastRadarEvent,
    ViewRadarAt {
        position: Vec3,
    },
    ViewCommandCenter,

    // Invalid command placeholder
    Invalid,
}

/// Target types for guard command
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GuardTarget {
    Position(Vec3),
    Object(ObjectId),
}

/// Target types for special powers
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PowerTarget {
    Location(Vec3),
    Object(ObjectId),
    None,
}

/// Target types for weapon commands
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WeaponTarget {
    Location(Vec3),
    Object(ObjectId),
}

/// Target types for combat drops
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DropTarget {
    Location(Vec3),
    Object(ObjectId),
}

/// Special power types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SpecialPowerType {
    Airstrike,
    Artillery,
    CarpetBomb,
    ClusterMines,
    DaisyCutter,
    EmergencyRepair,
    FuelAirBomb,
    Healing,
    IonCannon,
    NapalmStrike,
    NuclearMissile,
    ParticleCannon,
    RadarScan,
    ScudStorm,
    SpyDrone,
    SuperweaponCountermeasures,
    Invalid,
}

/// Weapon slot identifiers
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WeaponSlot {
    Primary,
    Secondary,
    Tertiary,
    AntiAir,
    Slot(u32),
}

/// Command evaluation results
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommandResult {
    Success,
    InvalidTarget,
    OutOfRange,
    InsufficientResources,
    InvalidCommand,
    UnitBusy,
    TargetDestroyed,
    RequiresLineOfSight,
    InvalidLocation,
    CannotAttackTarget,
    CannotMoveToLocation,
    BuildingBlocked,
}

/// Command evaluation mode
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommandEvaluateType {
    DoCommand,
    DoHint,
    EvaluateOnly,
}

/// A complete game command with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameCommand {
    pub command_type: CommandType,
    pub player_id: u32,
    pub command_id: u32,
    pub timestamp: SystemTime,
    pub selected_units: Vec<ObjectId>,
    pub modifier_keys: ModifierKeys,
}

/// Mouse/keyboard modifier keys
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ModifierKeys {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// Information needed for command creation from mouse input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseCommandContext {
    pub world_position: Vec3,
    pub target_object: Option<ObjectId>,
    pub screen_position: Vec2,
    pub viewport_size: Option<Vec2>,
    pub world_min: Option<Vec3>,
    pub world_max: Option<Vec3>,
    pub mouse_button: MouseButton,
    pub modifier_keys: ModifierKeys,
    pub is_drag: bool,
    pub drag_start: Option<Vec2>,
    pub drag_end: Option<Vec2>,
    pub drag_start_world: Option<Vec3>,
    pub drag_end_world: Option<Vec3>,
}

/// Mouse button types
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Command system state for tracking mode
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommandMode {
    Normal,
    ForceAttack,
    ForceMove,
    Waypoint,
    BuildMode { template_name: String },
    SpecialPower { power_type: SpecialPowerType },
}

/// Main command system that handles all RTS commands
pub struct CommandSystem {
    /// Current command mode (force attack, build mode, etc.)
    pub current_mode: CommandMode,

    /// Commands waiting to be processed
    command_queue: VecDeque<GameCommand>,

    /// Current command ID counter
    next_command_id: u32,

    /// Mouse drag tracking
    mouse_drag_start: Option<Vec2>,
    mouse_down_time: Option<Instant>,

    /// Command history for undo/replay
    command_history: Vec<GameCommand>,

    /// Player-specific command settings
    player_settings: HashMap<u32, PlayerCommandSettings>,
}

/// Per-player command settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerCommandSettings {
    pub auto_attack: bool,
    pub smart_select: bool,
    pub formation_move: bool,
    pub waypoint_mode: bool,
}

impl Default for PlayerCommandSettings {
    fn default() -> Self {
        Self {
            auto_attack: false,
            smart_select: true,
            formation_move: true,
            waypoint_mode: false,
        }
    }
}

impl Default for CommandSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandSystem {
    /// Create a new command system
    pub fn new() -> Self {
        Self {
            current_mode: CommandMode::Normal,
            command_queue: VecDeque::new(),
            next_command_id: 1,
            mouse_drag_start: None,
            mouse_down_time: None,
            command_history: Vec::new(),
            player_settings: HashMap::new(),
        }
    }

    /// Get (or lazily create) mutable command settings for a player
    fn player_settings_mut(&mut self, player_id: u32) -> &mut PlayerCommandSettings {
        self.player_settings.entry(player_id).or_default()
    }

    /// Read-only view of a player's settings (creating default when missing).
    pub fn player_settings(&mut self, player_id: u32) -> PlayerCommandSettings {
        self.player_settings_mut(player_id).clone()
    }

    /// Enable or disable waypoint mode for a player.
    pub fn set_waypoint_mode_for_player(&mut self, player_id: u32, enabled: bool) {
        self.player_settings_mut(player_id).waypoint_mode = enabled;
    }

    /// Toggle auto-attack preference and return the new value.
    pub fn toggle_auto_attack(&mut self, player_id: u32) -> bool {
        let settings = self.player_settings_mut(player_id);
        settings.auto_attack = !settings.auto_attack;
        settings.auto_attack
    }

    /// Toggle whether moves should preserve formation and return the new value.
    pub fn toggle_formation_move(&mut self, player_id: u32) -> bool {
        let settings = self.player_settings_mut(player_id);
        settings.formation_move = !settings.formation_move;
        settings.formation_move
    }

    /// Toggle whether selection should attempt smart grouping and return the new value.
    pub fn toggle_smart_select(&mut self, player_id: u32) -> bool {
        let settings = self.player_settings_mut(player_id);
        settings.smart_select = !settings.smart_select;
        settings.smart_select
    }

    /// Select units matching predicate for the player and queue the selection command.
    pub fn select_units_by_predicate<F>(
        &mut self,
        player_id: u32,
        modifier_keys: ModifierKeys,
        game_logic: &GameLogic,
        mut predicate: F,
    ) -> bool
    where
        F: FnMut(&Object) -> bool,
    {
        let player = match game_logic.get_player(player_id) {
            Some(player) => player,
            None => return false,
        };

        let mut units = Vec::new();
        for (&id, obj) in game_logic.get_objects().iter() {
            if obj.team == player.team && obj.is_selectable() && predicate(obj) {
                units.push(id);
            }
        }

        if units.is_empty() {
            return false;
        }

        let command = self.create_command(
            CommandType::CreateSelectedGroup {
                create_new: !modifier_keys.shift,
                units: units.clone(),
            },
            &units,
            player_id,
            modifier_keys,
        );
        self.queue_command(command);
        true
    }

    /// Build a command that selects all objects matching the double-clicked target
    fn create_select_similar_command(
        &mut self,
        target_id: ObjectId,
        player_id: u32,
        modifier_keys: ModifierKeys,
        game_logic: &GameLogic,
    ) -> Option<GameCommand> {
        let target = game_logic.get_object(target_id)?;
        let player = game_logic.get_player(player_id)?;

        // Only allow selecting similar units that belong to the same team
        if target.team != player.team {
            return None;
        }

        let template_name = target.template_name.clone();
        let object_type = target.object_type;
        let mut units: Vec<ObjectId> = game_logic
            .get_objects()
            .iter()
            .filter_map(|(&id, obj)| {
                if obj.team == target.team
                    && obj.is_selectable()
                    && (obj.template_name == template_name
                        || (modifier_keys.alt && obj.object_type == object_type))
                {
                    Some(id)
                } else {
                    None
                }
            })
            .collect();

        if units.is_empty() {
            return None;
        }

        if !units.contains(&target_id) {
            units.push(target_id);
        }

        let command_units = units.clone();
        Some(self.create_command(
            CommandType::CreateSelectedGroup {
                create_new: true,
                units: command_units,
            },
            units.as_slice(),
            player_id,
            modifier_keys,
        ))
    }

    /// Set the current command mode
    pub fn set_mode(&mut self, mode: CommandMode) {
        self.current_mode = mode.clone();
        log::debug!("Command mode changed to: {:?}", mode);
    }

    /// Process mouse input and create appropriate commands
    pub fn process_mouse_input(
        &mut self,
        context: &MouseCommandContext,
        selected_units: &[ObjectId],
        player_id: u32,
        game_logic: &GameLogic,
    ) -> Option<GameCommand> {
        if context.is_drag {
            self.mouse_drag_start = context.drag_start.or(Some(context.screen_position));
        } else {
            self.mouse_drag_start = None;
        }

        match context.mouse_button {
            MouseButton::Left => {
                self.process_left_click(context, selected_units, player_id, game_logic)
            }
            MouseButton::Right => {
                self.process_right_click(context, selected_units, player_id, game_logic)
            }
            MouseButton::Middle => {
                self.process_middle_click(context, selected_units, player_id, game_logic)
            }
        }
    }

    /// Process left mouse click
    fn process_left_click(
        &mut self,
        context: &MouseCommandContext,
        selected_units: &[ObjectId],
        player_id: u32,
        game_logic: &GameLogic,
    ) -> Option<GameCommand> {
        match &self.current_mode {
            CommandMode::Normal => {
                let now = Instant::now();
                let is_double_click = self
                    .mouse_down_time
                    .map(|last| now.duration_since(last) <= DOUBLE_CLICK_THRESHOLD)
                    .unwrap_or(false)
                    && self.player_settings_mut(player_id).smart_select
                    && !context.is_drag;
                self.mouse_down_time = Some(now);

                if is_double_click {
                    if let Some(target_id) = context.target_object {
                        if let Some(command) = self.create_select_similar_command(
                            target_id,
                            player_id,
                            context.modifier_keys,
                            game_logic,
                        ) {
                            return Some(command);
                        }
                    }
                }

                if context.is_drag {
                    // Area selection
                    Some(self.create_selection_command(context, player_id, game_logic))
                } else if let Some(target_id) = context.target_object {
                    // Select single unit
                    let create_new = !context.modifier_keys.shift;
                    Some(self.create_command(
                        CommandType::CreateSelectedGroup {
                            create_new,
                            units: vec![target_id],
                        },
                        selected_units,
                        player_id,
                        context.modifier_keys,
                    ))
                } else {
                    None
                }
            }
            CommandMode::BuildMode { template_name } => {
                // Place building
                Some(self.create_command(
                    CommandType::DozerConstruct {
                        template_name: template_name.clone(),
                        location: context.world_position,
                    },
                    selected_units,
                    player_id,
                    context.modifier_keys,
                ))
            }
            CommandMode::SpecialPower { power_type } => {
                // Use special power
                let target = if let Some(target_id) = context.target_object {
                    PowerTarget::Object(target_id)
                } else {
                    PowerTarget::Location(context.world_position)
                };

                Some(self.create_command(
                    CommandType::DoSpecialPower {
                        power_type: power_type.clone(),
                        target,
                    },
                    selected_units,
                    player_id,
                    context.modifier_keys,
                ))
            }
            _ => None,
        }
    }

    /// Process right mouse click - creates movement and attack commands
    fn process_right_click(
        &mut self,
        context: &MouseCommandContext,
        selected_units: &[ObjectId],
        player_id: u32,
        game_logic: &GameLogic,
    ) -> Option<GameCommand> {
        if selected_units.is_empty() {
            return None;
        }

        let (mut waypoint_mode, auto_attack) = {
            let settings = self.player_settings_mut(player_id);
            (settings.waypoint_mode, settings.auto_attack)
        };

        if context.modifier_keys.alt {
            waypoint_mode = true;
        }

        // Determine command type based on mode and target
        let mode = if waypoint_mode {
            CommandMode::Waypoint
        } else {
            self.current_mode.clone()
        };

        let mut command_type = match &mode {
            CommandMode::ForceAttack => {
                if let Some(target_id) = context.target_object {
                    CommandType::ForceAttackObject { target_id }
                } else {
                    CommandType::ForceAttackGround {
                        location: context.world_position,
                    }
                }
            }
            CommandMode::ForceMove => CommandType::ForceMoveTo {
                destination: context.world_position,
            },
            CommandMode::Waypoint => CommandType::AddWaypoint {
                destination: context.world_position,
            },
            _ => {
                // Context-sensitive command
                self.determine_context_command(context, selected_units, game_logic)
            }
        };

        if auto_attack {
            if let CommandType::MoveTo { destination, .. } = command_type {
                command_type = CommandType::AttackMoveTo { destination };
            }
        }

        Some(self.create_command(
            command_type,
            selected_units,
            player_id,
            context.modifier_keys,
        ))
    }

    /// Process middle mouse click
    fn process_middle_click(
        &mut self,
        _context: &MouseCommandContext,
        _selected_units: &[ObjectId],
        _player_id: u32,
        _game_logic: &GameLogic,
    ) -> Option<GameCommand> {
        // Middle click typically used for camera controls
        None
    }

    /// Determine context-sensitive command based on target and selected units
    fn determine_context_command(
        &self,
        context: &MouseCommandContext,
        selected_units: &[ObjectId],
        game_logic: &GameLogic,
    ) -> CommandType {
        if let Some(target_id) = context.target_object {
            if let Some(target_obj) = game_logic.get_object(target_id) {
                // Check if target is a resource/harvestable and selected units can gather
                if self.can_gather_from_target(selected_units, target_obj, game_logic) {
                    return CommandType::Gather { target_id };
                }

                // Check if target is enemy - attack
                if self.can_attack_target(selected_units, target_obj, game_logic) {
                    return CommandType::AttackObject { target_id };
                }

                // Check if target is repairable
                if self.can_repair_target(selected_units, target_obj, game_logic) {
                    return CommandType::Repair { target_id };
                }

                // Check if target is enterable
                if self.can_enter_target(selected_units, target_obj, game_logic) {
                    return CommandType::Enter { target_id };
                }

                // Check if target provides healing/repair services
                if self.can_get_serviced_at_target(selected_units, target_obj, game_logic) {
                    let target_building_type = target_obj
                        .building_data
                        .as_ref()
                        .map(|b| b.building_type)
                        .unwrap_or(BuildingType::CommandCenter);
                    if target_building_type == BuildingType::HealPad
                        || target_obj.is_medical_facility()
                    {
                        return CommandType::GetHealed { target_id };
                    } else {
                        return CommandType::GetRepaired { target_id };
                    }
                }
            }
        }

        // Default to move command
        if context.modifier_keys.ctrl {
            // Attack-move if ctrl is held
            CommandType::AttackMoveTo {
                destination: context.world_position,
            }
        } else {
            CommandType::MoveTo {
                destination: context.world_position,
                waypoints: Vec::new(),
            }
        }
    }

    /// Create area selection command from drag
    fn create_selection_command(
        &mut self,
        context: &MouseCommandContext,
        player_id: u32,
        game_logic: &GameLogic,
    ) -> GameCommand {
        let player = match game_logic.get_player(player_id) {
            Some(player) => player,
            None => {
                return self.create_command(
                    CommandType::CreateSelectedGroup {
                        create_new: !context.modifier_keys.shift,
                        units: Vec::new(),
                    },
                    &[],
                    player_id,
                    context.modifier_keys,
                );
            }
        };

        let drag_start = context.drag_start.unwrap_or(context.screen_position);
        let drag_end = context.drag_end.unwrap_or(context.screen_position);
        let viewport_size = context.viewport_size.unwrap_or(Vec2::new(800.0, 600.0));
        let world_min = context.world_min.unwrap_or(Vec3::new(-400.0, 0.0, -300.0));
        let world_max = context.world_max.unwrap_or(Vec3::new(400.0, 0.0, 300.0));
        let drag_start_world = context
            .drag_start_world
            .unwrap_or_else(|| screen_to_world(drag_start, viewport_size, world_min, world_max));
        let drag_end_world = context
            .drag_end_world
            .unwrap_or_else(|| screen_to_world(drag_end, viewport_size, world_min, world_max));

        let min_x = drag_start_world.x.min(drag_end_world.x);
        let max_x = drag_start_world.x.max(drag_end_world.x);
        let min_z = drag_start_world.z.min(drag_end_world.z);
        let max_z = drag_start_world.z.max(drag_end_world.z);

        let mut units = Vec::new();
        for (&id, obj) in game_logic.get_objects().iter() {
            if obj.team != player.team || !obj.is_selectable() {
                continue;
            }

            let obj_pos = obj.get_position();
            if obj_pos.x >= min_x && obj_pos.x <= max_x && obj_pos.z >= min_z && obj_pos.z <= max_z
            {
                units.push(id);
            }
        }

        self.create_command(
            CommandType::CreateSelectedGroup {
                create_new: !context.modifier_keys.shift,
                units,
            },
            &[],
            player_id,
            context.modifier_keys,
        )
    }

    /// Create a game command with metadata
    fn create_command(
        &mut self,
        command_type: CommandType,
        selected_units: &[ObjectId],
        player_id: u32,
        modifier_keys: ModifierKeys,
    ) -> GameCommand {
        let command = GameCommand {
            command_type,
            player_id,
            command_id: self.next_command_id,
            timestamp: SystemTime::now(),
            selected_units: selected_units.to_vec(),
            modifier_keys,
        };

        self.next_command_id += 1;
        command
    }

    /// Queue command for execution
    pub fn queue_command(&mut self, command: GameCommand) {
        log::debug!("Queuing command: {:?}", command.command_type);
        self.command_queue.push_back(command);
    }

    /// Create and queue a command immediately (used by keyboard shortcuts).
    pub fn queue_immediate_command(
        &mut self,
        command_type: CommandType,
        selected_units: &[ObjectId],
        player_id: u32,
        modifier_keys: ModifierKeys,
    ) {
        let command = self.create_command(command_type, selected_units, player_id, modifier_keys);
        self.queue_command(command);
    }

    /// Process all queued commands
    pub fn process_commands(&mut self, game_logic: &mut GameLogic) -> Vec<CommandResult> {
        let mut results = Vec::new();

        while let Some(command) = self.command_queue.pop_front() {
            let result = self.execute_command(&command, game_logic);
            results.push(result);

            // Add to history for replay/undo
            self.command_history.push(command);
        }

        results
    }

    /// Execute a single command
    pub fn execute_command(
        &self,
        command: &GameCommand,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        let mut executor =
            crate::command_executor::CommandExecutor::new(game_logic, command.player_id);
        match executor.execute_command(command.clone()) {
            Ok(result) => result,
            Err(err) => {
                log::warn!(
                    "Failed to execute command {:?} for player {}: {}",
                    command.command_type,
                    command.player_id,
                    err
                );
                CommandResult::InvalidCommand
            }
        }
    }

    /// Execute move command - core RTS functionality
    fn execute_move_command(
        &self,
        units: &[ObjectId],
        destination: Vec3,
        _waypoints: &[Vec3],
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        let mut all_success = true;

        for &unit_id in units {
            if let Some(unit) = game_logic.get_object_mut(unit_id) {
                if unit.can_move() {
                    unit.set_destination(destination);
                    unit.set_ai_state(AIState::Moving);
                    log::debug!("Unit {} moving to {:?}", unit_id.0, destination);
                } else {
                    all_success = false;
                }
            } else {
                all_success = false;
            }
        }

        if all_success {
            CommandResult::Success
        } else {
            CommandResult::InvalidTarget
        }
    }

    /// Execute attack command - core RTS functionality
    fn execute_attack_command(
        &self,
        units: &[ObjectId],
        target_id: ObjectId,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        // Check if target exists and is attackable
        if let Some(target) = game_logic.get_object(target_id) {
            if target.is_dead() {
                return CommandResult::TargetDestroyed;
            }
        } else {
            return CommandResult::InvalidTarget;
        }

        let mut all_success = true;

        for &unit_id in units {
            if let Some(unit) = game_logic.get_object_mut(unit_id) {
                if unit.can_attack() {
                    unit.set_target(Some(target_id));
                    unit.set_ai_state(AIState::Attacking);
                    log::debug!("Unit {} attacking target {}", unit_id.0, target_id.0);
                } else {
                    all_success = false;
                }
            } else {
                all_success = false;
            }
        }

        if all_success {
            CommandResult::Success
        } else {
            CommandResult::InvalidTarget
        }
    }

    /// Execute attack-move command
    fn execute_attack_move_command(
        &self,
        units: &[ObjectId],
        destination: Vec3,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        let mut all_success = true;

        for &unit_id in units {
            if let Some(unit) = game_logic.get_object_mut(unit_id) {
                if unit.can_move() && unit.can_attack() {
                    unit.set_destination(destination);
                    unit.set_ai_state(AIState::AttackMoving);
                    log::debug!("Unit {} attack-moving to {:?}", unit_id.0, destination);
                } else {
                    all_success = false;
                }
            } else {
                all_success = false;
            }
        }

        if all_success {
            CommandResult::Success
        } else {
            CommandResult::InvalidTarget
        }
    }

    /// Execute force attack command
    fn execute_force_attack_command(
        &self,
        units: &[ObjectId],
        target_id: ObjectId,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        // Force attack doesn't check relationships - attack anything
        let mut all_success = true;

        for &unit_id in units {
            if let Some(unit) = game_logic.get_object_mut(unit_id) {
                if unit.can_attack() {
                    unit.set_target(Some(target_id));
                    unit.set_ai_state(AIState::Attacking);
                    unit.set_force_attack(true);
                    log::debug!("Unit {} force-attacking target {}", unit_id.0, target_id.0);
                } else {
                    all_success = false;
                }
            } else {
                all_success = false;
            }
        }

        if all_success {
            CommandResult::Success
        } else {
            CommandResult::InvalidTarget
        }
    }

    /// Execute force attack ground command
    fn execute_force_attack_ground_command(
        &self,
        units: &[ObjectId],
        location: Vec3,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        let mut all_success = true;

        for &unit_id in units {
            if let Some(unit) = game_logic.get_object_mut(unit_id) {
                if unit.can_attack() {
                    unit.set_target_location(Some(location));
                    unit.set_ai_state(AIState::AttackingGround);
                    log::debug!(
                        "Unit {} force-attacking ground at {:?}",
                        unit_id.0,
                        location
                    );
                } else {
                    all_success = false;
                }
            } else {
                all_success = false;
            }
        }

        if all_success {
            CommandResult::Success
        } else {
            CommandResult::InvalidTarget
        }
    }

    /// Execute stop command
    fn execute_stop_command(
        &self,
        units: &[ObjectId],
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        for &unit_id in units {
            if let Some(unit) = game_logic.get_object_mut(unit_id) {
                unit.stop();
                unit.set_ai_state(AIState::Idle);
                log::debug!("Unit {} stopped", unit_id.0);
            }
        }
        CommandResult::Success
    }

    /// Execute scatter command by pushing units away from their current positions.
    fn execute_scatter_command(
        &self,
        units: &[ObjectId],
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        if units.is_empty() {
            return CommandResult::InvalidCommand;
        }

        const BASE_DISTANCE: f32 = 25.0;
        const DISTANCE_VARIATION: f32 = 10.0;

        for (index, &unit_id) in units.iter().enumerate() {
            if let Some(unit) = game_logic.get_object_mut(unit_id) {
                if !unit.can_move() {
                    continue;
                }
                let origin = unit.get_position();
                let angle = ((unit_id.0 as usize + index) as f32 * 0.318_309_87) % TAU;
                let distance = BASE_DISTANCE + (index as f32 % DISTANCE_VARIATION).abs();
                let offset = Vec3::new(angle.cos(), 0.0, angle.sin()) * distance;
                let destination = origin + offset;
                unit.set_destination(destination);
                unit.set_ai_state(AIState::Moving);
                log::debug!("Unit {} scattering toward {:?}", unit_id.0, destination);
            }
        }

        CommandResult::Success
    }

    /// Arrange selected units into a grid formation centered around their centroid.
    fn execute_create_formation_command(
        &self,
        units: &[ObjectId],
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        if units.is_empty() {
            return CommandResult::InvalidCommand;
        }

        let mut movable_units = Vec::new();
        for &unit_id in units {
            if let Some(unit) = game_logic.get_object(unit_id) {
                if unit.can_move() {
                    movable_units.push((unit_id, unit.get_position()));
                }
            }
        }

        if movable_units.is_empty() {
            return CommandResult::InvalidCommand;
        }

        let mut centroid = Vec3::ZERO;
        for (_, position) in &movable_units {
            centroid += *position;
        }
        centroid /= movable_units.len() as f32;

        let columns = (movable_units.len() as f32).sqrt().ceil() as usize;
        let rows = movable_units.len().div_ceil(columns);
        let spacing = 20.0;

        for (index, (unit_id, _)) in movable_units.iter().enumerate() {
            let row = (index / columns) as f32;
            let column = (index % columns) as f32;
            let offset_x = (column - (columns as f32 - 1.0) * 0.5) * spacing;
            let offset_z = (row - (rows as f32 - 1.0) * 0.5) * spacing;
            let destination = centroid + Vec3::new(offset_x, 0.0, offset_z);

            if let Some(unit) = game_logic.get_object_mut(*unit_id) {
                unit.set_destination(destination);
                unit.set_ai_state(AIState::Moving);
                log::debug!("Unit {} forming up at {:?}", unit_id.0, destination);
            }
        }

        CommandResult::Success
    }

    fn execute_view_command_center(
        &self,
        player_id: u32,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        let player = match game_logic.get_player(player_id) {
            Some(player) => player,
            None => return CommandResult::InvalidCommand,
        };

        if let Some(position) = game_logic.command_center_position(player.team) {
            game_logic.request_camera_focus(position);
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Execute guard command
    fn execute_guard_command(
        &self,
        units: &[ObjectId],
        target: &GuardTarget,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        for &unit_id in units {
            if let Some(unit) = game_logic.get_object_mut(unit_id) {
                match target {
                    GuardTarget::Position(pos) => {
                        unit.set_guard_position(Some(*pos));
                        unit.set_ai_state(AIState::GuardingArea);
                        log::debug!("Unit {} guarding position {:?}", unit_id.0, pos);
                    }
                    GuardTarget::Object(target_id) => {
                        unit.set_guard_target(Some(*target_id));
                        unit.set_ai_state(AIState::GuardingObject);
                        log::debug!("Unit {} guarding object {}", unit_id.0, target_id.0);
                    }
                }
            }
        }
        CommandResult::Success
    }

    /// Execute construction command
    fn execute_construct_command(
        &self,
        units: &[ObjectId],
        template_name: &str,
        location: Vec3,
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        let (build_cost, is_structure) = match game_logic.get_templates().get(template_name) {
            Some(t) => (
                t.build_cost,
                t.is_kind_of(crate::game_logic::KindOf::Structure),
            ),
            None => return CommandResult::InvalidCommand,
        };

        if !is_structure {
            return CommandResult::InvalidCommand;
        }

        // Find a constructor unit
        for &unit_id in units {
            let team = match game_logic.get_object(unit_id) {
                Some(unit) if unit.can_construct() => unit.team,
                Some(_) => continue,
                None => continue,
            };

            {
                let Some(player) = game_logic.get_player_mut_by_team(team) else {
                    continue;
                };

                if !player.spend_resources(&build_cost) {
                    return CommandResult::InvalidCommand;
                }
            }

            let created =
                game_logic.create_object_under_construction(template_name, team, location);
            if created.is_none() {
                if let Some(player) = game_logic.get_player_mut_by_team(team) {
                    player.resources.supplies = player
                        .resources
                        .supplies
                        .saturating_add(build_cost.supplies);
                }
                return CommandResult::InvalidCommand;
            }

            if let Some(unit) = game_logic.get_object_mut(unit_id) {
                unit.set_destination(location);
                unit.set_ai_state(AIState::Constructing);
            }

            log::debug!(
                "Unit {} constructing {} at {:?}",
                unit_id.0,
                template_name,
                location
            );
            return CommandResult::Success;
        }
        CommandResult::InvalidCommand
    }

    /// Execute selection command
    fn execute_selection_command(
        &self,
        player_id: u32,
        create_new: bool,
        units: &[ObjectId],
        game_logic: &mut GameLogic,
    ) -> CommandResult {
        if let Some(player) = game_logic.get_player_mut(player_id) {
            if create_new {
                player.selected_objects.clear();
            }

            for &unit_id in units {
                if !player.selected_objects.contains(&unit_id) {
                    player.selected_objects.push(unit_id);
                }
            }

            log::debug!(
                "Player {} selected {} units",
                player_id,
                player.selected_objects.len()
            );
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Validate if selected units can attack target
    fn can_attack_target(
        &self,
        units: &[ObjectId],
        target: &Object,
        game_logic: &GameLogic,
    ) -> bool {
        for &unit_id in units {
            if let Some(unit) = game_logic.get_object(unit_id) {
                if unit.can_attack() && unit.team != target.team && !target.is_dead() {
                    return true;
                }
            }
        }
        false
    }

    /// Validate if selected units can gather from a resource target
    fn can_gather_from_target(
        &self,
        units: &[ObjectId],
        target: &Object,
        game_logic: &GameLogic,
    ) -> bool {
        if !target.is_alive() {
            return false;
        }
        let target_is_resource = target.is_kind_of(KindOf::Harvestable)
            || target.is_kind_of(KindOf::Resource)
            || target.object_type == crate::game_logic::ObjectType::Supply;
        if !target_is_resource {
            return false;
        }
        // Check if any selected unit is a worker/harvester on the same team
        for &unit_id in units {
            if let Some(unit) = game_logic.get_object(unit_id) {
                if unit.is_worker()
                    && unit.team == target.team
                    && unit.is_alive()
                    && unit.can_move()
                {
                    return true;
                }
            }
        }
        false
    }

    /// Validate if selected units can repair target
    fn can_repair_target(
        &self,
        units: &[ObjectId],
        target: &Object,
        game_logic: &GameLogic,
    ) -> bool {
        for &unit_id in units {
            if let Some(unit) = game_logic.get_object(unit_id) {
                if unit.can_repair() && unit.team == target.team && target.is_damaged() {
                    return true;
                }
            }
        }
        false
    }

    /// Validate if selected units can enter target
    fn can_enter_target(
        &self,
        units: &[ObjectId],
        target: &Object,
        game_logic: &GameLogic,
    ) -> bool {
        if !target.can_contain() || !target.is_alive() || target.status.under_construction {
            return false;
        }

        let target_has_occupants = !target.contained_units().is_empty();

        for &unit_id in units {
            let Some(unit) = game_logic.get_object(unit_id) else {
                continue;
            };

            if unit_id == target.id
                || !unit.is_alive()
                || unit.status.under_construction
                || !unit.can_move()
                || unit.is_kind_of(KindOf::Structure)
            {
                continue;
            }

            let target_contains_unit = target.contained_units().contains(&unit_id);
            let target_has_space = target.has_capacity_for(1);
            if !target_contains_unit && !target_has_space {
                continue;
            }

            if target.team != unit.team
                && target.team != Team::Neutral
                && (target.is_faction_structure() || target_has_occupants)
            {
                continue;
            }

            return true;
        }

        false
    }

    /// Validate if selected units can get services at target
    fn can_get_serviced_at_target(
        &self,
        units: &[ObjectId],
        target: &Object,
        game_logic: &GameLogic,
    ) -> bool {
        if !target.is_alive() || target.status.under_construction {
            return false;
        }

        let target_building_type = target
            .building_data
            .as_ref()
            .map(|b| b.building_type)
            .unwrap_or(BuildingType::CommandCenter);

        for &unit_id in units {
            if let Some(unit) = game_logic.get_object(unit_id) {
                if unit.team != target.team
                    || !unit.is_alive()
                    || !unit.can_move()
                    || !(unit.is_damaged() || unit.is_injured())
                {
                    continue;
                }

                let can_use_service = match target_building_type {
                    BuildingType::HealPad => unit.is_kind_of(KindOf::Infantry),
                    BuildingType::RepairPad => {
                        unit.is_kind_of(KindOf::Vehicle) && !unit.is_kind_of(KindOf::Aircraft)
                    }
                    BuildingType::Airfield => unit.is_kind_of(KindOf::Aircraft),
                    _ => false,
                };

                if can_use_service {
                    return true;
                }
            }
        }
        false
    }

    /// Get current selected units for a player
    pub fn get_selected_units(&self, player_id: u32, game_logic: &GameLogic) -> Vec<ObjectId> {
        if let Some(player) = game_logic.get_player(player_id) {
            player.selected_objects.clone()
        } else {
            Vec::new()
        }
    }

    /// Clear command queue
    pub fn clear_queue(&mut self) {
        self.command_queue.clear();
    }

    /// Get command history
    pub fn get_command_history(&self) -> &[GameCommand] {
        &self.command_history
    }
}

/// Global command system instance
static COMMAND_SYSTEM: OnceLock<Mutex<CommandSystem>> = OnceLock::new();

/// Initialize the global command system
pub fn init_command_system() {
    let _ = COMMAND_SYSTEM.get_or_init(|| {
        log::info!("Command system initialized");
        Mutex::new(CommandSystem::new())
    });
}

/// Get the global command system instance
pub fn get_command_system() -> &'static Mutex<CommandSystem> {
    COMMAND_SYSTEM.get_or_init(|| {
        log::info!("Command system initialized");
        Mutex::new(CommandSystem::new())
    })
}

// Extension methods for Object to support command system
pub trait CommandableObject {
    fn can_move(&self) -> bool;
    fn can_attack(&self) -> bool;
    fn can_construct(&self) -> bool;
    fn can_repair(&self) -> bool;
    fn can_contain(&self) -> bool;
    fn is_damaged(&self) -> bool;
    fn is_injured(&self) -> bool;
    fn is_dead(&self) -> bool;
    fn is_medical_facility(&self) -> bool;
    fn provides_repair(&self) -> bool;
    fn provides_healing(&self) -> bool;
    fn has_capacity_for(&self, other: &Object) -> bool;
    fn set_destination(&mut self, destination: Vec3);
    fn set_target(&mut self, target: Option<ObjectId>);
    fn set_target_location(&mut self, location: Option<Vec3>);
    fn set_guard_position(&mut self, position: Option<Vec3>);
    fn set_guard_target(&mut self, target: Option<ObjectId>);
    fn set_force_attack(&mut self, force: bool);
    fn stop(&mut self);
}

impl CommandableObject for Object {
    fn can_move(&self) -> bool {
        // Check if object has mobility
        matches!(
            self.object_type,
            crate::game_logic::ObjectType::Vehicle
                | crate::game_logic::ObjectType::Infantry
                | crate::game_logic::ObjectType::Aircraft
        )
    }

    fn can_attack(&self) -> bool {
        // Check if object has weapons
        self.health.current > 0.0
            && !matches!(self.object_type, crate::game_logic::ObjectType::Supply)
    }

    fn can_construct(&self) -> bool {
        self.can_move()
            && (self.is_kind_of(crate::game_logic::KindOf::Worker)
                || self.template_name.contains("Dozer")
                || self.template_name.contains("Worker")
                || self.template_name.contains("Harvester")
                || self.template_name.contains("Collector"))
    }

    fn can_repair(&self) -> bool {
        self.can_construct() // Dozers can repair
    }

    fn can_contain(&self) -> bool {
        Object::can_contain(self)
    }

    fn is_damaged(&self) -> bool {
        self.health.current < self.max_health && self.health.current > 0.0
    }

    fn is_injured(&self) -> bool {
        self.is_damaged() // Same as damaged for now
    }

    fn is_dead(&self) -> bool {
        self.health.current <= 0.0
    }

    fn is_medical_facility(&self) -> bool {
        self.building_data
            .as_ref()
            .map(|b| b.building_type == BuildingType::HealPad)
            .unwrap_or_else(|| {
                let lower = self.template_name.to_ascii_lowercase();
                lower.contains("hospital") || lower.contains("heal") || lower.contains("medic")
            })
    }

    fn provides_repair(&self) -> bool {
        self.building_data
            .as_ref()
            .map(|b| {
                matches!(
                    b.building_type,
                    BuildingType::RepairPad | BuildingType::Airfield
                )
            })
            .unwrap_or_else(|| {
                matches!(self.object_type, crate::game_logic::ObjectType::Building)
                    && (self.template_name.contains("Repair")
                        || self.template_name.contains("Service")
                        || self.template_name.contains("Airfield"))
            })
    }

    fn provides_healing(&self) -> bool {
        self.is_medical_facility()
    }

    fn has_capacity_for(&self, _other: &Object) -> bool {
        Object::has_capacity_for(self, 1)
    }

    fn set_destination(&mut self, destination: Vec3) {
        Object::set_destination(self, destination);
    }

    fn set_target(&mut self, target: Option<ObjectId>) {
        Object::set_target(self, target);
    }

    fn set_target_location(&mut self, location: Option<Vec3>) {
        Object::set_target_location(self, location);
    }

    fn set_guard_position(&mut self, position: Option<Vec3>) {
        Object::set_guard_position(self, position);
    }

    fn set_guard_target(&mut self, target: Option<ObjectId>) {
        Object::set_guard_target(self, target);
    }

    fn set_force_attack(&mut self, force: bool) {
        Object::set_force_attack(self, force);
    }

    fn stop(&mut self) {
        Object::stop(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{GameLogic, Object, ObjectType};
    use game_engine::common::global_data::with_global_data_restored;

    #[test]
    fn test_command_creation() {
        let mut system = CommandSystem::new();
        let context = MouseCommandContext {
            world_position: Vec3::new(100.0, 0.0, 100.0),
            target_object: None,
            screen_position: Vec2::new(400.0, 300.0),
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: MouseButton::Right,
            modifier_keys: ModifierKeys::default(),
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };

        let game_logic = GameLogic::new();
        let selected_units = vec![ObjectId(1)];

        if let Some(command) = system.process_mouse_input(&context, &selected_units, 0, &game_logic)
        {
            match command.command_type {
                CommandType::MoveTo { destination, .. } => {
                    assert_eq!(destination, Vec3::new(100.0, 0.0, 100.0));
                }
                _ => panic!("Expected MoveTo command"),
            }
        } else {
            panic!("Expected command to be created");
        }
    }

    #[test]
    fn test_command_execution() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};

        let system = CommandSystem::new();
        let mut game_logic = GameLogic::new();

        // Create test object using a minimal thing template
        let mut template = ThingTemplate::new("TestUnit");
        template.add_kind_of(KindOf::Vehicle);
        template.set_health(100.0);

        let mut obj = Object::new(template, ObjectId(1), Team::USA);
        obj.position = Vec3::new(0.0, 0.0, 0.0);
        game_logic.add_object(obj);

        let command = GameCommand {
            command_type: CommandType::MoveTo {
                destination: Vec3::new(50.0, 0.0, 50.0),
                waypoints: Vec::new(),
            },
            player_id: 0,
            command_id: 1,
            timestamp: SystemTime::now(),
            selected_units: vec![ObjectId(1)],
            modifier_keys: ModifierKeys::default(),
        };

        let result = system.execute_command(&command, &mut game_logic);
        assert_eq!(result, CommandResult::Success);
    }

    #[test]
    fn right_click_heal_pad_issues_get_healed() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};

        let mut system = CommandSystem::new();
        let mut game_logic = GameLogic::new();

        let mut infantry_template = ThingTemplate::new("TestInfantry");
        infantry_template
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        let mut infantry = Object::new(infantry_template, ObjectId(1), Team::USA);
        let _ = infantry.take_damage(25.0);
        game_logic.add_object(infantry);

        let mut heal_pad_template = ThingTemplate::new("TestHealPad");
        heal_pad_template
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(900.0);
        let heal_pad = Object::new(heal_pad_template, ObjectId(2), Team::USA);
        game_logic.add_object(heal_pad);

        let context = MouseCommandContext {
            world_position: Vec3::new(0.0, 0.0, 0.0),
            target_object: Some(ObjectId(2)),
            screen_position: Vec2::new(0.0, 0.0),
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: MouseButton::Right,
            modifier_keys: ModifierKeys::default(),
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };

        let command = system
            .process_mouse_input(&context, &[ObjectId(1)], 0, &game_logic)
            .expect("right click should generate a command");
        assert!(
            matches!(
                command.command_type,
                CommandType::GetHealed {
                    target_id: ObjectId(2)
                }
            ),
            "heal pad target should issue GetHealed"
        );
    }

    #[test]
    fn right_click_repair_pad_issues_get_repaired() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};

        let mut system = CommandSystem::new();
        let mut game_logic = GameLogic::new();

        let mut vehicle_template = ThingTemplate::new("TestTank");
        vehicle_template
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(250.0);
        let mut vehicle = Object::new(vehicle_template, ObjectId(10), Team::USA);
        let _ = vehicle.take_damage(30.0);
        game_logic.add_object(vehicle);

        let mut repair_pad_template = ThingTemplate::new("TestRepairPad");
        repair_pad_template
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1000.0);
        let repair_pad = Object::new(repair_pad_template, ObjectId(11), Team::USA);
        game_logic.add_object(repair_pad);

        let context = MouseCommandContext {
            world_position: Vec3::new(0.0, 0.0, 0.0),
            target_object: Some(ObjectId(11)),
            screen_position: Vec2::new(0.0, 0.0),
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: MouseButton::Right,
            modifier_keys: ModifierKeys::default(),
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };

        let command = system
            .process_mouse_input(&context, &[ObjectId(10)], 0, &game_logic)
            .expect("right click should generate a command");
        assert!(
            matches!(
                command.command_type,
                CommandType::GetRepaired {
                    target_id: ObjectId(11)
                }
            ),
            "repair pad target should issue GetRepaired"
        );
    }

    #[test]
    fn drag_selection_prefers_world_drag_bounds_when_provided() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        let mut system = CommandSystem::new();
        let mut game_logic = GameLogic::new();
        game_logic.add_player(Player::new(0, Team::USA, "TestPlayer", true));

        let mut template = ThingTemplate::new("TestUnit");
        template
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);

        let mut near = Object::new(template.clone(), ObjectId(31), Team::USA);
        near.set_position(Vec3::new(10.0, 0.0, 10.0));
        game_logic.add_object(near);

        let mut far = Object::new(template, ObjectId(32), Team::USA);
        far.set_position(Vec3::new(120.0, 0.0, 120.0));
        game_logic.add_object(far);

        let context = MouseCommandContext {
            world_position: Vec3::new(0.0, 0.0, 0.0),
            target_object: None,
            screen_position: Vec2::new(0.0, 0.0),
            viewport_size: Some(Vec2::new(1024.0, 768.0)),
            world_min: Some(Vec3::new(-256.0, 0.0, -256.0)),
            world_max: Some(Vec3::new(256.0, 0.0, 256.0)),
            mouse_button: MouseButton::Left,
            modifier_keys: ModifierKeys::default(),
            is_drag: true,
            drag_start: Some(Vec2::new(999.0, 999.0)),
            drag_end: Some(Vec2::new(1000.0, 1000.0)),
            drag_start_world: Some(Vec3::new(0.0, 0.0, 0.0)),
            drag_end_world: Some(Vec3::new(50.0, 0.0, 50.0)),
        };

        let command = system
            .process_mouse_input(&context, &[], 0, &game_logic)
            .expect("drag selection should produce command");

        match command.command_type {
            CommandType::CreateSelectedGroup { units, .. } => {
                assert!(units.contains(&ObjectId(31)));
                assert!(!units.contains(&ObjectId(32)));
            }
            other => panic!("expected drag CreateSelectedGroup command, got {other:?}"),
        }
    }

    #[test]
    fn queue_upgrade_deducts_once_per_team_and_prevents_duplicate_queue() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        let system = CommandSystem::new();
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 5000;
        game_logic.add_player(player);

        let mut template = ThingTemplate::new("AmericaSupplyCenter");
        template
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);

        let producer_a = Object::new(template.clone(), ObjectId(201), Team::USA);
        let producer_b = Object::new(template, ObjectId(202), Team::USA);
        game_logic.add_object(producer_a);
        game_logic.add_object(producer_b);

        let queue_command = GameCommand {
            command_type: CommandType::QueueUpgrade {
                upgrade_name: "Upgrade_AmericaSupplyLines".to_string(),
            },
            player_id: 0,
            command_id: 1,
            timestamp: SystemTime::now(),
            selected_units: vec![ObjectId(201), ObjectId(202)],
            modifier_keys: ModifierKeys::default(),
        };

        let first_result = system.execute_command(&queue_command, &mut game_logic);
        assert_eq!(first_result, CommandResult::Success);

        let player_after_first = game_logic.get_player(0).expect("player should exist");
        assert_eq!(
            player_after_first.resources.supplies, 4000,
            "upgrade cost should be charged once per team, not per selected unit"
        );
        assert!(player_after_first
            .queued_upgrades
            .contains("Upgrade_AmericaSupplyLines"));

        let second_result = system.execute_command(&queue_command, &mut game_logic);
        assert_eq!(second_result, CommandResult::InvalidCommand);
    }

    #[test]
    fn queue_upgrade_identity_matches_ini_name_variants() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        let system = CommandSystem::new();
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 5000;
        game_logic.add_player(player);

        let mut template = ThingTemplate::new("AmericaSupplyCenter");
        template
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        game_logic.add_object(Object::new(template, ObjectId(251), Team::USA));

        let queue_command = GameCommand {
            command_type: CommandType::QueueUpgrade {
                upgrade_name: "Upgrade_AmericaSupplyLines".to_string(),
            },
            player_id: 0,
            command_id: 30,
            timestamp: SystemTime::now(),
            selected_units: vec![ObjectId(251)],
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            system.execute_command(&queue_command, &mut game_logic),
            CommandResult::Success
        );

        let variant_command = GameCommand {
            command_type: CommandType::QueueUpgrade {
                upgrade_name: "upgradeamericasupplylines".to_string(),
            },
            player_id: 0,
            command_id: 31,
            timestamp: SystemTime::now(),
            selected_units: vec![ObjectId(251)],
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            system.execute_command(&variant_command, &mut game_logic),
            CommandResult::InvalidCommand,
            "same upgrade should not be charged twice when naming style differs"
        );

        let cancel_variant = GameCommand {
            command_type: CommandType::CancelUpgrade {
                upgrade_name: "UPGRADE_AMERICA_SUPPLY_LINES".to_string(),
            },
            player_id: 0,
            command_id: 32,
            timestamp: SystemTime::now(),
            selected_units: vec![ObjectId(251)],
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            system.execute_command(&cancel_variant, &mut game_logic),
            CommandResult::Success,
            "cancel should find the queued upgrade by normalized INI identity"
        );

        let player = game_logic.get_player(0).expect("player should exist");
        assert_eq!(player.resources.supplies, 5000);
        assert!(player.queued_upgrades.is_empty());
    }

    #[test]
    fn purchase_science_identity_matches_command_name_variants() {
        use crate::game_logic::{Player, Team};

        let system = CommandSystem::new();
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 3000;
        game_logic.add_player(player);

        let purchase_command = GameCommand {
            command_type: CommandType::PurchaseScience {
                science_name: "A10Strike1".to_string(),
            },
            player_id: 0,
            command_id: 40,
            timestamp: SystemTime::now(),
            selected_units: Vec::new(),
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            system.execute_command(&purchase_command, &mut game_logic),
            CommandResult::Success
        );

        let variant_command = GameCommand {
            command_type: CommandType::PurchaseScience {
                science_name: "a10_strike_1".to_string(),
            },
            player_id: 0,
            command_id: 41,
            timestamp: SystemTime::now(),
            selected_units: Vec::new(),
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            system.execute_command(&variant_command, &mut game_logic),
            CommandResult::InvalidCommand,
            "same science should not be charged twice when naming style differs"
        );

        let player = game_logic.get_player(0).expect("player should exist");
        assert_eq!(
            player.resources.supplies, 1500,
            "duplicate science variant should not spend supplies"
        );
        assert!(player.has_unlocked_science("a10_strike_1"));
    }

    #[test]
    fn sell_refunds_queued_production() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        with_global_data_restored(|| {
            game_engine::common::global_data::write().sell_percentage = 0.5;

            let system = CommandSystem::new();
            let mut game_logic = GameLogic::new();
            let mut player = Player::new(0, Team::USA, "USA", true);
            player.resources.supplies = 1_000;
            game_logic.add_player(player);

            let mut barracks = ThingTemplate::new("TestBarracks");
            barracks
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::Selectable)
                .set_health(1_000.0)
                .set_cost(1_000, -1);
            game_logic
                .templates
                .insert("TestBarracks".to_string(), barracks);

            let mut infantry = ThingTemplate::new("TestInfantry");
            infantry
                .add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .set_health(100.0)
                .set_cost(100, 0);
            game_logic
                .templates
                .insert("TestInfantry".to_string(), infantry);

            let barracks_id = game_logic
                .create_object("TestBarracks", Team::USA, Vec3::ZERO)
                .expect("barracks should be created");

            let queue_command = GameCommand {
                command_type: CommandType::QueueUnitCreate {
                    template_name: "TestInfantry".to_string(),
                    quantity: 1,
                },
                player_id: 0,
                command_id: 50,
                timestamp: SystemTime::now(),
                selected_units: vec![barracks_id],
                modifier_keys: ModifierKeys::default(),
            };
            assert_eq!(
                system.execute_command(&queue_command, &mut game_logic),
                CommandResult::Success
            );
            assert_eq!(
                game_logic.get_player(0).unwrap().resources.supplies,
                900,
                "queued unit should charge before selling"
            );

            let sell_command = GameCommand {
                command_type: CommandType::Sell {
                    object_id: barracks_id,
                },
                player_id: 0,
                command_id: 51,
                timestamp: SystemTime::now(),
                selected_units: vec![barracks_id],
                modifier_keys: ModifierKeys::default(),
            };
            assert_eq!(
                system.execute_command(&sell_command, &mut game_logic),
                CommandResult::Success
            );

            assert_eq!(
                game_logic.get_player(0).unwrap().resources.supplies,
                1_500,
                "selling should refund both the structure sell value and queued production"
            );
            assert!(
                game_logic
                    .find_object(barracks_id)
                    .and_then(|object| object.building_data.as_ref())
                    .map(|building| building.production_queue.is_empty())
                    .unwrap_or(true),
                "sell should drain queued production before destroying the producer"
            );
        });
    }

    #[test]
    fn sell_refund_uses_global_sell_percentage() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        with_global_data_restored(|| {
            game_engine::common::global_data::write().sell_percentage = 0.25;

            let system = CommandSystem::new();
            let mut game_logic = GameLogic::new();
            let mut player = Player::new(0, Team::USA, "USA", true);
            player.resources.supplies = 0;
            game_logic.add_player(player);

            let mut barracks = ThingTemplate::new("TestBarracks");
            barracks
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::Selectable)
                .set_health(1_000.0)
                .set_cost(1_000, -1);
            game_logic
                .templates
                .insert("TestBarracks".to_string(), barracks);

            let barracks_id = game_logic
                .create_object("TestBarracks", Team::USA, Vec3::ZERO)
                .expect("barracks should be created");

            // Re-assert sell percentage immediately before sell so the production
            // path is proven to consume the live GlobalData value under isolation.
            assert!(
                (game_engine::common::global_data::read().sell_percentage - 0.25).abs()
                    < f32::EPSILON,
                "test isolation must preserve configured SellPercentage"
            );

            let sell_command = GameCommand {
                command_type: CommandType::Sell {
                    object_id: barracks_id,
                },
                player_id: 0,
                command_id: 52,
                timestamp: SystemTime::now(),
                selected_units: vec![barracks_id],
                modifier_keys: ModifierKeys::default(),
            };
            assert_eq!(
                system.execute_command(&sell_command, &mut game_logic),
                CommandResult::Success
            );

            assert_eq!(
                game_logic.get_player(0).unwrap().resources.supplies,
                250,
                "sell refund should use GlobalData SellPercentage"
            );
        });
    }

    #[test]
    fn cancel_construction_refunds_full_build_cost() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        let system = CommandSystem::new();
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 0;
        game_logic.add_player(player);

        let mut barracks = ThingTemplate::new("TestBarracks");
        barracks
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1_000.0)
            .set_cost(1_000, -1);
        game_logic
            .templates
            .insert("TestBarracks".to_string(), barracks);

        let barracks_id = game_logic
            .create_object_under_construction("TestBarracks", Team::USA, Vec3::ZERO)
            .expect("under-construction barracks should be created");

        let cancel_command = GameCommand {
            command_type: CommandType::DozerCancelConstruct {
                object_id: barracks_id,
            },
            player_id: 0,
            command_id: 60,
            timestamp: SystemTime::now(),
            selected_units: vec![],
            modifier_keys: ModifierKeys::default(),
        };

        assert_eq!(
            system.execute_command(&cancel_command, &mut game_logic),
            CommandResult::Success
        );
        game_logic.update();

        assert!(
            game_logic.get_object(barracks_id).is_none(),
            "cancelled construction should be destroyed"
        );
        assert_eq!(
            game_logic.get_player(0).unwrap().resources.supplies,
            1_000,
            "C++ dozer cancel refunds the full build cost"
        );
    }

    #[test]
    fn cancel_construction_rejects_enemy_structure() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        let system = CommandSystem::new();
        let mut game_logic = GameLogic::new();
        let mut usa = Player::new(0, Team::USA, "USA", true);
        usa.resources.supplies = 0;
        game_logic.add_player(usa);
        let mut gla = Player::new(2, Team::GLA, "GLA", false);
        gla.resources.supplies = 0;
        game_logic.add_player(gla);

        let mut barracks = ThingTemplate::new("TestBarracks");
        barracks
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(1_000.0)
            .set_cost(1_000, -1);
        game_logic
            .templates
            .insert("TestBarracks".to_string(), barracks);

        let barracks_id = game_logic
            .create_object_under_construction("TestBarracks", Team::USA, Vec3::ZERO)
            .expect("under-construction barracks should be created");

        let cancel_command = GameCommand {
            command_type: CommandType::DozerCancelConstruct {
                object_id: barracks_id,
            },
            player_id: 2,
            command_id: 61,
            timestamp: SystemTime::now(),
            selected_units: vec![],
            modifier_keys: ModifierKeys::default(),
        };

        assert_eq!(
            system.execute_command(&cancel_command, &mut game_logic),
            CommandResult::InvalidTarget
        );
        game_logic.update();

        assert!(
            game_logic.get_object(barracks_id).is_some(),
            "enemy cancel command must not destroy the target"
        );
        assert_eq!(
            game_logic.get_player(2).unwrap().resources.supplies,
            0,
            "enemy cancel command must not refund the issuing player"
        );
    }

    #[test]
    fn cancel_upgrade_refunds_only_when_upgrade_is_queued() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        let system = CommandSystem::new();
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 3000;
        game_logic.add_player(player);

        let mut template = ThingTemplate::new("AmericaSupplyCenter");
        template
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        let producer = Object::new(template, ObjectId(301), Team::USA);
        game_logic.add_object(producer);

        let queue_command = GameCommand {
            command_type: CommandType::QueueUpgrade {
                upgrade_name: "Upgrade_AmericaSupplyLines".to_string(),
            },
            player_id: 0,
            command_id: 10,
            timestamp: SystemTime::now(),
            selected_units: vec![ObjectId(301)],
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            system.execute_command(&queue_command, &mut game_logic),
            CommandResult::Success
        );

        let cancel_command = GameCommand {
            command_type: CommandType::CancelUpgrade {
                upgrade_name: "Upgrade_AmericaSupplyLines".to_string(),
            },
            player_id: 0,
            command_id: 11,
            timestamp: SystemTime::now(),
            selected_units: vec![ObjectId(301)],
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            system.execute_command(&cancel_command, &mut game_logic),
            CommandResult::Success
        );

        let player_after_cancel = game_logic.get_player(0).expect("player should exist");
        assert_eq!(
            player_after_cancel.resources.supplies, 3000,
            "cancel should refund the queued upgrade cost"
        );
        assert!(!player_after_cancel
            .queued_upgrades
            .contains("Upgrade_AmericaSupplyLines"));

        assert_eq!(
            system.execute_command(&cancel_command, &mut game_logic),
            CommandResult::InvalidCommand,
            "cancelling a non-queued upgrade should not issue another refund"
        );
    }

    #[test]
    fn queue_upgrade_requires_constructed_building_source() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        let system = CommandSystem::new();
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 3000;
        game_logic.add_player(player);

        let mut unit_template = ThingTemplate::new("TestUnit");
        unit_template
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        game_logic.add_object(Object::new(unit_template, ObjectId(351), Team::USA));

        let command = GameCommand {
            command_type: CommandType::QueueUpgrade {
                upgrade_name: "Upgrade_AmericaSupplyLines".to_string(),
            },
            player_id: 0,
            command_id: 12,
            timestamp: SystemTime::now(),
            selected_units: vec![ObjectId(351)],
            modifier_keys: ModifierKeys::default(),
        };

        assert_eq!(
            system.execute_command(&command, &mut game_logic),
            CommandResult::InvalidCommand
        );
        let player_after = game_logic.get_player(0).expect("player should exist");
        assert_eq!(
            player_after.resources.supplies, 3000,
            "non-producing units must not charge upgrade resources"
        );
        assert!(player_after.queued_upgrades.is_empty());
    }

    #[test]
    fn queued_upgrade_completes_during_simulation_update() {
        use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

        let system = CommandSystem::new();
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 3000;
        game_logic.add_player(player);

        let mut template = ThingTemplate::new("AmericaSupplyCenter");
        template
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        let producer = Object::new(template, ObjectId(401), Team::USA);
        game_logic.add_object(producer);

        let command = GameCommand {
            command_type: CommandType::QueueUpgrade {
                upgrade_name: "Upgrade_AmericaSupplyLines".to_string(),
            },
            player_id: 0,
            command_id: 20,
            timestamp: SystemTime::now(),
            selected_units: vec![ObjectId(401)],
            modifier_keys: ModifierKeys::default(),
        };
        assert_eq!(
            system.execute_command(&command, &mut game_logic),
            CommandResult::Success
        );

        let player_after_queue = game_logic.get_player(0).expect("player should exist");
        assert!(player_after_queue
            .queued_upgrades
            .contains("Upgrade_AmericaSupplyLines"));
        assert!(!player_after_queue
            .unlocked_sciences
            .contains("Upgrade_AmericaSupplyLines"));

        game_logic.update();

        let player_after_update = game_logic
            .get_player(0)
            .expect("player should exist after update");
        assert!(!player_after_update
            .queued_upgrades
            .contains("Upgrade_AmericaSupplyLines"));
        assert!(player_after_update
            .unlocked_sciences
            .contains("Upgrade_AmericaSupplyLines"));
        assert_eq!(
            system.execute_command(&command, &mut game_logic),
            CommandResult::InvalidCommand,
            "completed upgrades should not be queued or charged again"
        );
    }
}
