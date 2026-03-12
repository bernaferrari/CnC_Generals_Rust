//! Binary message format compatible with C++ original
//!
//! This module implements the exact binary wire format used by the original C++
//! implementation of C&C Generals Zero Hour. This ensures that Rust and C++ clients
//! can communicate in multiplayer games.
//!
//! ## Binary Format
//!
//! All multi-byte integers are in little-endian format (x86 native).
//!
//! ### Message Header (8 bytes):
//! ```text
//! +---+---+---+---+---+---+---+---+
//! | Type  | PlayerID  | CommandID |
//! +---+---+---+---+---+---+---+---+
//! | Frame Number (4 bytes)        |
//! +---+---+---+---+---+---+---+---+
//! ```
//!
//! - Type (1 byte): NetCommandType enum value
//! - PlayerID (1 byte): Player who sent command (0-7)
//! - CommandID (2 bytes): Unique command ID (u16)
//! - Frame Number (4 bytes): Execution frame number (u32)
//!
//! Each message type then has its own payload format defined below.

use crate::commands::game_message::GameMessage;
use crate::commands::{
    AckData, ChatData, CommandPayload, DisconnectVoteData, DisconnectVoteType,
    FileAnnouncementData, FileProgressData, FileTransferData, FrameInfoData, GameCommandData,
    NetCommand, NetCommandType, PlayerLeaveData, ProgressData, ProgressType, RunAheadData,
    RunAheadMetricsData,
};
use crate::error::{NetworkError, NetworkResult};
use crate::file_transfer::{FileMetadata, TransferType};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use chrono::Utc;
use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use tracing::{trace, warn};
use uuid::Uuid;

/// Maximum size for a single message payload
const MAX_PAYLOAD_SIZE: usize = 1024;

/// Binary message parser/serializer with C++ compatibility
pub struct BinaryMessageCodec {
    /// Whether to validate sizes strictly
    strict_validation: bool,
}

impl BinaryMessageCodec {
    /// Create a new binary codec with default settings
    pub fn new() -> Self {
        Self {
            strict_validation: true,
        }
    }

    /// Create a codec with custom validation settings
    pub fn with_validation(strict_validation: bool) -> Self {
        Self { strict_validation }
    }

    /// Serialize a NetCommand to binary format matching C++ layout
    pub fn serialize(&self, command: &NetCommand) -> NetworkResult<Vec<u8>> {
        let mut buffer = Vec::with_capacity(256);

        // Write header (8 bytes total)
        buffer.write_u8(command.command_type as u8)?;
        buffer.write_u8(command.player_id)?;
        buffer.write_u16::<LittleEndian>(command.sequence)?; // CommandID
        buffer.write_u32::<LittleEndian>(command.execution_frame)?;

        // Write payload based on command type
        self.write_payload(&mut buffer, &command.payload, command.command_type)?;

        trace!(
            "Serialized command type {:?} ({} bytes)",
            command.command_type,
            buffer.len()
        );

        Ok(buffer)
    }

    /// Deserialize binary data into a NetCommand
    pub fn deserialize(&self, data: &[u8]) -> NetworkResult<NetCommand> {
        if data.len() < 8 {
            return Err(NetworkError::invalid_packet(format!(
                "message too short: {} bytes (minimum 8)",
                data.len()
            )));
        }

        let mut cursor = Cursor::new(data);

        // Read header (8 bytes)
        let type_byte = cursor.read_u8()?;
        let command_type = NetCommandType::from(type_byte);
        let player_id = cursor.read_u8()?;
        let command_id = cursor.read_u16::<LittleEndian>()?;
        let execution_frame = cursor.read_u32::<LittleEndian>()?;

        // Read payload
        let payload = self.read_payload(&mut cursor, command_type)?;

        trace!(
            "Deserialized command type {:?} from {} bytes",
            command_type,
            data.len()
        );

        Ok(NetCommand {
            id: Uuid::new_v4(), // Generate new UUID (not in wire format)
            command_type,
            player_id,
            execution_frame,
            timestamp: Utc::now(),
            priority: NetCommand::default_priority(command_type),
            flags: NetCommand::default_flags(command_type),
            payload,
            signature: None,
            sequence: command_id,
        })
    }

