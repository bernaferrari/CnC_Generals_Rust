//! Command validation system for ensuring game integrity
//!
//! This module provides validation logic for network commands to prevent
//! cheating, ensure game state consistency, and validate command parameters.

use crate::commands::{CommandPayload, GameCommandData, NetCommand, NetCommandType};
use crate::error::{NetworkError, NetworkResult};
use std::collections::HashSet;
use tracing::{debug, warn};

/// Command validation rules
pub struct ValidationRules {
    /// Maximum commands per frame per player
    pub max_commands_per_frame: usize,
    /// Maximum game command parameters
    pub max_game_parameters: usize,
    /// Allowed command types per game state
    pub allowed_commands: HashSet<NetCommandType>,
    /// Enable strict validation
    pub strict_mode: bool,
}

impl Default for ValidationRules {
    fn default() -> Self {
        let mut allowed_commands = HashSet::new();
        allowed_commands.insert(NetCommandType::GameCommand);
        allowed_commands.insert(NetCommandType::Chat);
        allowed_commands.insert(NetCommandType::KeepAlive);
        allowed_commands.insert(NetCommandType::AckBoth);
        allowed_commands.insert(NetCommandType::AckStage1);
        allowed_commands.insert(NetCommandType::AckStage2);

        Self {
            max_commands_per_frame: 32,
            max_game_parameters: 16,
            allowed_commands,
            strict_mode: true,
        }
    }
}

/// Command validator
pub struct CommandValidator {
    rules: ValidationRules,
}

impl CommandValidator {
    /// Create new validator
    pub fn new() -> Self {
        Self::with_rules(ValidationRules::default())
    }

    /// Create validator with custom rules
    pub fn with_rules(rules: ValidationRules) -> Self {
        Self { rules }
    }

    /// Validate a network command
    pub fn validate_command(&self, command: &NetCommand) -> NetworkResult<()> {
        // Basic validation
        self.validate_basic(command)?;

        // Type-specific validation
        self.validate_command_type(command)?;

        // Payload validation
        self.validate_payload(command)?;

        debug!("Command {} validation passed", command.id);
        Ok(())
    }

    /// Basic command validation
    fn validate_basic(&self, command: &NetCommand) -> NetworkResult<()> {
        // Check player ID range
        if command.player_id >= crate::config::MAX_PLAYERS {
            return Err(NetworkError::invalid_command(format!(
                "invalid player ID: {} (max: {})",
                command.player_id,
                crate::config::MAX_PLAYERS - 1
            )));
        }

        // Check if command type is allowed
        if self.rules.strict_mode && !self.rules.allowed_commands.contains(&command.command_type) {
            return Err(NetworkError::invalid_command(format!(
                "command type not allowed: {:?}",
                command.command_type
            )));
        }

        // Check sequence number validity
        if command.sequence > u16::MAX {
            return Err(NetworkError::invalid_command("invalid sequence number"));
        }

        Ok(())
    }

    /// Validate command type specific rules
    fn validate_command_type(&self, command: &NetCommand) -> NetworkResult<()> {
        match command.command_type {
            NetCommandType::GameCommand => {
                // Game commands must have execution frame
                if command.execution_frame == 0 && self.rules.strict_mode {
                    warn!("Game command with frame 0: {}", command.id);
                }
            }
            NetCommandType::Chat => {
                // Chat commands don't need frame synchronization
                if command.execution_frame != 0 {
                    return Err(NetworkError::invalid_command(
                        "chat command should not have execution frame",
                    ));
                }
            }
            NetCommandType::KeepAlive => {
                // Keep-alive commands should be minimal
                if command.execution_frame != 0 {
                    return Err(NetworkError::invalid_command(
                        "keep-alive command should not have execution frame",
                    ));
                }
            }
            NetCommandType::AckBoth | NetCommandType::AckStage1 | NetCommandType::AckStage2 => {
                // Acknowledgment commands should not have execution frames
                if command.execution_frame != 0 {
                    return Err(NetworkError::invalid_command(
                        "acknowledgment command should not have execution frame",
                    ));
                }
            }
            _ => {
                // Other command types have basic validation
            }
        }

        Ok(())
    }

