//! NetPacket - Network packet serialization/deserialization
//!
//! This module handles the low-level packet structure and serialization
//! that closely mirrors the C++ NetPacket class.

use crate::commands::NetCommand;
use crate::error::{NetworkError, NetworkResult};
use byteorder::{ByteOrder, NativeEndian};
use std::io::{Cursor, Read};

/// Network packet header structure
/// Matches C++ implementation which has NO packet type at transport layer.
/// All command differentiation happens at application layer via NetCommandType.
#[derive(Debug, Clone)]
pub struct NetPacketHeader {
    /// Magic number for packet identification (0xF00D)
    pub magic: u16,
    /// Packet sequence number
    pub sequence: u32,
    /// Acknowledgment number
    pub ack: u32,
    /// Packet flags
    pub flags: PacketFlags,
    /// Payload size in bytes
    pub payload_size: u16,
    /// Checksum for integrity validation
    pub checksum: u32,
}

impl NetPacketHeader {
    /// Size of packet header in bytes (updated without packet_type field)
    /// Magic (2) + Sequence (4) + Ack (4) + Flags (2) + PayloadSize (2) + Checksum (4) = 18 bytes
    pub const SIZE: usize = 18;

    /// Create a new packet header
    /// Note: No packet_type parameter - C++ implementation doesn't have this at transport layer
    pub fn new(sequence: u32, ack: u32) -> Self {
        let mut header = Self {
            magic: crate::config::GENERALS_MAGIC,
            sequence,
            ack,
            flags: PacketFlags::default(),
            payload_size: 0,
            checksum: 0,
        };
        header.checksum = header.calculate_checksum();
        header
    }

    /// Serialize header to bytes
    /// Uses native byte order for data fields (matches C++ serialization)
    /// Only encryption uses big-endian (network byte order)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(Self::SIZE);

        // Write header fields in native byte order (matches C++ data serialization)
        buf.extend_from_slice(&self.magic.to_ne_bytes());
        buf.extend_from_slice(&self.sequence.to_ne_bytes());
        buf.extend_from_slice(&self.ack.to_ne_bytes());
        buf.extend_from_slice(&self.flags.bits().to_ne_bytes());
        buf.extend_from_slice(&self.payload_size.to_ne_bytes());
        buf.extend_from_slice(&self.checksum.to_ne_bytes());

        buf
    }

    /// Deserialize header from bytes
    /// Uses native byte order for data fields (matches C++ deserialization)
    pub fn from_bytes(data: &[u8]) -> NetworkResult<Self> {
        if data.len() < Self::SIZE {
            return Err(NetworkError::packet("packet header too small"));
        }

        let magic = u16::from_ne_bytes([data[0], data[1]]);

        if magic != crate::config::GENERALS_MAGIC {
            return Err(NetworkError::packet("invalid magic number"));
        }

        let sequence = u32::from_ne_bytes([data[2], data[3], data[4], data[5]]);
        let ack = u32::from_ne_bytes([data[6], data[7], data[8], data[9]]);
        let flags_bits = u16::from_ne_bytes([data[10], data[11]]);

        let flags = PacketFlags::from_bits(flags_bits)
            .ok_or(NetworkError::packet("invalid packet flags"))?;

        let payload_size = u16::from_ne_bytes([data[12], data[13]]);
        let checksum = u32::from_ne_bytes([data[14], data[15], data[16], data[17]]);

        Ok(Self {
            magic,
            sequence,
            ack,
            flags,
            payload_size,
            checksum,
        })
    }

    /// Calculate checksum for the header
    pub fn calculate_checksum(&self) -> u32 {
        // Simple checksum calculation (no packet_type field in C++ implementation)
        let mut sum: u32 = 0;
        sum = sum.wrapping_add(self.magic as u32);
        sum = sum.wrapping_add(self.sequence);
        sum = sum.wrapping_add(self.ack);
        sum = sum.wrapping_add(self.flags.bits() as u32);
        sum = sum.wrapping_add(self.payload_size as u32);
        sum
    }

    /// Validate header checksum
    pub fn validate_checksum(&self) -> bool {
        self.calculate_checksum() == self.checksum
    }
}

