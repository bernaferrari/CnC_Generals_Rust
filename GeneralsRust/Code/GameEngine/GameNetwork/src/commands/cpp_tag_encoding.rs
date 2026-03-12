//! C++ Tag-Value Encoding for Network Commands
//!
//! This module implements the C++ wire format for network command headers.
//! Unlike the Rust fixed 8-byte binary header, C++ uses variable-length
//! tag-value encoding with ASCII markers.
//!
//! ## C++ Format (from NetPacket.cpp:43-76)
//!
//! ```text
//! Tag-Value Pairs:
//! 'T' [1 byte] = command_type
//! 'P' [1 byte] = player_id
//! 'C' [2 bytes LE] = command_id
//! 'R' [1 byte] = relay (usually 0)
//! 'F' [4 bytes LE] = frame_number (optional, may be omitted)
//! 'D' = start of data payload
//! ```
//!
//! ## Example C++ packet:
//!
//! ```text
//! T 04 P 00 C 01 00 R 00 F 05 00 00 00 D [payload...]
//! ```

use crate::commands::{NetCommand, NetCommandType};
use crate::error::{NetworkError, NetworkResult};

/// C++ command header structure
#[derive(Debug, Clone, PartialEq)]
pub struct CppHeader {
    /// Command type
    pub command_type: NetCommandType,
    /// Player ID who sent command
    pub player_id: u8,
    /// Command sequence ID
    pub command_id: u16,
    /// Relay flag (usually 0)
    pub relay: u8,
    /// Frame number for execution (0 if not specified)
    pub frame: u32,
}

impl Default for CppHeader {
    fn default() -> Self {
        Self {
            command_type: NetCommandType::Unknown,
            player_id: 0,
            command_id: 0,
            relay: 0,
            frame: 0,
        }
    }
}

impl CppHeader {
    /// Create a new C++ header from a NetCommand
    pub fn from_command(cmd: &NetCommand) -> Self {
        Self {
            command_type: cmd.command_type,
            player_id: cmd.player_id,
            command_id: cmd.sequence,
            relay: 0,
            frame: cmd.execution_frame,
        }
    }

    /// Convert this header to a NetCommand (partial - needs payload)
    pub fn to_command(&self) -> NetCommand {
        NetCommand::new(
            self.command_type,
            self.player_id,
            self.frame,
            crate::commands::CommandPayload::Generic(Vec::new()),
        )
        .with_sequence(self.command_id)
    }
}

/// Encode command header using C++ tag-value format
///
/// # Arguments
///
/// * `cmd` - Network command to encode header for
///
/// # Returns
///
/// Vector of bytes containing the C++ tag-value encoded header
///
/// # Example
///
/// ```
/// use game_network::commands::NetCommand;
/// use game_network::commands::cpp_tag_encoding::encode_cpp_header;
///
/// let cmd = NetCommand::keep_alive(0);
/// let header = encode_cpp_header(&cmd);
/// // header now contains: T 09 P 00 C 00 00 R 00 D
/// ```
pub fn encode_cpp_header(cmd: &NetCommand) -> Vec<u8> {
    let mut buf = Vec::with_capacity(20);

    // Type tag
    buf.push(b'T');
    buf.push(cmd.command_type as u8);

    // Player tag
    buf.push(b'P');
    buf.push(cmd.player_id);

    // Command ID tag (little-endian u16)
    buf.push(b'C');
    buf.extend_from_slice(&cmd.sequence.to_le_bytes());

    // Relay tag
    buf.push(b'R');
    buf.push(0); // relay is usually 0

    // Frame tag (only if execution_frame > 0)
    if cmd.execution_frame > 0 {
        buf.push(b'F');
        buf.extend_from_slice(&cmd.execution_frame.to_le_bytes());
    }

    // Data tag marks start of payload
    buf.push(b'D');

    buf
}

