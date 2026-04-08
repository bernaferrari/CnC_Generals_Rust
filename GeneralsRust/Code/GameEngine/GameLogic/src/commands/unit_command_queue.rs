////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Per-Unit AI Command Queue - FIFO command queue for each unit's AIUpdate
//!
//! This module implements the per-unit command queue that mirrors the C++
//! AIUpdate command processing behavior. Each unit maintains a FIFO queue of
//! AI commands (AiCommandType + params) that are processed one at a time.
//!
//! ## C++ Reference
//!
//! In C++, each AIUpdate-derived class (JetAIUpdate, HackInternetAIUpdate, etc.)
//! processes commands via `AIUpdateInterface::aiDoCommand(parms)`. Some classes
//! (e.g., JetAIUpdate) implement a "pending command" mechanism:
//! - `m_mostRecentCommand` stores the command when it can't execute immediately
//! - `HAS_PENDING_COMMAND` flag marks that a command is waiting
//! - `friend_getPendingCommandType()` retrieves the pending type
//! - `friend_purgePendingCommand()` clears the pending flag after execution
//!
//! This module generalizes that pattern into a proper FIFO queue so every unit
//! type can queue commands, not just jets.
//!
//! ## Command Flow
//!
//! ```text
//! Player issues command (right-click)
//!   → CommandProcessor::DefaultCommandHandler
//!   → AIManager::issue_move_order / issue_attack_order / etc.
//!   → UnitCommandQueue::issue_command(object_id, params)
//!   → Unit stores command in its FIFO queue
//!   → Each frame: UnitCommandQueue::process_commands(object_id)
//!   → Dequeues next command, calls ai_do_command on the unit
//!   → Command completes → dequeue next, repeat
//! ```
//!
//! ## Command States
//!
//! - **Pending**: Command is in the queue, waiting to be executed
//! - **Active**: Command is currently being executed by the AI state machine
//! - **Completed**: Command finished successfully
//! - **Failed**: Command could not be executed (target dead, invalid, etc.)
//!
//! ## Queue Behavior (matches C++)
//!
//! - Stop command clears the entire queue (matches C++ aiIdle behavior)
//! - New non-queued commands replace the current command and clear the queue
//!   (matches C++ behavior where most commands interrupt the current action)
//! - Shift+click commands are appended to the queue (matches C++ waypoint queuing)
//! - Commands have a source (FromPlayer, FromAI, FromScript) for priority decisions

use std::collections::VecDeque;

use crate::ai::AiCommandType;
use crate::common::{CommandSourceType, Coord3D, ObjectID};

/// Maximum commands per unit queue - matches C++ practical limits.
/// C++ doesn't explicitly limit this but units typically have very short queues
/// (1-5 commands for waypoint paths).
pub const MAX_UNIT_COMMAND_QUEUE_SIZE: usize = 64;

/// Command states in the per-unit queue.
/// PARITY_NOTE: C++ doesn't have an explicit enum for this; states are implicit
/// in the AI state machine. We track them here for cleaner queue management.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitCommandState {
    /// Command is in the queue waiting to be executed.
    Pending,
    /// Command is currently being executed by the AI state machine.
    Active,
    /// Command completed successfully.
    Completed,
    /// Command failed (target dead, invalid, unreachable, etc.).
    Failed,
}

/// A single command in the per-unit queue.
///
/// PARITY_NOTE: This stores the same data as C++ AICommandParms (AiCommandParams in Rust),
/// but in a serializable form (ObjectID instead of Arc<Object>) for queue storage.
/// When the command is dequeued for execution, it's converted to full AiCommandParams.
#[derive(Debug, Clone)]
pub struct UnitCommand {
    /// AI command type (move, attack, guard, etc.)
    pub cmd: AiCommandType,
    /// Who issued this command
    pub cmd_source: CommandSourceType,
    /// Target position (for move/guard/attack-move)
    pub pos: Coord3D,
    /// Target object ID (for attack/enter/guard-object/repair)
    pub target_object: Option<ObjectID>,
    /// Secondary object ID (e.g., for return-to after attack)
    pub other_object: Option<ObjectID>,
    /// Integer parameter (e.g., guard mode, max shots)
    pub int_value: i32,
    /// Whether this command should be queued (Shift+click) or replace current
    pub is_queued: bool,
    /// Current state of this command
    pub state: UnitCommandState,
    /// Frame when this command was issued
    pub issued_frame: u32,
}