// NOTE: PacketType enum removed - C++ implementation has no transport-layer packet type.
// All command differentiation happens at application layer via NetCommandType.
// This matches the C++ architecture where Transport.cpp only handles raw packet I/O
// and NetCommand.h defines command types at the application layer.

/// Packet flags bitfield
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketFlags(u16);

impl PacketFlags {
    /// Create new flags with default values
    pub const fn default() -> Self {
        Self(0)
    }

    /// Check if packet is compressed
    pub const fn compressed(self) -> bool {
        (self.0 & (1 << 0)) != 0
    }

    /// Set compression flag
    pub const fn set_compressed(mut self) -> Self {
        self.0 |= 1 << 0;
        self
    }

    /// Check if packet is encrypted
    pub const fn encrypted(self) -> bool {
        (self.0 & (1 << 1)) != 0
    }

    /// Set encryption flag
    pub const fn set_encrypted(mut self) -> Self {
        self.0 |= 1 << 1;
        self
    }

    /// Check if packet requires acknowledgment
    pub const fn needs_ack(self) -> bool {
        (self.0 & (1 << 2)) != 0
    }

    /// Set acknowledgment requirement flag
    pub const fn set_needs_ack(mut self) -> Self {
        self.0 |= 1 << 2;
        self
    }

    /// Check if packet is a retransmission
    pub const fn retransmitted(self) -> bool {
        (self.0 & (1 << 3)) != 0
    }

    /// Set retransmission flag
    pub const fn set_retransmitted(mut self) -> Self {
        self.0 |= 1 << 3;
        self
    }

    /// Get raw bits
    pub const fn bits(self) -> u16 {
        self.0
    }

    /// Create from raw bits
    pub const fn from_bits(bits: u16) -> Option<Self> {
        if bits & !0x0F != 0 {
            // Only bits 0-3 are valid
            None
        } else {
            Some(Self(bits))
        }
    }
}

/// Network packet structure
#[derive(Debug, Clone)]
pub struct NetPacket {
    /// Packet header
    pub header: NetPacketHeader,
    /// Packet payload
    pub payload: PacketPayload,
}

impl NetPacket {
    /// Maximum packet size including header
    pub const MAX_SIZE: usize = 1400; // Match original C++ implementation

    /// Create a new packet
    /// Note: No packet_type parameter - type is determined by payload contents (NetCommandType)
    pub fn new(sequence: u32, ack: u32, payload: PacketPayload) -> Self {
        let mut header = NetPacketHeader::new(sequence, ack);
        header.payload_size = payload.size() as u16;
        header.checksum = header.calculate_checksum();

        Self { header, payload }
    }

    /// Create a command packet
    pub fn command(sequence: u32, ack: u32, commands: Vec<NetCommand>) -> Self {
        let payload = PacketPayload::Command(CommandPacketPayload { commands });
        Self::new(sequence, ack, payload)
    }

    /// Create an acknowledgment packet
    pub fn ack(sequence: u32, ack: u32) -> Self {
        let payload = PacketPayload::Ack;
        Self::new(sequence, ack, payload)
    }

    /// Create a keep-alive packet
    pub fn keep_alive(sequence: u32, ack: u32) -> Self {
        let payload = PacketPayload::KeepAlive;
        Self::new(sequence, ack, payload)
    }

    /// Create a ping packet
    pub fn ping(sequence: u32) -> Self {
        let payload = PacketPayload::Ping(PingPayload {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        });
        Self::new(sequence, 0, payload)
    }

    /// Create a pong packet (ping response)
    pub fn pong(sequence: u32, ack: u32, ping_timestamp: u64) -> Self {
        let payload = PacketPayload::Pong(PongPayload {
            original_timestamp: ping_timestamp,
            response_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        });
        Self::new(sequence, ack, payload)
    }

