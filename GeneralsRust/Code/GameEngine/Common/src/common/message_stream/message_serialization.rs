#![allow(missing_docs)]

//! Message Serialization and Network Integration
//!
//! This module provides serialization/deserialization capabilities for GameMessages
//! to support network transmission and replay recording.

use super::game_message::*;
use log::{debug, error, warn};
use std::io::{self, Read, Write};

/// Error types for message serialization
#[derive(Debug)]
pub enum SerializationError {
    IoError(io::Error),
    InvalidMessageType(u16),
    InvalidArgumentType(u8),
    BufferTooSmall,
    CorruptedData,
    UnsupportedVersion(u8),
}

impl std::fmt::Display for SerializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SerializationError::IoError(e) => write!(f, "IO error: {}", e),
            SerializationError::InvalidMessageType(t) => write!(f, "Invalid message type: {}", t),
            SerializationError::InvalidArgumentType(t) => write!(f, "Invalid argument type: {}", t),
            SerializationError::BufferTooSmall => write!(f, "Buffer too small"),
            SerializationError::CorruptedData => write!(f, "Corrupted data"),
            SerializationError::UnsupportedVersion(v) => write!(f, "Unsupported version: {}", v),
        }
    }
}

impl std::error::Error for SerializationError {}

impl From<io::Error> for SerializationError {
    fn from(err: io::Error) -> Self {
        SerializationError::IoError(err)
    }
}

/// Message serialization protocol version
const SERIALIZATION_VERSION: u8 = 1;
const NETWORK_COMMAND_ID_START: u16 = 79;
const NETWORK_COMMAND_ID_END_EXCLUSIVE: u16 = 149;

/// Message header structure
#[derive(Debug, Clone)]
pub struct MessageHeader {
    pub version: u8,
    pub message_type_id: u16,
    pub player_index: i32,
    pub argument_count: u8,
    pub message_size: u32, // Total size including header
}

impl MessageHeader {
    /// Size of the header in bytes
    pub const SIZE: usize = 12;

    pub fn new(
        message_type_id: u16,
        player_index: i32,
        argument_count: u8,
        message_size: u32,
    ) -> Self {
        Self {
            version: SERIALIZATION_VERSION,
            message_type_id,
            player_index,
            argument_count,
            message_size,
        }
    }

    /// Serialize header to bytes
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0] = self.version;
        bytes[1..3].copy_from_slice(&self.message_type_id.to_le_bytes());
        bytes[3..7].copy_from_slice(&self.player_index.to_le_bytes());
        bytes[7] = self.argument_count;
        bytes[8..12].copy_from_slice(&self.message_size.to_le_bytes());
        bytes
    }

    /// Deserialize header from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SerializationError> {
        if bytes.len() < Self::SIZE {
            return Err(SerializationError::BufferTooSmall);
        }

        let version = bytes[0];
        if version != SERIALIZATION_VERSION {
            return Err(SerializationError::UnsupportedVersion(version));
        }

        let message_type_id = u16::from_le_bytes([bytes[1], bytes[2]]);
        let player_index = i32::from_le_bytes([bytes[3], bytes[4], bytes[5], bytes[6]]);
        let argument_count = bytes[7];
        let message_size = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);

        Ok(Self {
            version,
            message_type_id,
            player_index,
            argument_count,
            message_size,
        })
    }
}

/// Message serialization utilities
pub struct MessageSerializer;

/// Returns true if the message is inside the replay/network command band.
///
/// C++ parity is `MSG_BEGIN_NETWORK_MESSAGES < type < MSG_END_NETWORK_MESSAGES`.
/// In Rust serialization IDs this corresponds to [79, 149).
pub fn is_network_command_message(message_type: &GameMessageType) -> bool {
    MessageSerializer::get_message_type_id(message_type)
        .map(|id| (NETWORK_COMMAND_ID_START..NETWORK_COMMAND_ID_END_EXCLUSIVE).contains(&id))
        .unwrap_or(false)
}

