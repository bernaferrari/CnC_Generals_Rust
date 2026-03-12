//! C++-Compatible Tag-Based Command Serialization
//!
//! This module implements tag-based serialization matching the C++ NetPacket format
//! to ensure interoperability between Rust and C++ network components.
//!
//! ## C++ Format (from NetPacket.cpp lines 43-148)
//! Commands are serialized with single-character tags followed by data:
//! - 'T' [1 byte: command type]
//! - 'R' [1 byte: relay]
//! - 'P' [1 byte: playerID]
//! - 'C' [2 bytes: commandID (little-endian)]
//! - 'F' [4 bytes: frame number (little-endian)]
//! - 'D' [variable length: command-specific data]
//!
//! The tag order in C++ varies by command type, but typically follows: T, F, R, P, C, D

use crate::commands::game_message::{
    Coord3D, GameMessage, GameMessageArgument, GameMessageArgumentDataType,
    GameMessageArgumentValue,
};
use crate::commands::{
    AckData, ChatData, CommandPayload, DisconnectFrameData, DisconnectPlayerData,
    DisconnectScreenOffData, DisconnectVoteData, FileAnnouncementData, FileProgressData,
    FileTransferData, FrameInfoData, FrameResendRequestData, GameCommandData, NetCommand,
    NetCommandType, PlayerLeaveData, ProgressData, RunAheadData, RunAheadMetricsData,
};
use crate::error::{NetworkError, NetworkResult};
use byteorder::ReadBytesExt;
use std::io::{Cursor, Read};
use tracing::{debug, trace};

/// NetCommandRef structure matching C++ implementation
/// This represents the on-wire format compatible with C++
#[derive(Debug, Clone)]
pub struct NetCommandRef {
    /// Command type
    pub command_type: NetCommandType,
    /// Relay flag
    pub relay: u8,
    /// Player ID who sent this command
    pub player_id: u8,
    /// Command ID (u16 in C++)
    pub id: u16,
    /// Execution frame number
    pub execution_frame: u32,
    /// Command payload
    pub payload: CommandPayload,
}

impl NetCommandRef {
    /// Create from NetCommand
    pub fn from_net_command(cmd: &NetCommand) -> Self {
        // Convert UUID to u16 for C++ compatibility (use lower 16 bits)
        let id = (cmd.id.as_u128() & 0xFFFF) as u16;

        Self {
            command_type: cmd.command_type,
            relay: 0, // Default relay value
            player_id: cmd.player_id,
            id,
            execution_frame: cmd.execution_frame,
            payload: cmd.payload.clone(),
        }
    }

    /// Convert to NetCommand
    pub fn to_net_command(&self) -> NetCommand {
        NetCommand::new(
            self.command_type,
            self.player_id,
            self.execution_frame,
            self.payload.clone(),
        )
    }
}

/// Serialize command to C++-compatible tag-based format
///
/// # Format
/// The serialization follows the C++ NetPacket format with tags:
/// - 'T' + command_type (1 byte)
/// - 'F' + execution_frame (4 bytes, little-endian)
/// - 'R' + relay (1 byte)
/// - 'P' + player_id (1 byte)
/// - 'C' + command_id (2 bytes, little-endian)
/// - 'D' + command-specific data
///
/// Note: The order T, F, R, P, C, D matches the C++ FillBufferWithGameCommand
pub fn serialize_command_cpp_compat(cmd: &NetCommandRef) -> Vec<u8> {
    let mut buf = Vec::new();

    // Type tag - 'T' followed by command type byte
    buf.push(b'T');
    buf.push(cmd.command_type as u8);

    // Frame tag - 'F' followed by 4-byte frame number (little-endian)
    buf.push(b'F');
    buf.extend_from_slice(&cmd.execution_frame.to_le_bytes());

    // Relay tag - 'R' followed by relay byte
    buf.push(b'R');
    buf.push(cmd.relay);

    // Player ID tag - 'P' followed by player ID byte
    buf.push(b'P');
    buf.push(cmd.player_id);

    // Command ID tag - 'C' followed by 2-byte command ID (little-endian)
    buf.push(b'C');
    buf.extend_from_slice(&cmd.id.to_le_bytes());

    // Data tag - 'D' followed by command-specific data
    buf.push(b'D');
    append_command_data(cmd, &mut buf);

    trace!(
        "Serialized C++ compat command type={:?} id={} size={}",
        cmd.command_type,
        cmd.id,
        buf.len()
    );

    buf
}

/// Deserialize command from C++-compatible tag-based format
///
/// # Format
/// Parses tags in any order (C++ allows flexible ordering):
/// - 'T' + command_type (1 byte)
/// - 'R' + relay (1 byte)
/// - 'P' + player_id (1 byte)
/// - 'C' + command_id (2 bytes, little-endian)
/// - 'F' + execution_frame (4 bytes, little-endian)
/// - 'D' + command-specific data
pub fn deserialize_command_cpp_compat(data: &[u8]) -> NetworkResult<NetCommandRef> {
    if data.is_empty() {
        return Err(NetworkError::deserialization("empty command data"));
    }

    let mut offset = 0;
    let mut cmd_type = NetCommandType::Unknown;
    let mut relay = 0u8;
    let mut player_id = 0u8;
    let mut cmd_id = 0u16;
    let mut frame = 0u32;
    let mut data_offset = None;

    // Parse tags - C++ allows them in any order
    while offset < data.len() {
        let tag = data[offset];
        offset += 1;

        match tag {
            b'T' => {
                if offset >= data.len() {
                    return Err(NetworkError::deserialization("truncated command type"));
                }
                cmd_type = NetCommandType::from(data[offset]);
                offset += 1;
            }
            b'R' => {
                if offset >= data.len() {
                    return Err(NetworkError::deserialization("truncated relay"));
                }
                relay = data[offset];
                offset += 1;
            }
            b'P' => {
                if offset >= data.len() {
                    return Err(NetworkError::deserialization("truncated player ID"));
                }
                player_id = data[offset];
                offset += 1;
            }
            b'C' => {
                if offset + 2 > data.len() {
                    return Err(NetworkError::deserialization("truncated command ID"));
                }
                cmd_id = u16::from_le_bytes([data[offset], data[offset + 1]]);
                offset += 2;
            }
            b'F' => {
                if offset + 4 > data.len() {
                    return Err(NetworkError::deserialization("truncated frame number"));
                }
                frame = u32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                offset += 4;
            }
            b'D' => {
                // Data section starts here
                data_offset = Some(offset);
                break;
            }
            _ => {
                return Err(NetworkError::deserialization(format!(
                    "invalid tag: {:#x}",
                    tag
                )));
            }
        }
    }

    // Parse command-specific data
    let payload = if let Some(data_start) = data_offset {
        parse_command_data(cmd_type, &data[data_start..])?
    } else {
        // Some commands have no data (e.g., KeepAlive)
        CommandPayload::Generic(Vec::new())
    };

    trace!(
        "Deserialized C++ compat command type={:?} id={} frame={}",
        cmd_type,
        cmd_id,
        frame
    );

    Ok(NetCommandRef {
        command_type: cmd_type,
        relay,
        player_id,
        id: cmd_id,
        execution_frame: frame,
        payload,
    })
}