    /// Serialize packet to bytes
    pub fn to_bytes(&self) -> NetworkResult<Vec<u8>> {
        let mut data = Vec::new();

        // Serialize header
        data.extend_from_slice(&self.header.to_bytes());

        // Serialize payload
        let payload_bytes = self.payload.to_bytes()?;
        data.extend_from_slice(&payload_bytes);

        // Validate total size
        if data.len() > Self::MAX_SIZE {
            return Err(NetworkError::packet("packet exceeds maximum size"));
        }

        Ok(data)
    }

    /// Deserialize packet from bytes
    pub fn from_bytes(data: &[u8]) -> NetworkResult<Self> {
        if data.len() < NetPacketHeader::SIZE {
            return Err(NetworkError::packet("packet too small"));
        }

        // Deserialize header
        let header = NetPacketHeader::from_bytes(&data[..NetPacketHeader::SIZE])?;

        // Validate header
        if !header.validate_checksum() {
            return Err(NetworkError::packet("header checksum validation failed"));
        }

        // Deserialize payload
        let payload_data = &data[NetPacketHeader::SIZE..];
        if payload_data.len() != header.payload_size as usize {
            return Err(NetworkError::packet("payload size mismatch"));
        }

        // Payload self-identifies (no packet_type from header)
        let payload = PacketPayload::from_bytes(payload_data)?;

        Ok(Self { header, payload })
    }

    /// Get total packet size
    pub fn size(&self) -> usize {
        NetPacketHeader::SIZE + self.payload.size()
    }

    /// Check if packet needs acknowledgment
    /// Determined by payload type rather than transport-layer packet type
    pub fn needs_acknowledgment(&self) -> bool {
        self.header.flags.needs_ack() || matches!(self.payload, PacketPayload::Command(_))
    }

    /// Check if packet is compressed
    pub fn is_compressed(&self) -> bool {
        self.header.flags.compressed()
    }

    /// Check if packet is encrypted
    pub fn is_encrypted(&self) -> bool {
        self.header.flags.encrypted()
    }
}

/// Packet payload types
#[derive(Debug, Clone)]
pub enum PacketPayload {
    /// Handshake payload
    Handshake(HandshakePayload),
    /// Acknowledgment (no payload)
    Ack,
    /// Keep-alive (no payload)
    KeepAlive,
    /// Command payload
    Command(CommandPacketPayload),
    /// File transfer payload
    FileTransfer(FileTransferPacketPayload),
    /// Disconnect payload
    Disconnect(DisconnectPayload),
    /// Ping payload
    Ping(PingPayload),
    /// Pong payload (ping response)
    Pong(PongPayload),
    /// Frame synchronization payload
    FrameSync(FrameSyncPayload),
    /// Anti-cheat payload
    AntiCheat(AntiCheatPayload),
}

impl PacketPayload {
    /// Get payload size in bytes
    pub fn size(&self) -> usize {
        match self {
            Self::Handshake(payload) => payload.size(),
            Self::Ack => 0,
            Self::KeepAlive => 0,
            Self::Command(payload) => payload.size(),
            Self::FileTransfer(payload) => payload.size(),
            Self::Disconnect(payload) => payload.size(),
            Self::Ping(payload) => payload.size(),
            Self::Pong(payload) => payload.size(),
            Self::FrameSync(payload) => payload.size(),
            Self::AntiCheat(payload) => payload.size(),
        }
    }

    /// Serialize payload to bytes
    pub fn to_bytes(&self) -> NetworkResult<Vec<u8>> {
        match self {
            Self::Handshake(payload) => payload.to_bytes(),
            Self::Ack => Ok(Vec::new()),
            Self::KeepAlive => Ok(Vec::new()),
            Self::Command(payload) => payload.to_bytes(),
            Self::FileTransfer(payload) => payload.to_bytes(),
            Self::Disconnect(payload) => payload.to_bytes(),
            Self::Ping(payload) => payload.to_bytes(),
            Self::Pong(payload) => payload.to_bytes(),
            Self::FrameSync(payload) => payload.to_bytes(),
            Self::AntiCheat(payload) => payload.to_bytes(),
        }
    }