impl MessageSerializer {
    pub(crate) fn encode_message_arguments(
        message_type: &GameMessageType,
    ) -> Vec<GameMessageArgumentType> {
        use GameMessageType::*;
        match message_type {
            FrameTick(frame) => vec![GameMessageArgumentType::Timestamp(*frame)],
            AreaSelection(region) => vec![GameMessageArgumentType::PixelRegion(region.clone())],
            DoMoveTo(coord) | DoForceMoveTO(coord) | DoAttackMoveTo(coord) | AddWaypoint(coord) => {
                vec![GameMessageArgumentType::Location(coord.clone())]
            }
            SetRallyPoint(unit, coord) => vec![
                GameMessageArgumentType::ObjectID(*unit),
                GameMessageArgumentType::Location(coord.clone()),
            ],
            DoForceAttackGround(coord) => {
                vec![GameMessageArgumentType::Location(coord.clone())]
            }
            DoGuardPosition(coord, guard_mode) => vec![
                GameMessageArgumentType::Location(coord.clone()),
                GameMessageArgumentType::Integer(*guard_mode),
            ],
            DoAttackObject(target) | DoForceAttackObject(target) => {
                vec![GameMessageArgumentType::ObjectID(*target)]
            }
            DoGuardObject(target, guard_mode) => vec![
                GameMessageArgumentType::ObjectID(*target),
                GameMessageArgumentType::Integer(*guard_mode),
            ],
            DozerCancelConstruct(unit) | Sell(unit) => {
                vec![GameMessageArgumentType::ObjectID(*unit)]
            }
            CombatDropAtObject(target) => vec![GameMessageArgumentType::ObjectID(*target)],
            DozerConstruct(building_type, coord, angle) => vec![
                GameMessageArgumentType::Integer(*building_type as i32),
                GameMessageArgumentType::Location(coord.clone()),
                GameMessageArgumentType::Real(*angle),
            ],
            DozerConstructLine(building_type, start, end, angle) => vec![
                GameMessageArgumentType::Integer(*building_type as i32),
                GameMessageArgumentType::Location(start.clone()),
                GameMessageArgumentType::Real(*angle),
                GameMessageArgumentType::Location(end.clone()),
            ],
            DoAttackSquad(units) => units
                .iter()
                .map(|unit| GameMessageArgumentType::ObjectID(*unit))
                .collect(),
            DoWeapon(weapon_id)
            | PurchaseScience(weapon_id)
            | QueueUpgrade(weapon_id)
            | CancelUpgrade(weapon_id)
            | QueueUnitCreate(weapon_id)
            | CancelUnitCreate(weapon_id) => {
                vec![GameMessageArgumentType::Integer(*weapon_id as i32)]
            }
            DoSpecialPower(power_id, options, source) => vec![
                GameMessageArgumentType::Integer(*power_id as i32),
                GameMessageArgumentType::Integer(*options as i32),
                GameMessageArgumentType::ObjectID(*source),
            ],
            DoWeaponAtLocation(weapon_id, coord) => {
                vec![
                    GameMessageArgumentType::Integer(*weapon_id as i32),
                    GameMessageArgumentType::Location(coord.clone()),
                ]
            }
            DoSpecialPowerAtLocation(power_id, coord, angle, object_in_way, options, source) => {
                vec![
                    GameMessageArgumentType::Integer(*power_id as i32),
                    GameMessageArgumentType::Location(coord.clone()),
                    GameMessageArgumentType::Real(*angle),
                    GameMessageArgumentType::ObjectID(*object_in_way),
                    GameMessageArgumentType::Integer(*options as i32),
                    GameMessageArgumentType::ObjectID(*source),
                ]
            }
            CombatDropAtLocation(coord) => {
                vec![GameMessageArgumentType::Location(coord.clone())]
            }
            Exit(unit) => vec![GameMessageArgumentType::ObjectID(*unit)],
            EvacuateAtLocation(coord) => vec![GameMessageArgumentType::Location(coord.clone())],
            GetRepaired(target)
            | GetHealed(target)
            | DoRepair(target)
            | ResumeConstruction(target)
            | Dock(target) => vec![GameMessageArgumentType::ObjectID(*target)],
            Enter(unit, facility) => vec![
                GameMessageArgumentType::ObjectID(*unit),
                GameMessageArgumentType::ObjectID(*facility),
            ],
            DoSalvage(coord) => vec![GameMessageArgumentType::Location(coord.clone())],
            DoWeaponAtObject(weapon_id, target) => {
                vec![
                    GameMessageArgumentType::Integer(*weapon_id as i32),
                    GameMessageArgumentType::ObjectID(*target),
                ]
            }
            DoSpecialPowerAtObject(power_id, target, options, source) => vec![
                GameMessageArgumentType::Integer(*power_id as i32),
                GameMessageArgumentType::ObjectID(*target),
                GameMessageArgumentType::Integer(*options as i32),
                GameMessageArgumentType::ObjectID(*source),
            ],
            SetBeaconText(coord, text) => vec![
                GameMessageArgumentType::Location(coord.clone()),
                GameMessageArgumentType::String(text.clone()),
            ],
            PlaceBeacon(coord) | RemoveBeacon(coord) => {
                vec![GameMessageArgumentType::Location(coord.clone())]
            }
            DoSpecialPowerOverrideDestination(coord, power_type, source) => vec![
                GameMessageArgumentType::Location(coord.clone()),
                GameMessageArgumentType::Integer(*power_type as i32),
                GameMessageArgumentType::ObjectID(*source),
            ],
            SetReplayCamera(coord, pitch, zoom) => vec![
                GameMessageArgumentType::Location(coord.clone()),
                GameMessageArgumentType::Real(*pitch),
                GameMessageArgumentType::Real(*zoom),
            ],
            SelfDestruct(player_id) => vec![GameMessageArgumentType::Integer(*player_id as i32)],
            CreateFormation(units) => units
                .iter()
                .map(|unit| GameMessageArgumentType::ObjectID(*unit))
                .collect(),
            LogicCRC(crc) => vec![GameMessageArgumentType::Integer(*crc as i32)],
            SetMineClearingDetail(detail) => vec![GameMessageArgumentType::Integer(*detail as i32)],
            EnableRetaliationMode(player_id, enabled) => vec![
                GameMessageArgumentType::Integer(*player_id as i32),
                GameMessageArgumentType::Boolean(*enabled),
            ],
            InternetHack | DoCheer | ToggleOvercharge => Vec::new(),
            SwitchWeapons(slot) => {
                vec![GameMessageArgumentType::Integer(*slot as i32)]
            }
            ConvertToCarbomb(unit, target)
            | CaptureBuilding(unit, target)
            | DisableVehicleHack(unit, target)
            | StealCashHack(unit, target)
            | DisableBuildingHack(unit, target)
            | SnipeVehicle(unit, target) => vec![
                GameMessageArgumentType::ObjectID(*unit),
                GameMessageArgumentType::ObjectID(*target),
            ],
            _ => Vec::new(),
        }
    }

    pub(crate) fn decode_message_type(
        id: u16,
        args: &[GameMessageArgumentType],
    ) -> Result<(GameMessageType, usize), SerializationError> {
        use GameMessageType::*;

        struct ArgReader<'a> {
            args: &'a [GameMessageArgumentType],
            index: usize,
        }

        impl<'a> ArgReader<'a> {
            fn new(args: &'a [GameMessageArgumentType]) -> Self {
                Self { args, index: 0 }
            }

