#![allow(non_upper_case_globals)]
////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Base Command System - Core command types and functionality
//!
//! This module provides the fundamental command system that exactly matches
//! the C++ GameMessage and Command system from the original game.
//! All RTS player commands, AI commands, and script commands flow through this system.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

// Import common types - these should match C++ exactly
use crate::common::{
    AsciiString, Bool, Coord3D, DrawableID, ICoord2D, IRegion2D, Int, ObjectID, PlayerMaskType,
    Real, UnsignedByte, UnsignedInt, UnsignedShort, WideChar,
};
pub use game_engine::common::message_stream::CommandType;

/// Maximum arguments per command - matches C++ GameMessage limit
pub const MAX_COMMAND_ARGUMENTS: usize = 255;

/// Command argument data types - matches C++ GameMessageArgumentDataType exactly
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandArgumentDataType {
    Integer,
    Real,
    Boolean,
    ObjectID,
    DrawableId,
    TeamId,
    Location,
    Pixel,
    PixelRegion,
    Timestamp,
    WideChar,
    AsciiString,
    Unknown,
}

/// Command argument type union - matches C++ GameMessageArgumentType exactly
#[derive(Debug, Clone)]
pub enum CommandArgumentType {
    Integer(Int),
    Real(Real),
    Boolean(Bool),
    ObjectID(ObjectID),
    DrawableId(DrawableID),
    TeamId(UnsignedInt),
    SquadId(UnsignedInt),
    Location(Coord3D),
    Pixel(ICoord2D),
    PixelRegion(IRegion2D),
    Timestamp(UnsignedInt),
    WideChar(WideChar),
    AsciiString(AsciiString),
}

impl CommandArgumentType {
    /// Get the data type of this argument
    pub fn get_data_type(&self) -> CommandArgumentDataType {
        match self {
            CommandArgumentType::Integer(_) => CommandArgumentDataType::Integer,
            CommandArgumentType::Real(_) => CommandArgumentDataType::Real,
            CommandArgumentType::Boolean(_) => CommandArgumentDataType::Boolean,
            CommandArgumentType::ObjectID(_) => CommandArgumentDataType::ObjectID,
            CommandArgumentType::DrawableId(_) => CommandArgumentDataType::DrawableId,
            CommandArgumentType::TeamId(_) => CommandArgumentDataType::TeamId,
            CommandArgumentType::SquadId(_) => CommandArgumentDataType::TeamId,
            CommandArgumentType::Location(_) => CommandArgumentDataType::Location,
            CommandArgumentType::Pixel(_) => CommandArgumentDataType::Pixel,
            CommandArgumentType::PixelRegion(_) => CommandArgumentDataType::PixelRegion,
            CommandArgumentType::Timestamp(_) => CommandArgumentDataType::Timestamp,
            CommandArgumentType::WideChar(_) => CommandArgumentDataType::WideChar,
            CommandArgumentType::AsciiString(_) => CommandArgumentDataType::AsciiString,
        }
    }
}

/// Base Command class - matches C++ GameMessage exactly
#[derive(Debug, Clone)]
pub struct Command {
    /// Message type - what kind of command this is
    command_type: CommandType,

    /// Arguments for this command
    arguments: Vec<CommandArgumentType>,

    /// Player who issued this command
    player_index: Int,

    /// Unique command ID for tracking
    id: UnsignedInt,

    /// Timestamp when command was created
    timestamp: SystemTime,

    /// Execution frame (for network synchronization)
    execution_frame: UnsignedInt,

    /// Command priority
    priority: Int,
}

impl Command {
    /// Create new command - matches C++ GameMessage constructor
    pub fn new(command_type: CommandType) -> Self {
        Self {
            command_type,
            arguments: Vec::new(),
            player_index: -1,
            id: Self::generate_id(),
            timestamp: SystemTime::now(),
            execution_frame: 0,
            priority: 0,
        }
    }

    /// Generate unique command ID
    fn generate_id() -> UnsignedInt {
        use std::sync::atomic::{AtomicU32, Ordering};
        crate::system::engine_stores::active_services()
            .session_flags()
            .next_command_id
            .fetch_add(1, Ordering::Relaxed)
    }

    /// Get command type
    pub fn get_type(&self) -> CommandType {
        self.command_type
    }

    /// Get argument count - matches C++ getArgumentCount
    pub fn get_argument_count(&self) -> UnsignedByte {
        self.arguments.len() as UnsignedByte
    }

    /// Get player index - matches C++ getPlayerIndex
    pub fn get_player_index(&self) -> Int {
        self.player_index
    }

