////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! AI Manager - Bridges CommandProcessor to per-unit AI command execution.
//!
//! Implements the `AIManager` trait from `command_processor`, translating
//! high-level commands (move, attack, guard, stop) into per-unit `ai_do_command`
//! calls on the AI state machine.
//!
//! PARITY_NOTE: In C++, there is no single "AIManager" class. Commands flow from
//! GameLogicDispatch → AIUpdateInterface::aiDoCommand on each selected object.
//! This struct centralizes that dispatch to match the CommandProcessor trait
//! interface used by the existing Rust command system.

use std::collections::HashMap;
use std::sync::MutexGuard;
use std::sync::{Arc, Mutex, RwLock};

use super::command_processor::AIManager;
use super::unit_command_queue::{UnitCommand, UnitCommandQueue};
use crate::ai::{AiCommandParams, AiCommandType, GuardMode};
use crate::common::{CommandSourceType, Coord3D, ObjectID};
use crate::modules::AIUpdateInterfaceExt;
use crate::object::registry::OBJECT_REGISTRY;

/// Global per-unit command queues. Each unit has its own FIFO queue.
/// Indexed by ObjectID.
pub struct UnitCommandQueueManager {
    queues: HashMap<ObjectID, UnitCommandQueue>,
}

impl UnitCommandQueueManager {
    pub fn new() -> Self {
        Self {
            queues: HashMap::new(),
        }
    }

    pub fn get_or_create_queue(&mut self, object_id: ObjectID) -> &mut UnitCommandQueue {
        self.queues
            .entry(object_id)
            .or_insert_with(UnitCommandQueue::new)
    }

    pub fn get_queue(&self, object_id: ObjectID) -> Option<&UnitCommandQueue> {
        self.queues.get(&object_id)
    }

    pub fn get_queue_mut(&mut self, object_id: ObjectID) -> Option<&mut UnitCommandQueue> {
        self.queues.get_mut(&object_id)
    }

    pub fn remove_queue(&mut self, object_id: ObjectID) {
        self.queues.remove(&object_id);
    }

    pub fn clear_all(&mut self) {
        self.queues.clear();
    }
}

impl Default for UnitCommandQueueManager {
    fn default() -> Self {
        Self::new()
    }
}

use once_cell::sync::Lazy;

static UNIT_QUEUE_MANAGER: Lazy<Mutex<UnitCommandQueueManager>> =
    Lazy::new(|| Mutex::new(UnitCommandQueueManager::new()));

pub fn get_unit_queue_manager() -> MutexGuard<'static, UnitCommandQueueManager> {
    UNIT_QUEUE_MANAGER.lock().unwrap_or_else(|e| e.into_inner())
}

/// Concrete implementation of the `AIManager` trait.
///
/// PARITY_NOTE: In C++, there is no single AIManager class. This struct exists
/// to satisfy the CommandProcessor's trait-based dispatch while routing commands
/// through the per-unit queue system.
pub struct AIManagerImpl {
    cmd_source: CommandSourceType,
    current_frame: u32,
}

impl AIManagerImpl {
    pub fn new() -> Self {
        Self {
            cmd_source: CommandSourceType::FromPlayer,
            current_frame: 0,
        }
    }

    pub fn with_context(cmd_source: CommandSourceType, current_frame: u32) -> Self {
        Self {
            cmd_source,
            current_frame,
        }
    }
}

impl Default for AIManagerImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl AIManager for AIManagerImpl {
    fn issue_move_order(&mut self, objects: &[ObjectID], destination: Coord3D) -> bool {
        let mut any_ok = false;
        let mut manager = get_unit_queue_manager();

        for &object_id in objects {
            let cmd = UnitCommand::move_to_position(destination, self.cmd_source);
            if manager
                .get_or_create_queue(object_id)
                .issue_command(cmd, self.current_frame)
            {
                any_ok = true;
            }
        }
        // Must drop manager before execute_next_command_for_unit re-locks UNIT_QUEUE_MANAGER
        drop(manager);

        for &object_id in objects {
            execute_next_command_for_unit(object_id);
        }

        any_ok
    }

    fn issue_attack_order(&mut self, attackers: &[ObjectID], target: ObjectID) -> bool {
        let mut any_ok = false;
        let mut manager = get_unit_queue_manager();

        for &object_id in attackers {
            let cmd = UnitCommand::attack_object(target, self.cmd_source);
            if manager
                .get_or_create_queue(object_id)
                .issue_command(cmd, self.current_frame)
            {
                any_ok = true;
            }
        }
        drop(manager);

        for &object_id in attackers {
            execute_next_command_for_unit(object_id);
        }

        any_ok
    }