            fn read<T, F>(&mut self, convert: F) -> Result<T, SerializationError>
            where
                F: FnOnce(&GameMessageArgumentType) -> Option<T>,
            {
                let arg = self
                    .args
                    .get(self.index)
                    .ok_or(SerializationError::CorruptedData)?;
                self.index += 1;
                convert(arg).ok_or(SerializationError::InvalidArgumentType(self.index as u8))
            }

            fn read_object_id(&mut self) -> Result<u32, SerializationError> {
                self.read(|arg| match arg {
                    GameMessageArgumentType::ObjectID(value) => Some(*value),
                    _ => None,
                })
            }

            fn read_location(&mut self) -> Result<Coord3D, SerializationError> {
                self.read(|arg| match arg {
                    GameMessageArgumentType::Location(value) => Some(value.clone()),
                    _ => None,
                })
            }

            fn read_region(&mut self) -> Result<IRegion2D, SerializationError> {
                self.read(|arg| match arg {
                    GameMessageArgumentType::PixelRegion(value) => Some(value.clone()),
                    _ => None,
                })
            }

            fn read_timestamp(&mut self) -> Result<u32, SerializationError> {
                self.read(|arg| match arg {
                    GameMessageArgumentType::Timestamp(value) => Some(*value),
                    _ => None,
                })
            }

            fn read_int(&mut self) -> Result<i32, SerializationError> {
                self.read(|arg| match arg {
                    GameMessageArgumentType::Integer(value) => Some(*value),
                    _ => None,
                })
            }

            fn read_real(&mut self) -> Result<f32, SerializationError> {
                self.read(|arg| match arg {
                    GameMessageArgumentType::Real(value) => Some(*value),
                    _ => None,
                })
            }

            fn read_bool(&mut self) -> Result<bool, SerializationError> {
                self.read(|arg| match arg {
                    GameMessageArgumentType::Boolean(value) => Some(*value),
                    _ => None,
                })
            }

            fn read_string(&mut self) -> Result<String, SerializationError> {
                self.read(|arg| match arg {
                    GameMessageArgumentType::String(value) => Some(value.clone()),
                    _ => None,
                })
            }

            fn read_remaining_object_ids(&mut self) -> Result<Vec<ObjectID>, SerializationError> {
                let mut values = Vec::new();
                while self.index < self.args.len() {
                    match self.args.get(self.index) {
                        Some(GameMessageArgumentType::ObjectID(value)) => values.push(*value),
                        _ => {
                            return Err(SerializationError::InvalidArgumentType(self.index as u8));
                        }
                    }
                    self.index += 1;
                }
                Ok(values)
            }

            fn consumed(&self) -> usize {
                self.index
            }
        }

