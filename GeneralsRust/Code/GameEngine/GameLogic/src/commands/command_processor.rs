////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Command Processor - Command execution engine
//!
//! This module provides the command execution system that processes
//! commands from the queue and translates them into game actions.
//! Matches C++ command processing and GameLogic integration.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use super::command::{Command, CommandType, CommandValidation};
use super::command_queue::{
    get_command_queue_manager, CommandExecutionState, CommandPriority, QueuedCommand,
};
use super::rts_command::{RtsCommand, RtsCommandValidator};
use crate::action_manager::TheActionManager;
use crate::commands::get_selection_manager;
use crate::common::{
    audio::AudioEventRts, AsciiString, Bool, CommandSourceType, Coord3D, DrawableID, EvaEvent,
    FormationID, ICoord2D, IRegion2D, Int, KindOf, ObjectID, PlayerMaskType, Real, Relationship,
    UnsignedInt,
};
use crate::control_bar;
use crate::helpers::{
    TheAudio, TheEva, TheGameLogic, TheGameText, TheInGameUI, TheTerrainLogic, TheThingFactory,
};
use crate::modules::{
    AIUpdateInterfaceExt, ContainModuleInterfaceExt,
    SpecialPowerModuleInterface as EngineSpecialPowerModuleInterface,
    SpecialPowerUpdateInterface as EngineSpecialPowerUpdateInterface,
};
use crate::object::object_factory::{get_object_factory, GameObjectInstance};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object_manager::get_object_manager;
use crate::player::player_list;
use crate::system::beacon_manager::get_beacon_manager;
use crate::weapon::{WeaponLockType, WeaponSetType, WeaponSlotType, NO_MAX_SHOTS_LIMIT};
use game_engine::common::game_engine::get_game_engine;
use game_engine::common::ini::get_global_data as get_engine_global_data;
use game_engine::common::ini::ini_multiplayer::with_multiplayer_settings;
use game_engine::common::rts::{ScienceType, SCIENCE_INVALID};
use game_engine::common::system::radar::{
    get_radar_system, Coord3D as RadarCoord3D, RadarEventType,
};

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

/// Default command handler - implements basic RTS command execution
pub struct DefaultCommandHandler {
    validator: RtsCommandValidator,
    stats: CommandExecutionStats,
}

static NEXT_FORMATION_ID: AtomicU32 = AtomicU32::new(1);

impl DefaultCommandHandler {
    const BEACON_MATCH_THRESHOLD: Real = 3.0;

    pub fn new() -> Self {
        Self {
            validator: RtsCommandValidator::new(),
            stats: CommandExecutionStats::default(),
        }
    }