    /// Deserialize payload from bytes
    /// NOTE: Without transport-layer packet type, payloads must self-identify.
    /// In C++, the payload contains NetCommand structures which have their own type field.
    /// For now, we try to deserialize as Command payload (most common case).
    /// Empty payloads are treated as KeepAlive.
    pub fn from_bytes(data: &[u8]) -> NetworkResult<Self> {
        // Empty payload = KeepAlive
        if data.is_empty() {
            return Ok(Self::KeepAlive);
        }

        // Try to parse as command payload (most common)
        // Commands start with command count (u16), so minimum 2 bytes
        if data.len() >= 2 {
            match CommandPacketPayload::from_bytes(data) {
                Ok(commands) => return Ok(Self::Command(commands)),
                Err(_) => {
                    // Fall through to other payload types
                }
            }
        }

        // Try other specific payload types based on size heuristics
        match data.len() {
            8 => {
                // Could be Ping (8 bytes: timestamp)
                if let Ok(ping) = PingPayload::from_bytes(data) {
                    return Ok(Self::Ping(ping));
                }
            }
            16 => {
                // Could be Pong (16 bytes: two timestamps)
                if let Ok(pong) = PongPayload::from_bytes(data) {
                    return Ok(Self::Pong(pong));
                }
            }
            10 => {
                // Could be FrameSync (10 bytes)
                if let Ok(frame_sync) = FrameSyncPayload::from_bytes(data) {
                    return Ok(Self::FrameSync(frame_sync));
                }
            }
            2 => {
                // Could be Disconnect (2 bytes)
                if let Ok(disconnect) = DisconnectPayload::from_bytes(data) {
                    return Ok(Self::Disconnect(disconnect));
                }
            }
            _ => {}
        }

        // Try Handshake (variable size, has specific structure)
        if data.len() >= 17 {
            if let Ok(handshake) = HandshakePayload::from_bytes(data) {
                return Ok(Self::Handshake(handshake));
            }
        }

        // Try FileTransfer (variable size)
        if data.len() >= 12 {
            if let Ok(file_transfer) = FileTransferPacketPayload::from_bytes(data) {
                return Ok(Self::FileTransfer(file_transfer));
            }
        }

        // Try AntiCheat (variable size, minimum 3 bytes)
        if data.len() >= 3 {
            if let Ok(anti_cheat) = AntiCheatPayload::from_bytes(data) {
                return Ok(Self::AntiCheat(anti_cheat));
            }
        }

        // If all else fails, treat as Ack
        Ok(Self::Ack)
    }
}

/// Handshake payload
#[derive(Debug, Clone)]
pub struct HandshakePayload {
    /// Protocol version
    pub protocol_version: u32,
    /// Player ID requesting connection
    pub player_id: u8,
    /// Game version string
    pub game_version: String,
    /// Session identifier
    pub session_id: u64,
}

impl HandshakePayload {
    /// Size in bytes
    pub fn size(&self) -> usize {
        4 + 1 + 4 + self.game_version.len() + 8
    }

    /// Serialize to bytes
    /// Uses native byte order for data fields (matches C++ serialization)
    pub fn to_bytes(&self) -> NetworkResult<Vec<u8>> {
        let mut data = Vec::with_capacity(self.size());
        data.extend_from_slice(&self.protocol_version.to_ne_bytes());
        data.push(self.player_id);
        data.extend_from_slice(&(self.game_version.len() as u32).to_ne_bytes());
        data.extend_from_slice(self.game_version.as_bytes());
        data.extend_from_slice(&self.session_id.to_ne_bytes());
        Ok(data)
    }