        let mut reader = ArgReader::new(args);
        let message_type = match id {
            0 => Invalid,
            1 => FrameTick(reader.read_timestamp()?),
            25 => ClearGameData,
            26 => NewGame,
            87 => {
                let units = reader.read_remaining_object_ids()?;
                DoAttackSquad(units)
            }
            104 => Exit(reader.read_object_id()?),
            105 => {
                if reader.index < reader.args.len() {
                    let coord = reader.read_location()?;
                    EvacuateAtLocation(coord)
                } else {
                    Evacuate
                }
            }
            106 => ExecuteRailedTransport,
            88 => DoWeapon(reader.read_int()? as u32),
            89 => {
                let weapon = reader.read_int()? as u32;
                let coord = reader.read_location()?;
                DoWeaponAtLocation(weapon, coord)
            }
            90 => {
                let weapon = reader.read_int()? as u32;
                let target = reader.read_object_id()?;
                DoWeaponAtObject(weapon, target)
            }
            91 => {
                let power = reader.read_int()? as u32;
                let options = reader.read_int()? as u32;
                let source = reader.read_object_id()?;
                DoSpecialPower(power, options, source)
            }
            92 => {
                let power = reader.read_int()? as u32;
                let coord = reader.read_location()?;
                let angle = reader.read_real()?;
                let object_in_way = reader.read_object_id()?;
                let options = reader.read_int()? as u32;
                let source = reader.read_object_id()?;
                DoSpecialPowerAtLocation(power, coord, angle, object_in_way, options, source)
            }
            93 => {
                let power = reader.read_int()? as u32;
                let target = reader.read_object_id()?;
                let options = reader.read_int()? as u32;
                let source = reader.read_object_id()?;
                DoSpecialPowerAtObject(power, target, options, source)
            }
            94 => {
                let unit = reader.read_object_id()?;
                let coord = reader.read_location()?;
                SetRallyPoint(unit, coord)
            }
            95 => PurchaseScience(reader.read_int()? as u32),
            96 => QueueUpgrade(reader.read_int()? as u32),
            97 => CancelUpgrade(reader.read_int()? as u32),
            98 => QueueUnitCreate(reader.read_int()? as u32),
            99 => CancelUnitCreate(reader.read_int()? as u32),
            100 => {
                let building_type = reader.read_int()? as u32;
                let coord = reader.read_location()?;
                let angle = reader.read_real()?;
                DozerConstruct(building_type, coord, angle)
            }
            101 => {
                let building_type = reader.read_int()? as u32;
                let start = reader.read_location()?;
                let angle = reader.read_real()?;
                let end = reader.read_location()?;
                DozerConstructLine(building_type, start, end, angle)
            }
            102 => DozerCancelConstruct(reader.read_object_id()?),
            103 => Sell(reader.read_object_id()?),
            109 => AreaSelection(reader.read_region()?),
            107 => {
                let coord = reader.read_location()?;
                CombatDropAtLocation(coord)
            }
            108 => CombatDropAtObject(reader.read_object_id()?),
            119 => {
                let coord = reader.read_location()?;
                DoMoveTo(coord)
            }
            120 => {
                let coord = reader.read_location()?;
                DoAttackMoveTo(coord)
            }
            121 => {
                let coord = reader.read_location()?;
                DoForceMoveTO(coord)
            }
            110 => DoAttackObject(reader.read_object_id()?),
            111 => DoForceAttackObject(reader.read_object_id()?),
            112 => {
                let coord = reader.read_location()?;
                DoForceAttackGround(coord)
            }
            122 => {
                let coord = reader.read_location()?;
                AddWaypoint(coord)
            }
            123 => {
                let coord = reader.read_location()?;
                let guard_mode = reader.read_int()?;
                DoGuardPosition(coord, guard_mode)
            }
            124 => {
                let target = reader.read_object_id()?;
                let guard_mode = reader.read_int()?;
                DoGuardObject(target, guard_mode)
            }
            113 => GetRepaired(reader.read_object_id()?),
            114 => GetHealed(reader.read_object_id()?),
            115 => DoRepair(reader.read_object_id()?),
            116 => ResumeConstruction(reader.read_object_id()?),
            117 => {
                let unit = reader.read_object_id()?;
                let container = reader.read_object_id()?;
                Enter(unit, container)
            }
            118 => Dock(reader.read_object_id()?),
            137 => {
                let coord = reader.read_location()?;
                let power_type = reader.read_int()? as u32;
                let source = reader.read_object_id()?;
                DoSpecialPowerOverrideDestination(coord, power_type, source)
            }
            127 => InternetHack,
            128 => DoCheer,
            129 => ToggleOvercharge,
            130 => SwitchWeapons(reader.read_int()? as u32),
            131 => {
                let unit = reader.read_object_id()?;
                let target = reader.read_object_id()?;
                ConvertToCarbomb(unit, target)
            }
            132 => {
                let unit = reader.read_object_id()?;
                let target = reader.read_object_id()?;
                CaptureBuilding(unit, target)
            }
            133 => {
                let unit = reader.read_object_id()?;
                let target = reader.read_object_id()?;
                DisableVehicleHack(unit, target)
            }
            134 => {
                let unit = reader.read_object_id()?;
                let target = reader.read_object_id()?;
                StealCashHack(unit, target)
            }
            135 => {
                let unit = reader.read_object_id()?;
                let target = reader.read_object_id()?;
                DisableBuildingHack(unit, target)
            }
            136 => {
                let unit = reader.read_object_id()?;
                let target = reader.read_object_id()?;
                SnipeVehicle(unit, target)
            }
            138 => {
                let coord = reader.read_location()?;
                DoSalvage(coord)
            }
            140 => {
                let coord = reader.read_location()?;
                PlaceBeacon(coord)
            }
            141 => {
                let coord = reader.read_location()?;
                RemoveBeacon(coord)
            }
            142 => {
                let coord = reader.read_location()?;
                let text = reader.read_string()?;
                SetBeaconText(coord, text)
            }
            143 => {
                let coord = reader.read_location()?;
                let pitch = reader.read_real()?;
                let zoom = reader.read_real()?;
                SetReplayCamera(coord, pitch, zoom)
            }
            144 => {
                let player_id = reader.read_int()? as u32;
                SelfDestruct(player_id)
            }
            145 => {
                let units = reader.read_remaining_object_ids()?;
                CreateFormation(units)
            }
            146 => {
                let crc = reader.read_int()? as u32;
                LogicCRC(crc)
            }
            147 => {
                let detail = reader.read_int()? as u32;
                SetMineClearingDetail(detail)
            }
            148 => {
                let player_id = reader.read_int()? as u32;
                let enabled = reader.read_bool()?;
                EnableRetaliationMode(player_id, enabled)
            }
            125 => DoStop,
            126 => DoScatter,
            _ => return Err(SerializationError::InvalidMessageType(id)),
        };