    /// Execute movement command
    fn execute_move_command(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Extract movement parameters
        let mut target_position = None;
        let mut object_ids = Vec::new();

        for i in 0..command.command.get_argument_count() {
            if let Some(arg) = command.command.get_argument(i as Int) {
                match arg {
                    crate::commands::command::CommandArgumentType::Location(pos) => {
                        if target_position.is_none() {
                            target_position = Some(*pos);
                        }
                    }
                    crate::commands::command::CommandArgumentType::ObjectID(id) => {
                        object_ids.push(*id);
                    }
                    _ => {}
                }
            }
        }

        let position = match target_position {
            Some(pos) => pos,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "No target position specified",
                ))
            }
        };

        if object_ids.is_empty() {
            let selection_manager = get_selection_manager();
            object_ids = match selection_manager.read() {
                Ok(manager) => manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
                    .unwrap_or_default(),
                Err(_) => Vec::new(),
            };
        }

        if object_ids.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from(
                "No objects specified for movement",
            ));
        }

        // Validate objects exist and are controllable
        if let Some(object_manager) = &context.object_manager {
            if let Ok(om) = object_manager.read() {
                for object_id in &object_ids {
                    if let Some(obj) = om.get_object(*object_id) {
                        if !obj.is_alive() {
                            return CommandExecutionResult::Failed(AsciiString::from(&format!(
                                "Object {} is not alive",
                                object_id
                            )));
                        }
                        if !obj.can_be_controlled_by(context.player_id) {
                            return CommandExecutionResult::Failed(AsciiString::from(&format!(
                                "Player {} cannot control object {}",
                                context.player_id, object_id
                            )));
                        }
                    } else {
                        return CommandExecutionResult::Failed(AsciiString::from(&format!(
                            "Object {} not found",
                            object_id
                        )));
                    }
                }
            }
        }

        // Issue move order to AI system
        if let Some(ai_manager) = &context.ai_manager {
            if let Ok(mut ai) = ai_manager.write() {
                let accepted = match command.command.get_type() {
                    CommandType::AddWaypoint => ai.issue_waypoint_order(&object_ids, position),
                    CommandType::DoAttackMoveTo => {
                        ai.issue_attack_move_order(&object_ids, position)
                    }
                    _ => ai.issue_move_order(&object_ids, position),
                };

                if accepted {
                    CommandExecutionResult::Success
                } else {
                    CommandExecutionResult::Failed(AsciiString::from(
                        "AI system failed to process move order",
                    ))
                }
            } else {
                CommandExecutionResult::Failed(AsciiString::from("Cannot access AI manager"))
            }
        } else {
            CommandExecutionResult::Failed(AsciiString::from("AI manager not available"))
        }
    }

    /// Execute attack command
    fn execute_attack_command(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let mut ids = Vec::new();
        for i in 0..command.command.get_argument_count() {
            if let Some(crate::commands::command::CommandArgumentType::ObjectID(id)) =
                command.command.get_argument(i as Int)
            {
                ids.push(*id);
            }
        }

        let target = match ids.first().copied() {
            Some(id) => id,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from("No target specified"));
            }
        };

        // C++ attack commands act on the currently selected group and carry only the target id.
        // Allow explicit attacker lists as an override for legacy/test paths.
        let mut attacker_ids: Vec<ObjectID> = ids.iter().skip(1).copied().collect();
        if attacker_ids.is_empty() {
            let selection_manager = get_selection_manager();
            let selected = {
                match selection_manager.read() {
                    Ok(manager) => manager
                        .get_player_selection_ref(context.player_id)
                        .map(|selection| selection.get_selected_objects())
                        .unwrap_or_default(),
                    Err(_) => Vec::new(),
                }
            };
            if !selected.is_empty() {
                attacker_ids = selected;
            }
        }

        if attacker_ids.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from("No attackers specified"));
        }

        // Validate target exists
        if let Some(object_manager) = &context.object_manager {
            if let Ok(om) = object_manager.read() {
                if let Some(target_obj) = om.get_object(target) {
                    if !target_obj.is_alive() {
                        return CommandExecutionResult::Failed(AsciiString::from(
                            "Target is not alive",
                        ));
                    }
                } else {
                    return CommandExecutionResult::Failed(AsciiString::from("Target not found"));
                }

                // Validate attackers
                for attacker_id in &attacker_ids {
                    if let Some(obj) = om.get_object(*attacker_id) {
                        if !obj.is_alive() {
                            return CommandExecutionResult::Failed(AsciiString::from(&format!(
                                "Attacker {} is not alive",
                                attacker_id
                            )));
                        }
                        if !obj.can_be_controlled_by(context.player_id) {
                            return CommandExecutionResult::Failed(AsciiString::from(&format!(
                                "Player {} cannot control attacker {}",
                                context.player_id, attacker_id
                            )));
                        }
                    } else {
                        return CommandExecutionResult::Failed(AsciiString::from(&format!(
                            "Attacker {} not found",
                            attacker_id
                        )));
                    }
                }
            }
        }

        // Issue attack order to AI system
        if let Some(ai_manager) = &context.ai_manager {
            if let Ok(mut ai) = ai_manager.write() {
                if ai.issue_attack_order(&attacker_ids, target) {
                    CommandExecutionResult::Success
                } else {
                    CommandExecutionResult::Failed(AsciiString::from(
                        "AI system failed to process attack order",
                    ))
                }
            } else {
                CommandExecutionResult::Failed(AsciiString::from("Cannot access AI manager"))
            }
        } else {
            CommandExecutionResult::Failed(AsciiString::from("AI manager not available"))
        }
    }

    fn execute_targeted_group_command(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
        ai_command: crate::ai::AiCommandType,
        failure_label: &'static str,
    ) -> CommandExecutionResult {
        let (target, mut object_ids) = self.extract_target_and_sources(command);
        let target = match target {
            Some(id) => id,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from(&format!(
                    "No target specified for {}",
                    failure_label
                )))
            }
        };

        if object_ids.is_empty() {
            let selection_manager = get_selection_manager();
            object_ids = match selection_manager.read() {
                Ok(manager) => manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
                    .unwrap_or_default(),
                Err(_) => Vec::new(),
            };
        }

        if object_ids.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from(&format!(
                "No objects specified for {}",
                failure_label
            )));
        }

        if let Some(object_manager) = &context.object_manager {
            if let Ok(om) = object_manager.read() {
                if let Some(target_obj) = om.get_object(target) {
                    if !target_obj.is_alive() {
                        return CommandExecutionResult::Failed(AsciiString::from(
                            "Target is not alive",
                        ));
                    }
                } else {
                    return CommandExecutionResult::Failed(AsciiString::from("Target not found"));
                }

                for object_id in &object_ids {
                    if let Some(obj) = om.get_object(*object_id) {
                        if !obj.is_alive() {
                            return CommandExecutionResult::Failed(AsciiString::from(&format!(
                                "Object {} is not alive",
                                object_id
                            )));
                        }
                        if !obj.can_be_controlled_by(context.player_id) {
                            return CommandExecutionResult::Failed(AsciiString::from(&format!(
                                "Player {} cannot control object {}",
                                context.player_id, object_id
                            )));
                        }
                    } else {
                        return CommandExecutionResult::Failed(AsciiString::from(&format!(
                            "Object {} not found",
                            object_id
                        )));
                    }
                }
            }
        }

        if let Some(ai_manager) = &context.ai_manager {
            if let Ok(mut ai) = ai_manager.write() {
                if ai.issue_targeted_order(&object_ids, target, ai_command) {
                    CommandExecutionResult::Success
                } else {
                    CommandExecutionResult::Failed(AsciiString::from(&format!(
                        "AI system failed to process {} order",
                        failure_label
                    )))
                }
            } else {
                CommandExecutionResult::Failed(AsciiString::from("Cannot access AI manager"))
            }
        } else {
            CommandExecutionResult::Failed(AsciiString::from("AI manager not available"))
        }
    }

    /// Execute construction command
    fn execute_build_command(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        use game_engine::common::system::build_assistant;

        let mut builder_id: Option<ObjectID> = None;
        let mut build_template_id: Option<u32> = None;
        let mut build_template_name: Option<AsciiString> = None;
        let mut build_positions: Vec<Coord3D> = Vec::new();
        let mut build_angle: Option<Real> = None;

        for i in 0..command.command.get_argument_count() {
            if let Some(arg) = command.command.get_argument(i as Int) {
                match arg {
                    crate::commands::command::CommandArgumentType::ObjectID(id) => {
                        if builder_id.is_none() {
                            builder_id = Some(*id);
                        }
                    }
                    crate::commands::command::CommandArgumentType::Integer(value) => {
                        if build_template_id.is_none() {
                            build_template_id = Some(*value as u32);
                        }
                    }
                    crate::commands::command::CommandArgumentType::AsciiString(template) => {
                        if build_template_name.is_none() {
                            build_template_name = Some(template.clone());
                        }
                    }
                    crate::commands::command::CommandArgumentType::Location(pos) => {
                        build_positions.push(*pos);
                    }
                    crate::commands::command::CommandArgumentType::Real(real) => {
                        if build_angle.is_none() {
                            build_angle = Some(*real);
                        }
                    }
                    _ => {}
                }
            }
        }

        let builder = match builder_id {
            Some(id) => id,
            None => {
                let selection_manager = get_selection_manager();
                let mut selected_ids = Vec::new();
                if let Ok(manager) = selection_manager.read() {
                    if let Some(selection) = manager.get_player_selection_ref(context.player_id) {
                        selected_ids = selection.get_selected_objects();
                    }
                }

                let mut selected_builder = None;
                for object_id in &selected_ids {
                    if let Some(object_arc) = TheGameLogic::find_object_by_id(*object_id) {
                        if let Ok(object_guard) = object_arc.read() {
                            if object_guard.is_kind_of(KindOf::Dozer) {
                                selected_builder = Some(*object_id);
                                break;
                            }
                        }
                    }
                }

                let builder_id = selected_builder.or_else(|| selected_ids.first().copied());
                let Some(builder_id) = builder_id else {
                    return CommandExecutionResult::Failed(AsciiString::from(
                        "No builder specified",
                    ));
                };
                builder_id
            }
        };

        let template = match (build_template_id, build_template_name.as_ref()) {
            (Some(id), _) => TheThingFactory::find_template_by_id(id),
            (_, Some(name)) => TheThingFactory::find_template(name.as_str()),
            _ => None,
        };
        let Some(template) = template else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "No building template specified",
            ));
        };

        let angle = build_angle.unwrap_or(0.0);

        let (start_pos, end_pos) = match command.command.get_type() {
            CommandType::DozerConstructLine => {
                if build_positions.len() < 2 {
                    return CommandExecutionResult::Failed(AsciiString::from(
                        "No build line end specified",
                    ));
                }
                (build_positions[0], Some(build_positions[1]))
            }
            _ => {
                let Some(pos) = build_positions.first().copied() else {
                    return CommandExecutionResult::Failed(AsciiString::from(
                        "No build position specified",
                    ));
                };
                (pos, None)
            }
        };

        // Validate builder exists and is controllable
        let Some(builder_arc) = TheGameLogic::find_object_by_id(builder) else {
            return CommandExecutionResult::Failed(AsciiString::from("Builder not found"));
        };
        let Ok(builder_guard) = builder_arc.read() else {
            return CommandExecutionResult::Failed(AsciiString::from("Builder lock poisoned"));
        };
        if builder_guard.is_effectively_dead() {
            return CommandExecutionResult::Failed(AsciiString::from("Builder is not alive"));
        }
        let builder_owner = builder_guard
            .get_controlling_player_id()
            .map(|id| id as Int)
            .unwrap_or(-1);
        if builder_owner != -1 && builder_owner != context.player_id {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Player cannot control builder",
            ));
        }

        // Check resources (matches C++ ThingTemplate::getBuildCost behavior).
        let build_cost = ResourceCost {
            supplies: template.get_build_cost(),
            power: 0,
        };
        if let Some(player_manager) = &context.player_manager {
            if let Ok(pm) = player_manager.read() {
                if !pm.can_player_afford(context.player_id, &build_cost) {
                    return CommandExecutionResult::Failed(AsciiString::from(
                        "Insufficient resources",
                    ));
                }
            }
        }

        let player_index = if let Some(player_arc) = builder_guard.get_controlling_player() {
            if let Ok(player_guard) = player_arc.read() {
                player_guard.get_player_index() as u32
            } else {
                context.player_id as u32
            }
        } else {
            context.player_id as u32
        };

        let builder_snapshot = build_assistant::Object {
            id: builder_guard.get_id(),
            position: build_assistant::Coord3D {
                x: builder_guard.get_position().x,
                y: builder_guard.get_position().y,
                z: builder_guard.get_position().z,
            },
            orientation: builder_guard.get_orientation(),
        };
        let owning_player = build_assistant::Player { player_index };

        let mut assistant_template =
            build_assistant::ThingTemplate::new(template.get_name().as_str());
        let template_geometry = template.get_template_geometry_info();
        assistant_template.geometry_info.major_radius =
            template_geometry.get_major_radius().max(1.0);
        assistant_template.geometry_info.minor_radius =
            template_geometry.get_minor_radius().max(1.0);
        assistant_template.geometry_info.height =
            template_geometry.get_max_height_above_position().max(1.0);

        let Some(assistant) = build_assistant::get_build_assistant() else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Build assistant unavailable",
            ));
        };

        let mut _build_success = false;
        match end_pos {
            Some(end) => {
                assistant.build_object_line_now(
                    Some(&builder_snapshot),
                    &assistant_template,
                    &build_assistant::Coord3D {
                        x: start_pos.x,
                        y: start_pos.y,
                        z: start_pos.z,
                    },
                    &build_assistant::Coord3D {
                        x: end.x,
                        y: end.y,
                        z: end.z,
                    },
                    angle as f32,
                    &owning_player,
                );
                _build_success = true;
            }
            None => {
                let built = assistant.build_object_now(
                    Some(&builder_snapshot),
                    &assistant_template,
                    &build_assistant::Coord3D {
                        x: start_pos.x,
                        y: start_pos.y,
                        z: start_pos.z,
                    },
                    angle as f32,
                    &owning_player,
                );
                _build_success = built.is_some();
            }
        }

        if _build_success {
            let mut place_event = AudioEventRts::new("PlaceBuilding");
            place_event.set_object_id(builder);
            if let Some(audio) = TheAudio::get() {
                let _ = audio.add_audio_event(&place_event);
            }

            if let Some(player_manager) = &context.player_manager {
                if let Ok(mut pm) = player_manager.write() {
                    pm.modify_player_resources(
                        context.player_id,
                        -build_cost.supplies,
                        -build_cost.power,
                    );
                }
            }

            CommandExecutionResult::Success
        } else {
            CommandExecutionResult::Failed(AsciiString::from("Build failed"))
        }
    }

    fn execute_sell_command(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        use game_engine::common::system::build_assistant;

        let mut object_ids = Vec::new();
        for i in 0..command.command.get_argument_count() {
            if let Some(crate::commands::command::CommandArgumentType::ObjectID(id)) =
                command.command.get_argument(i as Int)
            {
                object_ids.push(*id);
            }
        }

        if object_ids.is_empty() {
            let selection_manager = get_selection_manager();
            object_ids = match selection_manager.read() {
                Ok(manager) => manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
                    .unwrap_or_default(),
                Err(_) => Vec::new(),
            };
        }

        let Some(mut assistant) = build_assistant::get_build_assistant() else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Build assistant unavailable",
            ));
        };
        let current_frame = TheGameLogic::get_frame();

        for object_id in object_ids {
            let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(object_guard) = object_arc.read() else {
                return CommandExecutionResult::Failed(AsciiString::from("Object lock poisoned"));
            };

            let owner = object_guard
                .get_controlling_player_id()
                .map(|id| id as Int)
                .unwrap_or(-1);
            if owner != -1 && owner != context.player_id {
                continue;
            }

            let sell_object = build_assistant::Object {
                id: object_guard.get_id(),
                position: build_assistant::Coord3D {
                    x: object_guard.get_position().x,
                    y: object_guard.get_position().y,
                    z: object_guard.get_position().z,
                },
                orientation: object_guard.get_orientation(),
            };
            assistant.sell_object(&sell_object, current_frame);
        }

        CommandExecutionResult::Success
    }

    fn execute_set_rally_point(
        &mut self,
        command: &QueuedCommand,
        _context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let object_id = command.command.get_argument(0).and_then(|arg| match arg {
            crate::commands::command::CommandArgumentType::ObjectID(id) => Some(*id),
            _ => None,
        });
        let destination = command.command.get_argument(1).and_then(|arg| match arg {
            crate::commands::command::CommandArgumentType::Location(pos) => Some(*pos),
            _ => None,
        });

        let (Some(object_id), Some(destination)) = (object_id, destination) else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "SetRallyPoint missing object or destination",
            ));
        };

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return CommandExecutionResult::Success;
        };
        let Ok(mut object_guard) = object_arc.write() else {
            return CommandExecutionResult::Failed(AsciiString::from("Object lock poisoned"));
        };

        let _ = object_guard.set_rally_point(&destination);
        CommandExecutionResult::Success
    }

    fn execute_set_mine_clearing_detail(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let mut object_ids = Vec::new();
        for i in 0..command.command.get_argument_count() {
            if let Some(crate::commands::command::CommandArgumentType::ObjectID(id)) =
                command.command.get_argument(i as Int)
            {
                object_ids.push(*id);
            }
        }

        if object_ids.is_empty() {
            let selection_manager = get_selection_manager();
            object_ids = match selection_manager.read() {
                Ok(manager) => manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
                    .unwrap_or_default(),
                Err(_) => Vec::new(),
            };
        }

        for object_id in object_ids {
            let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(mut object_guard) = object_arc.write() else {
                return CommandExecutionResult::Failed(AsciiString::from("Object lock poisoned"));
            };

            object_guard.set_weapon_set_flag(WeaponSetType::MineClearingDetail);
        }

        CommandExecutionResult::Success
    }

    /// Execute stop command
    fn execute_stop_command(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let mut object_ids = Vec::new();

        for i in 0..command.command.get_argument_count() {
            if let Some(arg) = command.command.get_argument(i as Int) {
                match arg {
                    crate::commands::command::CommandArgumentType::ObjectID(id) => {
                        object_ids.push(*id);
                    }
                    _ => {}
                }
            }
        }

        if object_ids.is_empty() {
            let selection_manager = get_selection_manager();
            object_ids = match selection_manager.read() {
                Ok(manager) => manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
                    .unwrap_or_default(),
                Err(_) => Vec::new(),
            };
        }

        if object_ids.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from("No objects specified"));
        }

        // Issue stop order to AI system
        if let Some(ai_manager) = &context.ai_manager {
            if let Ok(mut ai) = ai_manager.write() {
                if ai.issue_stop_order(&object_ids) {
                    CommandExecutionResult::Success
                } else {
                    CommandExecutionResult::Failed(AsciiString::from(
                        "AI system failed to process stop order",
                    ))
                }
            } else {
                CommandExecutionResult::Failed(AsciiString::from("Cannot access AI manager"))
            }
        } else {
            CommandExecutionResult::Failed(AsciiString::from("AI manager not available"))
        }
    }

    /// Execute scatter command (stop + jittered move targets)
    fn execute_scatter_command(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let mut object_ids = Vec::new();
        for i in 0..command.command.get_argument_count() {
            if let Some(arg) = command.command.get_argument(i as Int) {
                if let crate::commands::command::CommandArgumentType::ObjectID(id) = arg {
                    object_ids.push(*id);
                }
            }
        }

        if object_ids.is_empty() {
            let selection_manager = get_selection_manager();
            object_ids = match selection_manager.read() {
                Ok(manager) => manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
                    .unwrap_or_default(),
                Err(_) => Vec::new(),
            };
        }

        if object_ids.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from("No objects to scatter"));
        }

        // Capture current positions for per-unit offsets
        let mut positions: Vec<(ObjectID, Coord3D)> = Vec::new();
        if let Some(object_manager) = &context.object_manager {
            if let Ok(om) = object_manager.read() {
                for id in &object_ids {
                    if let Some(obj) = om.get_object(*id) {
                        positions.push((*id, obj.get_position()));
                    }
                }
            }
        }

        if positions.len() != object_ids.len() {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Unable to resolve objects for scatter",
            ));
        }

        // Stop current actions first
        if let Some(ai_manager) = &context.ai_manager {
            if let Ok(mut ai) = ai_manager.write() {
                let _ = ai.issue_stop_order(&object_ids);
            }
        }

        // Deterministic jitter seeded by frame/player
        let seed = (context.current_frame as u64) ^ ((context.player_id as u64) << 32);
        let mut rng = StdRng::seed_from_u64(seed);
        let mut all_ok = true;

        if let Some(ai_manager) = &context.ai_manager {
            if let Ok(mut ai) = ai_manager.write() {
                for (object_id, pos) in positions {
                    let angle = rng.gen::<f32>() * std::f32::consts::TAU;
                    let radius = rng.gen_range(8.0f32..22.0f32);
                    let dx = radius * angle.cos();
                    let dz = radius * angle.sin();
                    let dest = Coord3D::new(pos.x + dx, pos.y, pos.z + dz);

                    if !ai.issue_move_order(&[object_id], dest) {
                        all_ok = false;
                    }
                }
            } else {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "Cannot access AI manager",
                ));
            }
        } else {
            return CommandExecutionResult::Failed(AsciiString::from("AI manager not available"));
        }

        if all_ok {
            CommandExecutionResult::Success
        } else {
            CommandExecutionResult::Failed(AsciiString::from("Failed to issue scatter move orders"))
        }
    }

    /// Execute self destruct on a set of objects
    fn execute_self_destruct(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let object_ids = self.extract_object_ids(command);
        if object_ids.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from(
                "No objects to self destruct",
            ));
        }

        // Validate ownership/alive before issuing destruction.
        if let Some(object_manager) = &context.object_manager {
            if let Ok(om) = object_manager.read() {
                for id in &object_ids {
                    if let Some(obj) = om.get_object(*id) {
                        if !obj.is_alive() {
                            return CommandExecutionResult::Failed(AsciiString::from(
                                "Cannot self-destruct a dead object",
                            ));
                        }
                        if !obj.can_be_controlled_by(context.player_id) {
                            return CommandExecutionResult::Failed(AsciiString::from(
                                "Player cannot self-destruct this object",
                            ));
                        }
                    } else {
                        return CommandExecutionResult::Failed(AsciiString::from(
                            "Object not found",
                        ));
                    }
                }
            }
        }

        if let Some(object_manager) = &context.object_manager {
            if let Ok(mut om) = object_manager.write() {
                let mut all_ok = true;
                for id in &object_ids {
                    if !om.destroy_object(*id) {
                        all_ok = false;
                    }
                }
                if all_ok {
                    CommandExecutionResult::Success
                } else {
                    CommandExecutionResult::Failed(AsciiString::from(
                        "Failed to destroy one or more objects",
                    ))
                }
            } else {
                CommandExecutionResult::Failed(AsciiString::from("Cannot access object manager"))
            }
        } else {
            CommandExecutionResult::Failed(AsciiString::from("Object manager not available"))
        }
    }

    /// Execute a special power command with basic validation against power registry and targets.
    fn execute_special_power(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        use crate::commands::command::CommandArgumentType;
        use crate::common::INVALID_OBJECT_ID;
        use crate::modules::SpecialPowerCommandOptions;
        use crate::object_creation_list::nuggets::INVALID_ANGLE;

        let cmd_type = command.command.get_type();
        let mut power_id: Option<u32> = None;
        let mut command_options = SpecialPowerCommandOptions::NONE;
        let mut target_location: Option<Coord3D> = None;
        let mut target_object: Option<ObjectID> = None;
        let mut source_object: Option<ObjectID> = None;
        let mut angle: f32 = INVALID_ANGLE;
        let mut object_in_way: Option<ObjectID> = None;
        let mut override_power_type: Option<u32> = None;

        let arg_count = command.command.get_argument_count();
        let arg_at = |idx: Int| command.command.get_argument(idx);

        match cmd_type {
            CommandType::DoSpecialPower => {
                if let Some(CommandArgumentType::Integer(id)) = arg_at(0) {
                    power_id = Some(*id as u32);
                }
                if let Some(CommandArgumentType::Integer(options)) = arg_at(1) {
                    command_options =
                        SpecialPowerCommandOptions::from_bits_truncate(*options as u32);
                }
                if let Some(CommandArgumentType::ObjectID(id)) = arg_at(2) {
                    if *id != INVALID_OBJECT_ID {
                        source_object = Some(*id);
                    }
                }
            }
            CommandType::DoSpecialPowerAtLocation => {
                if let Some(CommandArgumentType::Integer(id)) = arg_at(0) {
                    power_id = Some(*id as u32);
                }
                if let Some(CommandArgumentType::Location(pos)) = arg_at(1) {
                    target_location = Some(*pos);
                }
                if let Some(CommandArgumentType::Real(value)) = arg_at(2) {
                    angle = *value;
                }
                if let Some(CommandArgumentType::ObjectID(id)) = arg_at(3) {
                    if *id != INVALID_OBJECT_ID {
                        object_in_way = Some(*id);
                    }
                }
                if let Some(CommandArgumentType::Integer(options)) = arg_at(4) {
                    command_options =
                        SpecialPowerCommandOptions::from_bits_truncate(*options as u32);
                }
                if let Some(CommandArgumentType::ObjectID(id)) = arg_at(5) {
                    if *id != INVALID_OBJECT_ID {
                        source_object = Some(*id);
                    }
                }
            }
            CommandType::DoSpecialPowerAtObject => {
                if let Some(CommandArgumentType::Integer(id)) = arg_at(0) {
                    power_id = Some(*id as u32);
                }
                if let Some(CommandArgumentType::ObjectID(id)) = arg_at(1) {
                    if *id != INVALID_OBJECT_ID {
                        target_object = Some(*id);
                    }
                }
                if let Some(CommandArgumentType::Integer(options)) = arg_at(2) {
                    command_options =
                        SpecialPowerCommandOptions::from_bits_truncate(*options as u32);
                }
                if let Some(CommandArgumentType::ObjectID(id)) = arg_at(3) {
                    if *id != INVALID_OBJECT_ID {
                        source_object = Some(*id);
                    }
                }
            }
            CommandType::DoSpecialPowerOverrideDestination => {
                if let Some(CommandArgumentType::Location(pos)) = arg_at(0) {
                    target_location = Some(*pos);
                }
                if let Some(CommandArgumentType::Integer(value)) = arg_at(1) {
                    override_power_type = Some(*value as u32);
                }
                if let Some(CommandArgumentType::ObjectID(id)) = arg_at(2) {
                    if *id != INVALID_OBJECT_ID {
                        source_object = Some(*id);
                    }
                }
            }
            _ => {}
        }

        if cmd_type != CommandType::DoSpecialPowerOverrideDestination {
            if power_id.is_none() {
                for i in 0..arg_count {
                    if let Some(CommandArgumentType::Integer(id)) = arg_at(i as Int) {
                        power_id = Some(*id as u32);
                        break;
                    }
                }
            }
            if target_location.is_none() {
                for i in 0..arg_count {
                    if let Some(CommandArgumentType::Location(pos)) = arg_at(i as Int) {
                        target_location = Some(*pos);
                        break;
                    }
                }
            }
            if target_object.is_none() {
                for i in 0..arg_count {
                    if let Some(CommandArgumentType::ObjectID(id)) = arg_at(i as Int) {
                        if *id != INVALID_OBJECT_ID {
                            target_object = Some(*id);
                            break;
                        }
                    }
                }
            }
        }

        let object_exists = |object_id: ObjectID| -> bool {
            if let Some(object_manager) = &context.object_manager {
                if let Ok(om) = object_manager.read() {
                    if om.get_object(object_id).is_some() {
                        return true;
                    }
                }
            }
            TheGameLogic::find_object_by_id(object_id).is_some()
        };

        let object_position = |object_id: ObjectID| -> Option<Coord3D> {
            if let Some(object_manager) = &context.object_manager {
                if let Ok(om) = object_manager.read() {
                    if let Some(obj) = om.get_object(object_id) {
                        return Some(obj.get_position());
                    }
                }
            }
            TheGameLogic::find_object_by_id(object_id)
                .and_then(|obj| obj.read().ok().map(|guard| *guard.get_position()))
        };

        let object_is_alive = |object_id: ObjectID| -> bool {
            if let Some(object_manager) = &context.object_manager {
                if let Ok(om) = object_manager.read() {
                    if let Some(obj) = om.get_object(object_id) {
                        return obj.is_alive();
                    }
                }
            }
            TheGameLogic::find_object_by_id(object_id)
                .and_then(|obj| obj.read().ok().map(|guard| !guard.is_destroyed()))
                .unwrap_or(false)
        };

        let object_can_be_controlled_by = |object_id: ObjectID, player_id: Int| -> bool {
            if let Some(object_manager) = &context.object_manager {
                if let Ok(om) = object_manager.read() {
                    if let Some(obj) = om.get_object(object_id) {
                        return obj.can_be_controlled_by(player_id);
                    }
                }
            }
            let owner = TheGameLogic::find_object_by_id(object_id)
                .and_then(|obj| {
                    obj.read()
                        .ok()
                        .and_then(|guard| guard.get_controlling_player_id())
                        .map(|id| id as Int)
                })
                .unwrap_or(-1);
            owner == -1 || owner == player_id
        };

        if cmd_type == CommandType::DoSpecialPowerOverrideDestination {
            let location = match target_location {
                Some(pos) => pos,
                None => {
                    return CommandExecutionResult::Failed(AsciiString::from(
                        "Special power override requires a target location",
                    ))
                }
            };

            let mut source_ids = Vec::new();
            if let Some(source_id) = source_object {
                source_ids.push(source_id);
            } else {
                let selection_manager = get_selection_manager();
                let mut selected_ids = Vec::new();
                if let Ok(manager) = selection_manager.read() {
                    if let Some(selection) = manager.get_player_selection_ref(context.player_id) {
                        selected_ids = selection.get_selected_objects();
                    }
                }
                source_ids = selected_ids;
            }

            let mut any_overridden = false;
            if !source_ids.is_empty() {
                for id in &source_ids {
                    if !object_is_alive(*id) || !object_can_be_controlled_by(*id, context.player_id)
                    {
                        continue;
                    }
                    let Some(obj) = TheGameLogic::find_object_by_id(*id) else {
                        continue;
                    };
                    let Ok(obj_guard) = obj.read() else {
                        continue;
                    };
                    if let Some(power_type) = override_power_type {
                        let mut matches_power = false;
                        for module_handle in obj_guard.behavior_modules() {
                            module_handle.with_module(|module| {
                                let Some(sp_module) = module_special_power_interface(module) else {
                                    return;
                                };
                                let Some(template) = sp_module.get_special_power_template_full()
                                else {
                                    return;
                                };
                                if template.get_special_power_type() as u32 == power_type {
                                    matches_power = true;
                                }
                            });
                            if matches_power {
                                break;
                            }
                        }
                        if !matches_power {
                            for behavior_arc in obj_guard.get_behavior_modules() {
                                let Ok(mut behavior_guard) = behavior_arc.lock() else {
                                    continue;
                                };
                                let Some(sp_module) = behavior_guard.get_special_power() else {
                                    continue;
                                };
                                let Some(template) = sp_module.get_special_power_template_full()
                                else {
                                    continue;
                                };
                                if template.get_special_power_type() as u32 == power_type {
                                    matches_power = true;
                                    break;
                                }
                            }
                        }
                        if !matches_power {
                            continue;
                        }
                    }
                    let mut overridden_here = false;
                    for module_handle in obj_guard.behavior_modules() {
                        module_handle.with_module(|module| {
                            let Some(update) = module_special_power_update_interface(module) else {
                                return;
                            };
                            if update.does_special_power_have_overridable_destination_active()
                                || update.does_special_power_have_overridable_destination()
                            {
                                update.set_special_power_overridable_destination(&location);
                                overridden_here = true;
                            }
                        });
                    }
                    if !overridden_here {
                        for behavior_arc in obj_guard.get_behavior_modules() {
                            let Ok(mut behavior_guard) = behavior_arc.lock() else {
                                continue;
                            };
                            if let Some(update) =
                                behavior_guard.get_special_power_update_interface()
                            {
                                if update.does_special_power_have_overridable_destination_active()
                                    || update.does_special_power_have_overridable_destination()
                                {
                                    update.set_special_power_overridable_destination(&location);
                                    overridden_here = true;
                                }
                            }
                        }
                    }
                    if overridden_here {
                        any_overridden = true;
                    }
                }
            }

            // C++ GameLogicDispatch falls through to MSG_DO_ATTACK_OBJECT here.
            self.execute_override_destination_fallthrough_attack(command, context);

            return if any_overridden {
                CommandExecutionResult::Success
            } else {
                CommandExecutionResult::Failed(AsciiString::from(
                    "No overridable special power destination available",
                ))
            };
        }

        let pid = match power_id {
            Some(id) => id,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "Special power ID not specified",
                ))
            }
        };

        if cmd_type == CommandType::DoSpecialPowerAtObject && target_object.is_none() {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Special power requires a target object",
            ));
        }

        if cmd_type == CommandType::DoSpecialPowerAtLocation && target_location.is_none() {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Special power requires a target location",
            ));
        }

        // Validate target object existence (ownership is not enforced here because many powers are offensive).
        if let Some(target_id) = target_object {
            if !object_exists(target_id) {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "Special power target object not found",
                ));
            }
        }

        // Resolve the target position if not explicitly provided.
        if target_location.is_none() {
            if let Some(target_id) = target_object {
                target_location = object_position(target_id);
            }
        }

        let mut source_ids = Vec::new();
        if let Some(source_id) = source_object {
            source_ids.push(source_id);
        } else {
            let selection_manager = get_selection_manager();
            let mut selected_ids = Vec::new();
            if let Ok(manager) = selection_manager.read() {
                if let Some(selection) = manager.get_player_selection_ref(context.player_id) {
                    selected_ids = selection.get_selected_objects();
                }
            }
            source_ids = selected_ids;
        }

        if source_ids.is_empty() {
            return CommandExecutionResult::Success;
        }

        // Validate executor objects and attempt to execute special powers.
        let mut any_executed = false;
        for id in &source_ids {
            if !object_is_alive(*id) || !object_can_be_controlled_by(*id, context.player_id) {
                continue;
            }

            let Some(obj) = TheGameLogic::find_object_by_id(*id) else {
                continue;
            };
            let Ok(obj_guard) = obj.read() else {
                continue;
            };

            for module_handle in obj_guard.behavior_modules() {
                let mut executed_here = false;
                module_handle.with_module(|module| {
                    let Some(sp_module) = module_special_power_interface(module) else {
                        return;
                    };
                    let Some(template) = sp_module.get_special_power_template_full() else {
                        return;
                    };
                    if template.get_id() != pid {
                        return;
                    }

                    let allowed = match cmd_type {
                        CommandType::DoSpecialPower => TheActionManager::can_do_special_power(
                            &obj_guard,
                            template.as_ref(),
                            CommandSourceType::FromPlayer,
                            command_options.bits(),
                            true,
                        ),
                        CommandType::DoSpecialPowerAtObject => {
                            if let Some(target_id) = target_object {
                                if let Some(target_obj) = TheGameLogic::find_object_by_id(target_id)
                                {
                                    if let Ok(target_guard) = target_obj.read() {
                                        TheActionManager::can_do_special_power_at_object(
                                            &obj_guard,
                                            &target_guard,
                                            CommandSourceType::FromPlayer,
                                            template.as_ref(),
                                            command_options.bits(),
                                            true,
                                        )
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        }
                        CommandType::DoSpecialPowerAtLocation => {
                            if let Some(pos) = target_location {
                                let object_in_way_arc = object_in_way
                                    .and_then(|id| TheGameLogic::find_object_by_id(id));
                                let object_in_way_ref = match object_in_way_arc.as_ref() {
                                    Some(obj_arc) => obj_arc.read().ok(),
                                    None => None,
                                };
                                TheActionManager::can_do_special_power_at_location(
                                    &obj_guard,
                                    &pos,
                                    CommandSourceType::FromPlayer,
                                    template.as_ref(),
                                    object_in_way_ref.as_deref(),
                                    command_options.bits(),
                                    true,
                                )
                            } else {
                                false
                            }
                        }
                        _ => false,
                    };

                    if !allowed {
                        return;
                    }

                    match cmd_type {
                        CommandType::DoSpecialPower => {
                            sp_module.do_special_power(command_options);
                            executed_here = true;
                        }
                        CommandType::DoSpecialPowerAtObject => {
                            if let Some(target_id) = target_object {
                                sp_module.do_special_power_at_object(target_id, command_options);
                                executed_here = true;
                            }
                        }
                        CommandType::DoSpecialPowerAtLocation => {
                            if let Some(pos) = target_location {
                                sp_module.do_special_power_at_location(
                                    &pos,
                                    angle,
                                    command_options,
                                );
                                let _ = object_in_way;
                                executed_here = true;
                            }
                        }
                        _ => {}
                    }
                });

                if executed_here {
                    any_executed = true;
                    if let Ok(mut write_guard) = obj.write() {
                        write_guard.friend_set_undetected_defector(false);
                    }
                    break;
                }
            }
            if any_executed {
                continue;
            }
            for behavior_arc in obj_guard.get_behavior_modules() {
                let Ok(mut behavior_guard) = behavior_arc.lock() else {
                    continue;
                };
                let Some(sp_module) = behavior_guard.get_special_power() else {
                    continue;
                };
                let Some(template) = sp_module.get_special_power_template_full() else {
                    continue;
                };
                if template.get_id() != pid {
                    continue;
                }

                let allowed = match cmd_type {
                    CommandType::DoSpecialPower => TheActionManager::can_do_special_power(
                        &obj_guard,
                        template.as_ref(),
                        CommandSourceType::FromPlayer,
                        command_options.bits(),
                        true,
                    ),
                    CommandType::DoSpecialPowerAtObject => {
                        if let Some(target_id) = target_object {
                            if let Some(target_obj) = TheGameLogic::find_object_by_id(target_id) {
                                if let Ok(target_guard) = target_obj.read() {
                                    TheActionManager::can_do_special_power_at_object(
                                        &obj_guard,
                                        &target_guard,
                                        CommandSourceType::FromPlayer,
                                        template.as_ref(),
                                        command_options.bits(),
                                        true,
                                    )
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                    CommandType::DoSpecialPowerAtLocation => {
                        if let Some(pos) = target_location {
                            let object_in_way_arc =
                                object_in_way.and_then(|id| TheGameLogic::find_object_by_id(id));
                            let object_in_way_ref = match object_in_way_arc.as_ref() {
                                Some(obj_arc) => obj_arc.read().ok(),
                                None => None,
                            };
                            TheActionManager::can_do_special_power_at_location(
                                &obj_guard,
                                &pos,
                                CommandSourceType::FromPlayer,
                                template.as_ref(),
                                object_in_way_ref.as_deref(),
                                command_options.bits(),
                                true,
                            )
                        } else {
                            false
                        }
                    }
                    _ => false,
                };

                if !allowed {
                    continue;
                }

                match cmd_type {
                    CommandType::DoSpecialPower => {
                        sp_module.do_special_power(command_options);
                        any_executed = true;
                    }
                    CommandType::DoSpecialPowerAtObject => {
                        if let Some(target_id) = target_object {
                            sp_module.do_special_power_at_object(target_id, command_options);
                            any_executed = true;
                        }
                    }
                    CommandType::DoSpecialPowerAtLocation => {
                        if let Some(pos) = target_location {
                            sp_module.do_special_power_at_location(&pos, angle, command_options);
                            let _ = object_in_way;
                            any_executed = true;
                        }
                    }
                    _ => {}
                }

                if any_executed {
                    if let Ok(mut write_guard) = obj.write() {
                        write_guard.friend_set_undetected_defector(false);
                    }
                    break;
                }
            }
        }

        CommandExecutionResult::Success
    }

    fn execute_override_destination_fallthrough_attack(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) {
        let Some(target_id) = override_destination_fallthrough_target_id(command) else {
            return;
        };

        if TheGameLogic::find_object_by_id(target_id).is_none() {
            return;
        }

        let mut attack_command = Command::new(CommandType::DoAttackObject);
        attack_command.set_player_index(context.player_id);
        attack_command.append_object_id_argument(target_id);

        let queued_attack =
            QueuedCommand::new(attack_command, CommandPriority::High, context.current_frame);
        let _ = self.execute_attack_command(&queued_attack, context);
    }

    fn execute_place_beacon(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let mut position = match self.extract_command_location(command) {
            Some(position) => position,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "No beacon position supplied",
                ))
            }
        };

        if let Some(terrain) = TheTerrainLogic::get() {
            let extent = terrain.get_maximum_pathfind_extent();
            if !Self::is_in_region_no_z(&extent, &position) {
                position = terrain.find_closest_edge_point(&position);
            }
        }

        let (player_arc, local_player_arc) = match player_list().read() {
            Ok(list) => {
                let player = match list.get_player(context.player_id) {
                    Some(player) => Arc::clone(player),
                    None => {
                        return CommandExecutionResult::Failed(AsciiString::from(
                            "Player not found for beacon placement",
                        ))
                    }
                };
                (player, list.get_local_player().cloned())
            }
            Err(_) => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "Player list lock poisoned",
                ))
            }
        };

        let (template_name, player_display_name, player_defeated, player_team) = {
            let guard = match player_arc.read() {
                Ok(guard) => guard,
                Err(_) => {
                    return CommandExecutionResult::Failed(AsciiString::from(
                        "Player lock poisoned",
                    ))
                }
            };
            let template_name = guard
                .get_player_template()
                .map(|template| template.beacon_name.clone())
                .unwrap_or_default();
            let defeated = guard.is_defeated()
                || (!guard.has_any_units()
                    && !guard.has_any_buildings_counts_for_victory()
                    && !guard.has_any_objects());
            (
                template_name,
                guard.get_player_display_name().clone(),
                defeated,
                guard.get_default_team(),
            )
        };

        if player_defeated {
            self.notify_beacon_failed(context.player_id, &position, local_player_arc.as_ref());
            return CommandExecutionResult::Failed(AsciiString::from("Player is defeated"));
        }

        if template_name.is_empty() {
            self.notify_beacon_failed(context.player_id, &position, local_player_arc.as_ref());
            return CommandExecutionResult::Failed(AsciiString::from("Beacon template missing"));
        }

        let template = match TheThingFactory::find_template(&template_name) {
            Some(template) => template,
            None => {
                self.notify_beacon_failed(context.player_id, &position, local_player_arc.as_ref());
                return CommandExecutionResult::Failed(AsciiString::from(
                    "Beacon template not found",
                ));
            }
        };

        let max_beacons = with_multiplayer_settings(|settings| settings.max_beacons_per_player);
        let current_count = self.count_player_beacons(context.player_id, &template);
        if current_count >= max_beacons {
            self.notify_beacon_limit_reached(
                context.player_id,
                &position,
                local_player_arc.as_ref(),
            );
            return CommandExecutionResult::Failed(AsciiString::from("Too many beacons"));
        }

        let new_object = match TheThingFactory::get() {
            Ok(factory) => {
                let team_ref = player_team.as_ref().and_then(|team| team.read().ok());
                if let Some(team_guard) = team_ref.as_ref() {
                    factory.new_object(template.clone(), team_guard).ok()
                } else {
                    factory
                        .new_object_optional_team(template.clone(), None)
                        .ok()
                }
            }
            Err(_) => None,
        };

        let Some(beacon_object) = new_object else {
            self.notify_beacon_failed(context.player_id, &position, local_player_arc.as_ref());
            return CommandExecutionResult::Failed(AsciiString::from("Beacon creation failed"));
        };

        if let Ok(mut obj_guard) = beacon_object.write() {
            let _ = obj_guard.set_position(&position);
            obj_guard.set_producer(None);
        }

        let (local_visibility, local_allies) =
            self.beacon_visibility_and_allies(&player_arc, local_player_arc.as_ref());
        if local_visibility {
            let mut manager = match get_beacon_manager().lock() {
                Ok(lock) => lock,
                Err(_) => {
                    return CommandExecutionResult::Failed(AsciiString::from(
                        "Beacon manager lock poisoned",
                    ))
                }
            };
            manager.place_beacon(context.player_id, position, context.current_frame);

            self.notify_beacon_placed(
                context.player_id,
                &position,
                &player_display_name,
                local_player_arc.as_ref(),
            );
            if let Ok(mut radar) = get_radar_system().write() {
                let radar_pos = RadarCoord3D::new(position.x, position.y, position.z);
                radar.create_event(&radar_pos, RadarEventType::Information, 1.0);
            }
            if local_allies {
                let _ = TheEva::set_should_play(EvaEvent::BeaconDetected);
            }
            control_bar::mark_ui_dirty();
        } else {
            self.hide_beacon_for_local(&beacon_object);
        }

        CommandExecutionResult::Success
    }

    fn execute_remove_beacon(
        &mut self,
        _command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let (player_arc, _local_player_arc, is_local_player) = match player_list().read() {
            Ok(list) => {
                let player = match list.get_player(context.player_id) {
                    Some(player) => Arc::clone(player),
                    None => {
                        return CommandExecutionResult::Failed(AsciiString::from(
                            "Player not found for beacon removal",
                        ))
                    }
                };
                let local = list.get_local_player().cloned();
                let is_local = local
                    .as_ref()
                    .and_then(|player| {
                        player
                            .read()
                            .ok()
                            .map(|guard| guard.get_player_index() as Int)
                    })
                    .map(|index| index == context.player_id)
                    .unwrap_or(false);
                (player, local, is_local)
            }
            Err(_) => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "Player list lock poisoned",
                ))
            }
        };

        let selected_ids = {
            let Ok(guard) = player_arc.write() else {
                return CommandExecutionResult::Failed(AsciiString::from("Player lock poisoned"));
            };
            guard.get_current_selection_ids()
        };

        let mut removed_entries: Vec<(Int, Coord3D)> = Vec::new();
        let mut _removed_any = false;

        for object_id in selected_ids {
            let Some(obj_arc) = crate::object::registry::OBJECT_REGISTRY.get_object(object_id)
            else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let owner_id = obj_guard
                .get_controlling_player_id()
                .map(|id| id as Int)
                .unwrap_or(-1);
            if owner_id < 0 {
                continue;
            }
            let Some(owner_template) = self.resolve_beacon_template_for_player(owner_id) else {
                continue;
            };
            if !owner_template.is_equivalent_to(obj_guard.get_template().as_ref()) {
                continue;
            }

            let entry_position = *obj_guard.get_position();
            drop(obj_guard);

            if owner_id == context.player_id {
                let _ = TheGameLogic::destroy_object_by_id(object_id);
                _removed_any = true;
                removed_entries.push((owner_id, entry_position));
                control_bar::mark_ui_dirty();
            } else if is_local_player {
                self.hide_beacon_for_local(&obj_arc);
                removed_entries.push((owner_id, entry_position));
            }
        }

        if !removed_entries.is_empty() {
            if let Ok(mut manager) = get_beacon_manager().lock() {
                for (owner_id, pos) in removed_entries {
                    let _ = manager.remove_beacon(owner_id, &pos);
                }
            }
        }

        CommandExecutionResult::Success
    }

    fn execute_set_beacon_text(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let position = match self.extract_command_location(command) {
            Some(pos) => pos,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "No beacon position supplied",
                ))
            }
        };

        let text = match self.extract_command_text(command) {
            Some(text) => text,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from("No beacon text supplied"))
            }
        };

        let template = match self.resolve_beacon_template_for_player(context.player_id) {
            Some(template) => template,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from("Beacon template missing"))
            }
        };

        let mut found = false;
        let manager_handle = get_object_manager();
        if let Ok(manager) = manager_handle.read() {
            for object_id in manager.get_objects_owned_by_player(context.player_id as UnsignedInt) {
                let Some(instance) = manager.get_object(object_id) else {
                    continue;
                };
                let Ok(instance_guard) = instance.read() else {
                    continue;
                };
                let obj_arc = instance_guard.base.clone();
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };
                if !template.is_equivalent_to(obj_guard.get_template().as_ref()) {
                    continue;
                }
                if obj_guard.get_position().distance(position) > Self::BEACON_MATCH_THRESHOLD {
                    continue;
                }
                found = true;
                break;
            }
        }

        if !found {
            return CommandExecutionResult::Failed(AsciiString::from("Beacon not found"));
        }

        let mut manager = match get_beacon_manager().lock() {
            Ok(lock) => lock,
            Err(_) => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "Beacon manager lock poisoned",
                ))
            }
        };

        if manager.set_beacon_text(context.player_id, &position, text) {
            CommandExecutionResult::Success
        } else {
            CommandExecutionResult::Failed(AsciiString::from("Beacon not found"))
        }
    }

    fn is_in_region_no_z(region: &crate::common::Region3D, position: &Coord3D) -> bool {
        position.x >= region.lo.x
            && position.x <= region.hi.x
            && position.y >= region.lo.y
            && position.y <= region.hi.y
    }

    fn resolve_beacon_template_for_player(
        &self,
        player_id: Int,
    ) -> Option<Arc<dyn crate::common::ThingTemplate>> {
        let list = player_list().read().ok()?;
        let player_arc = list.get_player(player_id)?.clone();
        let player_guard = player_arc.read().ok()?;
        let template_name = player_guard
            .get_player_template()
            .map(|template| template.beacon_name.clone())?;
        if template_name.is_empty() {
            return None;
        }
        TheThingFactory::find_template(&template_name)
    }

    fn count_player_beacons(
        &self,
        player_id: Int,
        template: &Arc<dyn crate::common::ThingTemplate>,
    ) -> Int {
        let manager = get_object_manager();
        let Ok(manager_guard) = manager.read() else {
            return 0;
        };

        let mut count = 0;
        for object_id in manager_guard.get_objects_owned_by_player(player_id as UnsignedInt) {
            let Some(instance) = manager_guard.get_object(object_id) else {
                continue;
            };
            let Ok(instance_guard) = instance.read() else {
                continue;
            };
            let obj_arc = instance_guard.base.clone();
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if template.is_equivalent_to(obj_guard.get_template().as_ref()) {
                count += 1;
            }
        }
        count
    }

    fn is_beacon_visible_to_local(
        &self,
        player_arc: &Arc<RwLock<crate::player::Player>>,
        local_player: Option<&Arc<RwLock<crate::player::Player>>>,
    ) -> bool {
        let Some(local_player) = local_player else {
            return false;
        };
        let Ok(local_guard) = local_player.read() else {
            return false;
        };
        if local_guard.is_player_observer() {
            return true;
        }
        let Some(local_team) = local_guard.get_default_team() else {
            return false;
        };
        let Ok(local_team_guard) = local_team.read() else {
            return false;
        };
        let Ok(player_guard) = player_arc.read() else {
            return false;
        };
        matches!(
            player_guard.get_relationship_with_team(&local_team_guard),
            Relationship::Allies
        )
    }

    fn beacon_visibility_and_allies(
        &self,
        player_arc: &Arc<RwLock<crate::player::Player>>,
        local_player: Option<&Arc<RwLock<crate::player::Player>>>,
    ) -> (bool, bool) {
        let Some(local_player) = local_player else {
            return (false, false);
        };
        let Ok(local_guard) = local_player.read() else {
            return (false, false);
        };
        if local_guard.is_player_observer() {
            return (true, false);
        }
        let Some(local_team) = local_guard.get_default_team() else {
            return (false, false);
        };
        let Ok(local_team_guard) = local_team.read() else {
            return (false, false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return (false, false);
        };
        let relation = player_guard.get_relationship_with_team(&local_team_guard);
        let visible = matches!(relation, Relationship::Allies);
        let allies = matches!(relation, Relationship::Allies);
        (visible, allies)
    }

    fn notify_beacon_placed(
        &self,
        player_id: Int,
        position: &Coord3D,
        player_name: &str,
        _local_player: Option<&Arc<RwLock<crate::player::Player>>>,
    ) {
        let template = TheGameText::fetch("GUI:BeaconPlaced");
        let message = template.replace("%s", player_name);
        TheInGameUI::display_message(&message);

        if let Some(audio) = TheAudio::get() {
            let mut event = AudioEventRts::new("BeaconPlaced");
            event.set_position(&(position.x, position.y, position.z));
            event.set_player_index(player_id as u32);
            audio.add_audio_event(&event);
        }
    }

    fn notify_beacon_failed(
        &self,
        player_id: Int,
        position: &Coord3D,
        _local_player: Option<&Arc<RwLock<crate::player::Player>>>,
    ) {
        TheInGameUI::display_message(&TheGameText::fetch("GUI:BeaconPlacementFailed"));
        if let Some(audio) = TheAudio::get() {
            let mut event = AudioEventRts::new("BeaconPlacementFailed");
            event.set_position(&(position.x, position.y, position.z));
            event.set_player_index(player_id as u32);
            audio.add_audio_event(&event);
        }
    }

    fn notify_beacon_limit_reached(
        &self,
        player_id: Int,
        position: &Coord3D,
        local_player: Option<&Arc<RwLock<crate::player::Player>>>,
    ) {
        let local_matches = local_player
            .and_then(|player| {
                player
                    .read()
                    .ok()
                    .map(|guard| guard.get_player_index() as Int)
            })
            .map(|index| index == player_id)
            .unwrap_or(false);
        if !local_matches {
            return;
        }

        TheInGameUI::display_message(&TheGameText::fetch("GUI:TooManyBeacons"));
        if let Some(audio) = TheAudio::get() {
            let mut event = AudioEventRts::new("BeaconPlacementFailed");
            event.set_position(&(position.x, position.y, position.z));
            event.set_player_index(player_id as u32);
            audio.add_audio_event(&event);
        }
    }

    fn hide_beacon_for_local(&self, beacon_object: &Arc<RwLock<crate::object::Object>>) {
        let modules = match beacon_object.read() {
            Ok(guard) => guard.client_update_modules(),
            Err(_) => return,
        };
        for module in modules {
            let _ = module.with_module_downcast::<
                crate::object::update::beacon_client_update::BeaconClientUpdateModule,
                _,
                _,
            >(|beacon_update| {
                beacon_update.hide_beacon();
            });
        }
    }

    fn hide_non_owned_beacon_for_local(
        &self,
        position: &Coord3D,
        exclude_owner: Option<Int>,
    ) -> Vec<(Int, Coord3D)> {
        let manager = get_object_manager();
        let Ok(manager_guard) = manager.read() else {
            return Vec::new();
        };
        let mut hidden = Vec::new();
        let object_ids =
            manager_guard.find_objects_in_radius(*position, Self::BEACON_MATCH_THRESHOLD);
        for object_id in object_ids {
            let Some(instance) = manager_guard.get_object(object_id) else {
                continue;
            };
            let Ok(instance_guard) = instance.read() else {
                continue;
            };
            let obj_arc = instance_guard.base.clone();
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            if obj_guard.get_position().distance(*position) > Self::BEACON_MATCH_THRESHOLD {
                continue;
            }
            let owner_id = obj_guard
                .get_controlling_player_id()
                .map(|id| id as Int)
                .unwrap_or(-1);
            if owner_id < 0 {
                continue;
            }
            if exclude_owner.map(|id| id == owner_id).unwrap_or(false) {
                continue;
            }
            let Some(owner_template) = self.resolve_beacon_template_for_player(owner_id) else {
                continue;
            };
            if !owner_template.is_equivalent_to(obj_guard.get_template().as_ref()) {
                continue;
            }
            let entry_position = *obj_guard.get_position();
            drop(obj_guard);
            self.hide_beacon_for_local(&obj_arc);
            hidden.push((owner_id, entry_position));
        }
        hidden
    }

    fn extract_command_location(&self, command: &QueuedCommand) -> Option<Coord3D> {
        for i in 0..command.command.get_argument_count() {
            if let Some(arg) = command.command.get_argument(i as Int) {
                if let crate::commands::command::CommandArgumentType::Location(pos) = arg {
                    return Some(*pos);
                }
            }
        }
        None
    }

    fn extract_command_text(&self, command: &QueuedCommand) -> Option<AsciiString> {
        for i in 0..command.command.get_argument_count() {
            if let Some(arg) = command.command.get_argument(i as Int) {
                if let crate::commands::command::CommandArgumentType::AsciiString(text) = arg {
                    return Some(text.clone());
                }
            }
        }
        None
    }

    fn extract_object_ids(&self, command: &QueuedCommand) -> Vec<ObjectID> {
        let mut ids = Vec::new();
        for i in 0..command.command.get_argument_count() {
            if let Some(arg) = command.command.get_argument(i as Int) {
                if let crate::commands::command::CommandArgumentType::ObjectID(id) = arg {
                    ids.push(*id);
                }
            }
        }
        ids
    }

    fn extract_target_and_sources(
        &self,
        command: &QueuedCommand,
    ) -> (Option<ObjectID>, Vec<ObjectID>) {
        use crate::common::INVALID_OBJECT_ID;

        let ids = self.extract_object_ids(command);
        if ids.is_empty() {
            return (None, Vec::new());
        }

        let is_selection_marker = |id: ObjectID| id == 0 || id == INVALID_OBJECT_ID;

        if ids.len() >= 2 && is_selection_marker(ids[0]) {
            let target = Some(ids[1]);
            let sources: Vec<ObjectID> = ids.iter().skip(2).copied().collect();
            return (target, sources);
        }

        let target = Some(ids[0]);
        let sources: Vec<ObjectID> = ids.iter().skip(1).copied().collect();
        (target, sources)
    }

    /// Guard a position: move units to the position and hold.
    fn execute_guard_position(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let position = match self.extract_command_location(command) {
            Some(pos) => pos,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "No guard position specified",
                ))
            }
        };
        let mut guard_mode = crate::ai::GuardMode::Normal;
        for i in 0..command.command.get_argument_count() {
            if let Some(crate::commands::command::CommandArgumentType::Integer(mode)) =
                command.command.get_argument(i as Int)
            {
                guard_mode = crate::ai::GuardMode::from_i32(*mode);
                break;
            }
        }

        let mut object_ids = self.extract_object_ids(command);
        if object_ids.is_empty() {
            let selection_manager = get_selection_manager();
            object_ids = match selection_manager.read() {
                Ok(manager) => manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
                    .unwrap_or_default(),
                Err(_) => Vec::new(),
            };
        }
        if object_ids.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from(
                "No objects to guard position",
            ));
        }

        // Validate objects are controllable and alive.
        if let Some(object_manager) = &context.object_manager {
            if let Ok(om) = object_manager.read() {
                for id in &object_ids {
                    if let Some(obj) = om.get_object(*id) {
                        if !obj.is_alive() {
                            return CommandExecutionResult::Failed(AsciiString::from(
                                "Guard command includes dead object",
                            ));
                        }
                        if !obj.can_be_controlled_by(context.player_id) {
                            return CommandExecutionResult::Failed(AsciiString::from(
                                "Player cannot control object for guard command",
                            ));
                        }
                    } else {
                        return CommandExecutionResult::Failed(AsciiString::from(
                            "Guard command object not found",
                        ));
                    }
                }
            }
        }

        if let Some(ai_manager) = &context.ai_manager {
            if let Ok(mut ai) = ai_manager.write() {
                if ai.issue_guard_position_order(&object_ids, position, guard_mode) {
                    return CommandExecutionResult::Success;
                }
            }
        }

        CommandExecutionResult::Failed(AsciiString::from(
            "AI manager unavailable for guard position",
        ))
    }

    /// Guard an object: move units to the target object's position.
    fn execute_guard_object(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let mut target_id = None;
        let mut guard_mode = crate::ai::GuardMode::Normal;
        for i in 0..command.command.get_argument_count() {
            if let Some(arg) = command.command.get_argument(i as Int) {
                match arg {
                    crate::commands::command::CommandArgumentType::ObjectID(id) => {
                        if target_id.is_none() {
                            target_id = Some(*id);
                        }
                    }
                    crate::commands::command::CommandArgumentType::Integer(mode) => {
                        guard_mode = crate::ai::GuardMode::from_i32(*mode);
                    }
                    _ => {}
                }
            }
        }
        let target_id = target_id.ok_or_else(|| {
            CommandExecutionResult::Failed(AsciiString::from("No guard target object specified"))
        });
        let target_id = match target_id {
            Ok(id) => id,
            Err(res) => return res,
        };

        let _position = if let Some(object_manager) = &context.object_manager {
            if let Ok(om) = object_manager.read() {
                if let Some(obj) = om.get_object(target_id) {
                    obj.get_position()
                } else {
                    return CommandExecutionResult::Failed(AsciiString::from("Target not found"));
                }
            } else {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "Cannot access object manager",
                ));
            }
        } else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "No object manager available",
            ));
        };

        let mut object_ids = self.extract_object_ids(command);
        if object_ids.is_empty() {
            let selection_manager = get_selection_manager();
            object_ids = match selection_manager.read() {
                Ok(manager) => manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
                    .unwrap_or_default(),
                Err(_) => Vec::new(),
            };
        }
        if object_ids.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from("No objects to guard target"));
        }

        // Validate objects the player can control and are alive.
        if let Some(object_manager) = &context.object_manager {
            if let Ok(om) = object_manager.read() {
                for id in &object_ids {
                    if let Some(obj) = om.get_object(*id) {
                        if !obj.is_alive() {
                            return CommandExecutionResult::Failed(AsciiString::from(
                                "Guard command includes dead object",
                            ));
                        }
                        if !obj.can_be_controlled_by(context.player_id) {
                            return CommandExecutionResult::Failed(AsciiString::from(
                                "Player cannot control object for guard target",
                            ));
                        }
                    } else {
                        return CommandExecutionResult::Failed(AsciiString::from(
                            "Guard command object not found",
                        ));
                    }
                }
            }
        }

        if let Some(ai_manager) = &context.ai_manager {
            if let Ok(mut ai) = ai_manager.write() {
                if ai.issue_guard_object_order(&object_ids, target_id, guard_mode) {
                    return CommandExecutionResult::Success;
                }
            }
        }

        CommandExecutionResult::Failed(AsciiString::from("AI manager unavailable for guard object"))
    }

    /// Capture a structure: issue capture orders to selected units.
    fn execute_capture_building(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let (target, mut object_ids) = self.extract_target_and_sources(command);
        let target = match target {
            Some(id) => id,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "No target specified for capture",
                ))
            }
        };

        if object_ids.is_empty() {
            let selection_manager = get_selection_manager();
            object_ids = match selection_manager.read() {
                Ok(manager) => manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
                    .unwrap_or_default(),
                Err(_) => Vec::new(),
            };
        }

        if object_ids.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from(
                "No objects specified for capture",
            ));
        }

        let Some(target_arc) = TheGameLogic::find_object_by_id(target) else {
            return CommandExecutionResult::Failed(AsciiString::from("Target not found"));
        };
        let Ok(target_guard) = target_arc.read() else {
            return CommandExecutionResult::Failed(AsciiString::from("Target lock failed"));
        };
        if target_guard.is_effectively_dead() {
            return CommandExecutionResult::Failed(AsciiString::from("Target is not alive"));
        }

        if let Some(object_manager) = &context.object_manager {
            if let Ok(om) = object_manager.read() {
                for object_id in &object_ids {
                    if let Some(obj) = om.get_object(*object_id) {
                        if !obj.is_alive() {
                            return CommandExecutionResult::Failed(AsciiString::from(&format!(
                                "Object {} is not alive",
                                object_id
                            )));
                        }
                        if !obj.can_be_controlled_by(context.player_id) {
                            return CommandExecutionResult::Failed(AsciiString::from(&format!(
                                "Player {} cannot control object {}",
                                context.player_id, object_id
                            )));
                        }
                    } else {
                        return CommandExecutionResult::Failed(AsciiString::from(&format!(
                            "Object {} not found",
                            object_id
                        )));
                    }
                }
            }
        }

        let mut issued = 0;
        if let Ok(mut factory) = get_object_factory().write() {
            for object_id in &object_ids {
                let Some(GameObjectInstance::Unit(unit_arc)) = factory.get_object_mut(*object_id)
                else {
                    continue;
                };

                let Ok(unit_base) = unit_arc.read().map(|unit| unit.base_object()) else {
                    continue;
                };
                let Ok(unit_guard) = unit_base.read() else {
                    continue;
                };

                if !TheActionManager::can_capture_building(
                    &unit_guard,
                    &*target_guard,
                    CommandSourceType::FromPlayer,
                ) {
                    continue;
                }

                if let Ok(mut unit_guard) = unit_arc.write() {
                    let _ = unit_guard.give_capture_order(target, false);
                    issued += 1;
                }
            }
        }

        if issued > 0 {
            CommandExecutionResult::Success
        } else {
            CommandExecutionResult::Failed(AsciiString::from("No capture-capable units available"))
        }
    }

    fn execute_hack_special_power_at_object(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
        power_type: crate::common::types::SpecialPowerType,
        can_execute: fn(&crate::object::Object, &crate::object::Object, CommandSourceType) -> bool,
        failure_label: &'static str,
    ) -> CommandExecutionResult {
        use crate::common::INVALID_OBJECT_ID;
        use crate::modules::SpecialPowerCommandOptions;

        let (target_id, mut source_ids) = self.extract_target_and_sources(command);
        let target_id = target_id.filter(|id| *id != INVALID_OBJECT_ID);

        let target_id = match target_id {
            Some(id) => id,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from(&format!(
                    "No target specified for {}",
                    failure_label
                )));
            }
        };

        if source_ids.is_empty() {
            let selection_manager = get_selection_manager();
            let mut selected_ids = Vec::new();
            if let Ok(manager) = selection_manager.read() {
                if let Some(selection) = manager.get_player_selection_ref(context.player_id) {
                    selected_ids = selection.get_selected_objects();
                }
            }
            source_ids = selected_ids;
        }

        if source_ids.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from(&format!(
                "No objects specified for {}",
                failure_label
            )));
        }

        let Some(target_arc) = TheGameLogic::find_object_by_id(target_id) else {
            return CommandExecutionResult::Failed(AsciiString::from("Target not found"));
        };
        let Ok(target_guard) = target_arc.read() else {
            return CommandExecutionResult::Failed(AsciiString::from("Target lock failed"));
        };
        if target_guard.is_effectively_dead() {
            return CommandExecutionResult::Failed(AsciiString::from("Target is not alive"));
        }

        let mut any_executed = false;
        for source_id in &source_ids {
            let Some(source_arc) = TheGameLogic::find_object_by_id(*source_id) else {
                continue;
            };
            let Ok(source_guard) = source_arc.read() else {
                continue;
            };
            if source_guard.is_effectively_dead() {
                continue;
            }
            let source_owner = source_guard
                .get_controlling_player_id()
                .map(|id| id as Int)
                .unwrap_or(-1);
            if source_owner != -1 && source_owner != context.player_id {
                continue;
            }

            if !can_execute(&source_guard, &target_guard, CommandSourceType::FromPlayer) {
                continue;
            }

            let mut executed_here = false;
            for module_handle in source_guard.behavior_modules() {
                module_handle.with_module(|module| {
                    let Some(sp_module) = module_special_power_interface(module) else {
                        return;
                    };
                    if sp_module.get_power_type() != power_type as u32 {
                        return;
                    }
                    sp_module
                        .do_special_power_at_object(target_id, SpecialPowerCommandOptions::NONE);
                    executed_here = true;
                });
                if executed_here {
                    break;
                }
            }

            if !executed_here {
                for behavior_arc in source_guard.get_behavior_modules() {
                    let Ok(mut behavior_guard) = behavior_arc.lock() else {
                        continue;
                    };
                    let Some(sp_module) = behavior_guard.get_special_power() else {
                        continue;
                    };
                    if sp_module.get_power_type() != power_type as u32 {
                        continue;
                    }
                    sp_module
                        .do_special_power_at_object(target_id, SpecialPowerCommandOptions::NONE);
                    executed_here = true;
                    break;
                }
            }

            if executed_here {
                any_executed = true;
            }
        }

        if any_executed {
            CommandExecutionResult::Success
        } else {
            CommandExecutionResult::Failed(AsciiString::from(&format!(
                "No eligible units available for {}",
                failure_label
            )))
        }
    }

    fn execute_snipe_vehicle(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        use crate::common::INVALID_OBJECT_ID;

        let (target_id, mut attacker_ids) = self.extract_target_and_sources(command);
        let target_id = target_id.filter(|id| *id != INVALID_OBJECT_ID);

        let target_id = match target_id {
            Some(id) => id,
            None => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "No target specified for snipe vehicle",
                ));
            }
        };

        if attacker_ids.is_empty() {
            let selection_manager = get_selection_manager();
            let mut selected_ids = Vec::new();
            if let Ok(manager) = selection_manager.read() {
                if let Some(selection) = manager.get_player_selection_ref(context.player_id) {
                    selected_ids = selection.get_selected_objects();
                }
            }
            attacker_ids = selected_ids;
        }

        if attacker_ids.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from(
                "No attackers specified for snipe vehicle",
            ));
        }

        let Some(target_arc) = TheGameLogic::find_object_by_id(target_id) else {
            return CommandExecutionResult::Failed(AsciiString::from("Target not found"));
        };
        let Ok(target_guard) = target_arc.read() else {
            return CommandExecutionResult::Failed(AsciiString::from("Target lock failed"));
        };
        if target_guard.is_effectively_dead() {
            return CommandExecutionResult::Failed(AsciiString::from("Target is not alive"));
        }

        let mut eligible_attackers = Vec::new();
        for attacker_id in attacker_ids {
            let Some(attacker_arc) = TheGameLogic::find_object_by_id(attacker_id) else {
                continue;
            };
            let Ok(attacker_guard) = attacker_arc.read() else {
                continue;
            };
            if attacker_guard.is_effectively_dead() {
                continue;
            }
            let attacker_owner = attacker_guard
                .get_controlling_player_id()
                .map(|id| id as Int)
                .unwrap_or(-1);
            if attacker_owner != -1 && attacker_owner != context.player_id {
                continue;
            }
            if !TheActionManager::can_snipe_vehicle(
                &attacker_guard,
                &target_guard,
                CommandSourceType::FromPlayer,
            ) {
                continue;
            }
            eligible_attackers.push(attacker_id);
        }

        if eligible_attackers.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from(
                "No attackers can snipe vehicle",
            ));
        }

        if let Some(ai_manager) = &context.ai_manager {
            if let Ok(mut ai) = ai_manager.write() {
                if ai.issue_attack_order(&eligible_attackers, target_id) {
                    return CommandExecutionResult::Success;
                }
            }
        }

        CommandExecutionResult::Failed(AsciiString::from(
            "AI system failed to process snipe vehicle order",
        ))
    }

    /// Cheer simply succeeds; animation/audio handled client-side.
    fn execute_cheer(&self, _command: &QueuedCommand) -> CommandExecutionResult {
        CommandExecutionResult::Success
    }

    fn execute_overcharge_toggle(
        &self,
        _command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let selection_manager = get_selection_manager();
        let object_ids = match selection_manager.read() {
            Ok(manager) => manager
                .get_player_selection_ref(context.player_id)
                .map(|selection| selection.get_selected_objects())
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        };

        if object_ids.is_empty() {
            return CommandExecutionResult::Success;
        }

        let mut any_toggled = false;
        for object_id in object_ids {
            let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.write() else {
                continue;
            };
            let mut toggled = false;
            for module_handle in obj_guard.behavior_modules() {
                let matched = module_handle
                    .with_module_downcast::<
                        crate::object::behavior::overcharge_behavior::OverchargeBehaviorModule,
                        _,
                        _,
                    >(|overcharge_module| {
                        let _ = crate::object::behavior::behavior_module::OverchargeBehaviorInterface::toggle(
                            overcharge_module.behavior_mut(),
                        );
                    })
                    .is_some();
                if matched {
                    toggled = true;
                    break;
                }
            }

            if !toggled {
                for behavior in obj_guard.get_behavior_modules() {
                    if let Ok(mut behavior_guard) = behavior.lock() {
                        if let Some(overcharge) = behavior_guard.get_overcharge_behavior_interface()
                        {
                            let _ = overcharge.toggle();
                            toggled = true;
                            break;
                        }
                    }
                }
            }

            if toggled {
                any_toggled = true;
            }
        }

        let _ = any_toggled;
        CommandExecutionResult::Success
    }

    fn execute_switch_weapons(
        &self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Matches C++ MSG_SWITCH_WEAPONS: lock chosen weapon slot for the current selection.
        let weapon_slot = command.command.get_argument(0).and_then(|arg| match arg {
            crate::commands::command::CommandArgumentType::Integer(value) => match *value {
                0 => Some(WeaponSlotType::Primary),
                1 => Some(WeaponSlotType::Secondary),
                2 => Some(WeaponSlotType::Tertiary),
                _ => None,
            },
            _ => None,
        });
        let Some(weapon_slot) = weapon_slot else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "SwitchWeapons missing weapon slot",
            ));
        };

        let selection_manager = get_selection_manager();
        let Ok(manager) = selection_manager.read() else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Selection manager unavailable for SwitchWeapons",
            ));
        };
        let Some(selection) = manager.get_player_selection_ref(context.player_id) else {
            return CommandExecutionResult::Failed(AsciiString::from("No player selection"));
        };
        let selected = selection.get_selected_objects();
        if selected.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from("No selected objects"));
        }

        for object_id in selected {
            let Some(obj) = OBJECT_REGISTRY.get_object(object_id) else {
                continue;
            };
            let Ok(mut guard) = obj.write() else {
                continue;
            };
            if guard.is_destroyed() {
                continue;
            }
            if guard.get_controlling_player_id().map(|id| id as Int) != Some(context.player_id) {
                continue;
            }
            guard.set_weapon_lock(weapon_slot, WeaponLockType::LockedPermanently);
        }

        CommandExecutionResult::Success
    }

    fn execute_evacuate_command(
        &self,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Mirrors MSG_EVACUATE / AIGroup::groupEvacuate for the current selection.
        let selection_manager = get_selection_manager();
        let selected = selection_manager
            .read()
            .ok()
            .and_then(|manager| {
                manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
            })
            .unwrap_or_default();

        // C++ dispatch unlocks the entire selected group first, then issues
        // evacuation commands.
        for object_id in &selected {
            let Some(obj) = OBJECT_REGISTRY.get_object(*object_id) else {
                continue;
            };
            let Ok(mut guard) = obj.write() else {
                continue;
            };
            if guard.is_destroyed() {
                continue;
            }
            guard.release_weapon_lock(WeaponLockType::LockedTemporarily);
        }

        for object_id in selected {
            let Some(obj) = OBJECT_REGISTRY.get_object(object_id) else {
                continue;
            };
            let Ok(guard) = obj.read() else {
                continue;
            };
            if guard.is_destroyed() {
                continue;
            }

            let ai = guard.get_ai_update_interface();
            let is_aircraft = guard.is_kind_of(KindOf::Aircraft);
            let is_airborne_target = guard.is_airborne_target();
            let position = *guard.get_position();
            let contain = if ai.is_none() && guard.is_kind_of(KindOf::Structure) {
                guard.get_contain()
            } else {
                None
            };
            drop(guard);

            if let Some(ai) = ai {
                if is_aircraft && is_airborne_target {
                    let mut drop_position = position;
                    if let Some(terrain) = TheTerrainLogic::get() {
                        let layer = terrain.get_highest_layer_for_destination(&drop_position);
                        drop_position.z =
                            terrain.get_layer_height(drop_position.x, drop_position.y, layer);
                    }
                    ai.ai_move_to_and_evacuate(&drop_position, CommandSourceType::FromPlayer);
                } else if let Ok(mut ai_guard) = ai.lock() {
                    let params = crate::ai::AiCommandParams::new(
                        crate::ai::AiCommandType::Evacuate,
                        CommandSourceType::FromPlayer,
                    );
                    let _ = ai_guard.execute_command(&params);
                }
            } else if let Some(contain) = contain {
                let _ = contain.order_all_passengers_to_exit(CommandSourceType::FromPlayer, false);
            }
        }

        CommandExecutionResult::Success
    }

    fn execute_internet_hack_command(
        &self,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Mirrors MSG_INTERNET_HACK / AIGroup::groupHackInternet for the current selection.
        let selection_manager = get_selection_manager();
        let selected = selection_manager
            .read()
            .ok()
            .and_then(|manager| {
                manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
            })
            .unwrap_or_default();

        for object_id in &selected {
            let Some(obj) = OBJECT_REGISTRY.get_object(*object_id) else {
                continue;
            };
            let Ok(mut guard) = obj.write() else {
                continue;
            };
            if guard.is_destroyed() {
                continue;
            }
            if guard.get_controlling_player_id().map(|id| id as Int) != Some(context.player_id) {
                continue;
            }
            guard.release_weapon_lock(WeaponLockType::LockedTemporarily);
        }

        for object_id in selected {
            let Some(obj) = OBJECT_REGISTRY.get_object(object_id) else {
                continue;
            };
            let Ok(guard) = obj.read() else {
                continue;
            };
            if guard.is_destroyed() {
                continue;
            }
            if guard.get_controlling_player_id().map(|id| id as Int) != Some(context.player_id) {
                continue;
            }
            let Some(ai) = guard.get_ai_update_interface() else {
                continue;
            };
            drop(guard);

            let ai_lock = ai.lock();
            if let Ok(mut ai_guard) = ai_lock {
                let params = crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::HackInternet,
                    CommandSourceType::FromPlayer,
                );
                let _ = ai_guard.execute_command(&params);
            }
        }

        CommandExecutionResult::Success
    }

    fn execute_combat_drop_command(
        &self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let mut target_object = None;
        let mut target_position = self.extract_command_location(command);

        if command.command.get_type() == CommandType::CombatDropAtObject {
            target_object = self.extract_object_ids(command).first().copied();
            if let Some(target_id) = target_object {
                target_position = TheGameLogic::find_object_by_id(target_id)
                    .and_then(|obj| obj.read().ok().map(|guard| *guard.get_position()));
            }
        }

        let Some(position) = target_position else {
            return CommandExecutionResult::Failed(AsciiString::from("CombatDrop missing target"));
        };

        let selection_manager = get_selection_manager();
        let selected = selection_manager
            .read()
            .ok()
            .and_then(|manager| {
                manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
            })
            .unwrap_or_default();

        for object_id in selected {
            let Some(obj) = OBJECT_REGISTRY.get_object(object_id) else {
                continue;
            };
            let Ok(guard) = obj.read() else {
                continue;
            };
            if guard.is_destroyed() {
                continue;
            }
            if guard.get_controlling_player_id().map(|id| id as Int) != Some(context.player_id) {
                continue;
            }
            let Some(ai) = guard.get_ai_update_interface() else {
                continue;
            };
            drop(guard);

            let ai_lock = ai.lock();
            if let Ok(mut ai_guard) = ai_lock {
                let mut params = crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::CombatDrop,
                    CommandSourceType::FromPlayer,
                );
                params.obj = target_object;
                params.pos = position;
                let _ = ai_guard.execute_command(&params);
            }
        }

        CommandExecutionResult::Success
    }

    fn execute_weapon_target_command(
        &self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        use crate::commands::command::CommandArgumentType;

        let cmd_type = command.command.get_type();
        let weapon_slot = match command.command.get_argument(0) {
            Some(CommandArgumentType::Integer(0)) => WeaponSlotType::Primary,
            Some(CommandArgumentType::Integer(1)) => WeaponSlotType::Secondary,
            Some(CommandArgumentType::Integer(2)) => WeaponSlotType::Tertiary,
            _ => {
                return CommandExecutionResult::Failed(AsciiString::from(
                    "Weapon command missing weapon slot",
                ))
            }
        };

        let max_shots_to_fire_arg = if cmd_type == CommandType::DoWeapon {
            1
        } else {
            2
        };
        let max_shots_to_fire = match command.command.get_argument(max_shots_to_fire_arg) {
            Some(CommandArgumentType::Integer(value)) => *value,
            _ => NO_MAX_SHOTS_LIMIT,
        };

        let target_object = if cmd_type == CommandType::DoWeaponAtObject {
            match command.command.get_argument(1) {
                Some(CommandArgumentType::ObjectID(id)) => Some(*id),
                _ => {
                    return CommandExecutionResult::Failed(AsciiString::from(
                        "Weapon object command missing target",
                    ))
                }
            }
        } else {
            None
        };

        let target_position = if cmd_type == CommandType::DoWeaponAtLocation {
            match command.command.get_argument(1) {
                Some(CommandArgumentType::Location(pos)) => Some(*pos),
                _ => {
                    return CommandExecutionResult::Failed(AsciiString::from(
                        "Weapon location command missing target",
                    ))
                }
            }
        } else {
            None
        };

        let target_arc = match target_object {
            Some(id) => match TheGameLogic::find_object_by_id(id) {
                Some(obj) => Some(obj),
                None => return CommandExecutionResult::Success,
            },
            None => None,
        };

        let selection_manager = get_selection_manager();
        let selected = selection_manager
            .read()
            .ok()
            .and_then(|manager| {
                manager
                    .get_player_selection_ref(context.player_id)
                    .map(|selection| selection.get_selected_objects())
            })
            .unwrap_or_default();

        for object_id in selected {
            let Some(obj) = OBJECT_REGISTRY.get_object(object_id) else {
                continue;
            };
            let Ok(mut guard) = obj.write() else {
                continue;
            };
            if guard.is_destroyed() {
                continue;
            }
            if guard.get_controlling_player_id().map(|id| id as Int) != Some(context.player_id) {
                continue;
            }

            guard.set_weapon_lock(weapon_slot, WeaponLockType::LockedTemporarily);
            let Some(ai) = guard.get_ai_update_interface() else {
                continue;
            };
            let own_position = if cmd_type == CommandType::DoWeapon {
                Some(*guard.get_position())
            } else {
                None
            };
            drop(guard);

            if let Some(target) = &target_arc {
                ai.ai_attack_object(target, max_shots_to_fire, CommandSourceType::FromPlayer);
            } else if let Some(position) = target_position {
                ai.ai_attack_position(&position, max_shots_to_fire, CommandSourceType::FromPlayer);
            } else if let Some(position) = own_position {
                ai.ai_attack_position(&position, max_shots_to_fire, CommandSourceType::FromPlayer);
            }
        }

        CommandExecutionResult::Success
    }

    fn execute_enable_retaliation(
        &self,
        command: &QueuedCommand,
        _context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Matches C++ MSG_ENABLE_RETALIATION_MODE: sets per-player logical retaliation mode.
        let player_index = command.command.get_argument(0).and_then(|arg| match arg {
            crate::commands::command::CommandArgumentType::Integer(value) => Some(*value),
            _ => None,
        });
        let enable = command.command.get_argument(1).and_then(|arg| match arg {
            crate::commands::command::CommandArgumentType::Boolean(value) => Some(*value),
            _ => None,
        });
        let (Some(player_index), Some(enable)) = (player_index, enable) else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "EnableRetaliationMode missing args",
            ));
        };

        let list_lock = crate::player::player_list();
        let Ok(list) = list_lock.read() else {
            return CommandExecutionResult::Failed(AsciiString::from("Player list unavailable"));
        };
        let Some(player) = list.get_player(player_index) else {
            return CommandExecutionResult::Failed(AsciiString::from("Player not found"));
        };
        let Ok(mut guard) = player.write() else {
            return CommandExecutionResult::Failed(AsciiString::from("Failed to lock player"));
        };
        guard.set_logical_retaliation_mode_enabled(enable);
        CommandExecutionResult::Success
    }

    fn execute_purchase_science(
        &self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let science = command.command.get_argument(0).and_then(|arg| match arg {
            crate::commands::command::CommandArgumentType::Integer(value) => {
                Some(*value as ScienceType)
            }
            _ => None,
        });
        let Some(science) = science else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "PurchaseScience missing science",
            ));
        };

        if science == SCIENCE_INVALID {
            return CommandExecutionResult::Success;
        }

        let list_lock = crate::player::player_list();
        let Ok(list) = list_lock.read() else {
            return CommandExecutionResult::Failed(AsciiString::from("Player list unavailable"));
        };
        let Some(player) = list.get_player(context.player_id) else {
            return CommandExecutionResult::Success;
        };
        let Ok(mut guard) = player.write() else {
            return CommandExecutionResult::Failed(AsciiString::from("Failed to lock player"));
        };

        let _ = guard.attempt_to_purchase_science(science);
        CommandExecutionResult::Success
    }

    fn execute_create_formation(
        &self,
        _command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        // Matches C++ MSG_CREATE_FORMATION: toggles a "preserve relative offsets" formation on the
        // currently selected controllable units by assigning a shared FormationID and per-unit offset.
        let selection_manager = get_selection_manager();
        let Ok(manager) = selection_manager.read() else {
            return CommandExecutionResult::Failed(AsciiString::from(
                "Selection manager unavailable for CreateFormation",
            ));
        };
        let Some(selection) = manager.get_player_selection_ref(context.player_id) else {
            return CommandExecutionResult::Failed(AsciiString::from("No player selection"));
        };
        let selected = selection.get_selected_objects();
        if selected.is_empty() {
            return CommandExecutionResult::Failed(AsciiString::from("No selected objects"));
        }

        let mut count = 0usize;
        let mut center = Coord3D::new(0.0, 0.0, 0.0);
        let mut formation_id: Option<FormationID> = None;

        for object_id in &selected {
            let Some(obj) = OBJECT_REGISTRY.get_object(*object_id) else {
                continue;
            };
            let Ok(guard) = obj.read() else {
                continue;
            };
            if guard.is_destroyed() {
                continue;
            }
            if guard.is_disabled_by_type(crate::common::DisabledType::Held) {
                continue;
            }
            if guard.get_ai_update_interface().is_none() {
                continue;
            }

            let pos = guard.get_position();
            center.x += pos.x;
            center.y += pos.y;
            center.z += pos.z;

            let cur_id = guard.get_formation_id();
            if count == 0 {
                formation_id = Some(cur_id);
            } else if formation_id.map_or(false, |id| id != cur_id) {
                formation_id = None;
            }
            count += 1;
        }

        if count == 0 {
            return CommandExecutionResult::Failed(AsciiString::from(
                "No eligible objects for formation",
            ));
        }

        center.x /= count as f32;
        center.y /= count as f32;
        center.z /= count as f32;

        let is_formation = formation_id.map(|id| !id.is_none()).unwrap_or(false) && count >= 2;
        let is_formation =
            is_formation || (count == 1 && formation_id.map(|id| !id.is_none()).unwrap_or(false));

        let new_id = if is_formation {
            FormationID::NONE
        } else {
            FormationID::new(NEXT_FORMATION_ID.fetch_add(1, Ordering::Relaxed))
        };

        for object_id in selected {
            let Some(obj) = OBJECT_REGISTRY.get_object(object_id) else {
                continue;
            };
            let Ok(mut guard) = obj.write() else {
                continue;
            };
            if guard.is_destroyed() {
                continue;
            }
            if guard.is_disabled_by_type(crate::common::DisabledType::Held) {
                continue;
            }
            if guard.get_ai_update_interface().is_none() {
                continue;
            }
            if guard.get_controlling_player_id().map(|id| id as Int) != Some(context.player_id) {
                continue;
            }

            let pos = *guard.get_position();
            let offset = crate::common::Coord2D::new(pos.x - center.x, pos.y - center.y);
            guard.set_formation_id(new_id);
            guard.set_formation_offset(offset);
        }

        CommandExecutionResult::Success
    }

    fn execute_clear_game_data(&self) -> CommandExecutionResult {
        match TheGameLogic::clear_game_data() {
            Ok(()) => CommandExecutionResult::Success,
            Err(err) => CommandExecutionResult::Failed(AsciiString::from(&err)),
        }
    }

    fn execute_new_game(&self, command: &QueuedCommand) -> CommandExecutionResult {
        let read_int = |index: Int, fallback: Int| -> Int {
            match command.command.get_argument(index) {
                Some(crate::commands::command::CommandArgumentType::Integer(value)) => *value,
                _ => fallback,
            }
        };

        let game_mode = read_int(0, crate::system::game_logic::GAME_SINGLE_PLAYER);
        let difficulty = read_int(1, 1);
        let rank_points = read_int(2, 0);
        let max_fps_arg = read_int(3, -1);

        if max_fps_arg >= 0 {
            let default_fps = get_engine_global_data()
                .map(|data| data.read().frames_per_second_limit)
                .unwrap_or(30);
            let clamped_fps = if (1..=1000).contains(&max_fps_arg) {
                max_fps_arg
            } else {
                default_fps
            };

            if let Some(data) = get_engine_global_data() {
                let mut data = data.write();
                data.frames_per_second_limit = clamped_fps;
                data.use_fps_limit = true;
            }
            if let Some(engine) = get_game_engine() {
                let mut guard = engine.lock();
                guard.set_frames_per_second_limit(clamped_fps.max(0) as u32);
            }
        }

        TheGameLogic::prepare_new_game(game_mode, difficulty, rank_points);
        match TheGameLogic::start_new_game(false) {
            Ok(()) => CommandExecutionResult::Success,
            Err(err) => CommandExecutionResult::Failed(AsciiString::from(&err)),
        }
    }
}