/// Parse C++ tag-value header, return (header, remaining_data)
///
/// # Arguments
///
/// * `data` - Raw bytes to parse
///
/// # Returns
///
/// Tuple of (parsed header, remaining payload data slice)
///
/// # Errors
///
/// Returns error if:
/// - Data is incomplete
/// - Invalid tag encountered
/// - Missing required 'D' tag
///
/// # Example
///
/// ```no_run
/// use game_network::commands::cpp_tag_encoding::decode_cpp_header;
///
/// let packet = vec![b'T', 4, b'P', 0, b'C', 1, 0, b'R', 0, b'D'];
/// let (header, payload) = decode_cpp_header(&packet)?;
/// assert_eq!(header.command_type as u8, 4); // GameCommand
/// assert_eq!(header.player_id, 0);
/// # Ok::<(), game_network::error::NetworkError>(())
/// ```
pub fn decode_cpp_header(data: &[u8]) -> NetworkResult<(CppHeader, &[u8])> {
    let mut header = CppHeader::default();
    let mut offset = 0;

    while offset < data.len() {
        let tag = data[offset];
        match tag {
            b'T' => {
                offset += 1;
                if offset >= data.len() {
                    return Err(NetworkError::invalid_packet(
                        "incomplete T tag: missing type byte",
                    ));
                }
                let type_value = data[offset];
                header.command_type = NetCommandType::from(type_value);
                offset += 1;
            }
            b'P' => {
                offset += 1;
                if offset >= data.len() {
                    return Err(NetworkError::invalid_packet(
                        "incomplete P tag: missing player ID",
                    ));
                }
                header.player_id = data[offset];
                offset += 1;
            }
            b'C' => {
                offset += 1;
                if offset + 2 > data.len() {
                    return Err(NetworkError::invalid_packet(
                        "incomplete C tag: missing command ID",
                    ));
                }
                header.command_id = u16::from_le_bytes([data[offset], data[offset + 1]]);
                offset += 2;
            }
            b'R' => {
                offset += 1;
                if offset >= data.len() {
                    return Err(NetworkError::invalid_packet(
                        "incomplete R tag: missing relay byte",
                    ));
                }
                header.relay = data[offset];
                offset += 1;
            }
            b'F' => {
                offset += 1;
                if offset + 4 > data.len() {
                    return Err(NetworkError::invalid_packet(
                        "incomplete F tag: missing frame number",
                    ));
                }
                header.frame = u32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]);
                offset += 4;
            }
            b'D' => {
                offset += 1;
                // Rest is payload
                return Ok((header, &data[offset..]));
            }
            _ => {
                return Err(NetworkError::invalid_packet(format!(
                    "invalid tag: 0x{:02x} at offset {}",
                    tag, offset
                )));
            }
        }
    }

    Err(NetworkError::invalid_packet("missing 'D' tag"))
}