    /// Write payload based on command type
    fn write_payload(
        &self,
        buffer: &mut Vec<u8>,
        payload: &CommandPayload,
        command_type: NetCommandType,
    ) -> NetworkResult<()> {
        match (payload, command_type) {
            (CommandPayload::KeepAlive, NetCommandType::KeepAlive) => {
                // No payload for keep-alive
                Ok(())
            }

            (CommandPayload::Chat(data), NetCommandType::Chat)
            | (CommandPayload::Chat(data), NetCommandType::DisconnectChat) => {
                // Chat: text_length (u8) + UTF-16 string + player_mask (i32)
                // C++ Format (NetPacket.cpp:5583-5610):
                // - text_length: u8 (max 255 chars)
                // - text: u16[] (UTF-16 chars)
                // - player_mask: i32 (signed 4-byte int)

                // Ensure message fits in u8 length
                if data.message.len() > 255 {
                    return Err(NetworkError::invalid_command(
                        "Chat message too long (max 255 chars)",
                    ));
                }

                let utf16: Vec<u16> = data.message.encode_utf16().collect();

                // Write length as u8 (NOT u16)
                buffer.write_u8(utf16.len() as u8)?;

                // Write UTF-16 characters
                for ch in utf16 {
                    buffer.write_u16::<LittleEndian>(ch)?;
                }

                // Write target mask as i32 (NOT u8)
                buffer.write_i32::<LittleEndian>(data.target_mask)?;

                Ok(())
            }

            (CommandPayload::Ack(data), NetCommandType::AckBoth)
            | (CommandPayload::Ack(data), NetCommandType::AckStage1)
            | (CommandPayload::Ack(data), NetCommandType::AckStage2) => {
                // Ack: command_id (2 bytes) + original_player_id (1 byte)
                // Extract command ID from UUID (use first 2 bytes as approximation)
                let bytes = data.command_id.as_bytes();
                let cmd_id = u16::from_le_bytes([bytes[0], bytes[1]]);
                buffer.write_u16::<LittleEndian>(cmd_id)?;
                buffer.write_u8(0)?; // Original player ID (would need to track this properly)
                Ok(())
            }

            (CommandPayload::FrameInfo(data), NetCommandType::FrameInfo) => {
                // FrameInfo: command_count (2 bytes)
                buffer.write_u16::<LittleEndian>(data.command_count)?;
                Ok(())
            }

            (CommandPayload::Progress(data), NetCommandType::Progress) => {
                // Progress: percentage (1 byte)
                buffer.write_u8(data.percentage)?;
                Ok(())
            }

            // LoadComplete has no payload in the C++ protocol (header only).
            (CommandPayload::KeepAlive, NetCommandType::LoadComplete) => Ok(()),

            (CommandPayload::FileProgress(data), NetCommandType::FileProgress) => {
                // FileProgress: file_id (2 bytes) + progress (4 bytes as Int)
                buffer.write_u16::<LittleEndian>(data.file_id)?;
                buffer.write_i32::<LittleEndian>(data.progress)?;
                Ok(())
            }

            (CommandPayload::FileAnnouncement(data), NetCommandType::FileAnnounce) => {
                // FileAnnounce: file_id (2 bytes) + player_mask (1 byte) + portable_filename (string)
                buffer.write_u16::<LittleEndian>(data.command_id)?;
                buffer.write_u8(data.player_mask)?;

                // Write filename as null-terminated ASCII string
                buffer.write_all(data.metadata.filename.as_bytes())?;
                buffer.write_u8(0)?; // null terminator

                Ok(())
            }

            (CommandPayload::FileTransfer(data), NetCommandType::File) => {
                // File: filename (null-terminated) + length (u32) + data
                buffer.write_all(data.filename.as_bytes())?;
                buffer.write_u8(0)?;

                let len = data.data.len().min(u32::MAX as usize) as u32;
                buffer.write_u32::<LittleEndian>(len)?;
                buffer.write_all(&data.data[..len as usize])?;

                Ok(())
            }

            (CommandPayload::GameCommand(data), NetCommandType::GameCommand) => {
                // GameCommand: Use GameMessage serialization (argument order preserved)
                let mut game_msg = GameMessage::new(data.command_type, 0);

                if let Some(target) = data.target_id {
                    game_msg.add_object_id(target);
                }

                if let Some((x, y, z)) = data.position {
                    game_msg.add_location(crate::commands::game_message::Coord3D { x, y, z });
                }

                for (_, value) in ordered_parameters(&data.parameters) {
                    append_command_param_to_game_message(&mut game_msg, value)?;
                }

                // Serialize the GameMessage and append to buffer (C++ grouped format)
                let game_msg_bytes = game_msg.serialize_cpp_compatible()?;
                buffer.write_all(&game_msg_bytes)?;

                Ok(())
            }

            (CommandPayload::RunAheadMetrics(data), NetCommandType::RunAheadMetrics) => {
                // RunAheadMetrics: average_latency (float) + average_fps (int)
                buffer.write_f32::<LittleEndian>(data.average_latency)?;
                buffer.write_i32::<LittleEndian>(data.average_fps as i32)?;
                Ok(())
            }

            (CommandPayload::RunAhead(data), NetCommandType::RunAhead) => {
                // RunAhead: run_ahead (u16) + frame_rate (u8)
                // C++ Format (NetPacket.cpp:5471-5489): Payload: run_ahead (u16) + frame_rate (u8)
                buffer.write_u16::<LittleEndian>(data.run_ahead)?;
                buffer.write_u8(data.frame_rate)?;
                Ok(())
            }

            (CommandPayload::PlayerLeave(data), NetCommandType::PlayerLeave) => {
                // PlayerLeave: leaving_player_id (1 byte)
                // C++ Format (NetPacket.cpp:5437-5451): Payload contains the player ID who is leaving
                buffer.write_u8(data.leaving_player_id)?;
                Ok(())
            }

            (CommandPayload::DisconnectVote(data), NetCommandType::DisconnectVote) => {
                // DisconnectVote: target_slot (1 byte) + vote_frame (4 bytes)
                buffer.write_u8(data.target_slot)?;
                buffer.write_u32::<LittleEndian>(data.vote_frame)?;
                Ok(())
            }

            (CommandPayload::FrameResendRequest(data), NetCommandType::FrameResendRequest) => {
                // FrameResendRequest: frame_number (4 bytes as u32)
                buffer.write_u32::<LittleEndian>(data.frame_number)?;
                Ok(())
            }

            (CommandPayload::Generic(data), _) => {
                // Generic: raw bytes
                buffer.write_all(data)?;
                Ok(())
            }

            _ => {
                warn!(
                    "Unhandled payload type {:?} for command {:?}",
                    payload, command_type
                );
                // Write empty payload for unimplemented types
                Ok(())
            }
        }
    }

