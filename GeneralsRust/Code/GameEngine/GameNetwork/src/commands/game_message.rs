//! GameMessage serialization format matching C++ implementation
//!
//! This module implements the exact binary format used by the original C++ GameMessage
//! class, ensuring perfect compatibility for network play between Rust and C++ clients.
//!
//! ## Binary Format
//!
//! GameMessage has a complex structure with typed arguments:
//!
//! ### NetGameCommandMsg Structure (C++ NetCommandMsg subclass):
//! ```text
//! Base NetCommandMsg:
//!   - timestamp (u32)
//!   - execution_frame (u32)
//!   - player_id (u32)
//!   - id (u16)
//!   - command_type (NetCommandType, u8)
//!   - reference_count (i32) - not serialized over network
//!
//! NetGameCommandMsg specific:
//!   - num_args (i32)
//!   - arg_size (i32) - total size of all arguments in bytes
//!   - type (GameMessage::Type, u32)
//!   - arg_list (linked list of GameMessageArgument)
//! ```
//!
//! ### GameMessageArgument Structure:
//! ```text
//! +---+---+---+---+---+---+---+---+
//! | Type (u8) | Data (variable)   |
//! +---+---+---+---+---+---+---+---+
//! ```
//!
//! Type values match GameMessageArgumentDataType:
//! - 0: Integer (i32, 4 bytes)
//! - 1: Real (f32, 4 bytes)
//! - 2: Boolean (bool, 1 byte, but padded to 4 bytes in C++)
//! - 3: ObjectID (u32, 4 bytes)
//! - 4: DrawableID (u32, 4 bytes)
//! - 5: TeamID (u32, 4 bytes)
//! - 6: Location (Coord3D: 3x f32 = 12 bytes)
//! - 7: Pixel (ICoord2D: 2x i32 = 8 bytes)
//! - 8: PixelRegion (IRegion2D: 4x i32 = 16 bytes)
//! - 9: Timestamp (u32, 4 bytes)
//! - 10: WideChar (u16, 2 bytes, but padded to 4 bytes in C++)
//!
//! ## Wire Format
//!
//! When serialized for network transmission, the format is:
//! ```text
//! +---+---+---+---+---+---+---+---+
//! | Message Type (u32)            |
//! +---+---+---+---+---+---+---+---+
//! | Argument Count (u32)          |
//! +---+---+---+---+---+---+---+---+
//! | Arg 1 Type (u8) | Arg 1 Data  |
//! +---+---+---+---+---+---+---+---+
//! | Arg 2 Type (u8) | Arg 2 Data  |
//! +---+---+---+---+---+---+---+---+
//! ...
//! ```

use crate::error::{NetworkError, NetworkResult};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::Cursor;
use tracing::trace;

/// 3D coordinate structure matching C++ Coord3D
#[derive(Debug, Clone, PartialEq)]
#[repr(C)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Default for Coord3D {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

/// 2D integer coordinate matching C++ ICoord2D
#[derive(Debug, Clone, PartialEq)]
#[repr(C)]
pub struct ICoord2D {
    pub x: i32,
    pub y: i32,
}

impl Default for ICoord2D {
    fn default() -> Self {
        Self { x: 0, y: 0 }
    }
}

/// 2D integer region matching C++ IRegion2D
/// Uses lo/hi corner points like C++ BaseType.h
#[derive(Debug, Clone, PartialEq)]
#[repr(C)]
pub struct IRegion2D {
    pub lo: ICoord2D,
    pub hi: ICoord2D,
}

impl IRegion2D {
    /// Create from corner points
    pub fn new(x1: i32, y1: i32, x2: i32, y2: i32) -> Self {
        Self {
            lo: ICoord2D { x: x1, y: y1 },
            hi: ICoord2D { x: x2, y: y2 },
        }
    }

    /// Width of the region
    pub fn width(&self) -> i32 {
        self.hi.x - self.lo.x
    }

    /// Height of the region
    pub fn height(&self) -> i32 {
        self.hi.y - self.lo.y
    }
}

impl Default for IRegion2D {
    fn default() -> Self {
        Self {
            lo: ICoord2D { x: 0, y: 0 },
            hi: ICoord2D { x: 0, y: 0 },
        }
    }
}

/// Game message argument data type enum
/// Must match C++ GameMessageArgumentDataType exactly
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GameMessageArgumentDataType {
    Integer = 0,
    Real = 1,
    Boolean = 2,
    ObjectID = 3,
    DrawableID = 4,
    TeamID = 5,
    Location = 6,
    Pixel = 7,
    PixelRegion = 8,
    Timestamp = 9,
    WideChar = 10,
    Unknown = 255,
}