/// Append command-specific data based on command type
///
/// This matches the C++ FillBufferWith* functions
fn append_command_data(cmd: &NetCommandRef, buf: &mut Vec<u8>) {
    match (&cmd.payload, cmd.command_type) {
        (CommandPayload::GameCommand(data), NetCommandType::GameCommand) => {
            append_game_command_data(data, buf);
        }
        (CommandPayload::Ack(_), NetCommandType::AckBoth)
        | (CommandPayload::Ack(_), NetCommandType::AckStage1)
        | (CommandPayload::Ack(_), NetCommandType::AckStage2) => {
            // AckBoth: frame number (4 bytes)
            buf.extend_from_slice(&cmd.execution_frame.to_le_bytes());
        }
        (CommandPayload::FrameInfo(data), NetCommandType::FrameInfo) => {
            // Frame: frame number (4 bytes) + CRC (4 bytes)
            buf.extend_from_slice(&data.frame.to_le_bytes());
            buf.extend_from_slice(&data.checksum.to_le_bytes());
        }
        (CommandPayload::PlayerLeave(data), NetCommandType::PlayerLeave) => {
            // PlayerLeave: leaving_player_id (1 byte)
            // C++ Format (NetPacket.cpp:5437-5451): Payload contains the player ID who is leaving
            buf.push(data.leaving_player_id);
        }
        (CommandPayload::RunAheadMetrics(data), NetCommandType::RunAheadMetrics) => {
            // RunAheadMetrics: average_latency (4 bytes) + average_fps (4 bytes) + recommended_frames (2 bytes)
            buf.extend_from_slice(&data.average_latency.to_le_bytes());
            buf.extend_from_slice(&data.average_fps.to_le_bytes());
            buf.extend_from_slice(&data.recommended_frames.to_le_bytes());
        }
        (CommandPayload::Chat(data), NetCommandType::Chat) => {
            append_chat_data(data, buf);
        }
        (CommandPayload::Chat(data), NetCommandType::DisconnectChat) => {
            append_disconnect_chat_data(data, buf);
        }
        (CommandPayload::Progress(data), NetCommandType::Progress) => {
            // Progress: type (1 byte) + percentage (1 byte)
            buf.push(data.progress_type as u8);
            buf.push(data.percentage);
        }
        (CommandPayload::KeepAlive, NetCommandType::LoadComplete) => {
            // LoadComplete has NO payload in C++ (just the 'D' tag)
            // No data to append
        }
        (CommandPayload::FileProgress(data), NetCommandType::FileProgress) => {
            // FileProgress: file_id (2 bytes u16) + progress (4 bytes i32)
            buf.extend_from_slice(&data.file_id.to_le_bytes());
            buf.extend_from_slice(&data.progress.to_le_bytes());
        }
        (CommandPayload::FileAnnouncement(data), NetCommandType::FileAnnounce) => {
            append_file_announcement_data(data, buf);
        }
        (CommandPayload::FileTransfer(data), NetCommandType::File) => {
            append_file_transfer_data(data, buf);
        }
        (CommandPayload::DisconnectVote(data), NetCommandType::DisconnectVote) => {
            // DisconnectVote: target_slot (1 byte) + vote_frame (4 bytes) + vote_type (1 byte)
            buf.push(data.target_slot);
            buf.extend_from_slice(&data.vote_frame.to_le_bytes());
            buf.push(data.vote_type as u8);
        }
        (CommandPayload::Wrapper(data), NetCommandType::Wrapper) => {
            // Wrapper command serialization
            if let Ok(serialized) = data.serialize() {
                buf.extend_from_slice(&serialized);
            } else {
                debug!("Failed to serialize wrapper command");
            }
        }
        (CommandPayload::DisconnectPlayer(data), NetCommandType::DisconnectPlayer) => {
            // DisconnectPlayer: disconnect_slot (1 byte) + disconnect_frame (4 bytes)
            buf.push(data.disconnect_slot);
            buf.extend_from_slice(&data.disconnect_frame.to_le_bytes());
        }
        (CommandPayload::DisconnectFrame(data), NetCommandType::DisconnectFrame) => {
            // DisconnectFrame: disconnect_frame (4 bytes)
            buf.extend_from_slice(&data.disconnect_frame.to_le_bytes());
        }
        (CommandPayload::DisconnectScreenOff(data), NetCommandType::DisconnectScreenOff) => {
            // DisconnectScreenOff: new_frame (4 bytes)
            buf.extend_from_slice(&data.new_frame.to_le_bytes());
        }
        (CommandPayload::RunAhead(data), NetCommandType::RunAhead) => {
            // RunAhead: run_ahead (2 bytes u16) + frame_rate (1 byte u8)
            buf.extend_from_slice(&data.run_ahead.to_le_bytes());
            buf.push(data.frame_rate);
        }
        (CommandPayload::FrameResendRequest(data), NetCommandType::FrameResendRequest) => {
            // FrameResendRequest: frame_number (4 bytes)
            buf.extend_from_slice(&data.frame_number.to_le_bytes());
        }
        (CommandPayload::KeepAlive, NetCommandType::KeepAlive) => {
            // KeepAlive has no data
        }
        (CommandPayload::Generic(data), _) => {
            // Generic payload - just append the data
            buf.extend_from_slice(data);
        }
        _ => {
            // Unknown or unhandled command type - empty data
            debug!(
                "Unhandled command type for serialization: {:?}",
                cmd.command_type
            );
        }
    }
}

/// Append GameCommand data matching C++ format
///
/// C++ Format (from FillBufferWithGameCommand):
/// - GameMessage::Type (4 bytes)
/// - numTypes (1 byte) - number of argument type runs
/// - For each type run:
///   - type (1 byte)
///   - argCount (1 byte)
/// - For each argument in original order:
///   - value (size depends on type)
fn append_game_command_data(data: &GameCommandData, buf: &mut Vec<u8>) {
    let mut arguments = Vec::new();

    if let Some(target) = data.target_id {
        arguments.push(GameMessageArgument::new(
            GameMessageArgumentValue::ObjectID(target),
        ));
    }

    if let Some((x, y, z)) = data.position {
        arguments.push(GameMessageArgument::new(
            GameMessageArgumentValue::Location(Coord3D { x, y, z }),
        ));
    }

    for (_, value) in ordered_parameters(&data.parameters) {
        if let Some(arg) = command_param_to_argument(value) {
            arguments.push(arg);
        }
    }

    let game_msg = GameMessage {
        message_type: data.command_type,
        player_index: 0,
        arguments,
    };

    if let Ok(bytes) = game_msg.serialize_cpp_compatible() {
        buf.extend_from_slice(&bytes);
    } else {
        debug!("Failed to serialize GameMessage for C++ compat");
    }
}