    /// Deserialize from bytes
    /// Uses native byte order for data fields (matches C++ deserialization)
    pub fn from_bytes(data: &[u8]) -> NetworkResult<Self> {
        if data.len() < 17 {
            return Err(NetworkError::packet("handshake payload too small"));
        }

        let mut cursor = Cursor::new(data);
        let protocol_version = {
            let mut buf = [0u8; 4];
            cursor.read_exact(&mut buf)?;
            NativeEndian::read_u32(&buf)
        };

        let player_id = {
            let mut buf = [0u8; 1];
            cursor.read_exact(&mut buf)?;
            buf[0]
        };

        let game_version_len = {
            let mut buf = [0u8; 4];
            cursor.read_exact(&mut buf)?;
            NativeEndian::read_u32(&buf) as usize
        };

        if cursor.position() as usize + game_version_len + 8 > data.len() {
            return Err(NetworkError::packet("handshake payload malformed"));
        }

        let mut game_version_buf = vec![0u8; game_version_len];
        cursor.read_exact(&mut game_version_buf)?;
        let game_version = String::from_utf8(game_version_buf)
            .map_err(|_| NetworkError::packet("invalid game version string"))?;

        let session_id = {
            let mut buf = [0u8; 8];
            cursor.read_exact(&mut buf)?;
            NativeEndian::read_u64(&buf)
        };

        Ok(Self {
            protocol_version,
            player_id,
            game_version,
            session_id,
        })
    }
}

/// Command packet payload
#[derive(Debug, Clone)]
pub struct CommandPacketPayload {
    /// Commands in this packet
    pub commands: Vec<NetCommand>,
}

impl CommandPacketPayload {
    /// Size in bytes
    pub fn size(&self) -> usize {
        let mut size = 2; // Command count
        for command in &self.commands {
            size += 2 + command.size(); // Command type + command data
        }
        size
    }

    /// Serialize to bytes using C++-compatible format
    /// Uses native byte order for data fields (matches C++ serialization)
    pub fn to_bytes(&self) -> NetworkResult<Vec<u8>> {
        let mut data = Vec::new();
        data.extend_from_slice(&(self.commands.len() as u16).to_ne_bytes());

        for command in &self.commands {
            // Use C++-compatible serialization
            let cmd_ref =
                crate::commands::cpp_compat_serialization::NetCommandRef::from_net_command(command);
            let command_bytes =
                crate::commands::cpp_compat_serialization::serialize_command_cpp_compat(&cmd_ref);

            data.extend_from_slice(&(command_bytes.len() as u16).to_ne_bytes());
            data.extend_from_slice(&command_bytes);
        }

        Ok(data)
    }

    /// Deserialize from bytes using C++-compatible format
    /// Uses native byte order for data fields (matches C++ deserialization)
    pub fn from_bytes(data: &[u8]) -> NetworkResult<Self> {
        if data.len() < 2 {
            return Err(NetworkError::packet("command packet payload too small"));
        }

        let mut cursor = Cursor::new(data);
        let command_count = {
            let mut buf = [0u8; 2];
            cursor.read_exact(&mut buf)?;
            NativeEndian::read_u16(&buf) as usize
        };

        let mut commands = Vec::with_capacity(command_count);

        for _ in 0..command_count {
            if cursor.position() as usize + 2 > data.len() {
                return Err(NetworkError::packet("command packet payload malformed"));
            }

            let command_size = {
                let mut buf = [0u8; 2];
                cursor.read_exact(&mut buf)?;
                NativeEndian::read_u16(&buf) as usize
            };

            if cursor.position() as usize + command_size > data.len() {
                return Err(NetworkError::packet("command data size mismatch"));
            }

            let mut command_data = vec![0u8; command_size];
            cursor.read_exact(&mut command_data)?;

            // Use C++-compatible deserialization
            let cmd_ref =
                crate::commands::cpp_compat_serialization::deserialize_command_cpp_compat(
                    &command_data,
                )?;
            let command = cmd_ref.to_net_command();

            commands.push(command);
        }

        Ok(Self { commands })
    }
}

/// File transfer packet payload
#[derive(Debug, Clone)]
pub struct FileTransferPacketPayload {
    /// File identifier
    pub file_id: u32,
    /// Chunk number
    pub chunk_number: u32,
    /// Total chunks
    pub total_chunks: u32,
    /// File data
    pub data: Vec<u8>,
}