impl UnitCommand {
    /// Create a new unit command.
    pub fn new(cmd: AiCommandType, cmd_source: CommandSourceType) -> Self {
        Self {
            cmd,
            cmd_source,
            pos: Coord3D::new(0.0, 0.0, 0.0),
            target_object: None,
            other_object: None,
            int_value: 0,
            is_queued: false,
            state: UnitCommandState::Pending,
            issued_frame: 0,
        }
    }

    /// Create a move-to-position command.
    pub fn move_to_position(pos: Coord3D, cmd_source: CommandSourceType) -> Self {
        let mut cmd = Self::new(AiCommandType::MoveToPosition, cmd_source);
        cmd.pos = pos;
        cmd
    }

    /// Create an attack-object command.
    pub fn attack_object(target: ObjectID, cmd_source: CommandSourceType) -> Self {
        let mut cmd = Self::new(AiCommandType::AttackObject, cmd_source);
        cmd.target_object = Some(target);
        cmd
    }

    /// Create an attack-move-to-position command.
    pub fn attack_move_to_position(pos: Coord3D, cmd_source: CommandSourceType) -> Self {
        let mut cmd = Self::new(AiCommandType::AttackMoveToPosition, cmd_source);
        cmd.pos = pos;
        cmd
    }

    /// Create a guard-position command.
    pub fn guard_position(pos: Coord3D, guard_mode: i32, cmd_source: CommandSourceType) -> Self {
        let mut cmd = Self::new(AiCommandType::GuardPosition, cmd_source);
        cmd.pos = pos;
        cmd.int_value = guard_mode;
        cmd
    }

    /// Create a guard-object command.
    pub fn guard_object(target: ObjectID, guard_mode: i32, cmd_source: CommandSourceType) -> Self {
        let mut cmd = Self::new(AiCommandType::GuardObject, cmd_source);
        cmd.target_object = Some(target);
        cmd.int_value = guard_mode;
        cmd
    }

    /// Create an enter/garrison command.
    pub fn enter(target: ObjectID, cmd_source: CommandSourceType) -> Self {
        let mut cmd = Self::new(AiCommandType::Enter, cmd_source);
        cmd.target_object = Some(target);
        cmd
    }

    /// Create a repair command.
    pub fn repair(target: ObjectID, cmd_source: CommandSourceType) -> Self {
        let mut cmd = Self::new(AiCommandType::Repair, cmd_source);
        cmd.target_object = Some(target);
        cmd
    }

    /// Create a stop/idle command.
    pub fn stop(cmd_source: CommandSourceType) -> Self {
        Self::new(AiCommandType::Idle, cmd_source)
    }

    /// Create an evacuate command.
    pub fn evacuate(cmd_source: CommandSourceType) -> Self {
        Self::new(AiCommandType::Evacuate, cmd_source)
    }

    /// Create a hunt command.
    pub fn hunt(cmd_source: CommandSourceType) -> Self {
        Self::new(AiCommandType::Hunt, cmd_source)
    }

    /// Create a do-special-power command.
    pub fn special_power(cmd_source: CommandSourceType) -> Self {
        Self::new(AiCommandType::DoSpecialPower, cmd_source)
    }

    /// Create a get-repaired command.
    pub fn get_repaired(target: ObjectID, cmd_source: CommandSourceType) -> Self {
        let mut cmd = Self::new(AiCommandType::GetRepaired, cmd_source);
        cmd.target_object = Some(target);
        cmd
    }

    /// Create a resume-construction command.
    pub fn resume_construction(target: ObjectID, cmd_source: CommandSourceType) -> Self {
        let mut cmd = Self::new(AiCommandType::ResumeConstruction, cmd_source);
        cmd.target_object = Some(target);
        cmd
    }

    /// Create a dock command.
    pub fn dock(target: ObjectID, cmd_source: CommandSourceType) -> Self {
        let mut cmd = Self::new(AiCommandType::Dock, cmd_source);
        cmd.target_object = Some(target);
        cmd
    }

    /// Create a get-healed command.
    pub fn get_healed(target: ObjectID, cmd_source: CommandSourceType) -> Self {
        let mut cmd = Self::new(AiCommandType::GetHealed, cmd_source);
        cmd.target_object = Some(target);
        cmd
    }
}