impl From<u8> for GameMessageArgumentDataType {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Integer,
            1 => Self::Real,
            2 => Self::Boolean,
            3 => Self::ObjectID,
            4 => Self::DrawableID,
            5 => Self::TeamID,
            6 => Self::Location,
            7 => Self::Pixel,
            8 => Self::PixelRegion,
            9 => Self::Timestamp,
            10 => Self::WideChar,
            _ => Self::Unknown,
        }
    }
}

/// Game message argument value
/// Matches C++ GameMessageArgumentType union
#[derive(Debug, Clone)]
pub enum GameMessageArgumentValue {
    Integer(i32),
    Real(f32),
    Boolean(bool),
    ObjectID(u32),
    DrawableID(u32),
    TeamID(u32),
    Location(Coord3D),
    Pixel(ICoord2D),
    PixelRegion(IRegion2D),
    Timestamp(u32),
    WideChar(u16),
}

impl GameMessageArgumentValue {
    /// Get the data type of this argument value
    pub fn data_type(&self) -> GameMessageArgumentDataType {
        match self {
            Self::Integer(_) => GameMessageArgumentDataType::Integer,
            Self::Real(_) => GameMessageArgumentDataType::Real,
            Self::Boolean(_) => GameMessageArgumentDataType::Boolean,
            Self::ObjectID(_) => GameMessageArgumentDataType::ObjectID,
            Self::DrawableID(_) => GameMessageArgumentDataType::DrawableID,
            Self::TeamID(_) => GameMessageArgumentDataType::TeamID,
            Self::Location(_) => GameMessageArgumentDataType::Location,
            Self::Pixel(_) => GameMessageArgumentDataType::Pixel,
            Self::PixelRegion(_) => GameMessageArgumentDataType::PixelRegion,
            Self::Timestamp(_) => GameMessageArgumentDataType::Timestamp,
            Self::WideChar(_) => GameMessageArgumentDataType::WideChar,
        }
    }

    /// Get the size of this argument value in bytes (matching C++ sizeof)
    pub fn size_in_bytes(&self) -> usize {
        match self {
            Self::Integer(_) => 4,
            Self::Real(_) => 4,
            Self::Boolean(_) => 4, // C++ pads bool to 4 bytes in union
            Self::ObjectID(_) => 4,
            Self::DrawableID(_) => 4,
            Self::TeamID(_) => 4,
            Self::Location(_) => 12,    // 3 * f32
            Self::Pixel(_) => 8,        // 2 * i32
            Self::PixelRegion(_) => 16, // 4 * i32
            Self::Timestamp(_) => 4,
            Self::WideChar(_) => 4, // C++ pads wchar to 4 bytes in union
        }
    }
}

/// Game message argument structure
/// Matches C++ GameMessageArgument class
#[derive(Debug, Clone)]
pub struct GameMessageArgument {
    /// Argument data type
    pub data_type: GameMessageArgumentDataType,
    /// Argument value
    pub value: GameMessageArgumentValue,
}

impl GameMessageArgument {
    /// Create a new argument
    pub fn new(value: GameMessageArgumentValue) -> Self {
        Self {
            data_type: value.data_type(),
            value,
        }
    }

    /// Serialize this argument to bytes (C++ compatible - includes type prefix)
    pub fn serialize(&self, buffer: &mut Vec<u8>) -> NetworkResult<()> {
        // Write type byte
        buffer.write_u8(self.data_type as u8)?;

        // Write value
        self.serialize_value_only(buffer)
    }

    /// Serialize only the value without type prefix (for grouped encoding)
    pub fn serialize_value_only(&self, buffer: &mut Vec<u8>) -> NetworkResult<()> {
        // Write value based on type
        match &self.value {
            GameMessageArgumentValue::Integer(val) => {
                buffer.write_i32::<LittleEndian>(*val)?;
            }
            GameMessageArgumentValue::Real(val) => {
                buffer.write_f32::<LittleEndian>(*val)?;
            }
            GameMessageArgumentValue::Boolean(val) => {
                // C++ stores bool as 4 bytes (padded)
                buffer.write_u32::<LittleEndian>(if *val { 1 } else { 0 })?;
            }
            GameMessageArgumentValue::ObjectID(val) => {
                buffer.write_u32::<LittleEndian>(*val)?;
            }
            GameMessageArgumentValue::DrawableID(val) => {
                buffer.write_u32::<LittleEndian>(*val)?;
            }
            GameMessageArgumentValue::TeamID(val) => {
                buffer.write_u32::<LittleEndian>(*val)?;
            }
            GameMessageArgumentValue::Location(coord) => {
                buffer.write_f32::<LittleEndian>(coord.x)?;
                buffer.write_f32::<LittleEndian>(coord.y)?;
                buffer.write_f32::<LittleEndian>(coord.z)?;
            }
            GameMessageArgumentValue::Pixel(pixel) => {
                buffer.write_i32::<LittleEndian>(pixel.x)?;
                buffer.write_i32::<LittleEndian>(pixel.y)?;
            }
            GameMessageArgumentValue::PixelRegion(region) => {
                // C++ serializes IRegion2D as lo.x, lo.y, hi.x, hi.y
                buffer.write_i32::<LittleEndian>(region.lo.x)?;
                buffer.write_i32::<LittleEndian>(region.lo.y)?;
                buffer.write_i32::<LittleEndian>(region.hi.x)?;
                buffer.write_i32::<LittleEndian>(region.hi.y)?;
            }
            GameMessageArgumentValue::Timestamp(val) => {
                buffer.write_u32::<LittleEndian>(*val)?;
            }
            GameMessageArgumentValue::WideChar(val) => {
                // C++ stores wchar as 4 bytes (padded)
                buffer.write_u32::<LittleEndian>(*val as u32)?;
            }
        }

        Ok(())
    }