        Ok((message_type, reader.consumed()))
    }

    /// Serialize a GameMessage to bytes
    pub fn serialize(message: &GameMessage) -> Result<Vec<u8>, SerializationError> {
        let mut buffer = Vec::new();

        // Serialize message type and get type ID
        let message_type_id = Self::get_message_type_id(message.get_type())?;

        // Serialize arguments
        let auto_args = Self::encode_message_arguments(message.get_type());
        let mut arg_buffer = Vec::new();
        for arg in &auto_args {
            Self::serialize_argument(arg, &mut arg_buffer)?;
        }
        for arg in message.get_arguments() {
            Self::serialize_argument(&arg.data, &mut arg_buffer)?;
        }
        let total_arg_count = auto_args.len() + message.get_argument_count();

        // Calculate total size
        let total_size = (MessageHeader::SIZE + arg_buffer.len()) as u32;

        // Create and serialize header
        let header = MessageHeader::new(
            message_type_id,
            message.get_player_index(),
            total_arg_count as u8,
            total_size,
        );

        buffer.extend_from_slice(&header.to_bytes());
        buffer.extend_from_slice(&arg_buffer);

        debug!(
            "Serialized message type {} to {} bytes",
            message_type_id,
            buffer.len()
        );
        Ok(buffer)
    }

    /// Deserialize a GameMessage from bytes
    pub fn deserialize(bytes: &[u8]) -> Result<GameMessage, SerializationError> {
        if bytes.len() < MessageHeader::SIZE {
            return Err(SerializationError::BufferTooSmall);
        }

        // Parse header
        let header = MessageHeader::from_bytes(bytes)?;

        // Verify size
        if bytes.len() < header.message_size as usize {
            return Err(SerializationError::BufferTooSmall);
        }

        // Create message type from ID
        let mut parsed_args = Vec::with_capacity(header.argument_count as usize);

        // Deserialize arguments
        let mut offset = MessageHeader::SIZE;
        for _ in 0..header.argument_count {
            let (arg, bytes_read) = Self::deserialize_argument(&bytes[offset..])?;
            parsed_args.push(arg);
            offset += bytes_read;
        }

        let (message_type, consumed) =
            Self::decode_message_type(header.message_type_id, &parsed_args)?;
        let mut message = GameMessage::with_player(message_type, header.player_index);
        for arg in parsed_args.into_iter().skip(consumed) {
            match arg {
                GameMessageArgumentType::Integer(v) => message.append_integer_argument(v),
                GameMessageArgumentType::Real(v) => message.append_real_argument(v),
                GameMessageArgumentType::Boolean(v) => message.append_boolean_argument(v),
                GameMessageArgumentType::ObjectID(v) => message.append_object_id_argument(v),
                GameMessageArgumentType::DrawableID(v) => message.append_drawable_id_argument(v),
                GameMessageArgumentType::TeamID(v) => message.append_team_id_argument(v),
                GameMessageArgumentType::SquadID(v) => message.append_team_id_argument(v),
                GameMessageArgumentType::Location(v) => message.append_location_argument(v),
                GameMessageArgumentType::Pixel(v) => message.append_pixel_argument(v),
                GameMessageArgumentType::PixelRegion(v) => message.append_pixel_region_argument(v),
                GameMessageArgumentType::Timestamp(v) => message.append_timestamp_argument(v),
                GameMessageArgumentType::WideChar(v) => message.append_wide_char_argument(v),
                GameMessageArgumentType::String(v) => message.append_string_argument(v),
            }
        }

        debug!(
            "Deserialized message type {} with {} arguments",
            header.message_type_id, header.argument_count
        );
        Ok(message)
    }

    /// Serialize a single argument
    fn serialize_argument(
        arg: &GameMessageArgumentType,
        buffer: &mut Vec<u8>,
    ) -> Result<(), SerializationError> {
        match arg {
            GameMessageArgumentType::Integer(v) => {
                buffer.push(1); // Type ID
                buffer.extend_from_slice(&v.to_le_bytes());
            }
            GameMessageArgumentType::Real(v) => {
                buffer.push(2);
                buffer.extend_from_slice(&v.to_le_bytes());
            }
            GameMessageArgumentType::Boolean(v) => {
                buffer.push(3);
                buffer.push(if *v { 1 } else { 0 });
            }
            GameMessageArgumentType::ObjectID(v) => {
                buffer.push(4);
                buffer.extend_from_slice(&v.to_le_bytes());
            }
            GameMessageArgumentType::DrawableID(v) => {
                buffer.push(5);
                buffer.extend_from_slice(&v.to_le_bytes());
            }
            GameMessageArgumentType::TeamID(v) => {
                buffer.push(6);
                buffer.extend_from_slice(&v.to_le_bytes());
            }
            GameMessageArgumentType::SquadID(v) => {
                buffer.push(7);
                buffer.extend_from_slice(&v.to_le_bytes());
            }
            GameMessageArgumentType::Location(v) => {
                buffer.push(8);
                buffer.extend_from_slice(&v.x.to_le_bytes());
                buffer.extend_from_slice(&v.y.to_le_bytes());
                buffer.extend_from_slice(&v.z.to_le_bytes());
            }
            GameMessageArgumentType::Pixel(v) => {
                buffer.push(9);
                buffer.extend_from_slice(&v.x.to_le_bytes());
                buffer.extend_from_slice(&v.y.to_le_bytes());
            }
            GameMessageArgumentType::PixelRegion(v) => {
                buffer.push(10);
                buffer.extend_from_slice(&v.x.to_le_bytes());
                buffer.extend_from_slice(&v.y.to_le_bytes());
                buffer.extend_from_slice(&v.width.to_le_bytes());
                buffer.extend_from_slice(&v.height.to_le_bytes());
            }
            GameMessageArgumentType::Timestamp(v) => {
                buffer.push(11);
                buffer.extend_from_slice(&v.to_le_bytes());
            }
            GameMessageArgumentType::WideChar(v) => {
                buffer.push(12);
                buffer.extend_from_slice(&(*v as u32).to_le_bytes());
            }
            GameMessageArgumentType::String(value) => {
                buffer.push(13);
                let bytes = value.as_bytes();
                buffer.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                buffer.extend_from_slice(bytes);
            }
        }
        Ok(())
    }

    /// Deserialize a single argument
    fn deserialize_argument(
        bytes: &[u8],
    ) -> Result<(GameMessageArgumentType, usize), SerializationError> {
        if bytes.is_empty() {
            return Err(SerializationError::BufferTooSmall);
        }

        let type_id = bytes[0];
        let mut offset = 1;

        let arg = match type_id {
            1 => {
                // Integer
                if bytes.len() < offset + 4 {
                    return Err(SerializationError::BufferTooSmall);
                }
                let value = i32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
                offset += 4;
                GameMessageArgumentType::Integer(value)
            }
            2 => {
                // Real
                if bytes.len() < offset + 4 {
                    return Err(SerializationError::BufferTooSmall);
                }
                let value = f32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
                offset += 4;
                GameMessageArgumentType::Real(value)
            }
            3 => {
                // Boolean
                if bytes.len() < offset + 1 {
                    return Err(SerializationError::BufferTooSmall);
                }
                let value = bytes[1] != 0;
                offset += 1;
                GameMessageArgumentType::Boolean(value)
            }
            4 => {
                // ObjectID
                if bytes.len() < offset + 4 {
                    return Err(SerializationError::BufferTooSmall);
                }
                let value = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
                offset += 4;
                GameMessageArgumentType::ObjectID(value)
            }
            5 => {
                // DrawableID
                if bytes.len() < offset + 4 {
                    return Err(SerializationError::BufferTooSmall);
                }
                let value = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
                offset += 4;
                GameMessageArgumentType::DrawableID(value)
            }
            6 => {
                // TeamID
                if bytes.len() < offset + 4 {
                    return Err(SerializationError::BufferTooSmall);
                }
                let value = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
                offset += 4;
                GameMessageArgumentType::TeamID(value)
            }
            7 => {
                // SquadID
                if bytes.len() < offset + 4 {
                    return Err(SerializationError::BufferTooSmall);
                }
                let value = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
                offset += 4;
                GameMessageArgumentType::SquadID(value)
            }
            8 => {
                // Location
                if bytes.len() < offset + 12 {
                    return Err(SerializationError::BufferTooSmall);
                }
                let x = f32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
                let y = f32::from_le_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]);
                let z = f32::from_le_bytes([bytes[9], bytes[10], bytes[11], bytes[12]]);
                offset += 12;
                GameMessageArgumentType::Location(Coord3D { x, y, z })
            }
            9 => {
                // Pixel
                if bytes.len() < offset + 8 {
                    return Err(SerializationError::BufferTooSmall);
                }
                let x = i32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
                let y = i32::from_le_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]);
                offset += 8;
                GameMessageArgumentType::Pixel(ICoord2D { x, y })
            }
            10 => {
                // PixelRegion
                if bytes.len() < offset + 16 {
                    return Err(SerializationError::BufferTooSmall);
                }
                let x = i32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
                let y = i32::from_le_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]);
                let width = i32::from_le_bytes([bytes[9], bytes[10], bytes[11], bytes[12]]);
                let height = i32::from_le_bytes([bytes[13], bytes[14], bytes[15], bytes[16]]);
                offset += 16;
                GameMessageArgumentType::PixelRegion(IRegion2D {
                    x,
                    y,
                    width,
                    height,
                })
            }
            11 => {
                // Timestamp
                if bytes.len() < offset + 4 {
                    return Err(SerializationError::BufferTooSmall);
                }
                let value = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
                offset += 4;
                GameMessageArgumentType::Timestamp(value)
            }
            12 => {
                // WideChar
                if bytes.len() < offset + 4 {
                    return Err(SerializationError::BufferTooSmall);
                }
                let value = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
                offset += 4;
                GameMessageArgumentType::WideChar(char::from_u32(value).unwrap_or('\0'))
            }
            13 => {
                if bytes.len() < offset + 4 {
                    return Err(SerializationError::BufferTooSmall);
                }
                let len = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
                offset += 4;
                if bytes.len() < offset + len {
                    return Err(SerializationError::BufferTooSmall);
                }
                let value = String::from_utf8(bytes[offset..offset + len].to_vec())
                    .map_err(|_| SerializationError::CorruptedData)?;
                offset += len;
                GameMessageArgumentType::String(value)
            }
            _ => return Err(SerializationError::InvalidArgumentType(type_id)),
        };

        Ok((arg, offset))
    }

    /// Get a unique ID for each message type (simplified implementation)
    pub(crate) fn get_message_type_id(
        msg_type: &GameMessageType,
    ) -> Result<u16, SerializationError> {
        use GameMessageType::*;

        let id = match msg_type {
            Invalid => 0,
            FrameTick(..) => 1,
            RawMousePosition(..) => 2,
            RawMouseLeftButtonDown(..) => 3,
            RawMouseLeftDoubleClick(..) => 4,
            RawMouseLeftButtonUp(..) => 5,
            RawMouseLeftClick(..) => 6,
            RawMouseLeftDrag(..) => 7,
            RawMouseMiddleButtonDown(..) => 8,
            RawMouseMiddleDoubleClick(..) => 9,
            RawMouseMiddleButtonUp(..) => 10,
            RawMouseMiddleDrag(..) => 11,
            RawMouseRightButtonDown(..) => 12,
            RawMouseRightDoubleClick(..) => 13,
            RawMouseRightButtonUp(..) => 14,
            RawMouseRightDrag(..) => 15,
            RawMouseWheel(..) => 16,
            RawKeyDown(..) => 17,
            RawKeyUp(..) => 18,
            MouseLeftClick(..) => 19,
            MouseLeftDoubleClick(..) => 20,
            MouseMiddleClick(..) => 21,
            MouseMiddleDoubleClick(..) => 22,
            MouseRightClick(..) => 23,
            MouseRightDoubleClick(..) => 24,
            ClearGameData => 25,
            NewGame => 26,
            MetaSaveView(..) => 27,
            MetaViewView(..) => 28,
            MetaCreateTeam(..) => 29,
            MetaSelectTeam(..) => 30,
            MetaAddTeam(..) => 31,
            MetaViewTeam(..) => 32,
            MetaSelectMatchingUnits => 33,
            MetaSelectNextUnit => 34,
            MetaSelectPrevUnit => 35,
            MetaSelectNextWorker => 36,
            MetaSelectPrevWorker => 37,
            MetaSelectHero => 38,
            MetaSelectAll => 39,
            MetaSelectAllAircraft => 40,
            MetaScatter => 41,
            MetaStop => 42,
            MetaDeploy => 43,
            MetaCreateFormation => 44,
            MetaFollow => 45,
            MetaChatPlayers => 46,
            MetaChatAllies => 47,
            MetaChatEveryone => 48,
            MetaDiplomacy => 49,
            MetaOptions => 50,
            MetaViewCommandCenter => 51,
            MetaViewLastRadarEvent => 52,
            MetaToggleControlBar => 53,
            MetaBeginPathBuild => 54,
            MetaEndPathBuild => 55,
            MetaBeginForceAttack => 56,
            MetaEndForceAttack => 57,
            MetaBeginForceMove => 58,
            MetaEndForceMove => 59,
            MetaBeginWaypoints => 60,
            MetaEndWaypoints => 61,
            MetaBeginPreferSelection => 62,
            MetaEndPreferSelection => 63,
            MetaBeginCameraRotateLeft => 64,
            MetaEndCameraRotateLeft => 65,
            MetaBeginCameraRotateRight => 66,
            MetaEndCameraRotateRight => 67,
            MetaBeginCameraZoomIn => 68,
            MetaEndCameraZoomIn => 69,
            MetaBeginCameraZoomOut => 70,
            MetaEndCameraZoomOut => 71,
            MetaCameraReset => 72,
            MetaToggleCameraTracking => 73,
            MetaTakeScreenshot => 74,
            MetaAllCheer => 75,
            MetaToggleAttackMove => 76,
            MetaToggleFastForwardReplay => 77,
            MetaDemoInstantQuit => 78,
            CreateSelectedGroup(..) => 79,
            CreateSelectedGroupNoSound(..) => 80,
            DestroySelectedGroup(..) => 81,
            RemoveFromSelectedGroup(..) => 82,
            SelectedGroupCommand(..) => 83,
            CreateTeamSlot(..) => 84,
            SelectTeamSlot(..) => 85,
            AddTeamSlot(..) => 86,
            DoAttackSquad(..) => 87,
            DoWeapon(..) => 88,
            DoWeaponAtLocation(..) => 89,
            DoWeaponAtObject(..) => 90,
            DoSpecialPower(..) => 91,
            DoSpecialPowerAtLocation(..) => 92,
            DoSpecialPowerAtObject(..) => 93,
            SetRallyPoint(..) => 94,
            PurchaseScience(..) => 95,
            QueueUpgrade(..) => 96,
            CancelUpgrade(..) => 97,
            QueueUnitCreate(..) => 98,
            CancelUnitCreate(..) => 99,
            DozerConstruct(..) => 100,
            DozerConstructLine(..) => 101,
            DozerCancelConstruct(..) => 102,
            Sell(..) => 103,
            Exit(..) => 104,
            Evacuate | EvacuateAtLocation(..) => 105,
            ExecuteRailedTransport => 106,
            CombatDropAtLocation(..) => 107,
            CombatDropAtObject(..) => 108,
            AreaSelection(..) => 109,
            DoAttackObject(..) => 110,
            DoForceAttackObject(..) => 111,
            DoForceAttackGround(..) => 112,
            GetRepaired(..) => 113,
            GetHealed(..) => 114,
            DoRepair(..) => 115,
            ResumeConstruction(..) => 116,
            Enter(..) => 117,
            Dock(..) => 118,
            DoMoveTo(..) => 119,
            DoAttackMoveTo(..) => 120,
            DoForceMoveTO(..) => 121,
            AddWaypoint(..) => 122,
            DoGuardPosition(..) => 123,
            DoGuardObject(..) => 124,
            DoStop => 125,
            DoScatter => 126,
            InternetHack => 127,
            DoCheer => 128,
            ToggleOvercharge => 129,
            SwitchWeapons(..) => 130,
            ConvertToCarbomb(..) => 131,
            CaptureBuilding(..) => 132,
            DisableVehicleHack(..) => 133,
            StealCashHack(..) => 134,
            DisableBuildingHack(..) => 135,
            SnipeVehicle(..) => 136,
            DoSpecialPowerOverrideDestination(..) => 137,
            DoSalvage(..) => 138,
            ClearInGamePopupMessage => 139,
            PlaceBeacon(..) => 140,
            RemoveBeacon(..) => 141,
            SetBeaconText(..) => 142,
            SetReplayCamera(..) => 143,
            SelfDestruct(..) => 144,
            CreateFormation(..) => 145,
            LogicCRC(..) => 146,
            SetMineClearingDetail(..) => 147,
            EnableRetaliationMode(..) => 148,
            MouseoverDrawableHint(..) => 149,
            MouseoverLocationHint(..) => 150,
            ValidGUICommandHint => 151,
            InvalidGUICommandHint => 152,
            AreaSelectionHint(..) => 153,
            DoAttackObjectHint(..) => 154,
            ImpossibleAttackHint => 155,
            DoForceAttackObjectHint(..) => 156,
            DoForceAttackGroundHint(..) => 157,
            GetRepairedHint(..) => 158,
            GetHealedHint(..) => 159,
            DoRepairHint(..) => 160,
            ResumeConstructionHint(..) => 161,
            EnterHint(..) => 162,
            DockHint(..) => 163,
            DoMoveToHint(..) => 164,
            DoAttackMoveToHint(..) => 165,
            AddWaypointHint(..) => 166,
            HijackHint(..) => 167,
            SabotageHint(..) => 168,
            FirebombHint(..) => 169,
            ConvertToCarbombHint(..) => 170,
            CaptureBuildingHint(..) => 171,
            SnipeVehicleHint(..) => 172,
            DefectorHint(..) => 173,
            SetRallyPointHint(..) => 174,
            DoSpecialPowerOverrideDestinationHint(..) => 175,
            DoSalvageHint(..) => 176,
            DoInvalidHint => 177,
            DoAttackObjectAfterMovingHint(..) => 178,
            HackHint(..) => 179,
            Timestamp(..) => 180,
            ObjectCreated(..) => 181,
            ObjectDestroyed(..) => 182,
            ObjectPosition(..) => 183,
            ObjectOrientation(..) => 184,
            ObjectJoinedTeam(..) => 185,
        };

        Ok(id)
    }

    /// Create a message type from its ID (simplified implementation)
    fn create_message_type_from_id(id: u16) -> Result<GameMessageType, SerializationError> {
        Err(SerializationError::InvalidMessageType(id))
    }
}