    /// Set player index - matches C++ friend_setPlayerIndex
    pub fn set_player_index(&mut self, player_index: Int) {
        self.player_index = player_index;
    }

    /// Get command identifier
    pub fn get_id(&self) -> UnsignedInt {
        self.id
    }

    /// Get execution frame
    pub fn get_execution_frame(&self) -> UnsignedInt {
        self.execution_frame
    }

    /// Set execution frame
    pub fn set_execution_frame(&mut self, frame: UnsignedInt) {
        self.execution_frame = frame;
    }

    /// Get command as string for debugging - matches C++ getCommandAsAsciiString
    pub fn get_command_as_ascii_string(&self) -> AsciiString {
        self.command_type.as_ascii_string()
    }

    // Argument methods - match C++ GameMessage methods exactly

    /// Append integer argument - matches C++ appendIntegerArgument
    pub fn append_integer_argument(&mut self, arg: Int) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::Integer(arg));
        }
    }

    /// Append real argument - matches C++ appendRealArgument
    pub fn append_real_argument(&mut self, arg: Real) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::Real(arg));
        }
    }

    /// Append boolean argument - matches C++ appendBooleanArgument
    pub fn append_boolean_argument(&mut self, arg: Bool) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::Boolean(arg));
        }
    }

    /// Append object ID argument - matches C++ appendObjectIDArgument
    pub fn append_object_id_argument(&mut self, arg: ObjectID) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::ObjectID(arg));
        }
    }

    /// Append drawable ID argument - matches C++ appendDrawableIDArgument
    pub fn append_drawable_id_argument(&mut self, arg: DrawableID) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::DrawableId(arg));
        }
    }

    /// Append team ID argument - matches C++ appendTeamIDArgument
    pub fn append_team_id_argument(&mut self, arg: UnsignedInt) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::TeamId(arg));
        }
    }

    /// Append location argument - matches C++ appendLocationArgument
    pub fn append_location_argument(&mut self, arg: Coord3D) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::Location(arg));
        }
    }

    /// Append pixel argument - matches C++ appendPixelArgument
    pub fn append_pixel_argument(&mut self, arg: ICoord2D) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::Pixel(arg));
        }
    }

    /// Append pixel region argument - matches C++ appendPixelRegionArgument
    pub fn append_pixel_region_argument(&mut self, arg: IRegion2D) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::PixelRegion(arg));
        }
    }

    /// Append timestamp argument - matches C++ appendTimestampArgument
    pub fn append_timestamp_argument(&mut self, arg: UnsignedInt) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::Timestamp(arg));
        }
    }

    /// Append wide char argument - matches C++ appendWideCharArgument
    pub fn append_wide_char_argument(&mut self, arg: WideChar) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::WideChar(arg));
        }
    }

    /// Append ASCII string argument
    pub fn append_ascii_string_argument(&mut self, arg: AsciiString) {
        if self.arguments.len() < MAX_COMMAND_ARGUMENTS {
            self.arguments.push(CommandArgumentType::AsciiString(arg));
        }
    }

    /// Get argument at index - matches C++ getArgument
    pub fn get_argument(&self, arg_index: Int) -> Option<&CommandArgumentType> {
        if arg_index >= 0 && (arg_index as usize) < self.arguments.len() {
            Some(&self.arguments[arg_index as usize])
        } else {
            None
        }
    }

    /// Get argument data type at index - matches C++ getArgumentDataType
    pub fn get_argument_data_type(&self, arg_index: Int) -> CommandArgumentDataType {
        if let Some(arg) = self.get_argument(arg_index) {
            arg.get_data_type()
        } else {
            CommandArgumentDataType::Unknown
        }
    }

    /// Get sort number for command ordering - matches C++ getSortNumber
    pub fn get_sort_number(&self) -> Int {
        self.priority
    }

    /// Set priority for command ordering
    pub fn set_priority(&mut self, priority: Int) {
        self.priority = priority;
    }
}

/// Command validation results
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandValidation {
    Valid,
    InvalidType,
    InvalidArguments,
    InvalidPlayer,
    InvalidGameState,
    InsufficientResources,
    InvalidTarget,
    NotAllowed,
}

/// Command validator trait - allows custom validation logic
pub trait CommandValidator {
    /// Validate a command before execution
    fn validate_command(&self, command: &Command) -> CommandValidation;

    /// Check if player can execute this command type
    fn can_player_execute(&self, player_id: Int, command_type: CommandType) -> bool;
}

/// Default command validator with basic validation
pub struct DefaultCommandValidator;