    /// Deserialize an argument from bytes (C++ compatible)
    pub fn deserialize(cursor: &mut Cursor<&[u8]>) -> NetworkResult<Self> {
        let type_byte = cursor.read_u8()?;
        let data_type = GameMessageArgumentDataType::from(type_byte);

        let value = match data_type {
            GameMessageArgumentDataType::Integer => {
                GameMessageArgumentValue::Integer(cursor.read_i32::<LittleEndian>()?)
            }
            GameMessageArgumentDataType::Real => {
                GameMessageArgumentValue::Real(cursor.read_f32::<LittleEndian>()?)
            }
            GameMessageArgumentDataType::Boolean => {
                let val = cursor.read_u32::<LittleEndian>()?;
                GameMessageArgumentValue::Boolean(val != 0)
            }
            GameMessageArgumentDataType::ObjectID => {
                GameMessageArgumentValue::ObjectID(cursor.read_u32::<LittleEndian>()?)
            }
            GameMessageArgumentDataType::DrawableID => {
                GameMessageArgumentValue::DrawableID(cursor.read_u32::<LittleEndian>()?)
            }
            GameMessageArgumentDataType::TeamID => {
                GameMessageArgumentValue::TeamID(cursor.read_u32::<LittleEndian>()?)
            }
            GameMessageArgumentDataType::Location => {
                let x = cursor.read_f32::<LittleEndian>()?;
                let y = cursor.read_f32::<LittleEndian>()?;
                let z = cursor.read_f32::<LittleEndian>()?;
                GameMessageArgumentValue::Location(Coord3D { x, y, z })
            }
            GameMessageArgumentDataType::Pixel => {
                let x = cursor.read_i32::<LittleEndian>()?;
                let y = cursor.read_i32::<LittleEndian>()?;
                GameMessageArgumentValue::Pixel(ICoord2D { x, y })
            }
            GameMessageArgumentDataType::PixelRegion => {
                // C++ serializes IRegion2D as lo.x, lo.y, hi.x, hi.y
                let lo_x = cursor.read_i32::<LittleEndian>()?;
                let lo_y = cursor.read_i32::<LittleEndian>()?;
                let hi_x = cursor.read_i32::<LittleEndian>()?;
                let hi_y = cursor.read_i32::<LittleEndian>()?;
                GameMessageArgumentValue::PixelRegion(IRegion2D {
                    lo: ICoord2D { x: lo_x, y: lo_y },
                    hi: ICoord2D { x: hi_x, y: hi_y },
                })
            }
            GameMessageArgumentDataType::Timestamp => {
                GameMessageArgumentValue::Timestamp(cursor.read_u32::<LittleEndian>()?)
            }
            GameMessageArgumentDataType::WideChar => {
                let val = cursor.read_u32::<LittleEndian>()?;
                GameMessageArgumentValue::WideChar(val as u16)
            }
            GameMessageArgumentDataType::Unknown => {
                return Err(NetworkError::invalid_packet(format!(
                    "unknown argument type: {}",
                    type_byte
                )));
            }
        };

        Ok(Self { data_type, value })
    }