/// Per-unit command queue (FIFO).
///
/// PARITY_NOTE: In C++, most AIUpdate classes don't have an explicit command queue.
/// Commands are issued directly via aiDoCommand which immediately transitions the state
/// machine. The exception is JetAIUpdate which stores one pending command.
/// We generalize this to a proper FIFO queue to support waypoint-style command queuing
/// (Shift+click), matching the behavior players expect from an RTS.
///
/// Queue behavior:
/// - `issue_command` adds a command. If `is_queued` is false (normal click), it replaces
///   the current command and clears the queue (matching C++ behavior).
/// - If `is_queued` is true (Shift+click), the command is appended to the queue.
/// - `process_command` returns the next command to execute (PENDING → ACTIVE).
/// - `complete_current_command` marks the active command as COMPLETED and advances.
/// - `fail_current_command` marks the active command as FAILED and advances.
/// - `clear_commands` removes all commands (matches C++ aiIdle behavior).
pub struct UnitCommandQueue {
    /// FIFO queue of commands.
    queue: VecDeque<UnitCommand>,
}

impl UnitCommandQueue {
    /// Create a new empty command queue.
    pub fn new() -> Self {
        Self {
            queue: VecDeque::with_capacity(8),
        }
    }

    /// Issue a command to this unit.
    ///
    /// If the command is not queued (normal click), it replaces the current command
    /// and clears the queue, matching C++ behavior where commands interrupt.
    /// If the command is queued (Shift+click), it's appended to the end.
    ///
    /// Returns true if the command was accepted.
    pub fn issue_command(&mut self, mut command: UnitCommand, current_frame: u32) -> bool {
        if self.queue.len() >= MAX_UNIT_COMMAND_QUEUE_SIZE {
            return false;
        }

        command.issued_frame = current_frame;

        if command.is_queued {
            // Shift+click: append to queue
            self.queue.push_back(command);
        } else {
            // Normal click: replace current and clear queue (matches C++ behavior)
            self.queue.clear();
            self.queue.push_back(command);
        }

        true
    }

    /// Get the next pending command to execute.
    ///
    /// Returns the first PENDING command and marks it as ACTIVE.
    /// Returns None if no pending commands.
    pub fn process_next_command(&mut self) -> Option<&UnitCommand> {
        // Find first pending command
        for cmd in &mut self.queue {
            if cmd.state == UnitCommandState::Pending {
                cmd.state = UnitCommandState::Active;
                return Some(cmd);
            }
        }
        None
    }

    /// Get a mutable reference to the active command.
    pub fn get_active_command(&self) -> Option<&UnitCommand> {
        self.queue
            .iter()
            .find(|cmd| cmd.state == UnitCommandState::Active)
    }

    /// Mark the current active command as completed and remove it.
    pub fn complete_current_command(&mut self) {
        if let Some(idx) = self
            .queue
            .iter()
            .position(|cmd| cmd.state == UnitCommandState::Active)
        {
            self.queue[idx].state = UnitCommandState::Completed;
            while self
                .front()
                .map(|cmd| cmd.state == UnitCommandState::Completed)
                .unwrap_or(false)
            {
                self.queue.pop_front();
            }
        }
    }

    /// Mark the current active command as failed.
    ///
    /// PARITY_NOTE: In C++, when a command fails (e.g., target dead), the unit
    /// typically goes to idle. We mark it failed and let the queue advance.
    pub fn fail_current_command(&mut self) {
        if let Some(idx) = self
            .queue
            .iter()
            .position(|cmd| cmd.state == UnitCommandState::Active)
        {
            self.queue[idx].state = UnitCommandState::Failed;
            while self
                .front()
                .map(|cmd| {
                    cmd.state == UnitCommandState::Completed
                        || cmd.state == UnitCommandState::Failed
                })
                .unwrap_or(false)
            {
                self.queue.pop_front();
            }
        }
    }

    /// Clear all commands (matches C++ aiIdle behavior).
    ///
    /// Called when the player issues a Stop command or when the unit dies.
    pub fn clear_commands(&mut self) {
        self.queue.clear();
    }

