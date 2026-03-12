//! GameLogic Dispatch System - Command Queue and Message Processing
//!
//! This module implements the command dispatch system that routes player
//! commands and system messages to the appropriate handlers. It manages
//! command queues, priorities, and execution timing.
//!
//! ## C++ Reference
//!
//! This ports `GameLogicDispatch.cpp` from the original C++ codebase.
//!
//! ## Architecture
//!
//! The dispatch system maintains separate queues for:
//! - Player commands (movement, attack, build, etc.)
//! - System commands (pause, save, etc.)
//! - Network messages
//! - AI commands
//!
//! Commands are prioritized and executed in frame order to maintain
//! determinism across multiplayer sessions.

use crate::commands;
use crate::common::{AsciiString, Coord3D, Int, ObjectID, UnsignedInt};
use log::{debug, trace, warn};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

/// Global dispatcher singleton (matches the legacy `TheGameLogicDispatch`)
static GAME_LOGIC_DISPATCH: OnceLock<Mutex<GameLogicDispatch>> = OnceLock::new();

/// Retrieve the global dispatcher if it has been initialized
pub fn get_dispatch() -> Option<&'static Mutex<GameLogicDispatch>> {
    GAME_LOGIC_DISPATCH.get()
}

/// Initialize the global dispatcher (idempotent)
pub fn init_dispatch(max_players: Int) -> &'static Mutex<GameLogicDispatch> {
    GAME_LOGIC_DISPATCH.get_or_init(|| Mutex::new(GameLogicDispatch::new(max_players)))
}

/// Command types supported by the dispatch system
///
/// ## C++ Reference: Command categorization in GameLogicDispatch
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandKind {
    /// System-level command (pause, save, quit, etc.)
    System,
    /// Player-issued command
    Player,
    /// RTS gameplay command (move, attack, build)
    Rts,
    /// AI-generated command
    AI,
    /// Network synchronization command
    Network,
}

/// Command priority levels
///
/// Higher priority commands execute first within a frame.
/// This ensures critical commands (like pause) execute before
/// gameplay commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CommandPriority {
    /// Low priority - execute after normal commands
    Low = 0,
    /// Normal priority - default for most gameplay commands
    Normal = 1,
    /// High priority - execute before normal commands (system commands)
    High = 2,
    /// Critical priority - execute immediately (pause, disconnect, etc.)
    Critical = 3,
}

impl Default for CommandPriority {
    fn default() -> Self {
        CommandPriority::Normal
    }
}

/// Simplified command payload used by the dispatch system
///
/// ## C++ Reference: Command class in GameLogicDispatch.h
#[derive(Debug, Clone)]
pub struct Command {
    /// Player who issued the command (-1 for system commands)
    player_index: Int,
    /// Command category
    kind: CommandKind,
    /// Frame number when command should execute
    frame: UnsignedInt,
    /// Optional command payload data
    payload: Option<Vec<u8>>,
}

impl Command {
    /// Create a new player command
    pub fn new(player_index: Int, kind: CommandKind, frame: UnsignedInt) -> Self {
        Self {
            player_index,
            kind,
            frame,
            payload: None,
        }
    }

    /// Create a new system command
    pub fn for_system(kind: CommandKind, frame: UnsignedInt) -> Self {
        Self::new(-1, kind, frame)
    }

    /// Set the player index
    pub fn set_player_index(&mut self, index: Int) {
        self.player_index = index;
    }

    /// Set the execution frame
    pub fn set_execution_frame(&mut self, frame: UnsignedInt) {
        self.frame = frame;
    }

    /// Attach payload data
    pub fn set_payload(&mut self, payload: Vec<u8>) {
        self.payload = Some(payload);
    }

    /// Get command kind
    pub fn get_kind(&self) -> CommandKind {
        self.kind
    }

    /// Get player index
    pub fn get_player_index(&self) -> Int {
        self.player_index
    }

    /// Get execution frame
    pub fn get_frame(&self) -> UnsignedInt {
        self.frame
    }
}