    /// Deserialize argument value only (without type prefix)
    /// Used for grouped C++ format where type is known from header
    pub fn deserialize_value_only(
        data_type: GameMessageArgumentDataType,
        data: &[u8],
    ) -> NetworkResult<(Self, usize)> {
        let mut cursor = Cursor::new(data);
        #[allow(unused_assignments)]
        let mut bytes_read = 0;

        let value = match data_type {
            GameMessageArgumentDataType::Integer => {
                bytes_read = 4;
                GameMessageArgumentValue::Integer(cursor.read_i32::<LittleEndian>()?)
            }
            GameMessageArgumentDataType::Real => {
                bytes_read = 4;
                GameMessageArgumentValue::Real(cursor.read_f32::<LittleEndian>()?)
            }
            GameMessageArgumentDataType::Boolean => {
                bytes_read = 4;
                let val = cursor.read_u32::<LittleEndian>()?;
                GameMessageArgumentValue::Boolean(val != 0)
            }
            GameMessageArgumentDataType::ObjectID => {
                bytes_read = 4;
                GameMessageArgumentValue::ObjectID(cursor.read_u32::<LittleEndian>()?)
            }
            GameMessageArgumentDataType::DrawableID => {
                bytes_read = 4;
                GameMessageArgumentValue::DrawableID(cursor.read_u32::<LittleEndian>()?)
            }
            GameMessageArgumentDataType::TeamID => {
                bytes_read = 4;
                GameMessageArgumentValue::TeamID(cursor.read_u32::<LittleEndian>()?)
            }
            GameMessageArgumentDataType::Location => {
                bytes_read = 12;
                let x = cursor.read_f32::<LittleEndian>()?;
                let y = cursor.read_f32::<LittleEndian>()?;
                let z = cursor.read_f32::<LittleEndian>()?;
                GameMessageArgumentValue::Location(Coord3D { x, y, z })
            }
            GameMessageArgumentDataType::Pixel => {
                bytes_read = 8;
                let x = cursor.read_i32::<LittleEndian>()?;
                let y = cursor.read_i32::<LittleEndian>()?;
                GameMessageArgumentValue::Pixel(ICoord2D { x, y })
            }
            GameMessageArgumentDataType::PixelRegion => {
                bytes_read = 16;
                // C++ serializes IRegion2D as lo.x, lo.y, hi.x, hi.y
                let lo_x = cursor.read_i32::<LittleEndian>()?;
                let lo_y = cursor.read_i32::<LittleEndian>()?;
                let hi_x = cursor.read_i32::<LittleEndian>()?;
                let hi_y = cursor.read_i32::<LittleEndian>()?;
                GameMessageArgumentValue::PixelRegion(IRegion2D {
                    lo: ICoord2D { x: lo_x, y: lo_y },
                    hi: ICoord2D { x: hi_x, y: hi_y },
                })
            }
            GameMessageArgumentDataType::Timestamp => {
                bytes_read = 4;
                GameMessageArgumentValue::Timestamp(cursor.read_u32::<LittleEndian>()?)
            }
            GameMessageArgumentDataType::WideChar => {
                bytes_read = 4;
                let val = cursor.read_u32::<LittleEndian>()?;
                GameMessageArgumentValue::WideChar(val as u16)
            }
            GameMessageArgumentDataType::Unknown => {
                return Err(NetworkError::invalid_packet("unknown argument type"));
            }
        };

        Ok((Self { data_type, value }, bytes_read))
    }
}

/// Helper structure for C++ grouped argument encoding
/// Groups arguments by consecutive type runs (matches C++ GameMessageParser)
#[derive(Debug, Clone)]
pub struct GameMessageArgs {
    /// Groups arguments by type: (type_id, vec_of_values)
    pub groups: Vec<(GameMessageArgumentDataType, Vec<GameMessageArgument>)>,
}

impl GameMessageArgs {
    /// Create from a flat list of arguments
    pub fn from_arguments(arguments: Vec<GameMessageArgument>) -> Self {
        let mut groups: Vec<(GameMessageArgumentDataType, Vec<GameMessageArgument>)> = Vec::new();

        // Group arguments by consecutive runs of the same type
        for arg in arguments {
            match groups.last_mut() {
                Some((last_type, last_group)) if *last_type == arg.data_type => {
                    last_group.push(arg);
                }
                _ => {
                    groups.push((arg.data_type, vec![arg]));
                }
            }
        }

        Self { groups }
    }

    /// Flatten grouped arguments back to vector
    pub fn to_arguments(self) -> Vec<GameMessageArgument> {
        let mut arguments = Vec::new();
        for (_, group) in self.groups {
            arguments.extend(group);
        }
        arguments
    }

    /// Encode to C++ grouped format
    pub fn encode_grouped(&self) -> NetworkResult<Vec<u8>> {
        let mut buf = Vec::new();

        // Write number of type groups
        buf.write_u8(self.groups.len() as u8)?;

        // Write type headers
        for (type_id, args) in &self.groups {
            buf.write_u8(*type_id as u8)?;
            buf.write_u8(args.len() as u8)?;
        }

        // Write all values in group order (no type prefixes)
        for (_, args) in &self.groups {
            for arg in args {
                arg.serialize_value_only(&mut buf)?;
            }
        }

        Ok(buf)
    }