/// Append chat data
/// C++ Format (NetPacket.cpp:5583-5610):
/// - text_length: u8 (max 255 chars)
/// - text: u16[] (UTF-16 chars)
/// - player_mask: i32 (signed 4-byte int)
fn append_chat_data(data: &ChatData, buf: &mut Vec<u8>) {
    // Message length (1 byte u8) - max 255 chars
    let utf16: Vec<u16> = data.message.encode_utf16().collect();
    let msg_len = utf16.len().min(255);
    buf.push(msg_len as u8);

    // UTF-16 message
    for ch in utf16.iter().take(msg_len) {
        buf.extend_from_slice(&ch.to_le_bytes());
    }

    // Target mask (4 bytes i32, not 1 byte u8)
    buf.extend_from_slice(&data.target_mask.to_le_bytes());
}

/// Append disconnect chat data
/// C++ Format (NetPacket.cpp:5560-5582): NO target_mask for disconnect chat!
/// - text_length: u8 (max 255 chars)
/// - text: u16[] (UTF-16 chars)
fn append_disconnect_chat_data(data: &ChatData, buf: &mut Vec<u8>) {
    // Message length (1 byte u8) - max 255 chars
    let utf16: Vec<u16> = data.message.encode_utf16().collect();
    let msg_len = utf16.len().min(255);
    buf.push(msg_len as u8);

    // UTF-16 message
    for ch in utf16.iter().take(msg_len) {
        buf.extend_from_slice(&ch.to_le_bytes());
    }

    // NO target_mask for DisconnectChat!
}

/// Append file announcement data
fn append_file_announcement_data(data: &FileAnnouncementData, buf: &mut Vec<u8>) {
    // Filename (null-terminated)
    let filename_bytes = data.metadata.filename.as_bytes();
    buf.extend_from_slice(filename_bytes);
    buf.push(0);

    // File ID (u16)
    buf.extend_from_slice(&data.command_id.to_le_bytes());

    // Player mask (1 byte)
    buf.push(data.player_mask);
}

fn append_file_transfer_data(data: &FileTransferData, buf: &mut Vec<u8>) {
    // Filename (null-terminated)
    let filename_bytes = data.filename.as_bytes();
    buf.extend_from_slice(filename_bytes);
    buf.push(0);

    // Data length (u32)
    let len = data.data.len().min(u32::MAX as usize) as u32;
    buf.extend_from_slice(&len.to_le_bytes());

    // Data payload
    buf.extend_from_slice(&data.data[..len as usize]);
}

/// Parse command-specific data based on command type
fn parse_command_data(cmd_type: NetCommandType, data: &[u8]) -> NetworkResult<CommandPayload> {
    let mut cursor = Cursor::new(data);

    match cmd_type {
        NetCommandType::GameCommand => {
            let game_data = parse_game_command_data(&mut cursor)?;
            Ok(CommandPayload::GameCommand(game_data))
        }
        NetCommandType::AckBoth | NetCommandType::AckStage1 | NetCommandType::AckStage2 => {
            // Ack commands have frame number (4 bytes)
            if data.len() < 4 {
                return Ok(CommandPayload::Ack(AckData {
                    command_id: uuid::Uuid::nil(),
                }));
            }
            // For C++ compat, acks just contain the frame - we don't have a UUID
            Ok(CommandPayload::Ack(AckData {
                command_id: uuid::Uuid::nil(),
            }))
        }
        NetCommandType::FrameInfo => {
            let frame_data = parse_frame_info_data(&mut cursor)?;
            Ok(CommandPayload::FrameInfo(frame_data))
        }
        NetCommandType::PlayerLeave => {
            let leave_data = parse_player_leave_data(&mut cursor)?;
            Ok(CommandPayload::PlayerLeave(leave_data))
        }
        NetCommandType::RunAheadMetrics => {
            let metrics_data = parse_run_ahead_metrics_data(&mut cursor)?;
            Ok(CommandPayload::RunAheadMetrics(metrics_data))
        }
        NetCommandType::Chat => {
            let chat_data = parse_chat_data(&mut cursor)?;
            Ok(CommandPayload::Chat(chat_data))
        }
        NetCommandType::DisconnectChat => {
            let chat_data = parse_disconnect_chat_data(&mut cursor)?;
            Ok(CommandPayload::Chat(chat_data))
        }
        NetCommandType::Progress => {
            let progress_data = parse_progress_data(&mut cursor)?;
            Ok(CommandPayload::Progress(progress_data))
        }
        NetCommandType::LoadComplete => {
            // LoadComplete has NO payload (just the 'D' tag)
            Ok(CommandPayload::KeepAlive)
        }
        NetCommandType::FileProgress => {
            let file_progress_data = parse_file_progress_data(&mut cursor)?;
            Ok(CommandPayload::FileProgress(file_progress_data))
        }
        NetCommandType::FileAnnounce => {
            let file_announce_data = parse_file_announcement_data(&mut cursor)?;
            Ok(CommandPayload::FileAnnouncement(file_announce_data))
        }
        NetCommandType::File => {
            let file_transfer_data = parse_file_transfer_data(&mut cursor)?;
            Ok(CommandPayload::FileTransfer(file_transfer_data))
        }
        NetCommandType::DisconnectVote => {
            let vote_data = parse_disconnect_vote_data(&mut cursor)?;
            Ok(CommandPayload::DisconnectVote(vote_data))
        }
        NetCommandType::Wrapper => {
            let wrapper_data = crate::commands::wrapper::WrapperCommand::deserialize(data)?;
            Ok(CommandPayload::Wrapper(wrapper_data))
        }
        NetCommandType::DisconnectPlayer => {
            let disconnect_data = parse_disconnect_player_data(&mut cursor)?;
            Ok(CommandPayload::DisconnectPlayer(disconnect_data))
        }
        NetCommandType::DisconnectFrame => {
            let disconnect_data = parse_disconnect_frame_data(&mut cursor)?;
            Ok(CommandPayload::DisconnectFrame(disconnect_data))
        }
        NetCommandType::DisconnectScreenOff => {
            let disconnect_data = parse_disconnect_screen_off_data(&mut cursor)?;
            Ok(CommandPayload::DisconnectScreenOff(disconnect_data))
        }
        NetCommandType::RunAhead => {
            let run_ahead_data = parse_run_ahead_data(&mut cursor)?;
            Ok(CommandPayload::RunAhead(run_ahead_data))
        }
        NetCommandType::FrameResendRequest => {
            let resend_data = parse_frame_resend_request_data(&mut cursor)?;
            Ok(CommandPayload::FrameResendRequest(resend_data))
        }
        NetCommandType::KeepAlive => Ok(CommandPayload::KeepAlive),
        _ => {
            // Unknown command type - store as generic data
            Ok(CommandPayload::Generic(data.to_vec()))
        }
    }
}