/// Queued command with priority metadata
#[derive(Debug, Clone)]
struct QueuedCommand {
    command: Command,
    priority: CommandPriority,
    /// Timestamp when command was queued (for debugging)
    queued_at: Instant,
}

impl QueuedCommand {
    fn new(command: Command, priority: CommandPriority) -> Self {
        Self {
            command,
            priority,
            queued_at: Instant::now(),
        }
    }
}

/// RTS command payload for gameplay commands
///
/// ## C++ Reference: RtsCommand in GameLogicDispatch.h
#[derive(Debug, Clone, Default)]
pub struct RtsCommand {
    /// Command type identifier
    pub command_type: Option<String>,
    /// Target object ID (for attack, follow, etc.)
    pub target_id: Option<ObjectID>,
    /// Target position (for move, build, etc.)
    pub target_position: Option<(f32, f32, f32)>,
    /// Selected object IDs
    pub selected_objects: Vec<ObjectID>,
    /// Additional parameters
    pub parameters: Vec<String>,
    /// Raw payload data
    payload: Option<AsciiString>,
}

impl RtsCommand {
    /// Create a new RTS command
    pub fn new(payload: Option<AsciiString>) -> Self {
        Self {
            command_type: None,
            target_id: None,
            target_position: None,
            selected_objects: Vec::new(),
            parameters: Vec::new(),
            payload,
        }
    }

    /// Create a move command
    pub fn move_to(objects: Vec<ObjectID>, position: (f32, f32, f32)) -> Self {
        Self {
            command_type: Some("MOVE".to_string()),
            target_id: None,
            target_position: Some(position),
            selected_objects: objects,
            parameters: Vec::new(),
            payload: None,
        }
    }

    /// Create an attack command
    pub fn attack(attackers: Vec<ObjectID>, target: ObjectID) -> Self {
        Self {
            command_type: Some("ATTACK".to_string()),
            target_id: Some(target),
            target_position: None,
            selected_objects: attackers,
            parameters: Vec::new(),
            payload: None,
        }
    }

    /// Create a build command
    pub fn build(builder: ObjectID, structure_type: String, position: (f32, f32)) -> Self {
        Self {
            command_type: Some("BUILD".to_string()),
            target_id: None,
            target_position: Some((position.0, position.1, 0.0)),
            selected_objects: vec![builder],
            parameters: vec![structure_type],
            payload: None,
        }
    }
}

/// Minimal execution context for command processing
///
/// ## C++ Reference: CommandExecutionContext in GameLogicDispatch
#[derive(Debug)]
pub struct CommandExecutionContext {
    /// Current simulation frame
    pub current_frame: UnsignedInt,
    /// Player ID executing the command
    pub player_id: Int,
    /// When execution started
    pub execution_start_time: Instant,
    /// Whether this is a network command
    pub is_network_command: bool,
    /// Whether this is a replay command
    pub is_replay_command: bool,
    /// Commands processed this frame
    processed: Vec<Command>,
}

impl CommandExecutionContext {
    /// Create a new execution context
    pub fn new(frame: UnsignedInt, is_replay: bool) -> Self {
        Self {
            current_frame: frame,
            player_id: -1,
            execution_start_time: Instant::now(),
            is_network_command: false,
            is_replay_command: is_replay,
            processed: Vec::new(),
        }
    }

    /// Get the number of commands processed
    pub fn processed_count(&self) -> usize {
        self.processed.len()
    }

    /// Record a processed command
    pub fn record_command(&mut self, command: Command) {
        self.processed.push(command);
    }
}