    /// Read payload based on command type
    fn read_payload(
        &self,
        cursor: &mut Cursor<&[u8]>,
        command_type: NetCommandType,
    ) -> NetworkResult<CommandPayload> {
        match command_type {
            NetCommandType::KeepAlive => Ok(CommandPayload::KeepAlive),

            NetCommandType::Chat | NetCommandType::DisconnectChat => {
                // C++ Format (NetPacket.cpp:5583-5610):
                // - text_length: u8 (max 255 chars)
                // - text: u16[] (UTF-16 chars)
                // - player_mask: i32 (signed 4-byte int)

                // Read length as u8 (NOT u16)
                let str_len = cursor.read_u8()? as usize;

                // Read UTF-16 string
                let mut utf16_chars = Vec::with_capacity(str_len);
                for _ in 0..str_len {
                    utf16_chars.push(cursor.read_u16::<LittleEndian>()?);
                }

                let message = String::from_utf16(&utf16_chars).map_err(|e| {
                    NetworkError::invalid_packet(format!("invalid UTF-16 string: {}", e))
                })?;

                // Read target mask as i32 (NOT u8)
                let target_mask = cursor.read_i32::<LittleEndian>()?;

                Ok(CommandPayload::Chat(ChatData {
                    message,
                    target_mask,
                }))
            }

            NetCommandType::AckBoth | NetCommandType::AckStage1 | NetCommandType::AckStage2 => {
                let command_id = cursor.read_u16::<LittleEndian>()?;
                let _original_player_id = cursor.read_u8()?;

                // Generate UUID from command ID (approximate reverse of serialization)
                let mut uuid_bytes = [0u8; 16];
                uuid_bytes[0..2].copy_from_slice(&command_id.to_le_bytes());
                let command_uuid = Uuid::from_bytes(uuid_bytes);

                Ok(CommandPayload::Ack(AckData {
                    command_id: command_uuid,
                }))
            }

            NetCommandType::FrameInfo => {
                let command_count = cursor.read_u16::<LittleEndian>()?;

                Ok(CommandPayload::FrameInfo(FrameInfoData {
                    frame: 0, // Would be in header
                    command_count,
                    checksum: 0, // Would need to be calculated
                }))
            }

            NetCommandType::Progress => {
                let percentage = cursor.read_u8()?;

                Ok(CommandPayload::Progress(ProgressData {
                    progress_type: ProgressType::Loading,
                    percentage,
                }))
            }

            NetCommandType::LoadComplete => Ok(CommandPayload::KeepAlive),

            NetCommandType::FileProgress => {
                let file_id = cursor.read_u16::<LittleEndian>()?;
                let progress = cursor.read_i32::<LittleEndian>()?;

                Ok(CommandPayload::FileProgress(FileProgressData {
                    file_id,
                    progress,
                }))
            }

            NetCommandType::FileAnnounce => {
                let command_id = cursor.read_u16::<LittleEndian>()?;
                let player_mask = cursor.read_u8()?;

                // Read null-terminated string
                let mut filename_bytes = Vec::new();
                loop {
                    let byte = cursor.read_u8()?;
                    if byte == 0 {
                        break;
                    }
                    filename_bytes.push(byte);
                }

                let filename = String::from_utf8(filename_bytes).map_err(|e| {
                    NetworkError::invalid_packet(format!("invalid filename: {}", e))
                })?;

                Ok(CommandPayload::FileAnnouncement(FileAnnouncementData {
                    command_id,
                    player_mask,
                    metadata: FileMetadata {
                        filename,
                        file_size: 0,
                        checksum: [0u8; 32],
                        transfer_type: TransferType::Generic,
                    },
                }))
            }

            NetCommandType::File => {
                let mut filename_bytes = Vec::new();
                loop {
                    let byte = cursor.read_u8()?;
                    if byte == 0 {
                        break;
                    }
                    filename_bytes.push(byte);
                }

                let filename = String::from_utf8(filename_bytes).map_err(|e| {
                    NetworkError::invalid_packet(format!("invalid filename: {}", e))
                })?;

                let length = cursor.read_u32::<LittleEndian>()? as usize;
                let mut data = vec![0u8; length];
                cursor.read_exact(&mut data)?;

                Ok(CommandPayload::FileTransfer(FileTransferData {
                    file_id: 0,
                    filename,
                    data,
                    chunk_number: 0,
                    total_chunks: 1,
                    checksum: 0,
                }))
            }

            NetCommandType::GameCommand => {
                // Deserialize full GameMessage
                let pos = cursor.position() as usize;
                let remaining = &cursor.get_ref()[pos..];
                let game_msg = GameMessage::deserialize_cpp_compatible(remaining)?;

                // Convert GameMessage to GameCommandData
                // Extract common fields from arguments
                let mut target_id = None;
                let mut position = None;
                let mut parameters = HashMap::new();
                let mut arg_index = 0usize;

                for arg in &game_msg.arguments {
                    match &arg.value {
                        crate::commands::game_message::GameMessageArgumentValue::ObjectID(id) => {
                            if target_id.is_none() {
                                target_id = Some(*id);
                            } else if let Some(param) = argument_to_command_param(arg.value.clone())
                            {
                                let key = format!("arg{:03}", arg_index);
                                parameters.insert(key, param);
                                arg_index += 1;
                            }
                        }
                        crate::commands::game_message::GameMessageArgumentValue::Location(loc) => {
                            if position.is_none() {
                                position = Some((loc.x, loc.y, loc.z));
                            } else if let Some(param) = argument_to_command_param(arg.value.clone())
                            {
                                let key = format!("arg{:03}", arg_index);
                                parameters.insert(key, param);
                                arg_index += 1;
                            }
                        }
                        _ => {
                            if let Some(param) = argument_to_command_param(arg.value.clone()) {
                                let key = format!("arg{:03}", arg_index);
                                parameters.insert(key, param);
                                arg_index += 1;
                            }
                        }
                    }
                }

                Ok(CommandPayload::GameCommand(GameCommandData {
                    command_type: game_msg.message_type,
                    target_id,
                    position,
                    parameters,
                    checksum: 0,
                }))
            }

            NetCommandType::RunAheadMetrics => {
                let average_latency = cursor.read_f32::<LittleEndian>()?;
                let average_fps = cursor.read_i32::<LittleEndian>()? as u32;

                Ok(CommandPayload::RunAheadMetrics(RunAheadMetricsData {
                    average_latency,
                    average_fps,
                    recommended_frames: 0,
                }))
            }

            NetCommandType::RunAhead => {
                // C++ Format (NetPacket.cpp:5471-5489): Payload: run_ahead (u16) + frame_rate (u8)
                let run_ahead = cursor.read_u16::<LittleEndian>()?;
                let frame_rate = cursor.read_u8()?;

                Ok(CommandPayload::RunAhead(RunAheadData {
                    run_ahead,
                    frame_rate,
                }))
            }

            NetCommandType::PlayerLeave => {
                // C++ Format (NetPacket.cpp:5437-5451): Payload contains the player ID who is leaving
                let leaving_player_id = cursor.read_u8()?;

                Ok(CommandPayload::PlayerLeave(PlayerLeaveData {
                    leaving_player_id,
                }))
            }

            NetCommandType::DisconnectVote => {
                let target_slot = cursor.read_u8()?;
                let vote_frame = cursor.read_u32::<LittleEndian>()?;

                Ok(CommandPayload::DisconnectVote(DisconnectVoteData {
                    target_slot,
                    vote_frame,
                    vote_type: DisconnectVoteType::Kick,
                }))
            }

            NetCommandType::FrameResendRequest => {
                let frame_number = cursor.read_u32::<LittleEndian>()?;

                Ok(CommandPayload::FrameResendRequest(
                    crate::commands::FrameResendRequestData { frame_number },
                ))
            }

            _ => {
                // For unimplemented types, read remaining bytes as generic payload
                let pos = cursor.position() as usize;
                let remaining = &cursor.get_ref()[pos..];
                Ok(CommandPayload::Generic(remaining.to_vec()))
            }
        }
    }