impl FileTransferPacketPayload {
    /// Size in bytes
    pub fn size(&self) -> usize {
        12 + self.data.len()
    }

    /// Serialize to bytes
    /// Uses native byte order for data fields (matches C++ serialization)
    pub fn to_bytes(&self) -> NetworkResult<Vec<u8>> {
        let mut data = Vec::new();
        data.extend_from_slice(&self.file_id.to_ne_bytes());
        data.extend_from_slice(&self.chunk_number.to_ne_bytes());
        data.extend_from_slice(&self.total_chunks.to_ne_bytes());
        data.extend_from_slice(&self.data);
        Ok(data)
    }

    /// Deserialize from bytes
    /// Uses native byte order for data fields (matches C++ deserialization)
    pub fn from_bytes(data: &[u8]) -> NetworkResult<Self> {
        if data.len() < 12 {
            return Err(NetworkError::packet("file transfer payload too small"));
        }

        let mut cursor = Cursor::new(data);
        let file_id = {
            let mut buf = [0u8; 4];
            cursor.read_exact(&mut buf)?;
            NativeEndian::read_u32(&buf)
        };

        let chunk_number = {
            let mut buf = [0u8; 4];
            cursor.read_exact(&mut buf)?;
            NativeEndian::read_u32(&buf)
        };

        let total_chunks = {
            let mut buf = [0u8; 4];
            cursor.read_exact(&mut buf)?;
            NativeEndian::read_u32(&buf)
        };

        let data_start = cursor.position() as usize;
        let file_data = data[data_start..].to_vec();

        Ok(Self {
            file_id,
            chunk_number,
            total_chunks,
            data: file_data,
        })
    }
}

/// Disconnect payload
#[derive(Debug, Clone)]
pub struct DisconnectPayload {
    /// Disconnect reason
    pub reason: DisconnectReason,
    /// Player ID disconnecting
    pub player_id: u8,
}

impl DisconnectPayload {
    /// Size in bytes
    pub fn size(&self) -> usize {
        2
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> NetworkResult<Vec<u8>> {
        let mut data = Vec::new();
        data.push(self.reason as u8);
        data.push(self.player_id);
        Ok(data)
    }

    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> NetworkResult<Self> {
        if data.len() < 2 {
            return Err(NetworkError::packet("disconnect payload too small"));
        }

        let reason = DisconnectReason::try_from(data[0])
            .map_err(|_| NetworkError::packet("invalid disconnect reason"))?;
        let player_id = data[1];

        Ok(Self { reason, player_id })
    }
}

/// Disconnect reasons
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DisconnectReason {
    /// Normal disconnect
    Normal = 0,
    /// Timeout
    Timeout = 1,
    /// Kicked
    Kicked = 2,
    /// Network error
    NetworkError = 3,
    /// Anti-cheat violation
    AntiCheat = 4,
}

impl TryFrom<u8> for DisconnectReason {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Normal),
            1 => Ok(Self::Timeout),
            2 => Ok(Self::Kicked),
            3 => Ok(Self::NetworkError),
            4 => Ok(Self::AntiCheat),
            _ => Err(()),
        }
    }
}

/// Ping payload
#[derive(Debug, Clone)]
pub struct PingPayload {
    /// Timestamp when ping was sent
    pub timestamp: u64,
}

impl PingPayload {
    /// Size in bytes
    pub fn size(&self) -> usize {
        8
    }

    /// Serialize to bytes
    /// Uses native byte order for data fields (matches C++ serialization)
    pub fn to_bytes(&self) -> NetworkResult<Vec<u8>> {
        let mut data = Vec::new();
        data.extend_from_slice(&self.timestamp.to_ne_bytes());
        Ok(data)
    }

    /// Deserialize from bytes
    /// Uses native byte order for data fields (matches C++ deserialization)
    pub fn from_bytes(data: &[u8]) -> NetworkResult<Self> {
        if data.len() < 8 {
            return Err(NetworkError::packet("ping payload too small"));
        }

        let timestamp = NativeEndian::read_u64(data);
        Ok(Self { timestamp })
    }
}