/// Parse GameCommand data
fn parse_game_command_data(cursor: &mut Cursor<&[u8]>) -> NetworkResult<GameCommandData> {
    let command_type = cursor
        .read_u32::<byteorder::LittleEndian>()
        .map_err(|_| NetworkError::deserialization("truncated game command type"))?;

    let num_groups = cursor
        .read_u8()
        .map_err(|_| NetworkError::deserialization("truncated argument group count"))?
        as usize;

    let mut type_headers = Vec::with_capacity(num_groups);
    for _ in 0..num_groups {
        let type_id = cursor
            .read_u8()
            .map_err(|_| NetworkError::deserialization("truncated argument type"))?;
        let count = cursor
            .read_u8()
            .map_err(|_| NetworkError::deserialization("truncated argument count"))?;
        type_headers.push((GameMessageArgumentDataType::from(type_id), count));
    }

    let mut arguments = Vec::new();
    for (data_type, count) in type_headers {
        for _ in 0..count {
            let (arg, consumed) = GameMessageArgument::deserialize_value_only(
                data_type,
                &cursor.get_ref()[(cursor.position() as usize)..],
            )?;
            cursor.set_position(cursor.position() + consumed as u64);
            arguments.push(arg);
        }
    }

    let mut target_id = None;
    let mut position = None;
    let mut parameters = std::collections::HashMap::new();
    let mut arg_index = 0usize;

    for arg in arguments {
        match arg.value {
            GameMessageArgumentValue::ObjectID(id) if target_id.is_none() => {
                target_id = Some(id);
            }
            GameMessageArgumentValue::Location(loc) if position.is_none() => {
                position = Some((loc.x, loc.y, loc.z));
            }
            value => {
                if let Some(param) = argument_to_command_param(value) {
                    let key = format!("arg{:03}", arg_index);
                    parameters.insert(key, param);
                    arg_index += 1;
                }
            }
        }
    }

    Ok(GameCommandData {
        command_type,
        target_id,
        position,
        parameters,
        checksum: 0,
    })
}

fn ordered_parameters<'a>(
    parameters: &'a std::collections::HashMap<String, crate::commands::CommandParameter>,
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

fn command_param_to_argument(
    value: &crate::commands::CommandParameter,
) -> Option<GameMessageArgument> {
    let arg = match value {
        crate::commands::CommandParameter::Int(v) => GameMessageArgumentValue::Integer(*v),
        crate::commands::CommandParameter::Float(v) => GameMessageArgumentValue::Real(*v),
        crate::commands::CommandParameter::Bool(v) => GameMessageArgumentValue::Boolean(*v),
        crate::commands::CommandParameter::ObjectId(v) => GameMessageArgumentValue::ObjectID(*v),
        crate::commands::CommandParameter::Position(x, y, z) => {
            GameMessageArgumentValue::Location(Coord3D {
                x: *x,
                y: *y,
                z: *z,
            })
        }
        crate::commands::CommandParameter::DrawableId(v) => {
            GameMessageArgumentValue::DrawableID(*v)
        }
        crate::commands::CommandParameter::TeamId(v) => GameMessageArgumentValue::TeamID(*v),
        crate::commands::CommandParameter::Pixel(x, y) => {
            GameMessageArgumentValue::Pixel(crate::commands::game_message::ICoord2D {
                x: *x,
                y: *y,
            })
        }
        crate::commands::CommandParameter::PixelRegion(x1, y1, x2, y2) => {
            GameMessageArgumentValue::PixelRegion(crate::commands::game_message::IRegion2D::new(
                *x1, *y1, *x2, *y2,
            ))
        }
        crate::commands::CommandParameter::Timestamp(v) => GameMessageArgumentValue::Timestamp(*v),
        crate::commands::CommandParameter::WideChar(v) => GameMessageArgumentValue::WideChar(*v),
        crate::commands::CommandParameter::String(_) => {
            debug!("String parameter not supported in C++ GameMessage wire format");
            return None;
        }
    };

    Some(GameMessageArgument::new(arg))
}

fn argument_to_command_param(
    value: GameMessageArgumentValue,
) -> Option<crate::commands::CommandParameter> {
    match value {
        GameMessageArgumentValue::Integer(v) => Some(crate::commands::CommandParameter::Int(v)),
        GameMessageArgumentValue::Real(v) => Some(crate::commands::CommandParameter::Float(v)),
        GameMessageArgumentValue::Boolean(v) => Some(crate::commands::CommandParameter::Bool(v)),
        GameMessageArgumentValue::ObjectID(v) => {
            Some(crate::commands::CommandParameter::ObjectId(v))
        }
        GameMessageArgumentValue::DrawableID(v) => {
            Some(crate::commands::CommandParameter::DrawableId(v))
        }
        GameMessageArgumentValue::TeamID(v) => Some(crate::commands::CommandParameter::TeamId(v)),
        GameMessageArgumentValue::Location(loc) => Some(
            crate::commands::CommandParameter::Position(loc.x, loc.y, loc.z),
        ),
        GameMessageArgumentValue::Pixel(pixel) => {
            Some(crate::commands::CommandParameter::Pixel(pixel.x, pixel.y))
        }
        GameMessageArgumentValue::PixelRegion(region) => {
            Some(crate::commands::CommandParameter::PixelRegion(
                region.lo.x,
                region.lo.y,
                region.hi.x,
                region.hi.y,
            ))
        }
        GameMessageArgumentValue::Timestamp(v) => {
            Some(crate::commands::CommandParameter::Timestamp(v))
        }
        GameMessageArgumentValue::WideChar(v) => {
            Some(crate::commands::CommandParameter::WideChar(v))
        }
    }
}

/// Parse FrameInfo data
fn parse_frame_info_data(cursor: &mut Cursor<&[u8]>) -> NetworkResult<FrameInfoData> {
    let mut frame_buf = [0u8; 4];
    let mut checksum_buf = [0u8; 4];

    cursor
        .read_exact(&mut frame_buf)
        .map_err(|_| NetworkError::deserialization("truncated frame number"))?;
    cursor
        .read_exact(&mut checksum_buf)
        .map_err(|_| NetworkError::deserialization("truncated frame checksum"))?;

    Ok(FrameInfoData {
        frame: u32::from_le_bytes(frame_buf),
        command_count: 0, // Not in C++ format
        checksum: u32::from_le_bytes(checksum_buf),
    })
}

