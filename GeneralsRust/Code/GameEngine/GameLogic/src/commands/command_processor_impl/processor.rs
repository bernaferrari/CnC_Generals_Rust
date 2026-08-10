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