    /// Parse C++ grouped format
    pub fn decode_grouped(data: &[u8]) -> NetworkResult<Self> {
        let mut cursor = Cursor::new(data);
        let mut groups = Vec::new();

        // Read number of type groups
        let num_types = cursor.read_u8()? as usize;

        // Read type headers
        let mut type_headers: Vec<(GameMessageArgumentDataType, u8)> = Vec::new();
        for _ in 0..num_types {
            let type_id = cursor.read_u8()?;
            let count = cursor.read_u8()?;
            let data_type = GameMessageArgumentDataType::from(type_id);
            type_headers.push((data_type, count));
        }

        // Read values for each type group
        let pos = cursor.position() as usize;
        let remaining = &data[pos..];
        let mut value_offset = 0;

        for (data_type, count) in type_headers {
            let mut group_values = Vec::with_capacity(count as usize);

            for _ in 0..count {
                let (arg, consumed) = GameMessageArgument::deserialize_value_only(
                    data_type,
                    &remaining[value_offset..],
                )?;
                group_values.push(arg);
                value_offset += consumed;
            }

            groups.push((data_type, group_values));
        }

        Ok(GameMessageArgs { groups })
    }
}

/// Game message structure matching C++ GameMessage class
#[derive(Debug, Clone)]
pub struct GameMessage {
    /// Message type (from GameMessage::Type enum)
    pub message_type: u32,
    /// Player index who issued this command
    pub player_index: i32,
    /// List of typed arguments
    pub arguments: Vec<GameMessageArgument>,
}

impl GameMessage {
    /// Create a new game message
    pub fn new(message_type: u32, player_index: i32) -> Self {
        Self {
            message_type,
            player_index,
            arguments: Vec::new(),
        }
    }

    /// Add an integer argument
    pub fn add_integer(&mut self, value: i32) {
        self.arguments
            .push(GameMessageArgument::new(GameMessageArgumentValue::Integer(
                value,
            )));
    }

    /// Add a real (float) argument
    pub fn add_real(&mut self, value: f32) {
        self.arguments
            .push(GameMessageArgument::new(GameMessageArgumentValue::Real(
                value,
            )));
    }

    /// Add a boolean argument
    pub fn add_boolean(&mut self, value: bool) {
        self.arguments
            .push(GameMessageArgument::new(GameMessageArgumentValue::Boolean(
                value,
            )));
    }

    /// Add an object ID argument
    pub fn add_object_id(&mut self, value: u32) {
        self.arguments.push(GameMessageArgument::new(
            GameMessageArgumentValue::ObjectID(value),
        ));
    }

    /// Add a drawable ID argument
    pub fn add_drawable_id(&mut self, value: u32) {
        self.arguments.push(GameMessageArgument::new(
            GameMessageArgumentValue::DrawableID(value),
        ));
    }

    /// Add a team ID argument
    pub fn add_team_id(&mut self, value: u32) {
        self.arguments
            .push(GameMessageArgument::new(GameMessageArgumentValue::TeamID(
                value,
            )));
    }

    /// Add a location (3D coordinate) argument
    pub fn add_location(&mut self, location: Coord3D) {
        self.arguments.push(GameMessageArgument::new(
            GameMessageArgumentValue::Location(location),
        ));
    }

    /// Add a pixel (2D coordinate) argument
    pub fn add_pixel(&mut self, pixel: ICoord2D) {
        self.arguments
            .push(GameMessageArgument::new(GameMessageArgumentValue::Pixel(
                pixel,
            )));
    }

    /// Add a pixel region argument
    pub fn add_pixel_region(&mut self, region: IRegion2D) {
        self.arguments.push(GameMessageArgument::new(
            GameMessageArgumentValue::PixelRegion(region),
        ));
    }

    /// Add a timestamp argument
    pub fn add_timestamp(&mut self, value: u32) {
        self.arguments.push(GameMessageArgument::new(
            GameMessageArgumentValue::Timestamp(value),
        ));
    }

    /// Add a wide character argument
    pub fn add_wide_char(&mut self, value: u16) {
        self.arguments.push(GameMessageArgument::new(
            GameMessageArgumentValue::WideChar(value),
        ));
    }

    /// Get the number of arguments
    pub fn argument_count(&self) -> usize {
        self.arguments.len()
    }

    /// Calculate total size of arguments in bytes
    pub fn arguments_size(&self) -> usize {
        self.arguments
            .iter()
            .map(|arg| 1 + arg.value.size_in_bytes()) // 1 byte for type + data size
            .sum()
    }

    /// Serialize this game message to bytes (C++ NetGameCommandMsg wire format)
    pub fn serialize(&self) -> NetworkResult<Vec<u8>> {
        let mut buffer = Vec::with_capacity(256);

        // Write message type (u32)
        buffer.write_u32::<LittleEndian>(self.message_type)?;

        // Write argument count (u32)
        buffer.write_u32::<LittleEndian>(self.arguments.len() as u32)?;

        // Write each argument
        for arg in &self.arguments {
            arg.serialize(&mut buffer)?;
        }

        trace!(
            "Serialized GameMessage type {} with {} arguments ({} bytes)",
            self.message_type,
            self.arguments.len(),
            buffer.len()
        );

        Ok(buffer)
    }