impl CommandValidator for DefaultCommandValidator {
    fn validate_command(&self, command: &Command) -> CommandValidation {
        // Basic validation - check command type
        match command.get_type() {
            CommandType::Invalid => CommandValidation::InvalidType,
            CommandType::Count => CommandValidation::InvalidType,
            _ => CommandValidation::Valid,
        }
    }

    fn can_player_execute(&self, _player_id: Int, command_type: CommandType) -> bool {
        // Meta messages should not be sent over network
        if command_type.is_meta_message() {
            return false;
        }

        // Network messages are allowed
        command_type.is_network_message()
    }
}

/// Common command creation utilities
pub mod command_builder {
    use super::*;

    /// Create move to position command - matches common C++ pattern
    pub fn create_move_to_position(
        objects: Vec<ObjectID>,
        position: Coord3D,
        player_id: Int,
    ) -> Command {
        let mut command = Command::new(CommandType::DoMoveTo);
        command.set_player_index(player_id);

        // First argument is position
        command.append_location_argument(position);

        // Then add all object IDs
        for object_id in objects {
            command.append_object_id_argument(object_id);
        }

        command
    }

    /// Create attack object command - matches common C++ pattern
    pub fn create_attack_object(
        attackers: Vec<ObjectID>,
        target: ObjectID,
        player_id: Int,
    ) -> Command {
        let mut command = Command::new(CommandType::DoAttackObject);
        command.set_player_index(player_id);

        // First argument is target
        command.append_object_id_argument(target);

        // Then add all attacker IDs
        for attacker_id in attackers {
            command.append_object_id_argument(attacker_id);
        }

        command
    }

    /// Create area selection command - matches C++ AreaSelection
    pub fn create_area_selection(region: IRegion2D, player_id: Int) -> Command {
        let mut command = Command::new(CommandType::AreaSelection);
        command.set_player_index(player_id);
        command.append_pixel_region_argument(region);
        command
    }

    /// Create build structure command
    pub fn create_build_structure(builder: ObjectID, position: Coord3D, player_id: Int) -> Command {
        let mut command = Command::new(CommandType::DozerConstruct);
        command.set_player_index(player_id);
        command.append_object_id_argument(builder);
        command.append_location_argument(position);
        command
    }

    /// Create unit production command
    pub fn create_produce_unit(factory: ObjectID, count: Int, player_id: Int) -> Command {
        let mut command = Command::new(CommandType::QueueUnitCreate);
        command.set_player_index(player_id);
        command.append_object_id_argument(factory);
        command.append_integer_argument(count);
        command
    }

    /// Create stop command for units
    pub fn create_stop_command(objects: Vec<ObjectID>, player_id: Int) -> Command {
        let mut command = Command::new(CommandType::DoStop);
        command.set_player_index(player_id);

        for object_id in objects {
            command.append_object_id_argument(object_id);
        }

        command
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_creation() {
        let command = Command::new(CommandType::DoMoveTo);
        assert_eq!(command.get_type(), CommandType::DoMoveTo);
        assert_eq!(command.get_argument_count(), 0);
        assert!(command.id > 0);
    }

    #[test]
    fn test_command_arguments() {
        let mut command = Command::new(CommandType::DoAttackObject);
        command.append_integer_argument(42);
        command.append_real_argument(3.14);
        command.append_boolean_argument(true);

        assert_eq!(command.get_argument_count(), 3);

        if let Some(CommandArgumentType::Integer(val)) = command.get_argument(0) {
            assert_eq!(*val, 42);
        } else {
            panic!("Expected integer argument");
        }
    }

    #[test]
    fn test_command_types() {
        assert!(CommandType::DoMoveTo.is_network_message());
        assert!(!CommandType::MetaStop.is_network_message());
        assert!(CommandType::MetaStop.is_meta_message());
        assert!(CommandType::RawMouseLeftClick.is_raw_mouse_message());
    }

    #[test]
    fn test_command_builder() {
        let objects = vec![1, 2, 3];
        let position = [100.0, 200.0, 0.0];
        let command = command_builder::create_move_to_position(objects, position.into(), 1);

        assert_eq!(command.get_type(), CommandType::DoMoveTo);
        assert_eq!(command.get_player_index(), 1);
        assert_eq!(command.get_argument_count(), 4); // 1 position + 3 objects
    }

    #[test]
    fn test_command_validation() {
        let validator = DefaultCommandValidator;
        let valid_command = Command::new(CommandType::DoMoveTo);
        let invalid_command = Command::new(CommandType::Invalid);

        assert_eq!(
            validator.validate_command(&valid_command),
            CommandValidation::Valid
        );
        assert_eq!(
            validator.validate_command(&invalid_command),
            CommandValidation::InvalidType
        );
    }
}
