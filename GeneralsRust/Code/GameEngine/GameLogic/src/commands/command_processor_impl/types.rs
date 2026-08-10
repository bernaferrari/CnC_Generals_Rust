/// Wave 275: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

/// Command execution result
#[derive(Debug, Clone)]
pub enum CommandExecutionResult {
    Success,
    Failed(AsciiString),
    Deferred, // Command needs to be tried again later
    InvalidCommand,
    InvalidGameState,
}

impl CommandExecutionResult {
    pub fn is_success(&self) -> bool {
        matches!(self, CommandExecutionResult::Success)
    }

    pub fn get_error_message(&self) -> Option<&str> {
        match self {
            CommandExecutionResult::Failed(msg) => Some(msg),
            CommandExecutionResult::InvalidCommand => Some("Invalid command"),
            CommandExecutionResult::InvalidGameState => Some("Invalid game state"),
            _ => None,
        }
    }
}

fn module_special_power_interface(
    module: &mut dyn game_engine::common::thing::module::Module,
) -> Option<&mut dyn EngineSpecialPowerModuleInterface> {
    crate::object::special_power_interface_cast::module_special_power_interface(module)
        .map(|module| module as &mut dyn EngineSpecialPowerModuleInterface)
}

fn module_special_power_update_interface(
    module: &mut dyn game_engine::common::thing::module::Module,
) -> Option<&mut dyn EngineSpecialPowerUpdateInterface> {
    crate::object::special_power_interface_cast::module_special_power_update_interface(module)
        .map(|module| module as &mut dyn EngineSpecialPowerUpdateInterface)
}

/// Command execution statistics
#[derive(Debug, Clone, Default)]
pub struct CommandExecutionStats {
    pub commands_processed: u64,
    pub commands_succeeded: u64,
    pub commands_failed: u64,
    pub commands_deferred: u64,
    pub average_execution_time_ms: f64,
}

/// Command handler trait - allows pluggable command execution
pub trait CommandHandler {
    /// Execute a command and return the result
    fn execute_command(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult;

    /// Check if this handler can process the given command type
    fn can_handle(&self, command_type: CommandType) -> bool;

    /// Get handler priority (higher priority handlers are tried first)
    fn get_priority(&self) -> i32 {
        0
    }
}

/// Context passed to command handlers during execution
pub struct CommandExecutionContext {
    /// Current game frame
    pub current_frame: UnsignedInt,

    /// Player executing the command
    pub player_id: Int,

    /// Game state interfaces
    pub object_manager: Option<Arc<RwLock<dyn ObjectManager>>>,
    pub player_manager: Option<Arc<RwLock<dyn PlayerManager>>>,
    pub ai_manager: Option<Arc<RwLock<dyn AIManager>>>,

    /// Execution metadata
    pub execution_start_time: Instant,
    pub is_network_command: bool,
    pub is_replay_command: bool,
}

pub struct SelectionCommandHandler;

impl SelectionCommandHandler {
    pub fn new() -> Self {
        Self
    }

    fn execute_create_selected_group(
        &self,
        command: &Command,
        context: &CommandExecutionContext,
    ) -> CommandExecutionResult {
        use super::command::CommandArgumentType;
        use crate::commands::SelectionType;

        let create_new = matches!(
            command.get_argument(0),
            Some(CommandArgumentType::Boolean(true))
        );
        let selection_type = if create_new {
            SelectionType::Replace
        } else {
            SelectionType::Add
        };

        // Collect object IDs using iterator collect instead of manual loop
        let object_ids: Vec<ObjectID> = (1..(command.get_argument_count() as Int))
            .filter_map(|idx| {
                if let Some(CommandArgumentType::ObjectID(id)) = command.get_argument(idx) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        let selection_manager = get_selection_manager();
        let Ok(mut manager) = selection_manager.write() else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Selection manager lock poisoned",
            ));
        };
        let Some(selection) = manager.get_player_selection(context.player_id) else {
            return CommandExecutionResult::Failed(AsciiString::from("No player selection"));
        };

        if object_ids.is_empty() && selection_type == SelectionType::Replace {
            selection.clear_selection();
            return CommandExecutionResult::Success;
        }

        if selection.select_objects(object_ids, selection_type) {
            CommandExecutionResult::Success
        } else {
            CommandExecutionResult::Failed(AsciiString::from("Selection update failed"))
        }
    }