    /// Validate command payload
    fn validate_payload(&self, command: &NetCommand) -> NetworkResult<()> {
        match &command.payload {
            CommandPayload::GameCommand(game_data) => {
                self.validate_game_command(game_data, command.player_id)?;
            }
            CommandPayload::Chat(chat_data) => {
                // C++ uses max 255 chars for chat messages
                if chat_data.message.len() > 255 {
                    return Err(NetworkError::invalid_command(
                        "chat message too long (max 255 chars)",
                    ));
                }
                if chat_data.message.is_empty() {
                    return Err(NetworkError::invalid_command("empty chat message"));
                }
                // target_mask is i32 in C++, so we accept any i32 value
                // The mask is a bitfield where each bit represents a player (0-7)
                // Negative values and values outside the player range are technically valid
                // but may have special meanings in the C++ implementation
            }
            CommandPayload::KeepAlive => {
                // No additional validation needed
            }
            CommandPayload::Ack(ack_data) => {
                // Validate that we're acknowledging a valid UUID
                if ack_data.command_id.is_nil() {
                    return Err(NetworkError::invalid_command("invalid acknowledgment ID"));
                }
            }
            CommandPayload::Generic(data) => {
                if data.len() > 1024 {
                    return Err(NetworkError::invalid_command("generic payload too large"));
                }
            }
            _ => {
                // Other payload types have basic validation in the payload itself
            }
        }

        Ok(())
    }

    /// Validate game command data
    fn validate_game_command(
        &self,
        game_data: &GameCommandData,
        player_id: u8,
    ) -> NetworkResult<()> {
        // Check parameter count
        if game_data.parameters.len() > self.rules.max_game_parameters {
            return Err(NetworkError::invalid_command(format!(
                "too many game command parameters: {} (max: {})",
                game_data.parameters.len(),
                self.rules.max_game_parameters
            )));
        }

        // Validate command type range (game-specific)
        if game_data.command_type > 10000 {
            return Err(NetworkError::invalid_command("invalid game command type"));
        }

        // Validate position if present
        if let Some((x, y, z)) = game_data.position {
            if !self.validate_position(x, y, z) {
                return Err(NetworkError::invalid_command(
                    "invalid position coordinates",
                ));
            }
        }

        // Validate target ID if present
        if let Some(target_id) = game_data.target_id {
            if !self.validate_object_id(target_id, player_id) {
                return Err(NetworkError::invalid_command("invalid target object ID"));
            }
        }

        // Validate checksum (placeholder for actual game logic)
        if self.rules.strict_mode && game_data.checksum == 0 {
            warn!("Game command missing checksum: player {}", player_id);
        }

        Ok(())
    }

    /// Validate position coordinates
    fn validate_position(&self, x: f32, y: f32, z: f32) -> bool {
        // Basic range validation (adjust based on actual game map size)
        const MAX_COORD: f32 = 10000.0;
        const MIN_COORD: f32 = -10000.0;

        x >= MIN_COORD
            && x <= MAX_COORD
            && y >= MIN_COORD
            && y <= MAX_COORD
            && z >= MIN_COORD
            && z <= MAX_COORD
            && x.is_finite()
            && y.is_finite()
            && z.is_finite()
    }

    /// Validate object ID belongs to player
    fn validate_object_id(&self, object_id: u32, _player_id: u8) -> bool {
        // In a real game, this would check if the object belongs to the player
        // For now, just validate it's a reasonable ID
        object_id > 0 && object_id < 1_000_000
    }

    /// Validate multiple commands in a frame
    pub fn validate_frame_commands(&self, commands: &[NetCommand]) -> NetworkResult<()> {
        if commands.len() > self.rules.max_commands_per_frame {
            return Err(NetworkError::invalid_command(format!(
                "too many commands in frame: {} (max: {})",
                commands.len(),
                self.rules.max_commands_per_frame
            )));
        }

        // Validate each command
        for command in commands {
            self.validate_command(command)?;
        }

        // Check for duplicate command IDs (ignore nil UUIDs for commands that don't require IDs)
        let mut seen_ids = HashSet::new();
        for command in commands {
            if !command.id.is_nil() && !seen_ids.insert(command.id) {
                return Err(NetworkError::invalid_command(
                    "duplicate command ID in frame",
                ));
            }
        }

        Ok(())
    }

    /// Validate command sequence
    pub fn validate_command_sequence(
        &self,
        commands: &[NetCommand],
        expected_frame: u32,
    ) -> NetworkResult<()> {
        for (i, command) in commands.iter().enumerate() {
            // Check frame consistency
            if command.execution_frame != 0 && command.execution_frame != expected_frame {
                return Err(NetworkError::invalid_command(format!(
                    "command {} has wrong execution frame: {} (expected: {})",
                    i, command.execution_frame, expected_frame
                )));
            }

            // Check sequence numbers are consecutive
            if command.sequence != i as u16 {
                return Err(NetworkError::invalid_command(format!(
                    "command {} has wrong sequence: {} (expected: {})",
                    i, command.sequence, i
                )));
            }
        }

        Ok(())
    }
}

impl Default for CommandValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Anti-cheat validation extensions
pub struct AntiCheatValidator {
    validator: CommandValidator,
}