/// Calculate the overhead size of a C++ header for a given command
///
/// # Arguments
///
/// * `has_frame` - Whether the command includes a frame number
///
/// # Returns
///
/// Number of bytes the header will occupy
pub fn header_overhead(has_frame: bool) -> usize {
    // T(1) + type(1) + P(1) + player(1) + C(1) + cmd_id(2) + R(1) + relay(1) + D(1)
    let base_size = 10;

    // F(1) + frame(4)
    if has_frame {
        base_size + 5
    } else {
        base_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{CommandPayload, NetCommand};

    #[test]
    fn test_encode_header_without_frame() {
        let cmd = NetCommand::new(
            NetCommandType::KeepAlive,
            0,
            0, // no frame
            CommandPayload::KeepAlive,
        )
        .with_sequence(1);

        let encoded = encode_cpp_header(&cmd);

        // Expected: T 09 P 00 C 01 00 R 00 D
        assert_eq!(encoded[0], b'T');
        assert_eq!(encoded[1], NetCommandType::KeepAlive as u8);
        assert_eq!(encoded[2], b'P');
        assert_eq!(encoded[3], 0); // player 0
        assert_eq!(encoded[4], b'C');
        assert_eq!(encoded[5], 1); // sequence 1 (LE low byte)
        assert_eq!(encoded[6], 0); // sequence 1 (LE high byte)
        assert_eq!(encoded[7], b'R');
        assert_eq!(encoded[8], 0); // relay 0
        assert_eq!(encoded[9], b'D');
        assert_eq!(encoded.len(), 10); // No frame tag
    }

    #[test]
    fn test_encode_header_with_frame() {
        let cmd = NetCommand::new(
            NetCommandType::GameCommand,
            1,
            100, // frame 100
            CommandPayload::Generic(Vec::new()),
        )
        .with_sequence(256);

        let encoded = encode_cpp_header(&cmd);

        // Expected: T 04 P 01 C 00 01 R 00 F 64 00 00 00 D
        assert_eq!(encoded[0], b'T');
        assert_eq!(encoded[1], NetCommandType::GameCommand as u8);
        assert_eq!(encoded[2], b'P');
        assert_eq!(encoded[3], 1); // player 1
        assert_eq!(encoded[4], b'C');
        assert_eq!(encoded[5], 0); // sequence 256 (LE low byte)
        assert_eq!(encoded[6], 1); // sequence 256 (LE high byte)
        assert_eq!(encoded[7], b'R');
        assert_eq!(encoded[8], 0); // relay 0
        assert_eq!(encoded[9], b'F');
        assert_eq!(encoded[10], 100); // frame 100 (LE byte 0)
        assert_eq!(encoded[11], 0); // frame 100 (LE byte 1)
        assert_eq!(encoded[12], 0); // frame 100 (LE byte 2)
        assert_eq!(encoded[13], 0); // frame 100 (LE byte 3)
        assert_eq!(encoded[14], b'D');
        assert_eq!(encoded.len(), 15); // With frame tag
    }

    #[test]
    fn test_decode_header_without_frame() {
        let data = vec![
            b'T', 9, // KeepAlive
            b'P', 0, // player 0
            b'C', 1, 0, // command id 1
            b'R', 0,    // relay 0
            b'D', // data marker
            0x11, 0x22, // payload
        ];

        let (header, payload) = decode_cpp_header(&data).unwrap();

        assert_eq!(header.command_type, NetCommandType::KeepAlive);
        assert_eq!(header.player_id, 0);
        assert_eq!(header.command_id, 1);
        assert_eq!(header.relay, 0);
        assert_eq!(header.frame, 0);
        assert_eq!(payload, &[0x11, 0x22]);
    }

    #[test]
    fn test_decode_header_with_frame() {
        let data = vec![
            b'T', 4, // GameCommand
            b'P', 1, // player 1
            b'C', 0, 1, // command id 256
            b'R', 0, // relay 0
            b'F', 100, 0, 0, 0,    // frame 100
            b'D', // data marker
            0x33, 0x44, // payload
        ];

        let (header, payload) = decode_cpp_header(&data).unwrap();

        assert_eq!(header.command_type, NetCommandType::GameCommand);
        assert_eq!(header.player_id, 1);
        assert_eq!(header.command_id, 256);
        assert_eq!(header.relay, 0);
        assert_eq!(header.frame, 100);
        assert_eq!(payload, &[0x33, 0x44]);
    }

    #[test]
    fn test_decode_header_tags_in_different_order() {
        // C++ allows tags in any order (though typically in T-P-C-R-F order)
        let data = vec![
            b'P', 2, // player first
            b'T', 11, // then type (Chat)
            b'R', 0, // relay
            b'C', 5, 0,    // command id
            b'D', // data
        ];

        let (header, _) = decode_cpp_header(&data).unwrap();

        assert_eq!(header.command_type, NetCommandType::Chat);
        assert_eq!(header.player_id, 2);
        assert_eq!(header.command_id, 5);
        assert_eq!(header.relay, 0);
        assert_eq!(header.frame, 0);
    }

    #[test]
    fn test_decode_header_missing_d_tag() {
        let data = vec![
            b'T', 9, // KeepAlive
            b'P', 0, // player 0
            b'C', 1, 0, // command id 1
            b'R', 0, // relay 0
               // missing D tag
        ];

        let result = decode_cpp_header(&data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing 'D' tag"));
    }

    #[test]
    fn test_decode_header_invalid_tag() {
        let data = vec![
            b'T', 9, // KeepAlive
            b'X', 0, // invalid tag
            b'P', 0, b'C', 1, 0, b'R', 0, b'D',
        ];

        let result = decode_cpp_header(&data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid tag"));
    }

    #[test]
    fn test_decode_header_incomplete_data() {
        // Missing bytes for C tag
        let data = vec![
            b'T', 9, // KeepAlive
            b'P', 0, // player 0
            b'C', 1, // only one byte of command id
        ];

        let result = decode_cpp_header(&data);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("incomplete C tag"));
    }

    #[test]
    fn test_round_trip_without_frame() {
        let original = NetCommand::new(
            NetCommandType::Chat,
            3,
            0,
            CommandPayload::Generic(Vec::new()),
        )
        .with_sequence(42);

        let encoded = encode_cpp_header(&original);
        let (header, _) = decode_cpp_header(&encoded).unwrap();

        assert_eq!(header.command_type, original.command_type);
        assert_eq!(header.player_id, original.player_id);
        assert_eq!(header.command_id, original.sequence);
        assert_eq!(header.frame, 0);
    }

    #[test]
    fn test_round_trip_with_frame() {
        let original = NetCommand::new(
            NetCommandType::GameCommand,
            5,
            12345,
            CommandPayload::Generic(Vec::new()),
        )
        .with_sequence(999);

        let encoded = encode_cpp_header(&original);
        let (header, _) = decode_cpp_header(&encoded).unwrap();

        assert_eq!(header.command_type, original.command_type);
        assert_eq!(header.player_id, original.player_id);
        assert_eq!(header.command_id, original.sequence);
        assert_eq!(header.frame, original.execution_frame);
    }

    #[test]
    fn test_header_overhead_calculation() {
        assert_eq!(header_overhead(false), 10); // Without frame
        assert_eq!(header_overhead(true), 15); // With frame
    }

    #[test]
    fn test_cpp_header_from_command() {
        let cmd = NetCommand::new(
            NetCommandType::FrameInfo,
            2,
            500,
            CommandPayload::Generic(Vec::new()),
        )
        .with_sequence(123);

        let header = CppHeader::from_command(&cmd);

        assert_eq!(header.command_type, NetCommandType::FrameInfo);
        assert_eq!(header.player_id, 2);
        assert_eq!(header.command_id, 123);
        assert_eq!(header.relay, 0);
        assert_eq!(header.frame, 500);
    }

    #[test]
    fn test_encode_all_command_types() {
        // Test that all command types can be encoded
        let types = vec![
            NetCommandType::AckBoth,
            NetCommandType::FrameInfo,
            NetCommandType::GameCommand,
            NetCommandType::Chat,
            NetCommandType::KeepAlive,
            NetCommandType::FileAnnounce,
            NetCommandType::FrameResendRequest,
        ];

        for cmd_type in types {
            let cmd = NetCommand::new(cmd_type, 0, 0, CommandPayload::Generic(Vec::new()));
            let encoded = encode_cpp_header(&cmd);

            // Should always have T tag at start
            assert_eq!(encoded[0], b'T');
            assert_eq!(encoded[1], cmd_type as u8);

            // Should always end with D tag
            assert_eq!(*encoded.last().unwrap(), b'D');
        }
    }

    #[test]
    fn test_decode_header_with_payload() {
        // Realistic packet with actual payload data
        let data = vec![
            b'T', 4, // GameCommand
            b'P', 0, // player 0
            b'C', 100, 0, // command id 100
            b'R', 0, // relay
            b'F', 200, 0, 0, 0,    // frame 200
            b'D', // payload start
            // Payload (simulated game command data)
            0x01, 0x02, 0x03, 0x04, 0x05,
        ];

        let (header, payload) = decode_cpp_header(&data).unwrap();

        assert_eq!(header.command_type, NetCommandType::GameCommand);
        assert_eq!(header.player_id, 0);
        assert_eq!(header.command_id, 100);
        assert_eq!(header.frame, 200);
        assert_eq!(payload, &[0x01, 0x02, 0x03, 0x04, 0x05]);
    }
}
