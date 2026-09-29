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
use std::sync::{Arc, RwLock};

use super::command_processor::AIManager;
use super::unit_command_queue::{UnitCommand, UnitCommandQueue};
use crate::ai::{AiCommandParams, AiCommandType, GuardMode};
use crate::common::{CommandSourceType, Coord3D, ObjectID};
use crate::modules::AIUpdateInterfaceExt;
use crate::object::registry::OBJECT_REGISTRY;

/// Wave 425: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

/// Per-manager unit command queues. Each unit has its own FIFO queue,
/// indexed by ObjectID. Owned by [`AIManagerImpl`] so two games do not share entries.
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

/// Concrete implementation of the `AIManager` trait.
///
/// PARITY_NOTE: In C++, there is no single AIManager class. This struct exists
/// to satisfy the CommandProcessor's trait-based dispatch while routing commands
/// through the per-unit queue system.
///
/// Unit queues live on this value. Callers that already hold `&mut AIManagerImpl`
/// (or the `AIManager` lock around it) must not reach a process-global map.
pub struct AIManagerImpl {
    cmd_source: CommandSourceType,
    current_frame: u32,
    queues: UnitCommandQueueManager,
}

impl AIManagerImpl {
    pub fn new() -> Self {
        Self {
            cmd_source: CommandSourceType::FromPlayer,
            current_frame: 0,
            queues: UnitCommandQueueManager::new(),
        }
    }

    pub fn with_context(cmd_source: CommandSourceType, current_frame: u32) -> Self {
        Self {
            cmd_source,
            current_frame,
            queues: UnitCommandQueueManager::new(),
        }
    }

    /// Advance every unit queue owned by this manager. Called once per game frame.
    pub fn update_unit_command_queues(&mut self, current_frame: u32) {
        advance_owned_unit_queues(&mut self.queues, current_frame);
    }

    /// Clear one unit's queue (destroyed or sold).
    pub fn clear_unit_command_queue(&mut self, object_id: ObjectID) {
        self.queues.remove_queue(object_id);
    }

    /// Clear every unit queue owned by this manager (game end).
    pub fn clear_all_unit_command_queues(&mut self) {
        self.queues.clear_all();
    }
}

impl Default for AIManagerImpl {
    fn default() -> Self {
        Self::new()
    }
}

fn release_temporary_weapon_locks(objects: &[ObjectID]) {
    for &object_id in objects {
        let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj| {
            obj.release_weapon_lock(crate::weapon::WeaponLockType::LockedTemporarily);
        });
    }
}

impl AIManager for AIManagerImpl {
    fn issue_move_order(&mut self, objects: &[ObjectID], destination: Coord3D) -> bool {
        release_temporary_weapon_locks(objects);
        let mut any_ok = false;
        let frame = self.current_frame;
        let source = self.cmd_source;

        for &object_id in objects {
            let cmd = UnitCommand::move_to_position(destination, source);
            if self
                .queues
                .get_or_create_queue(object_id)
                .issue_command(cmd, frame)
            {
                any_ok = true;
            }
        }

        for &object_id in objects {
            execute_next_command_for_unit(&mut self.queues, object_id);
        }

        any_ok
    }

    fn issue_waypoint_order(&mut self, objects: &[ObjectID], destination: Coord3D) -> bool {
        release_temporary_weapon_locks(objects);
        let mut any_ok = false;
        let frame = self.current_frame;
        let source = self.cmd_source;

        for &object_id in objects {
            let mut cmd = UnitCommand::new(AiCommandType::FollowPathAppend, source);
            cmd.pos = destination;
            cmd.is_queued = true;
            if self
                .queues
                .get_or_create_queue(object_id)
                .issue_command(cmd, frame)
            {
                any_ok = true;
            }
        }

        for &object_id in objects {
            execute_next_command_for_unit(&mut self.queues, object_id);
        }

        any_ok
    }

    fn issue_attack_move_order(&mut self, objects: &[ObjectID], destination: Coord3D) -> bool {
        let mut any_ok = false;
        let frame = self.current_frame;
        let source = self.cmd_source;

        release_temporary_weapon_locks(objects);

        for &object_id in objects {
            let cmd = UnitCommand::attack_move_to_position(destination, source);
            if self
                .queues
                .get_or_create_queue(object_id)
                .issue_command(cmd, frame)
            {
                any_ok = true;
            }
        }

        for &object_id in objects {
            execute_next_command_for_unit(&mut self.queues, object_id);
        }

        any_ok
    }