/// Batch serialization for multiple messages
pub struct MessageBatch {
    messages: Vec<Vec<u8>>,
    total_size: usize,
}

impl MessageBatch {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            total_size: 0,
        }
    }

    /// Add a message to the batch
    pub fn add_message(&mut self, message: &GameMessage) -> Result<(), SerializationError> {
        let serialized = MessageSerializer::serialize(message)?;
        self.total_size += serialized.len();
        self.messages.push(serialized);
        Ok(())
    }

    /// Serialize entire batch with batch header
    pub fn serialize_batch(&self) -> Result<Vec<u8>, SerializationError> {
        let mut buffer = Vec::with_capacity(self.total_size + 8);

        // Batch header: message count (u32) + total size (u32)
        buffer.extend_from_slice(&(self.messages.len() as u32).to_le_bytes());
        buffer.extend_from_slice(&(self.total_size as u32).to_le_bytes());

        // Append all messages
        for msg in &self.messages {
            buffer.extend_from_slice(msg);
        }

        debug!(
            "Serialized batch of {} messages ({} bytes)",
            self.messages.len(),
            buffer.len()
        );
        Ok(buffer)
    }

    /// Deserialize a batch of messages
    pub fn deserialize_batch(bytes: &[u8]) -> Result<Vec<GameMessage>, SerializationError> {
        if bytes.len() < 8 {
            return Err(SerializationError::BufferTooSmall);
        }

        let message_count = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        let _total_size = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;

        let mut messages = Vec::with_capacity(message_count);
        let mut offset = 8;

        for _ in 0..message_count {
            if offset >= bytes.len() {
                return Err(SerializationError::BufferTooSmall);
            }

            let message = MessageSerializer::deserialize(&bytes[offset..])?;

            // Get the size of this message to advance offset
            let header = MessageHeader::from_bytes(&bytes[offset..])?;
            offset += header.message_size as usize;

            messages.push(message);
        }

        debug!("Deserialized batch of {} messages", messages.len());
        Ok(messages)
    }

    /// Get the number of messages in the batch
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if the batch is empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Get the total size of the batch
    pub fn total_size(&self) -> usize {
        self.total_size
    }
}