/// Command dispatcher responsible for queuing and executing player/system commands
///
/// ## C++ Reference: GameLogicDispatch class (GameLogicDispatch.cpp)
///
/// The dispatcher maintains separate queues for different command priorities
/// and ensures deterministic execution order for multiplayer synchronization.
#[derive(Debug)]
pub struct GameLogicDispatch {
    /// Maximum number of players
    max_players: Int,
    /// Command queue (sorted by priority and frame)
    queue: VecDeque<QueuedCommand>,
    /// Current simulation frame
    current_frame: UnsignedInt,
    /// Whether we're in replay mode
    is_replay: bool,
    /// Recently processed commands (for debugging/verification)
    processed_batches: Vec<Command>,
    /// Per-player command statistics
    player_stats: Vec<PlayerCommandStats>,
}

/// Statistics for command processing per player
#[derive(Debug, Clone, Default)]
pub struct PlayerCommandStats {
    commands_this_frame: usize,
    commands_total: usize,
    last_command_frame: UnsignedInt,
}

impl GameLogicDispatch {
    /// Create a new dispatcher instance
    ///
    /// ## C++ Reference: GameLogicDispatch constructor
    pub fn new(max_players: Int) -> Self {
        let mut player_stats = Vec::new();
        for _ in 0..max_players {
            player_stats.push(PlayerCommandStats::default());
        }

        Self {
            max_players,
            queue: VecDeque::new(),
            current_frame: 0,
            is_replay: false,
            processed_batches: Vec::new(),
            player_stats,
        }
    }

    /// Reset internal state and clear any pending commands
    ///
    /// ## C++ Reference: GameLogicDispatch::reset()
    pub fn reset(&mut self) {
        debug!("GameLogicDispatch::reset() - Clearing command queues");
        self.current_frame = 0;
        self.queue.clear();
        self.processed_batches.clear();
        for stats in &mut self.player_stats {
            *stats = PlayerCommandStats::default();
        }
    }

    /// Update the dispatcher for the current frame
    ///
    /// ## C++ Reference: GameLogicDispatch::update()
    ///
    /// This processes all commands scheduled for the current frame,
    /// executing them in priority order.
    pub fn update(&mut self, frame: UnsignedInt) -> Result<(), AsciiString> {
        self.current_frame = frame;

        trace!(
            "GameLogicDispatch::update(frame={}) - {} commands queued",
            frame,
            self.queue.len()
        );

        // Reset per-frame statistics
        for stats in &mut self.player_stats {
            stats.commands_this_frame = 0;
        }

        // Create execution context
        let mut context = CommandExecutionContext::new(frame, self.is_replay);

        // Sort queue by priority and frame (highest priority first)
        self.sort_queue_by_priority();

        // Process commands for this frame
        let mut commands_to_process = Vec::new();
        while let Some(queued) = self.queue.front() {
            // Only process commands scheduled for this frame or earlier
            if queued.command.frame > frame {
                break;
            }

            // Remove from queue
            let queued = self.queue.pop_front().unwrap();
            commands_to_process.push(queued);
        }

        // Execute commands in priority order
        for queued in commands_to_process {
            context.player_id = queued.command.player_index;

            // Update statistics
            if queued.command.player_index >= 0
                && (queued.command.player_index as usize) < self.player_stats.len()
            {
                let stats = &mut self.player_stats[queued.command.player_index as usize];
                stats.commands_this_frame += 1;
                stats.commands_total += 1;
                stats.last_command_frame = frame;
            }

            // Execute command (temporarily borrow as mutable)
            // Note: We need to restructure this to avoid borrowing issues
            let player_index = queued.command.player_index;
            let kind = queued.command.kind;

            if let Err(e) = self.execute_command(&queued.command, &mut context) {
                warn!(
                    "Command execution failed (frame={}, player={}, kind={:?}): {}",
                    frame, player_index, kind, e
                );
                // Continue processing other commands
            }

            context.record_command(queued.command.clone());
        }

        // Store processed commands for verification
        let processed_count = context.processed_count();
        let processed = std::mem::take(&mut context.processed);
        self.processed_batches.extend(processed.into_iter());

        trace!(
            "GameLogicDispatch::update(frame={}) complete - {} commands executed",
            frame,
            processed_count
        );

        Ok(())
    }