    fn issue_attack_order(&mut self, attackers: &[ObjectID], target: ObjectID) -> bool {
        release_temporary_weapon_locks(attackers);
        let mut any_ok = false;
        let frame = self.current_frame;
        let source = self.cmd_source;

        for &object_id in attackers {
            let cmd = UnitCommand::attack_object(target, source);
            if self
                .queues
                .get_or_create_queue(object_id)
                .issue_command(cmd, frame)
            {
                any_ok = true;
            }
        }

        for &object_id in attackers {
            execute_next_command_for_unit(&mut self.queues, object_id);
        }

        any_ok
    }

    fn issue_build_order(&mut self, builder: ObjectID, _template: &str, position: Coord3D) -> bool {
        let frame = self.current_frame;
        let source = self.cmd_source;
        let cmd = UnitCommand::move_to_position(position, source);
        let accepted = self
            .queues
            .get_or_create_queue(builder)
            .issue_command(cmd, frame);

        if accepted {
            execute_next_command_for_unit(&mut self.queues, builder);
        }

        accepted
    }

    fn issue_stop_order(&mut self, objects: &[ObjectID]) -> bool {
        let frame = self.current_frame;
        let source = self.cmd_source;

        for &object_id in objects {
            if let Some(queue) = self.queues.get_queue_mut(object_id) {
                let cmd = UnitCommand::stop(source);
                queue.issue_command(cmd, frame);
            }
        }

        // PARITY_NOTE: Stop immediately transitions to AI_IDLE (C++ aiIdle).
        for &object_id in objects {
            execute_ai_command_on_unit(&self.queues, object_id, AiCommandType::Idle, source);
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
        let frame = self.current_frame;
        let source = self.cmd_source;

        for &object_id in objects {
            let cmd_with_target = match ai_command {
                AiCommandType::Enter => UnitCommand::enter(target, source),
                AiCommandType::Repair => UnitCommand::repair(target, source),
                AiCommandType::Dock => UnitCommand::dock(target, source),
                AiCommandType::GetRepaired => UnitCommand::get_repaired(target, source),
                AiCommandType::GetHealed => UnitCommand::get_healed(target, source),
                AiCommandType::ResumeConstruction => {
                    UnitCommand::resume_construction(target, source)
                }
                _ => UnitCommand::new(ai_command, source),
            };

            if self
                .queues
                .get_or_create_queue(object_id)
                .issue_command(cmd_with_target, frame)
            {
                any_ok = true;
            }
        }

        for &object_id in objects {
            execute_next_command_for_unit(&mut self.queues, object_id);
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
        let frame = self.current_frame;
        let source = self.cmd_source;

        for &object_id in objects {
            let cmd = UnitCommand::guard_position(position, guard_mode.as_i32(), source);
            if self
                .queues
                .get_or_create_queue(object_id)
                .issue_command(cmd, frame)
            {
                any_ok = true;
            }
        }

        for &object_id in objects {
            execute_next_command_for_unit(&mut self.queues, object_id);
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
        let frame = self.current_frame;
        let source = self.cmd_source;

        for &object_id in objects {
            let cmd = UnitCommand::guard_object(target, guard_mode.as_i32(), source);
            if self
                .queues
                .get_or_create_queue(object_id)
                .issue_command(cmd, frame)
            {
                any_ok = true;
            }
        }

        for &object_id in objects {
            execute_next_command_for_unit(&mut self.queues, object_id);
        }

        any_ok
    }

    fn queue_waypoint_for_object(&mut self, object_id: ObjectID, pos: Coord3D) {
        // Wave 425: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        let Some(ai) = OBJECT_REGISTRY
            .with_object(object_id, |obj_guard| obj_guard.get_ai_update_interface())
            .flatten()
        else {
            return;
        };
        ai.queue_waypoint(&pos);
    }

    fn execute_waypoint_queue_for_object(&mut self, object_id: ObjectID) {
        // Wave 425: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        let Some(ai) = OBJECT_REGISTRY
            .with_object(object_id, |obj_guard| obj_guard.get_ai_update_interface())
            .flatten()
        else {
            return;
        };
        ai.execute_waypoint_queue();
    }
}

/// Execute the next command in a unit's queue.
///
/// This dequeues the next PENDING command and calls ai_do_command on the unit's
/// AI state machine.
fn execute_next_command_for_unit(queues: &mut UnitCommandQueueManager, object_id: ObjectID) {
    let (cmd_type, cmd_source) = {
        let Some(queue) = queues.get_queue_mut(object_id) else {
            return;
        };
        let Some(cmd) = queue.process_next_command() else {
            return;
        };
        (cmd.cmd, cmd.cmd_source)
    };

    execute_ai_command_on_unit(queues, object_id, cmd_type, cmd_source);
}

/// Execute an AI command directly on a unit's AI state machine.
///
/// PARITY_NOTE: This matches the C++ path where GameLogicDispatch calls
/// obj->getAI()->aiDoCommand(parms) or obj->getAIUpdateInterface()->aiDoCommand(parms).
fn execute_ai_command_on_unit(
    queues: &UnitCommandQueueManager,
    object_id: ObjectID,
    ai_cmd: AiCommandType,
    cmd_source: CommandSourceType,
) {
    // Wave 425: empty dual-world → no-op.
    if dual_world_registry_unavailable() {
        return;
    }

    let Some(ai) = OBJECT_REGISTRY
        .with_object(object_id, |obj_guard| obj_guard.get_ai_update_interface())
        .flatten()
    else {
        return;
    };

    let mut params = AiCommandParams::new(ai_cmd, cmd_source);
    if let Some(active) = queues
        .get_queue(object_id)
        .and_then(|queue| queue.get_active_command())
    {
        params.pos = active.pos;
        params.obj = active.target_object;
        params.other_obj = active.other_object;
        params.int_value = active.int_value;
    }

    let _ = ai.execute_command(&params);
}

/// For each unit with an active command, advance when the AI state machine is idle.
fn advance_owned_unit_queues(queues: &mut UnitCommandQueueManager, current_frame: u32) {
    let object_ids: Vec<ObjectID> = queues.queues.keys().copied().collect();

    for object_id in object_ids {
        if should_advance_unit_queue(object_id) {
            advance_unit_queue(queues, object_id, current_frame);
        }
    }
}

fn should_advance_unit_queue(object_id: ObjectID) -> bool {
    // Wave 425: empty dual-world → false.
    if dual_world_registry_unavailable() {
        return false;
    }

    let Some(ai) = OBJECT_REGISTRY
        .with_object(object_id, |obj_guard| obj_guard.get_ai_update_interface())
        .flatten()
    else {
        return false;
    };

    ai.is_idle()
}

fn advance_unit_queue(
    queues: &mut UnitCommandQueueManager,
    object_id: ObjectID,
    _current_frame: u32,
) {
    let next_cmd = {
        let Some(queue) = queues.get_queue_mut(object_id) else {
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
        execute_ai_command_on_unit(queues, object_id, cmd_type, cmd_source);
    }
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
    fn test_unit_queues_do_not_cross_managers() {
        let mut first = AIManagerImpl::with_context(CommandSourceType::FromPlayer, 1);
        let mut second = AIManagerImpl::with_context(CommandSourceType::FromPlayer, 2);
        let destination = Coord3D::new(1.0, 2.0, 0.0);

        assert!(first.issue_waypoint_order(&[42], destination));
        assert!(second.queues.get_queue(42).is_none());

        second.clear_all_unit_command_queues();
        assert!(first.queues.get_queue(42).is_some());

        first.clear_unit_command_queue(42);
        assert!(first.queues.get_queue(42).is_none());
    }

    #[test]
    fn test_ai_manager_impl_default() {
        let mut mgr = AIManagerImpl::new();
        // No objects → returns false for move/attack/guard
        assert!(!mgr.issue_move_order(&[], Coord3D::new(0.0, 0.0, 0.0)));
        assert!(!mgr.issue_waypoint_order(&[], Coord3D::new(0.0, 0.0, 0.0)));
        assert!(!mgr.issue_attack_move_order(&[], Coord3D::new(0.0, 0.0, 0.0)));
        assert!(!mgr.issue_attack_order(&[], 0));
        assert!(mgr.issue_stop_order(&[]));
    }

    #[test]
    fn test_ai_manager_waypoint_order_appends_follow_path_command() {
        let mut mgr = AIManagerImpl::with_context(CommandSourceType::FromPlayer, 17);
        let destination = Coord3D::new(10.0, 20.0, 3.0);

        assert!(mgr.issue_waypoint_order(&[42], destination));

        let queue = mgr.queues.get_queue(42).expect("waypoint queue expected");
        let active = queue
            .get_active_command()
            .expect("waypoint command should become active");
        assert_eq!(active.cmd, AiCommandType::FollowPathAppend);
        assert_eq!(active.pos, destination);
        assert!(active.is_queued);
        assert_eq!(active.issued_frame, 17);
    }

    #[test]
    fn test_ai_manager_attack_move_order_queues_attack_move_command() {
        let mut mgr = AIManagerImpl::with_context(CommandSourceType::FromPlayer, 23);
        let destination = Coord3D::new(30.0, 40.0, 5.0);

        assert!(mgr.issue_attack_move_order(&[77], destination));

        let queue = mgr
            .queues
            .get_queue(77)
            .expect("attack-move queue expected");
        let active = queue
            .get_active_command()
            .expect("attack-move command should become active");
        assert_eq!(active.cmd, AiCommandType::AttackMoveToPosition);
        assert_eq!(active.pos, destination);
        assert!(!active.is_queued);
        assert_eq!(active.issued_frame, 23);
    }
}