    fn execute_remove_from_selected_group(
        &self,
        command: &Command,
        context: &CommandExecutionContext,
    ) -> CommandExecutionResult {
        use super::command::CommandArgumentType;
        use crate::commands::SelectionType;

        let object_ids: Vec<_> = (0..(command.get_argument_count() as Int))
            .filter_map(|idx| match command.get_argument(idx) {
                Some(CommandArgumentType::ObjectID(id)) => Some(*id),
                _ => None,
            })
            .collect();

        if object_ids.is_empty() {
            return CommandExecutionResult::Success;
        }

        let selection_manager = get_selection_manager();
        let Ok(mut manager) = selection_manager.write() else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Selection manager lock poisoned",
            ));
        };
        let Some(selection) = manager.get_player_selection(context.player_id) else {
            return CommandExecutionResult::Failed(AsciiString::from("No player selection"));
        };

        if selection.select_objects(object_ids, SelectionType::Remove) {
            CommandExecutionResult::Success
        } else {
            CommandExecutionResult::Failed(AsciiString::from("Selection removal failed"))
        }
    }

    fn execute_destroy_selected_group(
        &self,
        context: &CommandExecutionContext,
    ) -> CommandExecutionResult {
        let selection_manager = get_selection_manager();
        let Ok(mut manager) = selection_manager.write() else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Selection manager lock poisoned",
            ));
        };
        let Some(selection) = manager.get_player_selection(context.player_id) else {
            return CommandExecutionResult::Failed(AsciiString::from("No player selection"));
        };
        selection.clear_selection();
        CommandExecutionResult::Success
    }

    fn execute_area_selection(
        &self,
        command: &Command,
        context: &CommandExecutionContext,
    ) -> CommandExecutionResult {
        use super::command::CommandArgumentType;
        use crate::commands::SelectionType;

        let Some(CommandArgumentType::PixelRegion(region)) = command.get_argument(0) else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "AreaSelection missing region",
            ));
        };

        let Some(object_manager) = &context.object_manager else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "AreaSelection missing object manager",
            ));
        };
        let Ok(object_manager) = object_manager.read() else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Object manager lock poisoned",
            ));
        };
        let object_ids = object_manager.get_objects_in_region(region);
        drop(object_manager);

        let selection_manager = get_selection_manager();
        let Ok(mut manager) = selection_manager.write() else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Selection manager lock poisoned",
            ));
        };
        let Some(selection) = manager.get_player_selection(context.player_id) else {
            return CommandExecutionResult::Failed(AsciiString::from("No player selection"));
        };

        if object_ids.is_empty() {
            selection.clear_selection();
            return CommandExecutionResult::Success;
        }

        if selection.select_objects(object_ids, SelectionType::Replace) {
            CommandExecutionResult::Success
        } else {
            CommandExecutionResult::Failed(AsciiString::from("Area selection update failed"))
        }
    }

    fn execute_team_command(
        &self,
        command_type: CommandType,
        context: &CommandExecutionContext,
    ) -> CommandExecutionResult {
        use crate::commands::MAX_CONTROL_GROUPS;

        let group_index = match command_type {
            CommandType::CreateTeam0 | CommandType::SelectTeam0 | CommandType::AddTeam0 => 0,
            CommandType::CreateTeam1 | CommandType::SelectTeam1 | CommandType::AddTeam1 => 1,
            CommandType::CreateTeam2 | CommandType::SelectTeam2 | CommandType::AddTeam2 => 2,
            CommandType::CreateTeam3 | CommandType::SelectTeam3 | CommandType::AddTeam3 => 3,
            CommandType::CreateTeam4 | CommandType::SelectTeam4 | CommandType::AddTeam4 => 4,
            CommandType::CreateTeam5 | CommandType::SelectTeam5 | CommandType::AddTeam5 => 5,
            CommandType::CreateTeam6 | CommandType::SelectTeam6 | CommandType::AddTeam6 => 6,
            CommandType::CreateTeam7 | CommandType::SelectTeam7 | CommandType::AddTeam7 => 7,
            CommandType::CreateTeam8 | CommandType::SelectTeam8 | CommandType::AddTeam8 => 8,
            CommandType::CreateTeam9 | CommandType::SelectTeam9 | CommandType::AddTeam9 => 9,
            _ => return CommandExecutionResult::InvalidCommand,
        };

        if group_index >= MAX_CONTROL_GROUPS {
            return CommandExecutionResult::InvalidCommand;
        }

        let selection_manager = get_selection_manager();
        let Ok(mut manager) = selection_manager.write() else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Selection manager lock poisoned",
            ));
        };
        let Some(selection) = manager.get_player_selection(context.player_id) else {
            return CommandExecutionResult::Failed(AsciiString::from("No player selection"));
        };

        let ok = match command_type {
            CommandType::CreateTeam0
            | CommandType::CreateTeam1
            | CommandType::CreateTeam2
            | CommandType::CreateTeam3
            | CommandType::CreateTeam4
            | CommandType::CreateTeam5
            | CommandType::CreateTeam6
            | CommandType::CreateTeam7
            | CommandType::CreateTeam8
            | CommandType::CreateTeam9 => selection.create_control_group(group_index),
            CommandType::SelectTeam0
            | CommandType::SelectTeam1
            | CommandType::SelectTeam2
            | CommandType::SelectTeam3
            | CommandType::SelectTeam4
            | CommandType::SelectTeam5
            | CommandType::SelectTeam6
            | CommandType::SelectTeam7
            | CommandType::SelectTeam8
            | CommandType::SelectTeam9 => selection.select_control_group(group_index, false),
            CommandType::AddTeam0
            | CommandType::AddTeam1
            | CommandType::AddTeam2
            | CommandType::AddTeam3
            | CommandType::AddTeam4
            | CommandType::AddTeam5
            | CommandType::AddTeam6
            | CommandType::AddTeam7
            | CommandType::AddTeam8
            | CommandType::AddTeam9 => selection.add_to_control_group(group_index),
            _ => false,
        };

        if ok {
            CommandExecutionResult::Success
        } else {
            CommandExecutionResult::Failed(AsciiString::from("Control group command failed"))
        }
    }
}