    /// Execute a single command
    fn execute_command(
        &mut self,
        command: &Command,
        context: &mut CommandExecutionContext,
    ) -> Result<(), AsciiString> {
        trace!(
            "Executing command: kind={:?}, player={}, frame={}",
            command.kind,
            command.player_index,
            command.frame
        );

        match command.kind {
            CommandKind::System => self.execute_system_command(command, context),
            CommandKind::Player => self.execute_player_command(command, context),
            CommandKind::Rts => self.execute_rts_command(command, context),
            CommandKind::AI => self.execute_ai_command(command, context),
            CommandKind::Network => self.execute_network_command(command, context),
        }
    }

    /// Execute a system command
    fn execute_system_command(
        &self,
        _command: &Command,
        _context: &mut CommandExecutionContext,
    ) -> Result<(), AsciiString> {
        // Stub: In full implementation, handle pause, save, quit, etc.
        Ok(())
    }

    /// Execute a player command
    fn execute_player_command(
        &self,
        _command: &Command,
        _context: &mut CommandExecutionContext,
    ) -> Result<(), AsciiString> {
        // Stub: In full implementation, handle player-specific commands
        Ok(())
    }

    /// Execute an RTS gameplay command
    fn execute_rts_command(
        &self,
        command: &Command,
        _context: &mut CommandExecutionContext,
    ) -> Result<(), AsciiString> {
        // NOTE: This dispatch command currently carries only kind/frame/payload metadata.
        // RTS command argument execution remains owned by the command processor path.
        if command.payload.is_none() {
            trace!("RTS dispatch command has no payload; skipping");
        }

        Ok(())
    }

    /// Execute an AI command
    fn execute_ai_command(
        &self,
        _command: &Command,
        _context: &mut CommandExecutionContext,
    ) -> Result<(), AsciiString> {
        // Stub: In full implementation, handle AI-generated commands
        Ok(())
    }

    /// Execute a network command
    ///
    /// ## C++ Reference
    ///
    /// Matches GameLogic.cpp line 3608 - network command execution
    ///
    /// ## Implementation Details
    ///
    /// Network commands arrive from the GameNetwork layer and must be:
    /// 1. Validated for frame synchronization
    /// 2. Translated from network format to game logic format
    /// 3. Executed through the command processor
    /// 4. Statistics tracked for debugging
    ///
    /// ## Arguments
    ///
    /// * `command` - Network command to execute
    /// * `context` - Execution context with frame information
    ///
    /// ## Returns
    ///
    /// Returns `Ok(())` on success, `Err(AsciiString)` on failure
    fn execute_network_command(
        &mut self,
        command: &Command,
        context: &mut CommandExecutionContext,
    ) -> Result<(), AsciiString> {
        trace!(
            "Executing network command: player={}, frame={}",
            command.player_index,
            command.frame
        );

        // Validate frame synchronization
        // Network commands must execute at the exact frame specified
        if command.frame != context.current_frame {
            warn!(
                "Network command frame mismatch: command.frame={}, current_frame={}",
                command.frame, context.current_frame
            );
            return Err(AsciiString::from("Frame synchronization error"));
        }

        // Mark as network command in context
        context.is_network_command = true;

        // Validate player index
        if command.player_index < 0 || command.player_index >= self.max_players {
            warn!(
                "Invalid player index in network command: {}",
                command.player_index
            );
            return Err(AsciiString::from("Invalid player index"));
        }

        // Extract and validate payload
        if command.payload.is_none() {
            debug!("Network command has no payload, treating as control command");
            // Control commands (pause, sync, etc.) have no payload
            return Ok(());
        }

        // In a full implementation, this would:
        // 1. Deserialize the payload into a GameMessage
        // 2. Validate the command checksum
        // 3. Pass to the command processor for execution
        // 4. Update game state accordingly
        //
        // For now, we validate the structure and accept it
        debug!(
            "Network command validated and ready for execution: player={}, frame={}",
            command.player_index, command.frame
        );

        // Track statistics
        if command.player_index >= 0 && (command.player_index as usize) < self.player_stats.len() {
            let stats = &mut self.player_stats[command.player_index as usize];
            stats.commands_this_frame += 1;
            stats.commands_total += 1;
        }

        Ok(())
    }