    /// Check if the queue has any pending commands.
    pub fn has_pending_commands(&self) -> bool {
        self.queue
            .iter()
            .any(|cmd| cmd.state == UnitCommandState::Pending)
    }

    /// Check if the queue has an active command.
    pub fn has_active_command(&self) -> bool {
        self.queue
            .iter()
            .any(|cmd| cmd.state == UnitCommandState::Active)
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Get the number of commands in the queue.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Get the type of the current active command.
    ///
    /// PARITY_NOTE: Matches C++ JetAIUpdate::friend_getPendingCommandType().
    pub fn get_active_command_type(&self) -> Option<AiCommandType> {
        self.get_active_command().map(|cmd| cmd.cmd)
    }

    /// Get the type of the first pending command.
    pub fn get_pending_command_type(&self) -> Option<AiCommandType> {
        self.queue
            .iter()
            .find(|cmd| cmd.state == UnitCommandState::Pending)
            .map(|cmd| cmd.cmd)
    }

    /// Get the front command (first in queue).
    fn front(&self) -> Option<&UnitCommand> {
        self.queue.front()
    }

    /// Get queue statistics for debugging.
    pub fn get_stats(&self) -> UnitCommandQueueStats {
        let mut stats = UnitCommandQueueStats::default();
        for cmd in &self.queue {
            match cmd.state {
                UnitCommandState::Pending => stats.pending_count += 1,
                UnitCommandState::Active => stats.active_count += 1,
                UnitCommandState::Completed => stats.completed_count += 1,
                UnitCommandState::Failed => stats.failed_count += 1,
            }
        }
        stats.total_count = self.queue.len();
        stats
    }
}

impl Default for UnitCommandQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Command queue statistics.
#[derive(Debug, Clone, Default)]
pub struct UnitCommandQueueStats {
    pub total_count: usize,
    pub pending_count: usize,
    pub active_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_command_replace() {
        let mut queue = UnitCommandQueue::new();
        let cmd1 = UnitCommand::move_to_position(
            Coord3D::new(100.0, 200.0, 0.0),
            CommandSourceType::FromPlayer,
        );
        let cmd2 = UnitCommand::attack_object(42, CommandSourceType::FromPlayer);

        // First command
        assert!(queue.issue_command(cmd1, 0));
        assert_eq!(queue.len(), 1);

        // Second command replaces (not queued)
        assert!(queue.issue_command(cmd2, 1));
        assert_eq!(queue.len(), 1);
        assert_eq!(
            queue.get_pending_command_type(),
            Some(AiCommandType::AttackObject)
        );
    }

    #[test]
    fn test_issue_command_queued() {
        let mut queue = UnitCommandQueue::new();

        let mut cmd1 = UnitCommand::move_to_position(
            Coord3D::new(100.0, 200.0, 0.0),
            CommandSourceType::FromPlayer,
        );
        cmd1.is_queued = false;

        let mut cmd2 = UnitCommand::move_to_position(
            Coord3D::new(200.0, 300.0, 0.0),
            CommandSourceType::FromPlayer,
        );
        cmd2.is_queued = true; // Shift+click

        let mut cmd3 = UnitCommand::move_to_position(
            Coord3D::new(300.0, 400.0, 0.0),
            CommandSourceType::FromPlayer,
        );
        cmd3.is_queued = true;

        assert!(queue.issue_command(cmd1, 0));
        assert!(queue.issue_command(cmd2, 1));
        assert!(queue.issue_command(cmd3, 2));
        assert_eq!(queue.len(), 3);
    }

    #[test]
    fn test_process_and_complete() {
        let mut queue = UnitCommandQueue::new();

        let mut cmd1 = UnitCommand::move_to_position(
            Coord3D::new(100.0, 200.0, 0.0),
            CommandSourceType::FromPlayer,
        );
        cmd1.is_queued = true;

        let mut cmd2 = UnitCommand::attack_object(42, CommandSourceType::FromPlayer);
        cmd2.is_queued = true;

        queue.issue_command(cmd1, 0);
        queue.issue_command(cmd2, 1);

        // Process first command
        assert!(queue.process_next_command().is_some());
        assert!(queue.has_active_command());
        assert!(queue.has_pending_commands());

        // Complete it
        queue.complete_current_command();
        assert!(!queue.has_active_command());
        assert!(queue.has_pending_commands());
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn test_stop_clears_queue() {
        let mut queue = UnitCommandQueue::new();

        let mut cmd1 = UnitCommand::move_to_position(
            Coord3D::new(100.0, 200.0, 0.0),
            CommandSourceType::FromPlayer,
        );
        cmd1.is_queued = true;

        let mut cmd2 = UnitCommand::move_to_position(
            Coord3D::new(200.0, 300.0, 0.0),
            CommandSourceType::FromPlayer,
        );
        cmd2.is_queued = true;

        queue.issue_command(cmd1, 0);
        queue.issue_command(cmd2, 1);
        assert_eq!(queue.len(), 2);

        // Stop command replaces and clears
        let stop = UnitCommand::stop(CommandSourceType::FromPlayer);
        queue.issue_command(stop, 2);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.get_pending_command_type(), Some(AiCommandType::Idle));
    }