impl Default for SelectionCommandHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandHandler for SelectionCommandHandler {
    fn execute_command(
        &mut self,
        queued: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        match queued.command.get_type() {
            CommandType::CreateSelectedGroup | CommandType::CreateSelectedGroupNoSound => {
                self.execute_create_selected_group(&queued.command, context)
            }
            CommandType::RemoveFromSelectedGroup => {
                self.execute_remove_from_selected_group(&queued.command, context)
            }
            CommandType::DestroySelectedGroup => self.execute_destroy_selected_group(context),
            CommandType::AreaSelection => self.execute_area_selection(&queued.command, context),
            CommandType::CreateTeam0
            | CommandType::CreateTeam1
            | CommandType::CreateTeam2
            | CommandType::CreateTeam3
            | CommandType::CreateTeam4
            | CommandType::CreateTeam5
            | CommandType::CreateTeam6
            | CommandType::CreateTeam7
            | CommandType::CreateTeam8
            | CommandType::CreateTeam9
            | CommandType::SelectTeam0
            | CommandType::SelectTeam1
            | CommandType::SelectTeam2
            | CommandType::SelectTeam3
            | CommandType::SelectTeam4
            | CommandType::SelectTeam5
            | CommandType::SelectTeam6
            | CommandType::SelectTeam7
            | CommandType::SelectTeam8
            | CommandType::SelectTeam9
            | CommandType::AddTeam0
            | CommandType::AddTeam1
            | CommandType::AddTeam2
            | CommandType::AddTeam3
            | CommandType::AddTeam4
            | CommandType::AddTeam5
            | CommandType::AddTeam6
            | CommandType::AddTeam7
            | CommandType::AddTeam8
            | CommandType::AddTeam9 => {
                self.execute_team_command(queued.command.get_type(), context)
            }
            _ => CommandExecutionResult::InvalidCommand,
        }
    }

    fn can_handle(&self, command_type: CommandType) -> bool {
        matches!(
            command_type,
            CommandType::CreateSelectedGroup
                | CommandType::CreateSelectedGroupNoSound
                | CommandType::DestroySelectedGroup
                | CommandType::RemoveFromSelectedGroup
                | CommandType::AreaSelection
                | CommandType::CreateTeam0
                | CommandType::CreateTeam1
                | CommandType::CreateTeam2
                | CommandType::CreateTeam3
                | CommandType::CreateTeam4
                | CommandType::CreateTeam5
                | CommandType::CreateTeam6
                | CommandType::CreateTeam7
                | CommandType::CreateTeam8
                | CommandType::CreateTeam9
                | CommandType::SelectTeam0
                | CommandType::SelectTeam1
                | CommandType::SelectTeam2
                | CommandType::SelectTeam3
                | CommandType::SelectTeam4
                | CommandType::SelectTeam5
                | CommandType::SelectTeam6
                | CommandType::SelectTeam7
                | CommandType::SelectTeam8
                | CommandType::SelectTeam9
                | CommandType::AddTeam0
                | CommandType::AddTeam1
                | CommandType::AddTeam2
                | CommandType::AddTeam3
                | CommandType::AddTeam4
                | CommandType::AddTeam5
                | CommandType::AddTeam6
                | CommandType::AddTeam7
                | CommandType::AddTeam8
                | CommandType::AddTeam9
        )
    }