impl CommandHandler for DefaultCommandHandler {
    fn execute_command(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        let start_time = Instant::now();

        // Update statistics
        self.stats.commands_processed += 1;

        let result = match command.command.get_type() {
            CommandType::ClearGameData => self.execute_clear_game_data(),
            CommandType::NewGame => self.execute_new_game(command),
            CommandType::DoMoveTo
            | CommandType::DoAttackMoveTo
            | CommandType::DoForceMoveTo
            | CommandType::AddWaypoint
            | CommandType::DoSalvage => self.execute_move_command(command, context),
            CommandType::DoAttackObject | CommandType::DoForceAttackObject => {
                self.execute_attack_command(command, context)
            }
            CommandType::Enter => self.execute_targeted_group_command(
                command,
                context,
                crate::ai::AiCommandType::Enter,
                "enter",
            ),
            CommandType::DoRepair => self.execute_targeted_group_command(
                command,
                context,
                crate::ai::AiCommandType::Repair,
                "repair",
            ),
            CommandType::Dock => self.execute_targeted_group_command(
                command,
                context,
                crate::ai::AiCommandType::Dock,
                "dock",
            ),
            CommandType::GetRepaired => self.execute_targeted_group_command(
                command,
                context,
                crate::ai::AiCommandType::GetRepaired,
                "get repaired",
            ),
            CommandType::GetHealed => self.execute_targeted_group_command(
                command,
                context,
                crate::ai::AiCommandType::GetHealed,
                "get healed",
            ),
            CommandType::ResumeConstruction => self.execute_targeted_group_command(
                command,
                context,
                crate::ai::AiCommandType::ResumeConstruction,
                "resume construction",
            ),
            CommandType::DozerConstruct | CommandType::DozerConstructLine => {
                self.execute_build_command(command, context)
            }
            CommandType::Sell => self.execute_sell_command(command, context),
            CommandType::SetRallyPoint => self.execute_set_rally_point(command, context),
            CommandType::SetMineClearingDetail => {
                self.execute_set_mine_clearing_detail(command, context)
            }
            CommandType::DoStop => self.execute_stop_command(command, context),
            CommandType::DoScatter => self.execute_scatter_command(command, context),
            CommandType::DoSpecialPower
            | CommandType::DoSpecialPowerAtLocation
            | CommandType::DoSpecialPowerAtObject
            | CommandType::DoSpecialPowerOverrideDestination => {
                self.execute_special_power(command, context)
            }
            CommandType::Evacuate => self.execute_evacuate_command(context),
            CommandType::InternetHack => self.execute_internet_hack_command(context),
            CommandType::CombatDropAtLocation | CommandType::CombatDropAtObject => {
                self.execute_combat_drop_command(command, context)
            }
            CommandType::DoWeapon
            | CommandType::DoWeaponAtLocation
            | CommandType::DoWeaponAtObject => self.execute_weapon_target_command(command, context),
            CommandType::DoGuardPosition => self.execute_guard_position(command, context),
            CommandType::DoGuardObject => self.execute_guard_object(command, context),
            CommandType::DoCheer => self.execute_cheer(command),
            CommandType::ToggleOvercharge => self.execute_overcharge_toggle(command, context),
            CommandType::SwitchWeapons => self.execute_switch_weapons(command, context),
            CommandType::ConvertToCarbomb => self.execute_targeted_group_command(
                command,
                context,
                crate::ai::AiCommandType::Enter,
                "convert to carbomb",
            ),
            CommandType::CaptureBuilding => self.execute_capture_building(command, context),
            CommandType::DisableVehicleHack => self.execute_hack_special_power_at_object(
                command,
                context,
                crate::common::types::SpecialPowerType::SpecialBlackLotusDisableVehicleHack,
                |obj, target, source| {
                    TheActionManager::can_disable_vehicle_via_hacking(obj, target, source, true)
                },
                "disable vehicle via hacking",
            ),
            CommandType::StealCashHack => self.execute_hack_special_power_at_object(
                command,
                context,
                crate::common::types::SpecialPowerType::SpecialBlackLotusStealCashHack,
                TheActionManager::can_steal_cash_via_hacking,
                "steal cash via hacking",
            ),
            CommandType::DisableBuildingHack => self.execute_hack_special_power_at_object(
                command,
                context,
                crate::common::types::SpecialPowerType::SpecialHackerDisableBuilding,
                TheActionManager::can_disable_building_via_hacking,
                "disable building via hacking",
            ),
            CommandType::SnipeVehicle => self.execute_snipe_vehicle(command, context),
            CommandType::EnableRetaliationMode => self.execute_enable_retaliation(command, context),
            CommandType::PurchaseScience => self.execute_purchase_science(command, context),
            CommandType::CreateFormation => self.execute_create_formation(command, context),
            CommandType::SelfDestruct => self.execute_self_destruct(command, context),
            CommandType::PlaceBeacon => self.execute_place_beacon(command, context),
            CommandType::RemoveBeacon => self.execute_remove_beacon(command, context),
            CommandType::SetBeaconText => self.execute_set_beacon_text(command, context),
            _ => CommandExecutionResult::Failed(AsciiString::from(&format!(
                "Unhandled command type: {:?}",
                command.command.get_type()
            ))),
        };

        // Update execution time statistics
        let execution_time = start_time.elapsed().as_millis() as f64;
        self.stats.average_execution_time_ms = (self.stats.average_execution_time_ms
            * (self.stats.commands_processed - 1) as f64
            + execution_time)
            / self.stats.commands_processed as f64;

        // Update result statistics
        match &result {
            CommandExecutionResult::Success => self.stats.commands_succeeded += 1,
            CommandExecutionResult::Failed(_)
            | CommandExecutionResult::InvalidCommand
            | CommandExecutionResult::InvalidGameState => self.stats.commands_failed += 1,
            CommandExecutionResult::Deferred => self.stats.commands_deferred += 1,
        }

        result
    }

