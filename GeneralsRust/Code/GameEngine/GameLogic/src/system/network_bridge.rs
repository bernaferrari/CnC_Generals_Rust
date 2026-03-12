//! Network Command Bridge
#![cfg(feature = "network")]
//!
//! Translates GameNetwork::NetCommand to GameLogic::Command
//!
//! ## C++ Reference
//!
//! This ports the command translation logic from:
//! - `/GeneralsMD/Code/GameEngine/Source/GameClient/GUICommandTranslator.cpp` (lines 93-183)
//! - `/GeneralsMD/Code/GameEngine/Source/GameNetwork/NetCommandList.cpp` (lines 104-311)
//! - `/GeneralsMD/Code/GameEngine/Source/GameLogic/GameLogic.cpp` (line 3608)
//!
//! ## Architecture
//!
//! The bridge performs these key functions:
//! 1. Validates incoming network commands
//! 2. Translates NetCommand payload to GameLogic Command format
//! 3. Validates frame synchronization (execution frame must be valid)
//! 4. Queues commands for execution at the correct frame
//!
//! ## Usage
//!
//! ```rust
//! use crate::system::network_bridge::NetworkCommandBridge;
//! use game_network::commands::NetCommand;
//!
//! // Translate a network command
//! let game_command = NetworkCommandBridge::translate(&net_cmd)?;
//!
//! // Queue for execution
//! let bridge = NetworkCommandBridge::new();
//! bridge.queue_network_command(net_cmd)?;
//! ```

use crate::commands::{Command, CommandArgumentType, CommandType};
use crate::common::{AsciiString, Coord3D, Int, ObjectID};
use crate::system::game_logic_dispatch::{
    Command as DispatchCommand, CommandExecutionContext, CommandKind,
};
use glam::Vec3;
use log::{debug, error, warn};
use std::collections::HashMap;

// Import game_network types
// Note: This requires game_network to be added as a dependency in Cargo.toml
use game_network::commands::{CommandParameter, CommandPayload, GameCommandData, NetCommand};
use game_network::error::{NetworkError, NetworkResult};

/// Maximum allowed frame drift for network commands
/// Commands scheduled too far in the future (>300 frames) are rejected
/// Matches C++ MAX_FRAMES_AHEAD constant
const MAX_FRAME_DRIFT: u32 = 300;

/// Network command translation bridge
///
/// ## C++ Reference
///
/// Ports the command translation logic from GUICommandTranslator.cpp
pub struct NetworkCommandBridge {
    /// Current frame number for validation
    current_frame: u32,
    /// Command statistics
    stats: BridgeStatistics,
}

/// Statistics for network command translation
#[derive(Debug, Clone, Default)]
pub struct BridgeStatistics {
    /// Total commands translated
    pub total_translated: u64,
    /// Total commands rejected
    pub total_rejected: u64,
    /// Total frame sync errors
    pub frame_sync_errors: u64,
    /// Total invalid payloads
    pub invalid_payloads: u64,
}

impl NetworkCommandBridge {
    /// Create a new network command bridge
    pub fn new() -> Self {
        Self {
            current_frame: 0,
            stats: BridgeStatistics::default(),
        }
    }

    /// Update the current frame
    pub fn set_current_frame(&mut self, frame: u32) {
        self.current_frame = frame;
    }

    /// Translate a network command to a game logic command
    ///
    /// ## C++ Reference
    ///
    /// Matches GUICommandTranslator.cpp command translation logic (lines 93-183)
    ///
    /// ## Arguments
    ///
    /// * `net_cmd` - Network command from GameNetwork
    ///
    /// ## Returns
    ///
    /// Returns `Ok(Command)` on success, `Err(String)` on failure
    pub fn translate(net_cmd: &NetCommand) -> Result<Command, String> {
        // Extract the game command payload
        match &net_cmd.payload {
            CommandPayload::GameCommand(data) => {
                Self::translate_game_command(data, net_cmd.player_id)
            }
            _ => {
                warn!(
                    "Network command is not a game command: {:?}",
                    net_cmd.command_type
                );
                Err("Not a game command".into())
            }
        }
    }