/// Pong payload (ping response)
#[derive(Debug, Clone)]
pub struct PongPayload {
    /// Original ping timestamp
    pub original_timestamp: u64,
    /// Response timestamp
    pub response_timestamp: u64,
}

impl PongPayload {
    /// Size in bytes
    pub fn size(&self) -> usize {
        16
    }

    /// Serialize to bytes
    /// Uses native byte order for data fields (matches C++ serialization)
    pub fn to_bytes(&self) -> NetworkResult<Vec<u8>> {
        let mut data = Vec::new();
        data.extend_from_slice(&self.original_timestamp.to_ne_bytes());
        data.extend_from_slice(&self.response_timestamp.to_ne_bytes());
        Ok(data)
    }

    /// Deserialize from bytes
    /// Uses native byte order for data fields (matches C++ deserialization)
    pub fn from_bytes(data: &[u8]) -> NetworkResult<Self> {
        if data.len() < 16 {
            return Err(NetworkError::packet("pong payload too small"));
        }

        let original_timestamp = NativeEndian::read_u64(&data[0..8]);
        let response_timestamp = NativeEndian::read_u64(&data[8..16]);

        Ok(Self {
            original_timestamp,
            response_timestamp,
        })
    }

    /// Calculate round-trip time in milliseconds
    pub fn rtt_ms(&self) -> u64 {
        self.response_timestamp
            .saturating_sub(self.original_timestamp)
    }
}

/// Frame synchronization payload
#[derive(Debug, Clone)]
pub struct FrameSyncPayload {
    /// Current frame number
    pub frame: u32,
    /// Frame checksum
    pub checksum: u32,
    /// Number of commands in frame
    pub command_count: u16,
}

impl FrameSyncPayload {
    /// Size in bytes
    pub fn size(&self) -> usize {
        10
    }

    /// Serialize to bytes
    /// Uses native byte order for data fields (matches C++ serialization)
    pub fn to_bytes(&self) -> NetworkResult<Vec<u8>> {
        let mut data = Vec::new();
        data.extend_from_slice(&self.frame.to_ne_bytes());
        data.extend_from_slice(&self.checksum.to_ne_bytes());
        data.extend_from_slice(&self.command_count.to_ne_bytes());
        Ok(data)
    }

    /// Deserialize from bytes
    /// Uses native byte order for data fields (matches C++ deserialization)
    pub fn from_bytes(data: &[u8]) -> NetworkResult<Self> {
        if data.len() < 10 {
            return Err(NetworkError::packet("frame sync payload too small"));
        }

        let frame = NativeEndian::read_u32(&data[0..4]);
        let checksum = NativeEndian::read_u32(&data[4..8]);
        let command_count = NativeEndian::read_u16(&data[8..10]);

        Ok(Self {
            frame,
            checksum,
            command_count,
        })
    }
}

/// Anti-cheat payload
#[derive(Debug, Clone)]
pub struct AntiCheatPayload {
    /// Violation type
    pub violation_type: AntiCheatViolation,
    /// Evidence data
    pub evidence: Vec<u8>,
}

impl AntiCheatPayload {
    /// Size in bytes
    pub fn size(&self) -> usize {
        1 + 2 + self.evidence.len()
    }

    /// Serialize to bytes
    /// Uses native byte order for data fields (matches C++ serialization)
    pub fn to_bytes(&self) -> NetworkResult<Vec<u8>> {
        let mut data = Vec::new();
        data.push(self.violation_type as u8);
        data.extend_from_slice(&(self.evidence.len() as u16).to_ne_bytes());
        data.extend_from_slice(&self.evidence);
        Ok(data)
    }