impl Default for MessageBatch {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_header_serialization() {
        let header = MessageHeader::new(42, 5, 3, 100);
        let bytes = header.to_bytes();

        assert_eq!(bytes.len(), MessageHeader::SIZE);

        let deserialized = MessageHeader::from_bytes(&bytes).unwrap();
        assert_eq!(deserialized.version, SERIALIZATION_VERSION);
        assert_eq!(deserialized.message_type_id, 42);
        assert_eq!(deserialized.player_index, 5);
        assert_eq!(deserialized.argument_count, 3);
        assert_eq!(deserialized.message_size, 100);
    }

    #[test]
    fn test_argument_serialization() {
        let mut buffer = Vec::new();

        // Test integer
        MessageSerializer::serialize_argument(&GameMessageArgumentType::Integer(42), &mut buffer)
            .unwrap();
        let (arg, size) = MessageSerializer::deserialize_argument(&buffer).unwrap();
        match arg {
            GameMessageArgumentType::Integer(v) => assert_eq!(v, 42),
            _ => panic!("Wrong type"),
        }

        // Test real
        buffer.clear();
        MessageSerializer::serialize_argument(&GameMessageArgumentType::Real(3.14), &mut buffer)
            .unwrap();
        let (arg, _) = MessageSerializer::deserialize_argument(&buffer).unwrap();
        match arg {
            GameMessageArgumentType::Real(v) => assert!((v - 3.14).abs() < 0.001),
            _ => panic!("Wrong type"),
        }

        // Test location
        buffer.clear();
        let loc = Coord3D {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        MessageSerializer::serialize_argument(&GameMessageArgumentType::Location(loc), &mut buffer)
            .unwrap();
        let (arg, _) = MessageSerializer::deserialize_argument(&buffer).unwrap();
        match arg {
            GameMessageArgumentType::Location(v) => {
                assert_eq!(v.x, 1.0);
                assert_eq!(v.y, 2.0);
                assert_eq!(v.z, 3.0);
            }
            _ => panic!("Wrong type"),
        }
    }

    #[test]
    fn test_message_batch() {
        let mut batch = MessageBatch::new();

        let msg1 = GameMessage::new(GameMessageType::Invalid);
        let msg2 = GameMessage::new(GameMessageType::NewGame);

        batch.add_message(&msg1).unwrap();
        batch.add_message(&msg2).unwrap();

        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());

        let serialized = batch.serialize_batch().unwrap();
        assert!(serialized.len() > 8); // At least header

        let deserialized = MessageBatch::deserialize_batch(&serialized).unwrap();
        assert_eq!(deserialized.len(), 2);
    }