    fn get_priority(&self) -> i32 {
        200
    }
}

/// Trait for object management interface
pub trait ObjectManager: Send + Sync {
    fn get_object(&self, id: ObjectID) -> Option<Arc<dyn GameObject>>;
    fn get_objects_in_region(&self, region: &IRegion2D) -> Vec<ObjectID>;
    fn create_object(
        &mut self,
        template: &str,
        position: Coord3D,
        player_id: Int,
    ) -> Option<ObjectID>;
    fn destroy_object(&mut self, id: ObjectID) -> bool;
}

/// Trait for player management interface  
pub trait PlayerManager: Send + Sync {
    fn get_player_resources(&self, player_id: Int) -> Option<PlayerResources>;
    fn modify_player_resources(&mut self, player_id: Int, supplies: Int, power: Int);
    fn can_player_afford(&self, player_id: Int, cost: &ResourceCost) -> bool;
}

/// Trait for AI management interface
pub trait AIManager: Send + Sync {
    fn issue_move_order(&mut self, objects: &[ObjectID], destination: Coord3D) -> bool;
    fn issue_waypoint_order(&mut self, objects: &[ObjectID], destination: Coord3D) -> bool {
        self.issue_move_order(objects, destination)
    }
    fn issue_attack_move_order(&mut self, objects: &[ObjectID], destination: Coord3D) -> bool {
        self.issue_move_order(objects, destination)
    }
    fn issue_attack_order(&mut self, attackers: &[ObjectID], target: ObjectID) -> bool;
    fn issue_build_order(&mut self, builder: ObjectID, template: &str, position: Coord3D) -> bool;
    fn issue_stop_order(&mut self, objects: &[ObjectID]) -> bool;
    fn issue_targeted_order(
        &mut self,
        objects: &[ObjectID],
        target: ObjectID,
        command: crate::ai::AiCommandType,
    ) -> bool;
    fn issue_guard_position_order(
        &mut self,
        objects: &[ObjectID],
        position: Coord3D,
        guard_mode: crate::ai::GuardMode,
    ) -> bool;
    fn issue_guard_object_order(
        &mut self,
        objects: &[ObjectID],
        target: ObjectID,
        guard_mode: crate::ai::GuardMode,
    ) -> bool;
    /// C++ parity: AIUpdateInterface::queueWaypoint() — queue a waypoint without starting execution
    fn queue_waypoint_for_object(&mut self, _object_id: ObjectID, _pos: Coord3D) {}
    /// C++ parity: AIUpdateInterface::executeWaypointQueue() — start executing queued waypoints
    fn execute_waypoint_queue_for_object(&mut self, _object_id: ObjectID) {}
}

/// Trait for game object interface
pub trait GameObject: Send + Sync {
    fn get_id(&self) -> ObjectID;
    fn get_position(&self) -> Coord3D;
    fn get_owner(&self) -> Int;
    fn is_alive(&self) -> bool;
    fn can_be_controlled_by(&self, player_id: Int) -> bool;
}

/// Resource cost structure
#[derive(Debug, Clone)]
pub struct ResourceCost {
    pub supplies: Int,
    pub power: Int,
}

/// Player resources
#[derive(Debug, Clone)]
pub struct PlayerResources {
    pub supplies: Int,
    pub power_available: Int,
    pub power_used: Int,
}

/// C++ parity: MAX_PATH_SUBJECTS from GameLogicDispatch.cpp line 72
const MAX_PATH_SUBJECTS: usize = 64;

/// Default command handler - implements basic RTS command execution
pub struct DefaultCommandHandler {
    validator: RtsCommandValidator,
    stats: CommandExecutionStats,
    /// C++ parity: build-plan state for shift-click waypoint chaining
    /// (theBuildPlan / thePlanSubject[] from GameLogicDispatch.cpp lines 73-75)
    build_plan_active: bool,
    build_plan_subjects: Vec<ObjectID>,
}

static NEXT_FORMATION_ID: AtomicU32 = AtomicU32::new(1);