    fn can_handle(&self, command_type: CommandType) -> bool {
        matches!(
            command_type,
            CommandType::ClearGameData
                | CommandType::NewGame
                | CommandType::DoMoveTo
                | CommandType::DoAttackMoveTo
                | CommandType::DoForceMoveTo
                | CommandType::AddWaypoint
                | CommandType::DoSalvage
                | CommandType::DoAttackObject
                | CommandType::DoForceAttackObject
                | CommandType::Enter
                | CommandType::DoRepair
                | CommandType::Dock
                | CommandType::GetRepaired
                | CommandType::GetHealed
                | CommandType::ResumeConstruction
                | CommandType::DozerConstruct
                | CommandType::DozerConstructLine
                | CommandType::Sell
                | CommandType::SetRallyPoint
                | CommandType::SetMineClearingDetail
                | CommandType::DoStop
                | CommandType::DoScatter
                | CommandType::DoSpecialPower
                | CommandType::DoSpecialPowerAtLocation
                | CommandType::DoSpecialPowerAtObject
                | CommandType::DoSpecialPowerOverrideDestination
                | CommandType::Evacuate
                | CommandType::InternetHack
                | CommandType::CombatDropAtLocation
                | CommandType::CombatDropAtObject
                | CommandType::DoWeapon
                | CommandType::DoWeaponAtLocation
                | CommandType::DoWeaponAtObject
                | CommandType::DoGuardPosition
                | CommandType::DoGuardObject
                | CommandType::DoCheer
                | CommandType::ToggleOvercharge
                | CommandType::SwitchWeapons
                | CommandType::ConvertToCarbomb
                | CommandType::CaptureBuilding
                | CommandType::DisableVehicleHack
                | CommandType::StealCashHack
                | CommandType::DisableBuildingHack
                | CommandType::SnipeVehicle
                | CommandType::EnableRetaliationMode
                | CommandType::PurchaseScience
                | CommandType::CreateFormation
                | CommandType::SelfDestruct
                | CommandType::PlaceBeacon
                | CommandType::RemoveBeacon
                | CommandType::SetBeaconText
        )
    }