    /// Validate that a serialized message matches expected format
    pub fn validate_format(&self, data: &[u8]) -> NetworkResult<()> {
        if data.len() < 8 {
            return Err(NetworkError::invalid_packet(
                "message shorter than minimum header size",
            ));
        }

        if self.strict_validation && data.len() > MAX_PAYLOAD_SIZE {
            return Err(NetworkError::invalid_packet(format!(
                "message exceeds maximum size: {} > {}",
                data.len(),
                MAX_PAYLOAD_SIZE
            )));
        }

        // Validate command type
        let type_byte = data[0];
        let command_type = NetCommandType::from(type_byte);
        if matches!(command_type, NetCommandType::Unknown) {
            return Err(NetworkError::invalid_packet(format!(
                "unknown command type: {}",
                type_byte
            )));
        }

        // Validate player ID
        let player_id = data[1];
        if player_id >= crate::config::MAX_PLAYERS {
            return Err(NetworkError::invalid_packet(format!(
                "invalid player ID: {}",
                player_id
            )));
        }

        Ok(())
    }
}

fn ordered_parameters<'a>(
    parameters: &'a HashMap<String, crate::commands::CommandParameter>,
) -> Vec<(&'a String, &'a crate::commands::CommandParameter)> {
    let mut indexed = Vec::new();
    let mut named = Vec::new();

    for (key, value) in parameters {
        if let Some(index) = parse_arg_index(key) {
            indexed.push((index, key, value));
        } else {
            named.push((key, value));
        }
    }

    indexed.sort_by_key(|(index, _, _)| *index);
    named.sort_by_key(|(key, _)| *key);

    let mut ordered = Vec::new();
    for (_, key, value) in indexed {
        ordered.push((key, value));
    }
    for (key, value) in named {
        ordered.push((key, value));
    }

    ordered
}