/// Parse PlayerLeave data
/// C++ Format (NetPacket.cpp:5437-5451): Payload contains the player ID who is leaving
fn parse_player_leave_data(cursor: &mut Cursor<&[u8]>) -> NetworkResult<PlayerLeaveData> {
    let mut player_id_buf = [0u8; 1];
    cursor
        .read_exact(&mut player_id_buf)
        .map_err(|_| NetworkError::deserialization("truncated player ID"))?;

    Ok(PlayerLeaveData {
        leaving_player_id: player_id_buf[0],
    })
}

/// Parse RunAheadMetrics data
fn parse_run_ahead_metrics_data(cursor: &mut Cursor<&[u8]>) -> NetworkResult<RunAheadMetricsData> {
    let mut latency_buf = [0u8; 4];
    let mut fps_buf = [0u8; 4];
    let mut frames_buf = [0u8; 2];

    cursor
        .read_exact(&mut latency_buf)
        .map_err(|_| NetworkError::deserialization("truncated latency"))?;
    cursor
        .read_exact(&mut fps_buf)
        .map_err(|_| NetworkError::deserialization("truncated fps"))?;
    cursor
        .read_exact(&mut frames_buf)
        .map_err(|_| NetworkError::deserialization("truncated frames"))?;

    Ok(RunAheadMetricsData {
        average_latency: f32::from_le_bytes(latency_buf),
        average_fps: u32::from_le_bytes(fps_buf),
        recommended_frames: u16::from_le_bytes(frames_buf),
    })
}

/// Parse Chat data
/// C++ Format (NetPacket.cpp:5583-5610):
/// - text_length: u8 (max 255 chars)
/// - text: u16[] (UTF-16 chars)
/// - player_mask: i32 (signed 4-byte int)
fn parse_chat_data(cursor: &mut Cursor<&[u8]>) -> NetworkResult<ChatData> {
    // Read length as u8 (NOT u16)
    let mut msg_len_buf = [0u8; 1];
    cursor
        .read_exact(&mut msg_len_buf)
        .map_err(|_| NetworkError::deserialization("truncated message length"))?;
    let msg_len = msg_len_buf[0] as usize;

    // Read UTF-16 characters
    let mut utf16_chars = Vec::with_capacity(msg_len);
    for _ in 0..msg_len {
        let mut char_buf = [0u8; 2];
        cursor
            .read_exact(&mut char_buf)
            .map_err(|_| NetworkError::deserialization("truncated UTF-16 character"))?;
        utf16_chars.push(u16::from_le_bytes(char_buf));
    }

    let message = String::from_utf16(&utf16_chars)
        .map_err(|_| NetworkError::deserialization("invalid UTF-16 string"))?;

    // Read target mask as i32 (NOT u8)
    let mut target_mask_buf = [0u8; 4];
    cursor
        .read_exact(&mut target_mask_buf)
        .map_err(|_| NetworkError::deserialization("truncated target mask"))?;

    Ok(ChatData {
        message,
        target_mask: i32::from_le_bytes(target_mask_buf),
    })
}

/// Parse Progress data
fn parse_progress_data(cursor: &mut Cursor<&[u8]>) -> NetworkResult<ProgressData> {
    let mut progress_type_buf = [0u8; 1];
    let mut percentage_buf = [0u8; 1];

    cursor
        .read_exact(&mut progress_type_buf)
        .map_err(|_| NetworkError::deserialization("truncated progress type"))?;
    cursor
        .read_exact(&mut percentage_buf)
        .map_err(|_| NetworkError::deserialization("truncated percentage"))?;

    Ok(ProgressData {
        progress_type: match progress_type_buf[0] {
            0 => crate::commands::ProgressType::Loading,
            1 => crate::commands::ProgressType::Connection,
            2 => crate::commands::ProgressType::FileTransfer,
            _ => crate::commands::ProgressType::Loading,
        },
        percentage: percentage_buf[0],
    })
}

/// Parse FileProgress data
/// C++ Format: file_id (u16) + progress (i32)
fn parse_file_progress_data(cursor: &mut Cursor<&[u8]>) -> NetworkResult<FileProgressData> {
    let mut file_id_buf = [0u8; 2];
    let mut progress_buf = [0u8; 4];

    cursor
        .read_exact(&mut file_id_buf)
        .map_err(|_| NetworkError::deserialization("truncated file id"))?;
    cursor
        .read_exact(&mut progress_buf)
        .map_err(|_| NetworkError::deserialization("truncated progress"))?;

    Ok(FileProgressData {
        file_id: u16::from_le_bytes(file_id_buf),
        progress: i32::from_le_bytes(progress_buf),
    })
}

/// Parse FileAnnouncement data
fn parse_file_announcement_data(cursor: &mut Cursor<&[u8]>) -> NetworkResult<FileAnnouncementData> {
    let filename = read_cstring(cursor)?;

    let mut cmd_id_buf = [0u8; 2];
    let mut player_mask_buf = [0u8; 1];

    cursor
        .read_exact(&mut cmd_id_buf)
        .map_err(|_| NetworkError::deserialization("truncated command id"))?;
    cursor
        .read_exact(&mut player_mask_buf)
        .map_err(|_| NetworkError::deserialization("truncated player mask"))?;

    Ok(FileAnnouncementData {
        command_id: u16::from_le_bytes(cmd_id_buf),
        player_mask: player_mask_buf[0],
        metadata: crate::file_transfer::FileMetadata {
            filename,
            file_size: 0,
            checksum: [0u8; 32],
            transfer_type: crate::file_transfer::TransferType::Generic,
        },
    })
}

/// Parse FileTransfer data
fn parse_file_transfer_data(cursor: &mut Cursor<&[u8]>) -> NetworkResult<FileTransferData> {
    let filename = read_cstring(cursor)?;

    let mut len_buf = [0u8; 4];
    cursor
        .read_exact(&mut len_buf)
        .map_err(|_| NetworkError::deserialization("truncated file length"))?;
    let len = u32::from_le_bytes(len_buf) as usize;

    let mut data = vec![0u8; len];
    cursor
        .read_exact(&mut data)
        .map_err(|_| NetworkError::deserialization("truncated file payload"))?;

    Ok(FileTransferData {
        file_id: 0,
        filename,
        data,
        chunk_number: 0,
        total_chunks: 1,
        checksum: 0,
    })
}

fn read_cstring(cursor: &mut Cursor<&[u8]>) -> NetworkResult<String> {
    let mut bytes = Vec::new();
    loop {
        let mut buf = [0u8; 1];
        cursor
            .read_exact(&mut buf)
            .map_err(|_| NetworkError::deserialization("truncated cstring"))?;
        if buf[0] == 0 {
            break;
        }
        bytes.push(buf[0]);
    }

    Ok(String::from_utf8_lossy(&bytes).to_string())
}