    fn get_priority(&self) -> i32 {
        100 // Default priority
    }
}

/// Main command processor - orchestrates command execution
pub struct CommandProcessor {
    /// Registered command handlers
    handlers: Vec<Box<dyn CommandHandler + Send>>,

    /// RTS command validator
    validator: RtsCommandValidator,

    /// Current frame number
    current_frame: UnsignedInt,

    /// Execution statistics
    total_stats: CommandExecutionStats,

    /// Performance monitoring
    frame_execution_times: Vec<f64>,
    max_execution_time_ms: f64,

    /// Settings
    enabled: bool,
}

impl CommandProcessor {
    /// Create new command processor
    pub fn new() -> Self {
        let mut processor = Self {
            handlers: Vec::new(),
            validator: RtsCommandValidator::new(),
            current_frame: 0,
            total_stats: CommandExecutionStats::default(),
            frame_execution_times: Vec::with_capacity(300), // 10 seconds at 30 FPS
            max_execution_time_ms: 16.66,                   // Target 60 FPS
            enabled: true,
        };

        // Register selection handler first (high priority, no gameplay side effects).
        processor.register_handler(Box::new(SelectionCommandHandler::new()));

        // Register default handler
        processor.register_handler(Box::new(DefaultCommandHandler::new()));

        processor
    }