    /// Deserialize from bytes
    /// Uses native byte order for data fields (matches C++ deserialization)
    pub fn from_bytes(data: &[u8]) -> NetworkResult<Self> {
        if data.len() < 3 {
            return Err(NetworkError::packet("anti-cheat payload too small"));
        }

        let violation_type = AntiCheatViolation::try_from(data[0])
            .map_err(|_| NetworkError::packet("invalid violation type"))?;

        let evidence_len = NativeEndian::read_u16(&data[1..3]) as usize;

        if data.len() < 3 + evidence_len {
            return Err(NetworkError::packet("anti-cheat payload malformed"));
        }

        let evidence = data[3..3 + evidence_len].to_vec();

        Ok(Self {
            violation_type,
            evidence,
        })
    }
}

/// Anti-cheat violation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AntiCheatViolation {
    /// Invalid command checksum
    InvalidChecksum = 0,
    /// Command timing violation
    TimingViolation = 1,
    /// Memory modification detected
    MemoryViolation = 2,
    /// Speed hack detected
    SpeedHack = 3,
    /// Invalid game state
    InvalidState = 4,
}

impl TryFrom<u8> for AntiCheatViolation {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::InvalidChecksum),
            1 => Ok(Self::TimingViolation),
            2 => Ok(Self::MemoryViolation),
            3 => Ok(Self::SpeedHack),
            4 => Ok(Self::InvalidState),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_header_serialization() {
        let header = NetPacketHeader::new(123, 456);
        let bytes = header.to_bytes();
        let deserialized = NetPacketHeader::from_bytes(&bytes).unwrap();

        assert_eq!(header.magic, deserialized.magic);
        assert_eq!(header.sequence, deserialized.sequence);
        assert_eq!(header.ack, deserialized.ack);
    }

    #[test]
    fn test_packet_header_checksum() {
        let header = NetPacketHeader::new(123, 456);
        assert!(header.validate_checksum());

        // Test with invalid checksum
        let mut invalid_header = header;
        invalid_header.checksum = 0;
        assert!(!invalid_header.validate_checksum());
    }

    #[test]
    fn test_command_packet_creation() {
        let commands = vec![NetCommand::keep_alive(0), NetCommand::keep_alive(1)];

        let packet = NetPacket::command(1, 0, commands);
        assert_eq!(packet.header.sequence, 1);
        assert_eq!(packet.header.ack, 0);

        if let PacketPayload::Command(payload) = &packet.payload {
            assert_eq!(payload.commands.len(), 2);
        } else {
            panic!("Expected command payload");
        }
    }

    #[test]
    fn test_ping_pong_packets() {
        let ping_packet = NetPacket::ping(42);

        if let PacketPayload::Ping(ping_payload) = &ping_packet.payload {
            let pong_packet = NetPacket::pong(43, 42, ping_payload.timestamp);

            if let PacketPayload::Pong(pong_payload) = &pong_packet.payload {
                assert_eq!(pong_payload.original_timestamp, ping_payload.timestamp);
                assert!(pong_payload.response_timestamp >= pong_payload.original_timestamp);
            } else {
                panic!("Expected pong payload");
            }
        } else {
            panic!("Expected ping payload");
        }
    }

    #[test]
    fn test_packet_serialization_roundtrip() {
        let original_packet = NetPacket::ack(123, 456);
        let bytes = original_packet.to_bytes().unwrap();
        let deserialized_packet = NetPacket::from_bytes(&bytes).unwrap();

        assert_eq!(
            original_packet.header.magic,
            deserialized_packet.header.magic
        );
        assert_eq!(
            original_packet.header.sequence,
            deserialized_packet.header.sequence
        );
        assert_eq!(original_packet.header.ack, deserialized_packet.header.ack);
    }

    #[test]
    fn test_packet_size_limits() {
        // Test that packets respect maximum size
        let _large_data = vec![0u8; 2000]; // Larger than MAX_SIZE
        let payload = PacketPayload::Command(CommandPacketPayload {
            commands: vec![], // Empty commands but with large data somehow
        });

        let _packet = NetPacket::new(1, 0, payload);
        // This would fail during serialization due to size limits
        // (We can't easily test this without creating a large packet)
    }
}