/// Parse DisconnectVote data
fn parse_disconnect_vote_data(cursor: &mut Cursor<&[u8]>) -> NetworkResult<DisconnectVoteData> {
    let mut target_slot_buf = [0u8; 1];
    let mut vote_frame_buf = [0u8; 4];
    let mut vote_type_buf = [0u8; 1];

    cursor
        .read_exact(&mut target_slot_buf)
        .map_err(|_| NetworkError::deserialization("truncated target slot"))?;
    cursor
        .read_exact(&mut vote_frame_buf)
        .map_err(|_| NetworkError::deserialization("truncated vote frame"))?;
    cursor
        .read_exact(&mut vote_type_buf)
        .map_err(|_| NetworkError::deserialization("truncated vote type"))?;

    Ok(DisconnectVoteData {
        target_slot: target_slot_buf[0],
        vote_frame: u32::from_le_bytes(vote_frame_buf),
        vote_type: match vote_type_buf[0] {
            0 => crate::commands::DisconnectVoteType::Kick,
            1 => crate::commands::DisconnectVoteType::Timeout,
            2 => crate::commands::DisconnectVoteType::NetworkIssues,
            _ => crate::commands::DisconnectVoteType::Kick,
        },
    })
}

/// Parse DisconnectChat data (NO target_mask)
/// C++ Format (NetPacket.cpp:5560-5582): text_length (u8) + text (u16[] UTF-16)
fn parse_disconnect_chat_data(cursor: &mut Cursor<&[u8]>) -> NetworkResult<ChatData> {
    // Read length as u8
    let mut msg_len_buf = [0u8; 1];
    cursor
        .read_exact(&mut msg_len_buf)
        .map_err(|_| NetworkError::deserialization("truncated message length"))?;
    let msg_len = msg_len_buf[0] as usize;

    // Read UTF-16 characters
    let mut utf16_chars = Vec::with_capacity(msg_len);
    for _ in 0..msg_len {
        let mut char_buf = [0u8; 2];
        cursor
            .read_exact(&mut char_buf)
            .map_err(|_| NetworkError::deserialization("truncated UTF-16 character"))?;
        utf16_chars.push(u16::from_le_bytes(char_buf));
    }

    let message = String::from_utf16(&utf16_chars)
        .map_err(|_| NetworkError::deserialization("invalid UTF-16 string"))?;

    // NO target_mask for DisconnectChat!
    Ok(ChatData {
        message,
        target_mask: -1, // Default value, not read from wire
    })
}

/// Parse DisconnectPlayer data
fn parse_disconnect_player_data(cursor: &mut Cursor<&[u8]>) -> NetworkResult<DisconnectPlayerData> {
    let mut slot_buf = [0u8; 1];
    let mut frame_buf = [0u8; 4];

    cursor
        .read_exact(&mut slot_buf)
        .map_err(|_| NetworkError::deserialization("truncated disconnect slot"))?;
    cursor
        .read_exact(&mut frame_buf)
        .map_err(|_| NetworkError::deserialization("truncated disconnect frame"))?;

    Ok(DisconnectPlayerData {
        disconnect_slot: slot_buf[0],
        disconnect_frame: u32::from_le_bytes(frame_buf),
    })
}

/// Parse DisconnectFrame data
fn parse_disconnect_frame_data(cursor: &mut Cursor<&[u8]>) -> NetworkResult<DisconnectFrameData> {
    let mut frame_buf = [0u8; 4];

    cursor
        .read_exact(&mut frame_buf)
        .map_err(|_| NetworkError::deserialization("truncated disconnect frame"))?;

    Ok(DisconnectFrameData {
        disconnect_frame: u32::from_le_bytes(frame_buf),
    })
}

/// Parse DisconnectScreenOff data
fn parse_disconnect_screen_off_data(
    cursor: &mut Cursor<&[u8]>,
) -> NetworkResult<DisconnectScreenOffData> {
    let mut frame_buf = [0u8; 4];

    cursor
        .read_exact(&mut frame_buf)
        .map_err(|_| NetworkError::deserialization("truncated new frame"))?;

    Ok(DisconnectScreenOffData {
        new_frame: u32::from_le_bytes(frame_buf),
    })
}

/// Parse RunAhead data
fn parse_run_ahead_data(cursor: &mut Cursor<&[u8]>) -> NetworkResult<RunAheadData> {
    let mut run_ahead_buf = [0u8; 2];
    let mut frame_rate_buf = [0u8; 1];

    cursor
        .read_exact(&mut run_ahead_buf)
        .map_err(|_| NetworkError::deserialization("truncated run ahead"))?;
    cursor
        .read_exact(&mut frame_rate_buf)
        .map_err(|_| NetworkError::deserialization("truncated frame rate"))?;

    Ok(RunAheadData {
        run_ahead: u16::from_le_bytes(run_ahead_buf),
        frame_rate: frame_rate_buf[0],
    })
}