    /// Translate a GameCommandData to a GameLogic Command
    ///
    /// ## C++ Reference
    ///
    /// Matches command translation in GUICommandTranslator.cpp
    ///
    /// ## Arguments
    ///
    /// * `data` - Game command data from network
    /// * `player_id` - Player who issued the command
    ///
    /// ## Returns
    ///
    /// Returns `Ok(Command)` on success, `Err(String)` on failure
    fn translate_game_command(data: &GameCommandData, player_id: u8) -> Result<Command, String> {
        // Map command_type from network to GameLogic CommandType
        let command_type = Self::map_command_type(data.command_type)?;

        // Create base command
        let mut command = Command::new(command_type);
        command.set_player_index(player_id as Int);

        if matches!(
            command_type,
            CommandType::DoSpecialPower
                | CommandType::DoSpecialPowerAtLocation
                | CommandType::DoSpecialPowerAtObject
                | CommandType::DoSpecialPowerOverrideDestination
        ) {
            let mut entries: Vec<_> = data.parameters.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            let params: Vec<CommandParameter> = entries
                .into_iter()
                .map(|(_, value)| value.clone())
                .collect();

            let param_int = |idx: usize| -> i32 {
                params
                    .get(idx)
                    .and_then(|p| match p {
                        CommandParameter::Int(value) => Some(*value),
                        _ => None,
                    })
                    .unwrap_or(0)
            };
            let param_float = |idx: usize| -> f32 {
                params
                    .get(idx)
                    .and_then(|p| match p {
                        CommandParameter::Float(value) => Some(*value),
                        _ => None,
                    })
                    .unwrap_or(0.0)
            };
            let param_object_id = |idx: usize| -> ObjectID {
                params
                    .get(idx)
                    .and_then(|p| match p {
                        CommandParameter::ObjectId(value) => Some(*value),
                        _ => None,
                    })
                    .unwrap_or(crate::common::INVALID_OBJECT_ID)
            };

            match command_type {
                CommandType::DoSpecialPower => {
                    command.append_integer_argument(param_int(0));
                    command.append_integer_argument(param_int(1));
                    command.append_object_id_argument(param_object_id(2));
                    return Ok(command);
                }
                CommandType::DoSpecialPowerAtLocation => {
                    let position = data
                        .position
                        .ok_or_else(|| "SpecialPowerAtLocation missing position".to_string())?;
                    let vec_position = Vec3::new(position.0, position.1, position.2);
                    command.append_integer_argument(param_int(0));
                    command.append_location_argument(vec_position);
                    command.append_real_argument(param_float(1));
                    command.append_object_id_argument(param_object_id(2));
                    command.append_integer_argument(param_int(3));
                    command.append_object_id_argument(param_object_id(4));
                    return Ok(command);
                }
                CommandType::DoSpecialPowerAtObject => {
                    let target_id = data
                        .target_id
                        .ok_or_else(|| "SpecialPowerAtObject missing target".to_string())?;
                    command.append_integer_argument(param_int(0));
                    command.append_object_id_argument(target_id);
                    command.append_integer_argument(param_int(1));
                    command.append_object_id_argument(param_object_id(2));
                    return Ok(command);
                }
                CommandType::DoSpecialPowerOverrideDestination => {
                    let position = data.position.ok_or_else(|| {
                        "SpecialPowerOverrideDestination missing position".to_string()
                    })?;
                    let vec_position = Vec3::new(position.0, position.1, position.2);
                    command.append_location_argument(vec_position);
                    command.append_integer_argument(param_int(0));
                    command.append_object_id_argument(param_object_id(1));
                    return Ok(command);
                }
                _ => {}
            }
        }

        // Add position argument if present
        if let Some(position) = data.position {
            let vec_position = Vec3::new(position.0, position.1, position.2);
            command.append_location_argument(vec_position);
        }

        // Add target object ID if present
        if let Some(target_id) = data.target_id {
            command.append_object_id_argument(target_id);
        }

        // Translate additional parameters
        for (_key, value) in &data.parameters {
            Self::append_translated_parameter(&mut command, value)?;
        }

        Ok(command)
    }