    /// Deserialize a game message from bytes (C++ NetGameCommandMsg wire format)
    pub fn deserialize(data: &[u8]) -> NetworkResult<Self> {
        if data.len() < 8 {
            return Err(NetworkError::invalid_packet(format!(
                "GameMessage too short: {} bytes (minimum 8)",
                data.len()
            )));
        }

        let mut cursor = Cursor::new(data);

        // Read message type (u32)
        let message_type = cursor.read_u32::<LittleEndian>()?;

        // Read argument count (u32)
        let arg_count = cursor.read_u32::<LittleEndian>()?;

        // Sanity check
        if arg_count > 256 {
            return Err(NetworkError::invalid_packet(format!(
                "GameMessage argument count too large: {}",
                arg_count
            )));
        }

        // Read each argument
        let mut arguments = Vec::with_capacity(arg_count as usize);
        for _ in 0..arg_count {
            arguments.push(GameMessageArgument::deserialize(&mut cursor)?);
        }

        trace!(
            "Deserialized GameMessage type {} with {} arguments",
            message_type,
            arg_count
        );

        Ok(Self {
            message_type,
            player_index: 0, // Player index comes from NetCommandMsg header
            arguments,
        })
    }

    /// Validate this game message
    pub fn validate(&self) -> NetworkResult<()> {
        // Check argument count is reasonable
        if self.arguments.len() > 255 {
            return Err(NetworkError::invalid_command(format!(
                "too many arguments: {}",
                self.arguments.len()
            )));
        }

        // Check total size doesn't exceed reasonable limits
        let total_size = self.arguments_size();
        if total_size > 4096 {
            return Err(NetworkError::invalid_command(format!(
                "arguments too large: {} bytes",
                total_size
            )));
        }

        Ok(())
    }

    /// Serialize this game message to bytes using C++ grouped format
    /// This format uses run-length groups that match C++ GameMessageParser
    pub fn serialize_cpp_compatible(&self) -> NetworkResult<Vec<u8>> {
        let mut buffer = Vec::with_capacity(256);

        // Write message type (u32)
        buffer.write_u32::<LittleEndian>(self.message_type)?;

        // Group arguments by type
        let args = GameMessageArgs::from_arguments(self.arguments.clone());

        // Encode using grouped format
        let grouped_bytes = args.encode_grouped()?;
        buffer.extend(grouped_bytes);

        trace!(
            "Serialized GameMessage type {} with {} argument groups ({} bytes) using C++ format",
            self.message_type,
            args.groups.len(),
            buffer.len()
        );

        Ok(buffer)
    }