    /// Register a command handler
    pub fn register_handler(&mut self, handler: Box<dyn CommandHandler + Send>) {
        self.handlers.push(handler);

        // Sort by priority (highest first)
        self.handlers
            .sort_by(|a, b| b.get_priority().cmp(&a.get_priority()));
    }

    /// Process commands for current frame
    pub fn process_frame(
        &mut self,
        frame: UnsignedInt,
        context: &mut CommandExecutionContext,
    ) -> Result<(), AsciiString> {
        if !self.enabled {
            return Ok(());
        }

        let frame_start = Instant::now();
        self.current_frame = frame;
        context.current_frame = frame;

        // Get commands from queue manager
        let queue_manager = get_command_queue_manager();
        let ready_commands = {
            let mut manager = queue_manager
                .lock()
                .map_err(|_| AsciiString::from("Failed to lock command queue manager"))?;
            manager.update_frame(frame)
        };

        // Process commands for each player
        for (player_id, commands) in ready_commands {
            context.player_id = player_id;

            for command in commands {
                let result = self.execute_single_command(&command, context);

                // Report execution result back to queue manager
                let mut manager = queue_manager
                    .lock()
                    .map_err(|_| AsciiString::from("Failed to lock command queue manager"))?;

                match result {
                    CommandExecutionResult::Success => {
                        manager.complete_command(player_id, command.get_id(), true, None);
                    }
                    CommandExecutionResult::Failed(_)
                    | CommandExecutionResult::InvalidCommand
                    | CommandExecutionResult::InvalidGameState => {
                        manager.complete_command(
                            player_id,
                            command.get_id(),
                            false,
                            result.get_error_message().map(|s| AsciiString::from(s)),
                        );
                    }
                    CommandExecutionResult::Deferred => {
                        // Command will remain in executing state to be retried
                    }
                }
            }
        }

        // Record frame execution time
        let frame_time = frame_start.elapsed().as_millis() as f64;
        self.frame_execution_times.push(frame_time);

        // Keep only recent frame times
        if self.frame_execution_times.len() > 300 {
            self.frame_execution_times.remove(0);
        }

        // Check for performance issues
        if frame_time > self.max_execution_time_ms {
            log::warn!(
                "Command processing took {:.2}ms (target: {:.2}ms)",
                frame_time,
                self.max_execution_time_ms
            );
        }

        Ok(())
    }