    /// Append a translated parameter to a command
    fn append_translated_parameter(
        command: &mut Command,
        param: &CommandParameter,
    ) -> Result<(), String> {
        match param {
            CommandParameter::Int(value) => command.append_integer_argument(*value),
            CommandParameter::Float(value) => command.append_real_argument(*value),
            CommandParameter::Bool(value) => command.append_boolean_argument(*value),
            CommandParameter::ObjectId(value) => command.append_object_id_argument(*value),
            CommandParameter::Position(x, y, z) => {
                let vec_position = Vec3::new(*x, *y, *z);
                command.append_location_argument(vec_position)
            }
            CommandParameter::String(value) => {
                command.append_ascii_string_argument(AsciiString::from(value.as_str()))
            }
        }
        Ok(())
    }

    /// Map network command type to GameLogic CommandType
    ///
    /// ## C++ Reference
    ///
    /// Command type mapping from GameMessage enumeration
    ///
    /// ## Arguments
    ///
    /// * `net_type` - Network command type (u32)
    ///
    /// ## Returns
    ///
    /// Returns `Ok(CommandType)` on success, `Err(String)` for unknown types
    fn map_command_type(net_type: u32) -> Result<CommandType, String> {
        // Map common RTS command types
        // These values should match the C++ GameMessage enumeration
        // Based on CommandType values from command.rs
        match net_type {
            1056 => Ok(CommandType::DozerConstruct), // MSG_DOZER_CONSTRUCT
            1071 => Ok(CommandType::DoAttackObject), // MSG_DO_ATTACK_OBJECT
            1086 => Ok(CommandType::DoStop),         // MSG_DO_STOP
            1084 => Ok(CommandType::DoGuardPosition), // MSG_DO_GUARD_POSITION
            1085 => Ok(CommandType::DoGuardObject),  // MSG_DO_GUARD_OBJECT
            1080 => Ok(CommandType::DoMoveTo),       // MSG_DO_MOVETO
            1054 => Ok(CommandType::QueueUnitCreate), // MSG_QUEUE_UNIT_CREATE
            1055 => Ok(CommandType::CancelUnitCreate), // MSG_CANCEL_UNIT_CREATE
            1059 => Ok(CommandType::Sell),           // MSG_SELL
            1076 => Ok(CommandType::DoRepair),       // MSG_DO_REPAIR
            1044 => Ok(CommandType::DoSpecialPower), // MSG_DO_SPECIAL_POWER
            1045 => Ok(CommandType::DoSpecialPowerAtLocation), // MSG_DO_SPECIAL_POWER_AT_LOCATION
            1046 => Ok(CommandType::DoSpecialPowerAtObject), // MSG_DO_SPECIAL_POWER_AT_OBJECT
            1100 => Ok(CommandType::DoSpecialPowerOverrideDestination), // MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION
            1041 => Ok(CommandType::DoWeapon),                          // MSG_DO_WEAPON
            1042 => Ok(CommandType::DoWeaponAtLocation),                // MSG_DO_WEAPON_AT_LOCATION
            1043 => Ok(CommandType::DoWeaponAtObject),                  // MSG_DO_WEAPON_AT_OBJECT
            1050 => Ok(CommandType::SetRallyPoint),                     // MSG_SET_RALLY_POINT
            1051 => Ok(CommandType::PurchaseScience),                   // MSG_PURCHASE_SCIENCE
            1052 => Ok(CommandType::QueueUpgrade),                      // MSG_QUEUE_UPGRADE
            1053 => Ok(CommandType::CancelUpgrade),                     // MSG_CANCEL_UPGRADE
            _ => {
                warn!("Unknown network command type: {}", net_type);
                Err(format!("Unknown command type: {}", net_type))
            }
        }
    }