fn parse_arg_index(key: &str) -> Option<u32> {
    if let Some(rest) = key.strip_prefix("arg") {
        if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
            return rest.parse::<u32>().ok();
        }
    }
    None
}

fn append_command_param_to_game_message(
    msg: &mut GameMessage,
    value: &crate::commands::CommandParameter,
) -> NetworkResult<()> {
    match value {
        crate::commands::CommandParameter::Int(v) => msg.add_integer(*v),
        crate::commands::CommandParameter::Float(v) => msg.add_real(*v),
        crate::commands::CommandParameter::Bool(v) => msg.add_boolean(*v),
        crate::commands::CommandParameter::ObjectId(v) => msg.add_object_id(*v),
        crate::commands::CommandParameter::DrawableId(v) => msg.add_drawable_id(*v),
        crate::commands::CommandParameter::TeamId(v) => msg.add_team_id(*v),
        crate::commands::CommandParameter::Position(x, y, z) => {
            msg.add_location(crate::commands::game_message::Coord3D {
                x: *x,
                y: *y,
                z: *z,
            });
        }
        crate::commands::CommandParameter::Pixel(x, y) => {
            msg.add_pixel(crate::commands::game_message::ICoord2D { x: *x, y: *y });
        }
        crate::commands::CommandParameter::PixelRegion(x1, y1, x2, y2) => {
            msg.add_pixel_region(crate::commands::game_message::IRegion2D::new(
                *x1, *y1, *x2, *y2,
            ));
        }
        crate::commands::CommandParameter::Timestamp(v) => msg.add_timestamp(*v),
        crate::commands::CommandParameter::WideChar(v) => msg.add_wide_char(*v),
        crate::commands::CommandParameter::String(_) => {
            warn!("String parameter not supported in GameMessage wire format");
        }
    }

    Ok(())
}