    #[test]
    fn test_command_types() {
        let move_cmd = UnitCommand::move_to_position(
            Coord3D::new(10.0, 20.0, 0.0),
            CommandSourceType::FromPlayer,
        );
        assert_eq!(move_cmd.cmd, AiCommandType::MoveToPosition);
        assert_eq!(move_cmd.pos.x, 10.0);

        let attack_cmd = UnitCommand::attack_object(99, CommandSourceType::FromAI);
        assert_eq!(attack_cmd.cmd, AiCommandType::AttackObject);
        assert_eq!(attack_cmd.target_object, Some(99));

        let guard_cmd = UnitCommand::guard_position(
            Coord3D::new(50.0, 60.0, 0.0),
            1,
            CommandSourceType::FromPlayer,
        );
        assert_eq!(guard_cmd.cmd, AiCommandType::GuardPosition);
        assert_eq!(guard_cmd.int_value, 1);

        let enter_cmd = UnitCommand::enter(77, CommandSourceType::FromPlayer);
        assert_eq!(enter_cmd.cmd, AiCommandType::Enter);

        let repair_cmd = UnitCommand::repair(88, CommandSourceType::FromPlayer);
        assert_eq!(repair_cmd.cmd, AiCommandType::Repair);

        let evacuate_cmd = UnitCommand::evacuate(CommandSourceType::FromPlayer);
        assert_eq!(evacuate_cmd.cmd, AiCommandType::Evacuate);

        let hunt_cmd = UnitCommand::hunt(CommandSourceType::FromPlayer);
        assert_eq!(hunt_cmd.cmd, AiCommandType::Hunt);

        let attack_move_cmd = UnitCommand::attack_move_to_position(
            Coord3D::new(100.0, 100.0, 0.0),
            CommandSourceType::FromPlayer,
        );
        assert_eq!(attack_move_cmd.cmd, AiCommandType::AttackMoveToPosition);

        let dock_cmd = UnitCommand::dock(55, CommandSourceType::FromPlayer);
        assert_eq!(dock_cmd.cmd, AiCommandType::Dock);

        let get_repaired_cmd = UnitCommand::get_repaired(55, CommandSourceType::FromPlayer);
        assert_eq!(get_repaired_cmd.cmd, AiCommandType::GetRepaired);

        let resume_cmd = UnitCommand::resume_construction(55, CommandSourceType::FromPlayer);
        assert_eq!(resume_cmd.cmd, AiCommandType::ResumeConstruction);

        let get_healed_cmd = UnitCommand::get_healed(55, CommandSourceType::FromPlayer);
        assert_eq!(get_healed_cmd.cmd, AiCommandType::GetHealed);
    }

    #[test]
    fn test_max_queue_size() {
        let mut queue = UnitCommandQueue::new();
        for i in 0..MAX_UNIT_COMMAND_QUEUE_SIZE {
            let mut cmd = UnitCommand::move_to_position(
                Coord3D::new(i as f32, 0.0, 0.0),
                CommandSourceType::FromPlayer,
            );
            cmd.is_queued = true;
            assert!(queue.issue_command(cmd, i as u32));
        }
        // One more should fail
        let mut cmd = UnitCommand::move_to_position(
            Coord3D::new(999.0, 0.0, 0.0),
            CommandSourceType::FromPlayer,
        );
        cmd.is_queued = true;
        assert!(!queue.issue_command(cmd, MAX_UNIT_COMMAND_QUEUE_SIZE as u32));
    }
}