    /// Sort queue by priority (highest first) and frame (earliest first)
    fn sort_queue_by_priority(&mut self) {
        // Convert to Vec, sort, and convert back to VecDeque
        let mut commands: Vec<_> = self.queue.drain(..).collect();
        commands.sort_by(|a, b| {
            // First by priority (highest first)
            b.priority
                .cmp(&a.priority)
                // Then by frame (earliest first)
                .then_with(|| a.command.frame.cmp(&b.command.frame))
        });
        self.queue = commands.into();
    }

    /// Register a player queue with the dispatcher
    ///
    /// ## C++ Reference: GameLogicDispatch::registerPlayer()
    pub fn register_player(&self, player_id: Int) -> Result<(), AsciiString> {
        if !(0..self.max_players).contains(&player_id) {
            return Err(AsciiString::from("Invalid player identifier"));
        }
        debug!("Registered player {} with dispatch system", player_id);
        Ok(())
    }

    /// Hook used when new objects enter the world
    ///
    /// ## C++ Reference: GameLogicDispatch::registerObject()
    pub fn register_object(&self, _object_id: ObjectID) -> Result<(), AsciiString> {
        // Selection/command systems use the registry object lookup, so no direct registration is needed.
        Ok(())
    }

    /// Install default bridges (command/selection/formation managers)
    pub fn ensure_default_managers(&mut self) {
        if let Err(err) = commands::initialize_command_system(self.max_players) {
            warn!("Failed to initialize command system: {}", err);
        }
    }

    /// Queue a command for a specific player
    ///
    /// ## C++ Reference: GameLogicDispatch::queuePlayerCommand()
    pub fn queue_player_command(
        &mut self,
        player_id: Int,
        mut command: Command,
    ) -> Result<(), AsciiString> {
        command.set_player_index(player_id);
        command.set_execution_frame(self.current_frame);

        let queued = QueuedCommand::new(command, CommandPriority::default());
        self.queue.push_back(queued);

        trace!("Queued player command for player {}", player_id);
        Ok(())
    }

    /// Convenience helper for queuing commands by kind
    pub fn queue_player_command_kind(
        &mut self,
        player_id: Int,
        kind: CommandKind,
    ) -> Result<(), AsciiString> {
        let command = Command::new(player_id, kind, self.current_frame);
        self.queue_player_command(player_id, command)
    }

    /// Queue a pre-built RTS command
    ///
    /// ## C++ Reference: GameLogicDispatch::queueRtsCommand()
    pub fn queue_rts_command(
        &mut self,
        player_id: Int,
        _rts_command: RtsCommand,
    ) -> Result<(), AsciiString> {
        let command = Command::new(player_id, CommandKind::Rts, self.current_frame);
        // In full implementation: serialize rts_command into command payload
        self.queue_player_command(player_id, command)
    }

    /// Queue a system-level command (player independent)
    ///
    /// ## C++ Reference: GameLogicDispatch::queueSystemCommand()
    pub fn queue_system_command(&mut self, mut command: Command) -> Result<(), AsciiString> {
        command.set_player_index(-1);
        command.set_execution_frame(self.current_frame);

        let queued = QueuedCommand::new(command, CommandPriority::High);
        self.queue.push_back(queued);

        trace!("Queued system command");
        Ok(())
    }

    /// Cancel all commands queued for a player
    ///
    /// ## C++ Reference: GameLogicDispatch::cancelPlayerCommands()
    pub fn cancel_player_commands(&mut self, player_id: Int) -> Result<(), AsciiString> {
        let count_before = self.queue.len();
        self.queue
            .retain(|queued| queued.command.player_index != player_id);
        let count_after = self.queue.len();

        debug!(
            "Cancelled {} commands for player {}",
            count_before - count_after,
            player_id
        );
        Ok(())
    }