    /// Translate a network parameter to a command argument
    ///
    /// ## Arguments
    ///
    /// * `param` - Network command parameter
    ///
    /// ## Returns
    ///
    /// Returns `Ok(CommandArgumentType)` on success, `Err(String)` on failure
    fn translate_parameter(param: &CommandParameter) -> Result<CommandArgumentType, String> {
        match param {
            CommandParameter::Int(value) => Ok(CommandArgumentType::Integer(*value)),
            CommandParameter::Float(value) => Ok(CommandArgumentType::Real(*value)),
            CommandParameter::Bool(value) => Ok(CommandArgumentType::Boolean(*value)),
            CommandParameter::ObjectId(value) => Ok(CommandArgumentType::ObjectID(*value)),
            CommandParameter::Position(x, y, z) => {
                let vec_position = Vec3::new(*x, *y, *z);
                Ok(CommandArgumentType::Location(vec_position))
            }
            CommandParameter::String(value) => Ok(CommandArgumentType::AsciiString(
                AsciiString::from(value.as_str()),
            )),
        }
    }

    /// Validate frame synchronization for network command
    ///
    /// ## C++ Reference
    ///
    /// Matches frame validation in GameLogic.cpp (line 3608)
    ///
    /// ## Arguments
    ///
    /// * `net_cmd` - Network command to validate
    ///
    /// ## Returns
    ///
    /// Returns `Ok(())` if frame is valid, `Err(String)` if invalid
    pub fn validate_frame_sync(&self, net_cmd: &NetCommand) -> Result<(), String> {
        let execution_frame = net_cmd.execution_frame;

        // Check if command is from the past
        if execution_frame < self.current_frame {
            warn!(
                "Network command frame {} is in the past (current: {})",
                execution_frame, self.current_frame
            );
            return Err(format!(
                "Command frame {} is in the past (current: {})",
                execution_frame, self.current_frame
            ));
        }

        // Check if command is too far in the future
        if execution_frame > self.current_frame + MAX_FRAME_DRIFT {
            warn!(
                "Network command frame {} is too far in future (current: {}, max drift: {})",
                execution_frame, self.current_frame, MAX_FRAME_DRIFT
            );
            return Err(format!(
                "Command frame {} exceeds max drift (current: {}, max: {})",
                execution_frame,
                self.current_frame,
                self.current_frame + MAX_FRAME_DRIFT
            ));
        }

        debug!(
            "Frame sync valid: execution_frame={}, current_frame={}",
            execution_frame, self.current_frame
        );
        Ok(())
    }

    /// Queue a network command for execution at the correct frame
    ///
    /// ## C++ Reference
    ///
    /// Matches command queuing in GameLogic.cpp
    ///
    /// ## Arguments
    ///
    /// * `net_cmd` - Network command to queue
    ///
    /// ## Returns
    ///
    /// Returns `Ok(())` on success, `Err(String)` on failure
    pub fn queue_network_command(&mut self, net_cmd: NetCommand) -> Result<(), String> {
        // Validate frame synchronization
        if let Err(e) = self.validate_frame_sync(&net_cmd) {
            self.stats.frame_sync_errors += 1;
            self.stats.total_rejected += 1;
            return Err(e);
        }

        // Translate network command to game command
        let game_command = match Self::translate(&net_cmd) {
            Ok(cmd) => cmd,
            Err(e) => {
                self.stats.invalid_payloads += 1;
                self.stats.total_rejected += 1;
                return Err(e);
            }
        };

        // Convert to dispatch command
        let dispatch_command = DispatchCommand::new(
            net_cmd.player_id as Int,
            CommandKind::Network,
            net_cmd.execution_frame,
        );

        // Queue the command for execution
        // In a full implementation, this would call into GameLogicDispatch
        // For now, we just validate the translation succeeds
        debug!(
            "Queued network command: player={}, frame={}, type={:?}",
            net_cmd.player_id,
            net_cmd.execution_frame,
            game_command.get_type()
        );

        self.stats.total_translated += 1;
        Ok(())
    }