    /// Execute a single command
    fn execute_single_command(
        &mut self,
        command: &QueuedCommand,
        context: &mut CommandExecutionContext,
    ) -> CommandExecutionResult {
        context.execution_start_time = Instant::now();
        context.is_network_command = command.command.get_type().is_network_message();

        // Find appropriate handler
        for handler in &mut self.handlers {
            if handler.can_handle(command.command.get_type()) {
                let result = handler.execute_command(command, context);

                // Update total statistics
                self.total_stats.commands_processed += 1;
                match &result {
                    CommandExecutionResult::Success => self.total_stats.commands_succeeded += 1,
                    CommandExecutionResult::Failed(_)
                    | CommandExecutionResult::InvalidCommand
                    | CommandExecutionResult::InvalidGameState => {
                        self.total_stats.commands_failed += 1
                    }
                    CommandExecutionResult::Deferred => self.total_stats.commands_deferred += 1,
                }

                return result;
            }
        }

        // No handler found
        self.total_stats.commands_processed += 1;
        self.total_stats.commands_failed += 1;
        CommandExecutionResult::Failed(AsciiString::from(&format!(
            "No handler for command type: {:?}",
            command.command.get_type()
        )))
    }

    /// Get execution statistics
    pub fn get_statistics(&self) -> &CommandExecutionStats {
        &self.total_stats
    }