fn argument_to_command_param(
    value: crate::commands::game_message::GameMessageArgumentValue,
) -> Option<crate::commands::CommandParameter> {
    match value {
        crate::commands::game_message::GameMessageArgumentValue::Integer(v) => {
            Some(crate::commands::CommandParameter::Int(v))
        }
        crate::commands::game_message::GameMessageArgumentValue::Real(v) => {
            Some(crate::commands::CommandParameter::Float(v))
        }
        crate::commands::game_message::GameMessageArgumentValue::Boolean(v) => {
            Some(crate::commands::CommandParameter::Bool(v))
        }
        crate::commands::game_message::GameMessageArgumentValue::ObjectID(v) => {
            Some(crate::commands::CommandParameter::ObjectId(v))
        }
        crate::commands::game_message::GameMessageArgumentValue::DrawableID(v) => {
            Some(crate::commands::CommandParameter::DrawableId(v))
        }
        crate::commands::game_message::GameMessageArgumentValue::TeamID(v) => {
            Some(crate::commands::CommandParameter::TeamId(v))
        }
        crate::commands::game_message::GameMessageArgumentValue::Location(loc) => Some(
            crate::commands::CommandParameter::Position(loc.x, loc.y, loc.z),
        ),
        crate::commands::game_message::GameMessageArgumentValue::Pixel(pixel) => {
            Some(crate::commands::CommandParameter::Pixel(pixel.x, pixel.y))
        }
        crate::commands::game_message::GameMessageArgumentValue::PixelRegion(region) => {
            Some(crate::commands::CommandParameter::PixelRegion(
                region.lo.x,
                region.lo.y,
                region.hi.x,
                region.hi.y,
            ))
        }
        crate::commands::game_message::GameMessageArgumentValue::Timestamp(v) => {
            Some(crate::commands::CommandParameter::Timestamp(v))
        }
        crate::commands::game_message::GameMessageArgumentValue::WideChar(v) => {
            Some(crate::commands::CommandParameter::WideChar(v))
        }
    }
}

impl Default for BinaryMessageCodec {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keep_alive_roundtrip() {
        let codec = BinaryMessageCodec::new();
        let command = NetCommand::keep_alive(0);

        let serialized = codec.serialize(&command).unwrap();
        let deserialized = codec.deserialize(&serialized).unwrap();

        assert_eq!(command.command_type, deserialized.command_type);
        assert_eq!(command.player_id, deserialized.player_id);
    }

    #[test]
    fn test_chat_roundtrip() {
        let codec = BinaryMessageCodec::new();
        let command = NetCommand::chat(1, "Hello World!".to_string(), 0xFF);

        let serialized = codec.serialize(&command).unwrap();
        let deserialized = codec.deserialize(&serialized).unwrap();

        assert_eq!(command.command_type, deserialized.command_type);
        assert_eq!(command.player_id, deserialized.player_id);

        if let CommandPayload::Chat(data) = &deserialized.payload {
            assert_eq!(data.message, "Hello World!");
            assert_eq!(data.target_mask, 0xFF);
        } else {
            panic!("Expected Chat payload");
        }
    }

    #[test]
    fn test_progress_roundtrip() {
        let codec = BinaryMessageCodec::new();
        let command = NetCommand::progress(0, ProgressType::Loading, 50);

        let serialized = codec.serialize(&command).unwrap();
        let deserialized = codec.deserialize(&serialized).unwrap();

        assert_eq!(command.command_type, deserialized.command_type);

        if let CommandPayload::Progress(data) = &deserialized.payload {
            assert_eq!(data.percentage, 50);
        } else {
            panic!("Expected Progress payload");
        }
    }

    #[test]
    fn test_binary_format_validation() {
        let codec = BinaryMessageCodec::new();

        // Too short
        assert!(codec.validate_format(&[1, 2, 3]).is_err());

        // Valid header
        let valid = vec![
            4u8, // GAMECOMMAND
            0,   // player 0
            0, 0, // command id
            0, 0, 0, 0, // frame
        ];
        assert!(codec.validate_format(&valid).is_ok());

        // Invalid player ID
        let invalid = vec![
            4u8, // GAMECOMMAND
            255, // invalid player
            0, 0, // command id
            0, 0, 0, 0, // frame
        ];
        assert!(codec.validate_format(&invalid).is_err());
    }

    #[test]
    fn test_header_layout() {
        let codec = BinaryMessageCodec::new();
        let command = NetCommand::keep_alive(3);

        let serialized = codec.serialize(&command).unwrap();

        // Verify exact byte layout
        assert_eq!(serialized[0], NetCommandType::KeepAlive as u8); // Type
        assert_eq!(serialized[1], 3); // Player ID
        assert_eq!(serialized.len(), 8); // Header only for KeepAlive
    }

    #[test]
    fn test_little_endian_frame_number() {
        let codec = BinaryMessageCodec::new();
        let mut command = NetCommand::keep_alive(0);
        command.execution_frame = 0x12345678;

        let serialized = codec.serialize(&command).unwrap();

        // Check little-endian encoding of frame number (bytes 4-7)
        assert_eq!(serialized[4], 0x78);
        assert_eq!(serialized[5], 0x56);
        assert_eq!(serialized[6], 0x34);
        assert_eq!(serialized[7], 0x12);
    }