    fn issue_build_order(&mut self, builder: ObjectID, _template: &str, position: Coord3D) -> bool {
        let mut manager = get_unit_queue_manager();

        let cmd = UnitCommand::move_to_position(position, self.cmd_source);
        let accepted = manager
            .get_or_create_queue(builder)
            .issue_command(cmd, self.current_frame);
        drop(manager);

        if accepted {
            execute_next_command_for_unit(builder);
        }

        accepted
    }

    fn issue_stop_order(&mut self, objects: &[ObjectID]) -> bool {
        let mut manager = get_unit_queue_manager();

        for &object_id in objects {
            if let Some(queue) = manager.get_queue_mut(object_id) {
                let cmd = UnitCommand::stop(self.cmd_source);
                queue.issue_command(cmd, self.current_frame);
            }
        }
        drop(manager);

        // PARITY_NOTE: Stop immediately transitions to AI_IDLE (C++ aiIdle).
        for &object_id in objects {
            execute_ai_command_on_unit(object_id, AiCommandType::Idle, self.cmd_source);
        }

        true
    }

    fn issue_targeted_order(
        &mut self,
        objects: &[ObjectID],
        target: ObjectID,
        ai_command: AiCommandType,
    ) -> bool {
        let mut any_ok = false;
        let mut manager = get_unit_queue_manager();

        for &object_id in objects {
            let cmd_with_target = match ai_command {
                AiCommandType::Enter => UnitCommand::enter(target, self.cmd_source),
                AiCommandType::Repair => UnitCommand::repair(target, self.cmd_source),
                AiCommandType::Dock => UnitCommand::dock(target, self.cmd_source),
                AiCommandType::GetRepaired => UnitCommand::get_repaired(target, self.cmd_source),
                AiCommandType::GetHealed => UnitCommand::get_healed(target, self.cmd_source),
                AiCommandType::ResumeConstruction => {
                    UnitCommand::resume_construction(target, self.cmd_source)
                }
                _ => UnitCommand::new(ai_command, self.cmd_source),
            };

            if manager
                .get_or_create_queue(object_id)
                .issue_command(cmd_with_target, self.current_frame)
            {
                any_ok = true;
            }
        }
        drop(manager);

        for &object_id in objects {
            execute_next_command_for_unit(object_id);
        }

        any_ok
    }

    fn issue_guard_position_order(
        &mut self,
        objects: &[ObjectID],
        position: Coord3D,
        guard_mode: GuardMode,
    ) -> bool {
        let mut any_ok = false;
        let mut manager = get_unit_queue_manager();

        for &object_id in objects {
            let cmd = UnitCommand::guard_position(position, guard_mode.as_i32(), self.cmd_source);
            if manager
                .get_or_create_queue(object_id)
                .issue_command(cmd, self.current_frame)
            {
                any_ok = true;
            }
        }
        drop(manager);

        for &object_id in objects {
            execute_next_command_for_unit(object_id);
        }

        any_ok
    }

    fn issue_guard_object_order(
        &mut self,
        objects: &[ObjectID],
        target: ObjectID,
        guard_mode: GuardMode,
    ) -> bool {
        let mut any_ok = false;
        let mut manager = get_unit_queue_manager();

        for &object_id in objects {
            let cmd = UnitCommand::guard_object(target, guard_mode.as_i32(), self.cmd_source);
            if manager
                .get_or_create_queue(object_id)
                .issue_command(cmd, self.current_frame)
            {
                any_ok = true;
            }
        }
        drop(manager);

        for &object_id in objects {
            execute_next_command_for_unit(object_id);
        }

        any_ok
    }
}

/// Execute the next command in a unit's queue.
///
/// This dequeues the next PENDING command and calls ai_do_command on the unit's
/// AI state machine.
fn execute_next_command_for_unit(object_id: ObjectID) {
    let (cmd_type, cmd_source) = {
        let mut manager = get_unit_queue_manager();
        let Some(queue) = manager.get_queue_mut(object_id) else {
            return;
        };
        let Some(cmd) = queue.process_next_command() else {
            return;
        };
        (cmd.cmd, cmd.cmd_source)
    };

    execute_ai_command_on_unit(object_id, cmd_type, cmd_source);
}