    /// Get bridge statistics
    pub fn get_statistics(&self) -> &BridgeStatistics {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_statistics(&mut self) {
        self.stats = BridgeStatistics::default();
    }
}

impl Default for NetworkCommandBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_network::commands::{GameCommandData, NetCommand, NetCommandType};
    use std::collections::HashMap;

    #[test]
    fn test_bridge_creation() {
        let bridge = NetworkCommandBridge::new();
        assert_eq!(bridge.current_frame, 0);
        assert_eq!(bridge.stats.total_translated, 0);
    }

    #[test]
    fn test_frame_validation_past() {
        let bridge = NetworkCommandBridge::new();
        // Current frame is 0, so any frame < 0 would be invalid
        // But u32 can't be negative, so we set current to 100
        let mut bridge = NetworkCommandBridge::new();
        bridge.set_current_frame(100);

        let net_cmd = NetCommand::game_command(
            0,
            50, // Past frame
            GameCommandData {
                command_type: 1080, // DoMoveTo
                target_id: None,
                position: None,
                parameters: HashMap::new(),
                checksum: 0,
            },
        );

        assert!(bridge.validate_frame_sync(&net_cmd).is_err());
    }

    #[test]
    fn test_frame_validation_future() {
        let mut bridge = NetworkCommandBridge::new();
        bridge.set_current_frame(100);

        let net_cmd = NetCommand::game_command(
            0,
            500, // Too far in future (> 100 + 300)
            GameCommandData {
                command_type: 1080, // DoMoveTo
                target_id: None,
                position: None,
                parameters: HashMap::new(),
                checksum: 0,
            },
        );

        assert!(bridge.validate_frame_sync(&net_cmd).is_err());
    }

    #[test]
    fn test_frame_validation_valid() {
        let mut bridge = NetworkCommandBridge::new();
        bridge.set_current_frame(100);

        let net_cmd = NetCommand::game_command(
            0,
            150, // Valid: 100 + 50 < 100 + 300
            GameCommandData {
                command_type: 1080, // DoMoveTo
                target_id: None,
                position: None,
                parameters: HashMap::new(),
                checksum: 0,
            },
        );

        assert!(bridge.validate_frame_sync(&net_cmd).is_ok());
    }

    #[test]
    fn test_command_type_mapping() {
        assert!(NetworkCommandBridge::map_command_type(1056).is_ok()); // DozerConstruct
        assert!(NetworkCommandBridge::map_command_type(1080).is_ok()); // DoMoveTo
        assert!(NetworkCommandBridge::map_command_type(999).is_err()); // Unknown
    }

    #[test]
    fn test_parameter_translation() {
        let param = CommandParameter::Int(42);
        let result = NetworkCommandBridge::translate_parameter(&param);
        assert!(result.is_ok());

        let param = CommandParameter::Position(1.0, 2.0, 3.0);
        let result = NetworkCommandBridge::translate_parameter(&param);
        assert!(result.is_ok());
    }

    #[test]
    fn test_game_command_translation() {
        let mut params = HashMap::new();
        params.insert("test".to_string(), CommandParameter::Int(123));

        let game_data = GameCommandData {
            command_type: 1080, // DoMoveTo
            target_id: Some(456),
            position: Some((10.0, 20.0, 30.0)),
            parameters: params,
            checksum: 0,
        };

        let result = NetworkCommandBridge::translate_game_command(&game_data, 0);
        assert!(result.is_ok());

        let command = result.unwrap();
        assert_eq!(command.get_player_index(), 0);
    }

    #[test]
    fn test_statistics_tracking() {
        let mut bridge = NetworkCommandBridge::new();
        assert_eq!(bridge.get_statistics().total_translated, 0);

        bridge.stats.total_translated = 10;
        assert_eq!(bridge.get_statistics().total_translated, 10);

        bridge.reset_statistics();
        assert_eq!(bridge.get_statistics().total_translated, 0);
    }
}