    #[test]
    fn test_evacuate_optional_location_round_trip() {
        let no_location = GameMessage::new(GameMessageType::Evacuate);
        let no_location_bytes = MessageSerializer::serialize(&no_location).unwrap();
        let no_location_round_trip = MessageSerializer::deserialize(&no_location_bytes).unwrap();
        assert_eq!(
            no_location_round_trip.get_type(),
            &GameMessageType::Evacuate
        );

        let target = Coord3D::new(10.0, 20.0, 3.0);
        let with_location = GameMessage::new(GameMessageType::EvacuateAtLocation(target.clone()));
        let with_location_bytes = MessageSerializer::serialize(&with_location).unwrap();
        let with_location_round_trip =
            MessageSerializer::deserialize(&with_location_bytes).unwrap();
        assert_eq!(
            with_location_round_trip.get_type(),
            &GameMessageType::EvacuateAtLocation(target)
        );
    }

    #[test]
    fn test_network_command_classifier_matches_cxx_network_band() {
        assert!(is_network_command_message(
            &GameMessageType::CreateSelectedGroup(false, Vec::new())
        ));
        assert!(is_network_command_message(&GameMessageType::LogicCRC(
            0x1234_5678
        )));
        assert!(is_network_command_message(
            &GameMessageType::EnableRetaliationMode(1, true)
        ));

        assert!(!is_network_command_message(
            &GameMessageType::MetaToggleControlBar
        ));
        assert!(!is_network_command_message(
            &GameMessageType::MouseoverDrawableHint(0)
        ));
        assert!(!is_network_command_message(&GameMessageType::NewGame));
    }
}