/// Execute an AI command directly on a unit's AI state machine.
///
/// PARITY_NOTE: This matches the C++ path where GameLogicDispatch calls
/// obj->getAI()->aiDoCommand(parms) or obj->getAIUpdateInterface()->aiDoCommand(parms).
fn execute_ai_command_on_unit(
    object_id: ObjectID,
    ai_cmd: AiCommandType,
    cmd_source: CommandSourceType,
) {
    let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
        return;
    };
    let Ok(obj_guard) = obj_arc.read() else {
        return;
    };

    let Some(ai) = obj_guard.get_ai_update_interface() else {
        return;
    };
    drop(obj_guard);

    let mut params = AiCommandParams::new(ai_cmd, cmd_source);

    if let Ok(mut manager) = UNIT_QUEUE_MANAGER.lock() {
        if let Some(queue) = manager.get_queue(object_id) {
            if let Some(active) = queue.get_active_command() {
                params.pos = active.pos;
                params.obj = active.target_object;
                params.other_obj = active.other_object;
                params.int_value = active.int_value;
            }
        }
    }

    let _ = ai.execute_command(&params);
}

/// Update all unit command queues. Called once per game frame.
///
/// For each unit with an active command, checks if the command has completed
/// (AI state machine returned to idle) and advances to the next queued command.
pub fn update_unit_command_queues(current_frame: u32) {
    let manager = get_unit_queue_manager();
    let object_ids: Vec<ObjectID> = manager.queues.keys().copied().collect();
    drop(manager);

    for object_id in object_ids {
        if should_advance_unit_queue(object_id) {
            advance_unit_queue(object_id, current_frame);
        }
    }
}

fn should_advance_unit_queue(object_id: ObjectID) -> bool {
    let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
        return false;
    };
    let Ok(obj_guard) = obj_arc.read() else {
        return false;
    };

    let Some(ai) = obj_guard.get_ai_update_interface() else {
        return false;
    };
    drop(obj_guard);

    ai.is_idle()
}

fn advance_unit_queue(_object_id: ObjectID, _current_frame: u32) {
    let next_cmd = {
        let mut manager = get_unit_queue_manager();
        let Some(queue) = manager.get_queue_mut(_object_id) else {
            return;
        };

        if queue.has_active_command() {
            queue.complete_current_command();
        }

        if queue.has_pending_commands() {
            queue
                .process_next_command()
                .map(|cmd| (cmd.cmd, cmd.cmd_source))
        } else {
            None
        }
    };

    if let Some((cmd_type, cmd_source)) = next_cmd {
        execute_ai_command_on_unit(_object_id, cmd_type, cmd_source);
    }
}

/// Clear a unit's command queue (e.g., when unit is destroyed or sold).
pub fn clear_unit_command_queue(object_id: ObjectID) {
    let mut manager = get_unit_queue_manager();
    manager.remove_queue(object_id);
}

/// Clear all unit command queues (e.g., on game end).
pub fn clear_all_unit_command_queues() {
    let mut manager = get_unit_queue_manager();
    manager.clear_all();
}

/// Create a new `AIManagerImpl` wrapped in `Arc<RwLock<dyn AIManager>>` for
/// use in `CommandExecutionContext`.
pub fn create_ai_manager(
    cmd_source: CommandSourceType,
    current_frame: u32,
) -> Arc<RwLock<dyn AIManager>> {
    Arc::new(RwLock::new(AIManagerImpl::with_context(
        cmd_source,
        current_frame,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unit_queue_manager() {
        let mut manager = UnitCommandQueueManager::new();
        assert!(manager.get_queue(1).is_none());

        let queue = manager.get_or_create_queue(1);
        assert_eq!(queue.len(), 0);
        assert!(manager.get_queue(1).is_some());
    }

    #[test]
    fn test_global_queue_manager() {
        let manager = get_unit_queue_manager();
        assert!(manager.get_queue(999).is_none());

        drop(manager);
        let mut manager = get_unit_queue_manager();
        manager.get_or_create_queue(999);
        drop(manager);

        let manager = get_unit_queue_manager();
        assert!(manager.get_queue(999).is_some());
    }

    #[test]
    fn test_ai_manager_impl_default() {
        let mut mgr = AIManagerImpl::new();
        // No objects → returns false for move/attack/guard
        assert!(!mgr.issue_move_order(&[], Coord3D::new(0.0, 0.0, 0.0)));
        assert!(!mgr.issue_attack_order(&[], 0));
        assert!(!mgr.issue_build_order(0, "test", Coord3D::new(0.0, 0.0, 0.0)));
        assert!(mgr.issue_stop_order(&[]));
    }
}