    #[test]
    fn test_frame_resend_request_roundtrip() {
        let codec = BinaryMessageCodec::new();
        let command = NetCommand::frame_resend_request(0, 95);

        let serialized = codec.serialize(&command).unwrap();
        let deserialized = codec.deserialize(&serialized).unwrap();

        assert_eq!(command.command_type, deserialized.command_type);
        assert_eq!(command.player_id, deserialized.player_id);

        if let CommandPayload::FrameResendRequest(data) = &deserialized.payload {
            assert_eq!(data.frame_number, 95);
        } else {
            panic!("Expected FrameResendRequest payload");
        }
    }

    #[test]
    fn test_frame_resend_request_serialization() {
        let codec = BinaryMessageCodec::new();
        let command = NetCommand::frame_resend_request(1, 100);

        let serialized = codec.serialize(&command).unwrap();

        // Header (8 bytes) + payload (4 bytes for u32)
        assert_eq!(serialized.len(), 12);

        // Check command type
        assert_eq!(serialized[0], NetCommandType::FrameResendRequest as u8);

        // Check player ID
        assert_eq!(serialized[1], 1);

        // Check frame number in payload (bytes 8-11, little-endian)
        let frame_bytes = [serialized[8], serialized[9], serialized[10], serialized[11]];
        assert_eq!(u32::from_le_bytes(frame_bytes), 100);
    }

    // ========================================================================
    // MAJOR FIX TESTS - C++ Compatibility
    // ========================================================================

    #[test]
    fn test_player_leave_uses_player_id() {
        // MAJOR FIX #2: PlayerLeave should encode player_id, not reason
        let codec = BinaryMessageCodec::new();

        use crate::commands::PlayerLeaveData;
        let command = NetCommand::new(
            NetCommandType::PlayerLeave,
            0,
            0,
            CommandPayload::PlayerLeave(PlayerLeaveData {
                leaving_player_id: 5,
            }),
        );

        let serialized = codec.serialize(&command).unwrap();

        // Header (8 bytes) + payload (1 byte for player_id)
        assert_eq!(serialized.len(), 9);

        // Check that byte 8 contains the leaving player ID
        assert_eq!(serialized[8], 5);

        // Roundtrip test
        let deserialized = codec.deserialize(&serialized).unwrap();
        if let CommandPayload::PlayerLeave(data) = deserialized.payload {
            assert_eq!(data.leaving_player_id, 5);
        } else {
            panic!("Expected PlayerLeave payload");
        }
    }

    #[test]
    fn test_run_ahead_command_format() {
        // MAJOR FIX #3: RunAhead command implementation
        let codec = BinaryMessageCodec::new();

        use crate::commands::RunAheadData;
        let command = NetCommand::new(
            NetCommandType::RunAhead,
            0,
            0,
            CommandPayload::RunAhead(RunAheadData {
                run_ahead: 5,
                frame_rate: 30,
            }),
        );

        let serialized = codec.serialize(&command).unwrap();

        // Header (8 bytes) + payload (2 bytes u16 + 1 byte u8 = 3 bytes)
        assert_eq!(serialized.len(), 11);

        // Check run_ahead (bytes 8-9, little-endian u16)
        let run_ahead_bytes = [serialized[8], serialized[9]];
        assert_eq!(u16::from_le_bytes(run_ahead_bytes), 5);

        // Check frame_rate (byte 10, u8)
        assert_eq!(serialized[10], 30);

        // Roundtrip test
        let deserialized = codec.deserialize(&serialized).unwrap();
        if let CommandPayload::RunAhead(data) = deserialized.payload {
            assert_eq!(data.run_ahead, 5);
            assert_eq!(data.frame_rate, 30);
        } else {
            panic!("Expected RunAhead payload");
        }
    }

    #[test]
    fn test_chat_uses_u8_length() {
        // MAJOR FIX #4: Chat message length should be u8, not u16
        let codec = BinaryMessageCodec::new();
        let command = NetCommand::chat(1, "Hello".to_string(), 0xFF);

        let serialized = codec.serialize(&command).unwrap();

        // Header (8 bytes) + length (1 byte u8) + UTF-16 chars (5 * 2 = 10 bytes) + mask (4 bytes i32)
        // Total: 8 + 1 + 10 + 4 = 23 bytes
        assert_eq!(serialized.len(), 23);

        // Check that byte 8 is the length as u8 (5 chars)
        assert_eq!(serialized[8], 5);

        // The old incorrect format would have had a u16 length at bytes 8-9
        // We verify it's NOT that by checking byte 9 is part of the UTF-16 string

        // Roundtrip test
        let deserialized = codec.deserialize(&serialized).unwrap();
        if let CommandPayload::Chat(data) = deserialized.payload {
            assert_eq!(data.message, "Hello");
        } else {
            panic!("Expected Chat payload");
        }
    }