    /// Deserialize a game message from bytes using C++ grouped format
    pub fn deserialize_cpp_compatible(data: &[u8]) -> NetworkResult<Self> {
        if data.len() < 5 {
            return Err(NetworkError::invalid_packet(format!(
                "GameMessage too short: {} bytes (minimum 5 for grouped format)",
                data.len()
            )));
        }

        let mut cursor = Cursor::new(data);

        // Read message type (u32)
        let message_type = cursor.read_u32::<LittleEndian>()?;

        // Read grouped arguments
        let pos = cursor.position() as usize;
        let remaining = &data[pos..];
        let args = GameMessageArgs::decode_grouped(remaining)?;

        // Flatten grouped arguments back to vector
        let arguments = args.to_arguments();

        trace!(
            "Deserialized GameMessage type {} with {} arguments using C++ format",
            message_type,
            arguments.len()
        );

        Ok(Self {
            message_type,
            player_index: 0, // Player index comes from NetCommandMsg header
            arguments,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_argument_serialization_integer() {
        let arg = GameMessageArgument::new(GameMessageArgumentValue::Integer(42));
        let mut buffer = Vec::new();
        arg.serialize(&mut buffer).unwrap();

        // Should be: type byte (0) + i32 value (42)
        assert_eq!(buffer.len(), 5);
        assert_eq!(buffer[0], 0); // Integer type

        let mut cursor = Cursor::new(buffer.as_slice());
        let deserialized = GameMessageArgument::deserialize(&mut cursor).unwrap();

        if let GameMessageArgumentValue::Integer(val) = deserialized.value {
            assert_eq!(val, 42);
        } else {
            panic!("Expected Integer value");
        }
    }

    #[test]
    fn test_argument_serialization_real() {
        let arg = GameMessageArgument::new(GameMessageArgumentValue::Real(3.14159));
        let mut buffer = Vec::new();
        arg.serialize(&mut buffer).unwrap();

        assert_eq!(buffer.len(), 5); // type + f32
        assert_eq!(buffer[0], 1); // Real type

        let mut cursor = Cursor::new(buffer.as_slice());
        let deserialized = GameMessageArgument::deserialize(&mut cursor).unwrap();

        if let GameMessageArgumentValue::Real(val) = deserialized.value {
            assert!((val - 3.14159).abs() < 0.0001);
        } else {
            panic!("Expected Real value");
        }
    }

    #[test]
    fn test_argument_serialization_boolean() {
        let arg = GameMessageArgument::new(GameMessageArgumentValue::Boolean(true));
        let mut buffer = Vec::new();
        arg.serialize(&mut buffer).unwrap();

        assert_eq!(buffer.len(), 5); // type + 4-byte padded bool
        assert_eq!(buffer[0], 2); // Boolean type

        let mut cursor = Cursor::new(buffer.as_slice());
        let deserialized = GameMessageArgument::deserialize(&mut cursor).unwrap();

        if let GameMessageArgumentValue::Boolean(val) = deserialized.value {
            assert!(val);
        } else {
            panic!("Expected Boolean value");
        }
    }

    #[test]
    fn test_argument_serialization_location() {
        let location = Coord3D {
            x: 10.0,
            y: 20.0,
            z: 5.0,
        };
        let arg = GameMessageArgument::new(GameMessageArgumentValue::Location(location.clone()));
        let mut buffer = Vec::new();
        arg.serialize(&mut buffer).unwrap();

        assert_eq!(buffer.len(), 13); // type + 3*f32
        assert_eq!(buffer[0], 6); // Location type

        let mut cursor = Cursor::new(buffer.as_slice());
        let deserialized = GameMessageArgument::deserialize(&mut cursor).unwrap();

        if let GameMessageArgumentValue::Location(coord) = deserialized.value {
            assert_eq!(coord.x, 10.0);
            assert_eq!(coord.y, 20.0);
            assert_eq!(coord.z, 5.0);
        } else {
            panic!("Expected Location value");
        }
    }

    #[test]
    fn test_game_message_serialization() {
        let mut msg = GameMessage::new(1000, 0);
        msg.add_integer(42);
        msg.add_real(3.14);
        msg.add_boolean(true);
        msg.add_location(Coord3D {
            x: 10.0,
            y: 20.0,
            z: 0.0,
        });

        let serialized = msg.serialize().unwrap();

        // Deserialize and verify
        let deserialized = GameMessage::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.message_type, 1000);
        assert_eq!(deserialized.arguments.len(), 4);

        // Verify first argument (integer)
        if let GameMessageArgumentValue::Integer(val) = &deserialized.arguments[0].value {
            assert_eq!(*val, 42);
        } else {
            panic!("Expected Integer argument");
        }

        // Verify second argument (real)
        if let GameMessageArgumentValue::Real(val) = &deserialized.arguments[1].value {
            assert!((val - 3.14).abs() < 0.01);
        } else {
            panic!("Expected Real argument");
        }
    }

    #[test]
    fn test_game_message_empty() {
        let msg = GameMessage::new(500, 1);
        let serialized = msg.serialize().unwrap();

        // Should just be type + count (both u32)
        assert_eq!(serialized.len(), 8);

        let deserialized = GameMessage::deserialize(&serialized).unwrap();
        assert_eq!(deserialized.message_type, 500);
        assert_eq!(deserialized.arguments.len(), 0);
    }

    #[test]
    fn test_game_message_validation() {
        let msg = GameMessage::new(1000, 0);
        assert!(msg.validate().is_ok());

        // Test excessive arguments
        let mut big_msg = GameMessage::new(2000, 0);
        for _ in 0..300 {
            big_msg.add_integer(0);
        }
        assert!(big_msg.validate().is_err());
    }

    #[test]
    fn test_round_trip_all_types() {
        let mut msg = GameMessage::new(1234, 0);

        msg.add_integer(-999);
        msg.add_real(123.456);
        msg.add_boolean(false);
        msg.add_object_id(777);
        msg.add_drawable_id(888);
        msg.add_team_id(5);
        msg.add_location(Coord3D {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        });
        msg.add_pixel(ICoord2D { x: 100, y: 200 });
        msg.add_pixel_region(IRegion2D::new(10, 20, 40, 60));
        msg.add_timestamp(12345678);
        msg.add_wide_char(0x0041); // 'A'

        let serialized = msg.serialize().unwrap();
        let deserialized = GameMessage::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.message_type, msg.message_type);
        assert_eq!(deserialized.arguments.len(), msg.arguments.len());

        // Verify each argument type preserved
        for (orig, deser) in msg.arguments.iter().zip(deserialized.arguments.iter()) {
            assert_eq!(orig.data_type, deser.data_type);
        }
    }

    #[test]
    fn test_cpp_grouped_encoding_mixed_types() {
        // Create message with mixed types
        let mut msg = GameMessage::new(0x1234, 0);
        msg.add_integer(100);
        msg.add_real(3.14);
        msg.add_integer(200);
        msg.add_boolean(true);
        msg.add_real(2.71);

        let encoded = msg.serialize_cpp_compatible().unwrap();

        // Verify structure:
        // [type: u32=0x1234] [num_groups=5] [INT,1] [REAL,1] [INT,1] [BOOL,1] [REAL,1] [values in order]
        assert_eq!(&encoded[0..4], &[0x34, 0x12, 0x00, 0x00]); // Message type LE
        assert_eq!(encoded[4], 5); // 5 type groups

        // Type headers should preserve run order
        assert_eq!(encoded[5], GameMessageArgumentDataType::Integer as u8);
        assert_eq!(encoded[6], 1); // 1 integer

        assert_eq!(encoded[7], GameMessageArgumentDataType::Real as u8);
        assert_eq!(encoded[8], 1); // 1 real

        assert_eq!(encoded[9], GameMessageArgumentDataType::Integer as u8);
        assert_eq!(encoded[10], 1); // 1 integer

        assert_eq!(encoded[11], GameMessageArgumentDataType::Boolean as u8);
        assert_eq!(encoded[12], 1); // 1 boolean

        assert_eq!(encoded[13], GameMessageArgumentDataType::Real as u8);
        assert_eq!(encoded[14], 1); // 1 real
    }

    #[test]
    fn test_cpp_grouped_encoding_round_trip() {
        let mut msg = GameMessage::new(5000, 0);
        msg.add_integer(42);
        msg.add_integer(84);
        msg.add_integer(126);
        msg.add_real(1.0);
        msg.add_real(2.0);
        msg.add_boolean(false);
        msg.add_location(Coord3D {
            x: 10.0,
            y: 20.0,
            z: 30.0,
        });

        let encoded = msg.serialize_cpp_compatible().unwrap();
        let decoded = GameMessage::deserialize_cpp_compatible(&encoded).unwrap();

        assert_eq!(decoded.message_type, msg.message_type);
        assert_eq!(decoded.arguments.len(), msg.arguments.len());

        // Run-length grouping preserves argument order
        for (orig, deser) in msg.arguments.iter().zip(decoded.arguments.iter()) {
            assert_eq!(orig.data_type, deser.data_type);
        }
    }

    #[test]
    fn test_cpp_grouped_encoding_single_type() {
        let mut msg = GameMessage::new(999, 0);
        msg.add_integer(1);
        msg.add_integer(2);
        msg.add_integer(3);

        let encoded = msg.serialize_cpp_compatible().unwrap();

        // [type: u32] [num_groups=1] [INT,3] [1, 2, 3]
        assert_eq!(encoded[4], 1); // 1 type group
        assert_eq!(encoded[5], GameMessageArgumentDataType::Integer as u8);
        assert_eq!(encoded[6], 3); // 3 values
    }

    #[test]
    fn test_cpp_grouped_encoding_empty() {
        let msg = GameMessage::new(777, 0);

        let encoded = msg.serialize_cpp_compatible().unwrap();
        let decoded = GameMessage::deserialize_cpp_compatible(&encoded).unwrap();

        assert_eq!(decoded.message_type, 777);
        assert_eq!(decoded.arguments.len(), 0);
    }

    #[test]
    fn test_game_message_args_grouping() {
        let mut args = Vec::new();
        args.push(GameMessageArgument::new(GameMessageArgumentValue::Integer(
            1,
        )));
        args.push(GameMessageArgument::new(GameMessageArgumentValue::Real(
            2.0,
        )));
        args.push(GameMessageArgument::new(GameMessageArgumentValue::Integer(
            3,
        )));

        let grouped = GameMessageArgs::from_arguments(args.clone());

        // Should have 3 groups (Integer, Real, Integer)
        assert_eq!(grouped.groups.len(), 3);
        assert_eq!(grouped.groups[0].0, GameMessageArgumentDataType::Integer);
        assert_eq!(grouped.groups[0].1.len(), 1);
        assert_eq!(grouped.groups[1].0, GameMessageArgumentDataType::Real);
        assert_eq!(grouped.groups[1].1.len(), 1);
        assert_eq!(grouped.groups[2].0, GameMessageArgumentDataType::Integer);
        assert_eq!(grouped.groups[2].1.len(), 1);
    }

    #[test]
    fn test_argument_value_only_serialization() {
        let arg = GameMessageArgument::new(GameMessageArgumentValue::Integer(42));
        let mut buffer = Vec::new();

        // Serialize value only (without type)
        arg.serialize_value_only(&mut buffer).unwrap();

        // Should be 4 bytes (just the i32 value)
        assert_eq!(buffer.len(), 4);
        assert_eq!(
            i32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]),
            42
        );
    }

    #[test]
    fn test_argument_value_only_deserialization() {
        let data = vec![100, 0, 0, 0]; // i32 value 100 in LE

        let (arg, consumed) = GameMessageArgument::deserialize_value_only(
            GameMessageArgumentDataType::Integer,
            &data,
        )
        .unwrap();

        assert_eq!(consumed, 4);
        if let GameMessageArgumentValue::Integer(val) = arg.value {
            assert_eq!(val, 100);
        } else {
            panic!("Expected Integer value");
        }
    }
}