/// Parse FrameResendRequest data
fn parse_frame_resend_request_data(
    cursor: &mut Cursor<&[u8]>,
) -> NetworkResult<FrameResendRequestData> {
    let mut frame_buf = [0u8; 4];

    cursor
        .read_exact(&mut frame_buf)
        .map_err(|_| NetworkError::deserialization("truncated frame number"))?;

    Ok(FrameResendRequestData {
        frame_number: u32::from_le_bytes(frame_buf),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_serialize_keep_alive() {
        let cmd = NetCommandRef {
            command_type: NetCommandType::KeepAlive,
            relay: 0,
            player_id: 1,
            id: 42,
            execution_frame: 100,
            payload: CommandPayload::KeepAlive,
        };

        let serialized = serialize_command_cpp_compat(&cmd);

        // Expected format: T[type]F[frame]R[relay]P[player]C[id]D
        assert_eq!(serialized[0], b'T');
        assert_eq!(serialized[1], NetCommandType::KeepAlive as u8);
        assert_eq!(serialized[2], b'F');
        assert_eq!(
            u32::from_le_bytes([serialized[3], serialized[4], serialized[5], serialized[6]]),
            100
        );
        assert_eq!(serialized[7], b'R');
        assert_eq!(serialized[8], 0);
        assert_eq!(serialized[9], b'P');
        assert_eq!(serialized[10], 1);
        assert_eq!(serialized[11], b'C');
        assert_eq!(u16::from_le_bytes([serialized[12], serialized[13]]), 42);
        assert_eq!(serialized[14], b'D');
    }

    #[test]
    fn test_deserialize_keep_alive() {
        // Manually construct a C++-format KeepAlive command
        let mut data = Vec::new();
        data.push(b'T');
        data.push(NetCommandType::KeepAlive as u8);
        data.push(b'F');
        data.extend_from_slice(&100u32.to_le_bytes());
        data.push(b'R');
        data.push(0);
        data.push(b'P');
        data.push(1);
        data.push(b'C');
        data.extend_from_slice(&42u16.to_le_bytes());
        data.push(b'D');

        let cmd = deserialize_command_cpp_compat(&data).unwrap();

        assert_eq!(cmd.command_type, NetCommandType::KeepAlive);
        assert_eq!(cmd.player_id, 1);
        assert_eq!(cmd.id, 42);
        assert_eq!(cmd.execution_frame, 100);
        assert_eq!(cmd.relay, 0);
    }

    #[test]
    fn test_round_trip_keep_alive() {
        let original = NetCommandRef {
            command_type: NetCommandType::KeepAlive,
            relay: 0,
            player_id: 2,
            id: 123,
            execution_frame: 500,
            payload: CommandPayload::KeepAlive,
        };

        let serialized = serialize_command_cpp_compat(&original);
        let deserialized = deserialize_command_cpp_compat(&serialized).unwrap();

        assert_eq!(original.command_type, deserialized.command_type);
        assert_eq!(original.player_id, deserialized.player_id);
        assert_eq!(original.id, deserialized.id);
        assert_eq!(original.execution_frame, deserialized.execution_frame);
        assert_eq!(original.relay, deserialized.relay);
    }

    #[test]
    fn test_serialize_frame_info() {
        let cmd = NetCommandRef {
            command_type: NetCommandType::FrameInfo,
            relay: 0,
            player_id: 0,
            id: 1,
            execution_frame: 200,
            payload: CommandPayload::FrameInfo(FrameInfoData {
                frame: 200,
                command_count: 5,
                checksum: 0xDEADBEEF,
            }),
        };

        let serialized = serialize_command_cpp_compat(&cmd);
        let deserialized = deserialize_command_cpp_compat(&serialized).unwrap();

        assert_eq!(cmd.command_type, deserialized.command_type);
        if let CommandPayload::FrameInfo(data) = deserialized.payload {
            assert_eq!(data.frame, 200);
            assert_eq!(data.checksum, 0xDEADBEEF);
        } else {
            panic!("Expected FrameInfo payload");
        }
    }

    #[test]
    fn test_serialize_chat() {
        let cmd = NetCommandRef {
            command_type: NetCommandType::Chat,
            relay: 0,
            player_id: 1,
            id: 10,
            execution_frame: 0,
            payload: CommandPayload::Chat(ChatData {
                message: "Hello, world!".to_string(),
                target_mask: 0xFF,
            }),
        };

        let serialized = serialize_command_cpp_compat(&cmd);
        let deserialized = deserialize_command_cpp_compat(&serialized).unwrap();

        assert_eq!(cmd.command_type, deserialized.command_type);
        if let CommandPayload::Chat(data) = deserialized.payload {
            assert_eq!(data.message, "Hello, world!");
            assert_eq!(data.target_mask, 0xFF);
        } else {
            panic!("Expected Chat payload");
        }
    }

    #[test]
    fn test_serialize_game_command() {
        let mut params = HashMap::new();
        params.insert(
            "target".to_string(),
            crate::commands::CommandParameter::ObjectId(999),
        );

        let cmd = NetCommandRef {
            command_type: NetCommandType::GameCommand,
            relay: 0,
            player_id: 0,
            id: 50,
            execution_frame: 1000,
            payload: CommandPayload::GameCommand(GameCommandData {
                command_type: 1,
                target_id: Some(123),
                position: Some((10.0, 20.0, 30.0)),
                parameters: params,
                checksum: 0x12345678,
            }),
        };

        let serialized = serialize_command_cpp_compat(&cmd);
        let deserialized = deserialize_command_cpp_compat(&serialized).unwrap();

        assert_eq!(cmd.command_type, deserialized.command_type);
        if let CommandPayload::GameCommand(data) = deserialized.payload {
            assert_eq!(data.command_type, 1);
            assert_eq!(data.target_id, Some(123));
            assert_eq!(data.position, Some((10.0, 20.0, 30.0)));
            assert_eq!(
                data.parameters.get("arg000"),
                Some(&crate::commands::CommandParameter::ObjectId(999))
            );
            assert_eq!(data.checksum, 0);
        } else {
            panic!("Expected GameCommand payload");
        }
    }

    #[test]
    fn test_tag_order_flexibility() {
        // Test that deserialization handles tags in different orders
        // C++ can send tags in any order, so we need to handle that

        // Order: T, R, P, C, F, D (different from our serialization)
        let mut data = Vec::new();
        data.push(b'T');
        data.push(NetCommandType::KeepAlive as u8);
        data.push(b'R');
        data.push(0);
        data.push(b'P');
        data.push(3);
        data.push(b'C');
        data.extend_from_slice(&99u16.to_le_bytes());
        data.push(b'F');
        data.extend_from_slice(&777u32.to_le_bytes());
        data.push(b'D');

        let cmd = deserialize_command_cpp_compat(&data).unwrap();

        assert_eq!(cmd.command_type, NetCommandType::KeepAlive);
        assert_eq!(cmd.player_id, 3);
        assert_eq!(cmd.id, 99);
        assert_eq!(cmd.execution_frame, 777);
        assert_eq!(cmd.relay, 0);
    }

    #[test]
    fn test_invalid_tag() {
        let data = vec![b'X', 0, 0, 0]; // Invalid tag 'X'
        let result = deserialize_command_cpp_compat(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_truncated_data() {
        let data = vec![b'T']; // Missing command type
        let result = deserialize_command_cpp_compat(&data);
        assert!(result.is_err());
    }

    // ===== PHASE 3 TESTS: Minor Fixes for 100% C++ Compatibility =====

    #[test]
    fn test_disconnect_chat_no_target_mask() {
        // DisconnectChat should NOT include target_mask (unlike regular Chat)
        let cmd = NetCommandRef {
            command_type: NetCommandType::DisconnectChat,
            relay: 0,
            player_id: 1,
            id: 10,
            execution_frame: 0,
            payload: CommandPayload::Chat(ChatData {
                message: "Player disconnected".to_string(),
                target_mask: -1, // Should not be serialized
            }),
        };

        let serialized = serialize_command_cpp_compat(&cmd);
        let deserialized = deserialize_command_cpp_compat(&serialized).unwrap();

        if let CommandPayload::Chat(data) = deserialized.payload {
            assert_eq!(data.message, "Player disconnected");
            // target_mask should be -1 (default) since it wasn't serialized
            assert_eq!(data.target_mask, -1);
        } else {
            panic!("Expected Chat payload for DisconnectChat");
        }
    }

    #[test]
    fn test_load_complete_no_payload() {
        // LoadComplete should have NO payload (just the 'D' tag)
        let cmd = NetCommandRef {
            command_type: NetCommandType::LoadComplete,
            relay: 0,
            player_id: 2,
            id: 15,
            execution_frame: 0,
            payload: CommandPayload::KeepAlive, // LoadComplete uses KeepAlive as empty payload
        };

        let serialized = serialize_command_cpp_compat(&cmd);
        let deserialized = deserialize_command_cpp_compat(&serialized).unwrap();

        // Should deserialize to KeepAlive payload (empty)
        assert!(matches!(deserialized.payload, CommandPayload::KeepAlive));
    }

    #[test]
    fn test_file_progress_i32_type() {
        // FileProgress should use i32 for progress, not u8
        let cmd = NetCommandRef {
            command_type: NetCommandType::FileProgress,
            relay: 0,
            player_id: 1,
            id: 20,
            execution_frame: 0,
            payload: CommandPayload::FileProgress(FileProgressData {
                file_id: 42,
                progress: 75, // i32 value
            }),
        };

        let serialized = serialize_command_cpp_compat(&cmd);
        let deserialized = deserialize_command_cpp_compat(&serialized).unwrap();

        if let CommandPayload::FileProgress(data) = deserialized.payload {
            assert_eq!(data.file_id, 42);
            assert_eq!(data.progress, 75);
        } else {
            panic!("Expected FileProgress payload");
        }
    }

    #[test]
    fn test_file_progress_negative_value() {
        // FileProgress can have negative values (e.g., error codes)
        let cmd = NetCommandRef {
            command_type: NetCommandType::FileProgress,
            relay: 0,
            player_id: 1,
            id: 21,
            execution_frame: 0,
            payload: CommandPayload::FileProgress(FileProgressData {
                file_id: 99,
                progress: -1, // Negative value for error
            }),
        };

        let serialized = serialize_command_cpp_compat(&cmd);
        let deserialized = deserialize_command_cpp_compat(&serialized).unwrap();

        if let CommandPayload::FileProgress(data) = deserialized.payload {
            assert_eq!(data.file_id, 99);
            assert_eq!(data.progress, -1); // Should preserve negative value
        } else {
            panic!("Expected FileProgress payload");
        }
    }

    #[test]
    fn test_disconnect_player_serialization() {
        let cmd = NetCommandRef {
            command_type: NetCommandType::DisconnectPlayer,
            relay: 0,
            player_id: 0,
            id: 30,
            execution_frame: 0,
            payload: CommandPayload::DisconnectPlayer(DisconnectPlayerData {
                disconnect_slot: 3,
                disconnect_frame: 1000,
            }),
        };

        let serialized = serialize_command_cpp_compat(&cmd);
        let deserialized = deserialize_command_cpp_compat(&serialized).unwrap();

        if let CommandPayload::DisconnectPlayer(data) = deserialized.payload {
            assert_eq!(data.disconnect_slot, 3);
            assert_eq!(data.disconnect_frame, 1000);
        } else {
            panic!("Expected DisconnectPlayer payload");
        }
    }

    #[test]
    fn test_disconnect_frame_serialization() {
        let cmd = NetCommandRef {
            command_type: NetCommandType::DisconnectFrame,
            relay: 0,
            player_id: 1,
            id: 31,
            execution_frame: 0,
            payload: CommandPayload::DisconnectFrame(DisconnectFrameData {
                disconnect_frame: 2500,
            }),
        };

        let serialized = serialize_command_cpp_compat(&cmd);
        let deserialized = deserialize_command_cpp_compat(&serialized).unwrap();

        if let CommandPayload::DisconnectFrame(data) = deserialized.payload {
            assert_eq!(data.disconnect_frame, 2500);
        } else {
            panic!("Expected DisconnectFrame payload");
        }
    }

    #[test]
    fn test_disconnect_screen_off_serialization() {
        let cmd = NetCommandRef {
            command_type: NetCommandType::DisconnectScreenOff,
            relay: 0,
            player_id: 2,
            id: 32,
            execution_frame: 0,
            payload: CommandPayload::DisconnectScreenOff(DisconnectScreenOffData {
                new_frame: 3000,
            }),
        };

        let serialized = serialize_command_cpp_compat(&cmd);
        let deserialized = deserialize_command_cpp_compat(&serialized).unwrap();

        if let CommandPayload::DisconnectScreenOff(data) = deserialized.payload {
            assert_eq!(data.new_frame, 3000);
        } else {
            panic!("Expected DisconnectScreenOff payload");
        }
    }

    #[test]
    fn test_run_ahead_serialization() {
        let cmd = NetCommandRef {
            command_type: NetCommandType::RunAhead,
            relay: 0,
            player_id: 0,
            id: 40,
            execution_frame: 0,
            payload: CommandPayload::RunAhead(RunAheadData {
                run_ahead: 5,
                frame_rate: 30,
            }),
        };

        let serialized = serialize_command_cpp_compat(&cmd);
        let deserialized = deserialize_command_cpp_compat(&serialized).unwrap();

        if let CommandPayload::RunAhead(data) = deserialized.payload {
            assert_eq!(data.run_ahead, 5);
            assert_eq!(data.frame_rate, 30);
        } else {
            panic!("Expected RunAhead payload");
        }
    }

    #[test]
    fn test_frame_resend_request_serialization() {
        let cmd = NetCommandRef {
            command_type: NetCommandType::FrameResendRequest,
            relay: 0,
            player_id: 1,
            id: 50,
            execution_frame: 0,
            payload: CommandPayload::FrameResendRequest(FrameResendRequestData {
                frame_number: 95,
            }),
        };

        let serialized = serialize_command_cpp_compat(&cmd);
        let deserialized = deserialize_command_cpp_compat(&serialized).unwrap();

        if let CommandPayload::FrameResendRequest(data) = deserialized.payload {
            assert_eq!(data.frame_number, 95);
        } else {
            panic!("Expected FrameResendRequest payload");
        }
    }

    #[test]
    fn test_wrapper_command_integration() {
        use crate::commands::wrapper::WrapperCommand;

        // Create a wrapper command for a large message
        let wrapper = WrapperCommand::new(
            123,             // wrapped_command_id
            0,               // chunk_number
            3,               // num_chunks
            0,               // data_offset
            1000,            // total_data_length
            vec![0xAB; 400], // chunk_data
        );

        let cmd = NetCommandRef {
            command_type: NetCommandType::Wrapper,
            relay: 0,
            player_id: 2,
            id: 60,
            execution_frame: 0,
            payload: CommandPayload::Wrapper(wrapper),
        };

        let serialized = serialize_command_cpp_compat(&cmd);
        let deserialized = deserialize_command_cpp_compat(&serialized).unwrap();

        if let CommandPayload::Wrapper(data) = deserialized.payload {
            assert_eq!(data.wrapped_command_id, 123);
            assert_eq!(data.chunk_number, 0);
            assert_eq!(data.num_chunks, 3);
            assert_eq!(data.total_data_length, 1000);
            assert_eq!(data.data.len(), 400);
            assert_eq!(data.data[0], 0xAB);
        } else {
            panic!("Expected Wrapper payload");
        }
    }
}