impl AntiCheatValidator {
    /// Create new anti-cheat validator
    pub fn new() -> Self {
        let mut rules = ValidationRules::default();
        rules.strict_mode = true;

        Self {
            validator: CommandValidator::with_rules(rules),
        }
    }

    /// Validate command with anti-cheat checks
    pub fn validate_with_anticheat(&self, command: &NetCommand) -> NetworkResult<()> {
        // Standard validation first
        self.validator.validate_command(command)?;

        // Additional anti-cheat checks
        self.check_timing_anomalies(command)?;
        self.check_impossible_commands(command)?;
        self.check_signature_validity(command)?;

        Ok(())
    }

    /// Check for timing anomalies
    fn check_timing_anomalies(&self, command: &NetCommand) -> NetworkResult<()> {
        let now = chrono::Utc::now();
        let age = now.signed_duration_since(command.timestamp);

        // Command is too old (more than 30 seconds)
        if age.num_seconds() > 30 {
            return Err(NetworkError::invalid_command("command too old"));
        }

        // Command is from the future (more than 5 seconds)
        if age.num_seconds() < -5 {
            return Err(NetworkError::invalid_command("command from future"));
        }

        Ok(())
    }

    /// Check for impossible game commands
    fn check_impossible_commands(&self, command: &NetCommand) -> NetworkResult<()> {
        if let CommandPayload::GameCommand(game_data) = &command.payload {
            // Check for impossible parameter combinations
            if game_data.parameters.len() > 8 && game_data.command_type < 10 {
                warn!("Suspicious game command: many parameters for simple command type");
            }

            // Check for rapid command execution (would need state tracking)
            // This is a placeholder for more sophisticated checks
        }

        Ok(())
    }

    /// Check command signature validity
    fn check_signature_validity(&self, command: &NetCommand) -> NetworkResult<()> {
        if let Some(signature) = &command.signature {
            // In a real implementation, this would verify the digital signature
            // For now, just check it's not empty
            if signature.is_empty() {
                return Err(NetworkError::invalid_command("empty command signature"));
            }

            // Signature should be reasonable length
            if signature.len() < 32 || signature.len() > 256 {
                return Err(NetworkError::invalid_command("invalid signature length"));
            }
        } else if command.flags.encrypted {
            return Err(NetworkError::invalid_command(
                "encrypted command missing signature",
            ));
        }

        Ok(())
    }
}

impl Default for AntiCheatValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::NetCommand;
    use std::collections::HashMap;

    #[test]
    fn test_basic_validation() {
        let validator = CommandValidator::new();
        let command = NetCommand::keep_alive(0);

        assert!(validator.validate_command(&command).is_ok());
    }

    #[test]
    fn test_invalid_player_id() {
        let validator = CommandValidator::new();
        let command = NetCommand::keep_alive(255); // Invalid player ID

        assert!(validator.validate_command(&command).is_err());
    }

    #[test]
    fn test_game_command_validation() {
        let validator = CommandValidator::new();
        let game_data = crate::commands::GameCommandData {
            command_type: 1,
            target_id: Some(123),
            position: Some((10.0, 20.0, 0.0)),
            parameters: HashMap::new(),
            checksum: 0,
        };

        let command = NetCommand::game_command(0, 100, game_data);
        assert!(validator.validate_command(&command).is_ok());
    }

    #[test]
    fn test_chat_validation() {
        let validator = CommandValidator::new();
        let command = NetCommand::chat(0, "Hello!".to_string(), 0xFF);

        assert!(validator.validate_command(&command).is_ok());

        // Test message too long
        let long_message = "x".repeat(300);
        let invalid_command = NetCommand::chat(0, long_message, 0xFF);
        assert!(validator.validate_command(&invalid_command).is_err());
    }

    #[test]
    fn test_frame_commands_validation() {
        let validator = CommandValidator::new();
        let commands = vec![NetCommand::keep_alive(0), NetCommand::keep_alive(1)];

        assert!(validator.validate_frame_commands(&commands).is_ok());

        // Test too many commands
        let many_commands: Vec<_> = (0..100)
            .map(|i| NetCommand::keep_alive((i % 8) as u8))
            .collect();
        assert!(validator.validate_frame_commands(&many_commands).is_err());
    }

    #[test]
    fn test_position_validation() {
        let validator = CommandValidator::new();

        assert!(validator.validate_position(100.0, 200.0, 0.0));
        assert!(!validator.validate_position(f32::INFINITY, 0.0, 0.0));
        assert!(!validator.validate_position(20000.0, 0.0, 0.0)); // Out of range
    }

    #[test]
    fn test_anticheat_validation() {
        let anticheat = AntiCheatValidator::new();
        let command = NetCommand::keep_alive(0);

        assert!(anticheat.validate_with_anticheat(&command).is_ok());
    }
}