    /// Access recent command statistics for debugging
    pub fn processed_commands(&self) -> &[Command] {
        &self.processed_batches
    }

    /// Get command statistics for a player
    pub fn get_player_stats(&self, player_id: Int) -> Option<&PlayerCommandStats> {
        if player_id >= 0 && (player_id as usize) < self.player_stats.len() {
            Some(&self.player_stats[player_id as usize])
        } else {
            None
        }
    }

    /// Record a batch summary from the AI subsystem
    pub fn record_command_batch(&mut self, processed: usize, backlog: usize) {
        if processed == 0 && backlog == 0 {
            return;
        }
        trace!("AI batch: processed={}, backlog={}", processed, backlog);
        self.processed_batches
            .push(Command::for_system(CommandKind::System, self.current_frame));
    }

    /// Get current frame number
    pub fn get_current_frame(&self) -> UnsignedInt {
        self.current_frame
    }

    /// Get queue size
    pub fn get_queue_size(&self) -> usize {
        self.queue.len()
    }

    /// Set replay mode
    pub fn set_replay_mode(&mut self, is_replay: bool) {
        self.is_replay = is_replay;
        debug!("Replay mode: {}", is_replay);
    }

    /// Queue a network command for execution at the correct frame
    ///
    /// ## C++ Reference
    ///
    /// Matches GameLogic.cpp network command queuing
    ///
    /// ## Implementation Details
    ///
    /// This method validates frame synchronization before queuing:
    /// 1. Check execution_frame is valid (not in past)
    /// 2. Check execution_frame is not too far in future (< MAX_FRAMES_AHEAD)
    /// 3. Queue command with Network priority
    ///
    /// ## Arguments
    ///
    /// * `command` - Network command to queue
    /// * `execution_frame` - Frame at which to execute
    ///
    /// ## Returns
    ///
    /// Returns `Ok(())` on success, `Err(AsciiString)` on failure
    pub fn queue_network_command(
        &mut self,
        mut command: Command,
        execution_frame: UnsignedInt,
    ) -> Result<(), AsciiString> {
        // Validate execution frame is not in the past
        if execution_frame < self.current_frame {
            warn!(
                "Network command execution frame {} is in past (current: {})",
                execution_frame, self.current_frame
            );
            return Err(AsciiString::from("Execution frame is in the past"));
        }

        // Validate execution frame is not too far in the future
        // Matches C++ MAX_FRAMES_AHEAD = 300
        const MAX_FRAMES_AHEAD: UnsignedInt = 300;
        if execution_frame > self.current_frame + MAX_FRAMES_AHEAD {
            warn!(
                "Network command execution frame {} is too far in future (current: {}, max: {})",
                execution_frame,
                self.current_frame,
                self.current_frame + MAX_FRAMES_AHEAD
            );
            return Err(AsciiString::from(
                "Execution frame exceeds maximum lookahead",
            ));
        }

        // Set the execution frame
        command.set_execution_frame(execution_frame);

        // Queue with high priority (network commands are critical for sync)
        let queued = QueuedCommand::new(command, CommandPriority::High);
        self.queue.push_back(queued);

        trace!(
            "Queued network command for frame {} (current: {})",
            execution_frame,
            self.current_frame
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispatch_creation() {
        let dispatch = GameLogicDispatch::new(8);
        assert_eq!(dispatch.max_players, 8);
        assert_eq!(dispatch.current_frame, 0);
        assert_eq!(dispatch.queue.len(), 0);
    }

    #[test]
    fn test_command_priority_ordering() {
        assert!(CommandPriority::Critical > CommandPriority::High);
        assert!(CommandPriority::High > CommandPriority::Normal);
        assert!(CommandPriority::Normal > CommandPriority::Low);
    }

    #[test]
    fn test_queue_player_command() {
        let mut dispatch = GameLogicDispatch::new(8);
        let command = Command::new(0, CommandKind::Player, 0);

        assert!(dispatch.queue_player_command(0, command).is_ok());
        assert_eq!(dispatch.queue.len(), 1);
    }

    #[test]
    fn test_cancel_player_commands() {
        let mut dispatch = GameLogicDispatch::new(8);

        // Queue commands for multiple players
        for player_id in 0..3 {
            let command = Command::new(player_id, CommandKind::Player, 0);
            dispatch.queue_player_command(player_id, command).unwrap();
        }

        assert_eq!(dispatch.queue.len(), 3);

        // Cancel player 1's commands
        dispatch.cancel_player_commands(1).unwrap();
        assert_eq!(dispatch.queue.len(), 2);

        // Verify only player 0 and 2 remain
        assert!(dispatch.queue.iter().all(|q| q.command.player_index != 1));
    }

    #[test]
    fn test_priority_sorting() {
        let mut dispatch = GameLogicDispatch::new(8);

        // Queue commands with different priorities
        let mut cmd_low = Command::new(0, CommandKind::Player, 0);
        let mut cmd_normal = Command::new(1, CommandKind::Player, 0);
        let mut cmd_high = Command::new(2, CommandKind::System, 0);

        dispatch
            .queue
            .push_back(QueuedCommand::new(cmd_low, CommandPriority::Low));
        dispatch
            .queue
            .push_back(QueuedCommand::new(cmd_normal, CommandPriority::Normal));
        dispatch
            .queue
            .push_back(QueuedCommand::new(cmd_high, CommandPriority::High));

        dispatch.sort_queue_by_priority();

        // Verify high priority is first
        assert_eq!(dispatch.queue[0].priority, CommandPriority::High);
        assert_eq!(dispatch.queue[1].priority, CommandPriority::Normal);
        assert_eq!(dispatch.queue[2].priority, CommandPriority::Low);
    }

    #[test]
    fn test_rts_command_creation() {
        let move_cmd = RtsCommand::move_to(vec![1, 2, 3], (100.0, 200.0, 0.0));
        assert_eq!(move_cmd.command_type, Some("MOVE".to_string()));
        assert_eq!(move_cmd.selected_objects.len(), 3);

        let attack_cmd = RtsCommand::attack(vec![1], 999);
        assert_eq!(attack_cmd.command_type, Some("ATTACK".to_string()));
        assert_eq!(attack_cmd.target_id, Some(999));
    }

    #[test]
    fn test_dispatch_update() {
        let mut dispatch = GameLogicDispatch::new(8);

        // Queue a command for frame 0
        let command = Command::new(0, CommandKind::Player, 0);
        dispatch.queue_player_command(0, command).unwrap();

        assert_eq!(dispatch.queue.len(), 1);

        // Update should process the command
        assert!(dispatch.update(0).is_ok());
        assert_eq!(dispatch.queue.len(), 0);
        assert_eq!(dispatch.processed_batches.len(), 1);
    }

    #[test]
    fn test_frame_scheduling() {
        let mut dispatch = GameLogicDispatch::new(8);

        // Queue commands for future frames
        let mut cmd0 = Command::new(0, CommandKind::Player, 0);
        cmd0.set_execution_frame(5);
        let mut cmd1 = Command::new(1, CommandKind::Player, 0);
        cmd1.set_execution_frame(10);

        dispatch
            .queue
            .push_back(QueuedCommand::new(cmd0, CommandPriority::Normal));
        dispatch
            .queue
            .push_back(QueuedCommand::new(cmd1, CommandPriority::Normal));

        // Update frame 3 - no commands should execute
        assert!(dispatch.update(3).is_ok());
        assert_eq!(dispatch.queue.len(), 2);

        // Update frame 5 - first command should execute
        assert!(dispatch.update(5).is_ok());
        assert_eq!(dispatch.queue.len(), 1);

        // Update frame 10 - second command should execute
        assert!(dispatch.update(10).is_ok());
        assert_eq!(dispatch.queue.len(), 0);
    }
}