    /// Get average frame execution time
    pub fn get_average_frame_time(&self) -> f64 {
        if self.frame_execution_times.is_empty() {
            0.0
        } else {
            self.frame_execution_times.iter().sum::<f64>() / self.frame_execution_times.len() as f64
        }
    }

    /// Enable/disable command processing
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Set maximum execution time per frame
    pub fn set_max_execution_time(&mut self, max_ms: f64) {
        self.max_execution_time_ms = max_ms;
    }
}

impl Default for CommandProcessor {
    fn default() -> Self {
        Self::new()
    }
}

/// Global command processor instance
use once_cell::sync::Lazy;
static COMMAND_PROCESSOR: Lazy<Arc<Mutex<CommandProcessor>>> =
    Lazy::new(|| Arc::new(Mutex::new(CommandProcessor::new())));

/// Get global command processor
pub fn get_command_processor() -> Arc<Mutex<CommandProcessor>> {
    COMMAND_PROCESSOR.clone()
}

// Command processor mock-based tests removed to avoid mocks in fidelity-critical code.

fn override_destination_fallthrough_target_id(command: &QueuedCommand) -> Option<ObjectID> {
    if command.command.get_type() != CommandType::DoSpecialPowerOverrideDestination {
        return None;
    }

    match command.command.get_argument(0) {
        Some(crate::commands::command::CommandArgumentType::Location(location)) => {
            Some(location.x.to_bits())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_handler_accepts_build_line_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::DozerConstruct));
        assert!(handler.can_handle(CommandType::DozerConstructLine));
    }

    #[test]
    fn default_handler_accepts_purchase_science_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::PurchaseScience));
    }

    #[test]
    fn default_handler_accepts_sell_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::Sell));
    }

    #[test]
    fn default_handler_accepts_set_rally_point_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::SetRallyPoint));
    }

    #[test]
    fn default_handler_accepts_mine_clearing_detail_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::SetMineClearingDetail));
    }

    #[test]
    fn default_handler_accepts_internet_hack_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::InternetHack));
    }

    #[test]
    fn default_handler_accepts_combat_drop_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::CombatDropAtLocation));
        assert!(handler.can_handle(CommandType::CombatDropAtObject));
    }

    #[test]
    fn default_handler_accepts_targeted_weapon_commands() {
        let handler = DefaultCommandHandler::new();

        assert!(handler.can_handle(CommandType::DoWeapon));
        assert!(handler.can_handle(CommandType::DoWeaponAtLocation));
        assert!(handler.can_handle(CommandType::DoWeaponAtObject));
    }

    #[test]
    fn override_destination_fallthrough_target_uses_location_x_bits() {
        let mut command = Command::new(CommandType::DoSpecialPowerOverrideDestination);
        command.set_player_index(2);
        command.append_location_argument(Coord3D::new(12.5, -4.0, 9.0));
        command.append_integer_argument(7);
        command.append_object_id_argument(1234);

        let queued = QueuedCommand::new(command, CommandPriority::Normal, 99);

        assert_eq!(
            override_destination_fallthrough_target_id(&queued),
            Some(12.5f32.to_bits())
        );
    }

    #[test]
    fn override_destination_fallthrough_target_ignores_non_override_commands() {
        let command = Command::new(CommandType::DoAttackObject);
        let queued = QueuedCommand::new(command, CommandPriority::Normal, 99);

        assert_eq!(override_destination_fallthrough_target_id(&queued), None);
    }
}