    #[test]
    fn test_chat_uses_i32_target_mask() {
        // MAJOR FIX #4: Chat target_mask should be i32, not u8
        let codec = BinaryMessageCodec::new();

        // Test with a value that wouldn't fit in u8
        let large_mask: i32 = 0x7FFFFFFF; // Max positive i32
        let command = NetCommand::chat(1, "Test".to_string(), large_mask);

        let serialized = codec.serialize(&command).unwrap();

        // Header (8 bytes) + length (1 byte) + UTF-16 chars (4 * 2 = 8 bytes) + mask (4 bytes)
        // Total: 8 + 1 + 8 + 4 = 21 bytes
        assert_eq!(serialized.len(), 21);

        // Check target_mask at the end (bytes 17-20, little-endian i32)
        let mask_bytes = [
            serialized[17],
            serialized[18],
            serialized[19],
            serialized[20],
        ];
        assert_eq!(i32::from_le_bytes(mask_bytes), large_mask);

        // Roundtrip test
        let deserialized = codec.deserialize(&serialized).unwrap();
        if let CommandPayload::Chat(data) = deserialized.payload {
            assert_eq!(data.message, "Test");
            assert_eq!(data.target_mask, large_mask);
        } else {
            panic!("Expected Chat payload");
        }
    }

    #[test]
    fn test_chat_negative_mask() {
        // Test negative i32 mask values (which might have special meaning in C++)
        let codec = BinaryMessageCodec::new();

        let negative_mask: i32 = -1; // All bits set
        let command = NetCommand::chat(0, "Broadcast".to_string(), negative_mask);

        let serialized = codec.serialize(&command).unwrap();
        let deserialized = codec.deserialize(&serialized).unwrap();

        if let CommandPayload::Chat(data) = deserialized.payload {
            assert_eq!(data.target_mask, -1);
        } else {
            panic!("Expected Chat payload");
        }
    }

    #[test]
    fn test_chat_max_length_255() {
        // MAJOR FIX #4: Chat messages are limited to 255 characters (u8 max)
        let codec = BinaryMessageCodec::new();

        // Create a message with exactly 255 characters
        let message = "A".repeat(255);
        let command = NetCommand::chat(0, message.clone(), 0);

        // Should serialize successfully
        let serialized = codec.serialize(&command).unwrap();
        assert!(serialized.len() > 0);

        // Roundtrip
        let deserialized = codec.deserialize(&serialized).unwrap();
        if let CommandPayload::Chat(data) = deserialized.payload {
            assert_eq!(data.message.len(), 255);
        } else {
            panic!("Expected Chat payload");
        }
    }

    #[test]
    fn test_chat_exceeds_255_fails() {
        // MAJOR FIX #4: Chat messages over 255 characters should fail
        let codec = BinaryMessageCodec::new();

        // Create a message with 256 characters (too long)
        let message = "A".repeat(256);
        let command = NetCommand::chat(0, message, 0);

        // Should fail to serialize
        let result = codec.serialize(&command);
        assert!(result.is_err());
    }

    #[test]
    fn test_all_major_fixes_integration() {
        // Integration test verifying all major fixes work together
        let codec = BinaryMessageCodec::new();

        // Test PlayerLeave with player_id
        let player_leave = NetCommand::new(
            NetCommandType::PlayerLeave,
            2,
            100,
            CommandPayload::PlayerLeave(crate::commands::PlayerLeaveData {
                leaving_player_id: 3,
            }),
        );
        let serialized = codec.serialize(&player_leave).unwrap();
        let deserialized = codec.deserialize(&serialized).unwrap();
        assert_eq!(deserialized.command_type, NetCommandType::PlayerLeave);

        // Test RunAhead command
        let run_ahead = NetCommand::run_ahead(1, 8, 60);
        let serialized = codec.serialize(&run_ahead).unwrap();
        let deserialized = codec.deserialize(&serialized).unwrap();
        assert_eq!(deserialized.command_type, NetCommandType::RunAhead);

        // Test Chat with u8 length and i32 mask
        let chat = NetCommand::chat(0, "Integration test".to_string(), -1);
        let serialized = codec.serialize(&chat).unwrap();
        let deserialized = codec.deserialize(&serialized).unwrap();
        assert_eq!(deserialized.command_type, NetCommandType::Chat);

        println!("All major fixes verified in integration test");
    }
}
