use serde::{Deserialize, Serialize};

// ============================================================================
// FRAME SYNCHRONIZATION CONSTANTS
// Matches C++ NetworkDefs.h lines 11-20
// ============================================================================

/// Maximum number of commands per frame
/// Matches C++ static const Int MAX_COMMANDS = 256
pub const MAX_COMMANDS: usize = 256;

/// Maximum frames ahead for prediction
/// Matches C++ NetworkUtil.cpp: Int MAX_FRAMES_AHEAD = 128
pub const MAX_FRAMES_AHEAD: u32 = 128;

/// Minimum run-ahead frames required
/// Matches C++ NetworkUtil.cpp: Int MIN_RUNAHEAD = 10
pub const MIN_RUNAHEAD: u32 = 10;

/// FRAME_DATA_LENGTH needs to be MAX_FRAMES_AHEAD+1 because a player on a different
/// computer can send commands for a frame that is one beyond twice the max runahead.
/// Matches C++ NetworkUtil.cpp: Int FRAME_DATA_LENGTH = (MAX_FRAMES_AHEAD+1)*2
pub const FRAME_DATA_LENGTH: usize = (MAX_FRAMES_AHEAD as usize + 1) * 2;

/// Number of frames to keep in history
/// Matches C++ NetworkUtil.cpp: Int FRAMES_TO_KEEP = (MAX_FRAMES_AHEAD/2) + 1
pub const FRAMES_TO_KEEP: usize = (MAX_FRAMES_AHEAD as usize / 2) + 1;

// ============================================================================
// PLAYER AND CONNECTION CONSTANTS
// Matches C++ NetworkDefs.h lines 23-29
// ============================================================================

/// The index of the highest possible player number. This is 0 based,
/// so the most players allowed in a game is MAX_PLAYER+1.
/// Matches C++ enum ConnectionNumbers::MAX_PLAYER = 7
pub const MAX_PLAYER: usize = 7;

/// Total number of player slots
/// Matches C++ static const Int MAX_SLOTS = MAX_PLAYER+1
pub const MAX_SLOTS: usize = MAX_PLAYER + 1;

/// Broadcast connection number
/// Matches C++ enum ConnectionNumbers::NUM_CONNECTIONS
pub const NUM_CONNECTIONS: usize = 9;

// ============================================================================
// PACKET SIZE CONSTANTS
// Matches C++ NetworkDefs.h lines 31-32
// ============================================================================

/// UDP (8 bytes) + IP header (28 bytes) = 36 bytes total.
/// We want a total packet size of 512, so 512 - 36 = 476
/// Matches C++ static const Int MAX_PACKET_SIZE = 476
pub const MAX_PACKET_SIZE: usize = 476;

/// Maximum message length for commands
/// Matches C++ #define MAX_MESSAGE_LEN 1024
pub const MAX_MESSAGE_LEN: usize = 1024;

/// Maximum number of messages in buffer
/// Matches C++ #define MAX_MESSAGES 128
pub const MAX_MESSAGES: usize = 128;

/// Number of commands that fit in a command packet
/// Matches C++ static const Int numCommandsPerCommandPacket
pub const NUM_COMMANDS_PER_COMMAND_PACKET: usize =
    (MAX_MESSAGE_LEN - std::mem::size_of::<u32>() - std::mem::size_of::<u16>())
        / std::mem::size_of::<GameMessage>();

// ============================================================================
// TIMING CONSTANTS (in milliseconds)
// Matches C++ NetworkDefs.h comments lines 162-180
// ============================================================================

/// Number of transport statistics tracking seconds
/// Matches C++ #define MAX_TRANSPORT_STATISTICS_SECONDS 30
pub const MAX_TRANSPORT_STATISTICS_SECONDS: usize = 30;

/// Number of seconds between keep-alive packets
/// This should be less than 30 just to keep firewall ports open
/// Matches C++ NETWORK_KEEPALIVE_DELAY = 20 (from comments)
pub const NETWORK_KEEPALIVE_DELAY: u64 = 20;

/// Milliseconds between stuck frame and disconnect dialog
/// Matches C++ NETWORK_DISCONNECT_TIME = 5000 (from comments)
pub const NETWORK_DISCONNECT_TIME: u64 = 5000;

/// Milliseconds between last keep alive and considering player disconnected
/// Matches C++ NETWORK_PLAYER_TIMEOUT_TIME = 60000 (from comments)
pub const NETWORK_PLAYER_TIMEOUT_TIME: u64 = 60000;

/// Base port number for transport socket
/// A player's slot number is added to this value to get their actual port number
/// Matches C++ static const Int NETWORK_BASE_PORT_NUMBER = 8088
pub const NETWORK_BASE_PORT_NUMBER: u16 = 8088;

// ============================================================================
// MAGIC NUMBER AND CRC
// Matches C++ NetworkDefs.h lines 152-153
// ============================================================================

/// Magic number for identifying a Generals packet
/// Matches C++ static const UnsignedShort GENERALS_MAGIC_NUMBER = 0xF00D
pub const GENERALS_MAGIC_NUMBER: u16 = 0xF00D;

// ============================================================================
// COMMAND PACKET STRUCTURE
// Matches C++ NetworkDefs.h lines 34-48 (CommandPacket struct)
// ============================================================================

/// Command packet - contains frame #, total # of commands, and each command.
/// This is what gets sent to each player every frame.
/// Matches C++ #pragma pack(push, 1) struct CommandPacket
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct CommandPacket {
    /// Frame number this command is for
    pub frame: u32,
    /// Number of commands in this packet
    pub num_commands: u16,
    /// Raw command data
    pub commands: [u8; NUM_COMMANDS_PER_COMMAND_PACKET * std::mem::size_of::<GameMessage>()],
}

// ============================================================================
// TRANSPORT MESSAGE STRUCTURES
// Matches C++ NetworkDefs.h lines 52-85
// ============================================================================

/// Transport message header
/// Matches C++ #pragma pack(push, 1) struct TransportMessageHeader
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct TransportMessageHeader {
    /// Packet-level CRC (must be first in packet)
    pub crc: u32,
    /// Magic number identifying Generals packets
    pub magic: u16,
}

/// Transport message - encapsulating info kept by the transport layer about each packet
/// Matches C++ #pragma pack(push, 1) struct TransportMessage
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct TransportMessage {
    /// Message header
    pub header: TransportMessageHeader,
    /// Message data
    pub data: [u8; MAX_MESSAGE_LEN],
    /// Length of valid data
    pub length: i32,
    /// Destination/source address
    pub addr: u32,
    /// Destination/source port
    pub port: u16,
}

/// Delayed transport message for latency simulation (debug/internal only)
/// Matches C++ #pragma pack(push, 1) struct DelayedTransportMessage
#[cfg(any(debug_assertions, feature = "internal"))]
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct DelayedTransportMessage {
    /// Time when this message should be delivered
    pub delivery_time: u32,
    /// The actual message
    pub message: TransportMessage,
}

// ============================================================================
// MESSAGE FLAGS
// Matches C++ NetworkDefs.h lines 87-96
// ============================================================================

/// Message type flags
/// Matches C++ enum NetMessageFlag
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetMessageFlag {
    /// Message is an acknowledgment
    Ack = 1,
    /// Message needs acknowledgment
    NeedAck = 2,
    /// Message is sequenced
    Sequenced = 4,
    /// Message supersedes previous
    Superceding = 8,
}

/// Type for storing message flags as a bitfield
/// Matches C++ typedef UnsignedByte NetMessageFlags
pub type NetMessageFlags = u8;

// ============================================================================
// NET COMMAND TYPES
// Matches C++ NetworkDefs.h lines 98-135 (enum NetCommandType)
// ============================================================================

/// Network command types
/// Matches C++ enum NetCommandType exactly
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NetCommandType {
    Unknown = -1,
    AckBoth = 0,
    AckStage1 = 1,
    AckStage2 = 2,
    FrameInfo = 3,
    GameCommand = 4,
    PlayerLeave = 5,
    RunAheadMetrics = 6,
    RunAhead = 7,
    DestroyPlayer = 8,
    KeepAlive = 9,
    DisconnectChat = 10,
    Chat = 11,
    ManglerQuery = 12,
    ManglerResponse = 13,
    Progress = 14,
    LoadComplete = 15,
    TimeoutStart = 16,
    Wrapper = 17,
    File = 18,
    FileAnnounce = 19,
    FileProgress = 20,
    FrameResendRequest = 21,
    // Disconnect menu command section
    DisconnectStart = 22,
    DisconnectKeepAlive = 23,
    DisconnectPlayer = 24,
    PacketRouterQuery = 25,
    PacketRouterAck = 26,
    DisconnectVote = 27,
    DisconnectFrame = 28,
    DisconnectScreenOff = 29,
    DisconnectEnd = 30,
}

impl NetCommandType {
    /// Convert from i32 to NetCommandType
    pub fn from_i32(value: i32) -> Self {
        match value {
            -1 => NetCommandType::Unknown,
            0 => NetCommandType::AckBoth,
            1 => NetCommandType::AckStage1,
            2 => NetCommandType::AckStage2,
            3 => NetCommandType::FrameInfo,
            4 => NetCommandType::GameCommand,
            5 => NetCommandType::PlayerLeave,
            6 => NetCommandType::RunAheadMetrics,
            7 => NetCommandType::RunAhead,
            8 => NetCommandType::DestroyPlayer,
            9 => NetCommandType::KeepAlive,
            10 => NetCommandType::DisconnectChat,
            11 => NetCommandType::Chat,
            12 => NetCommandType::ManglerQuery,
            13 => NetCommandType::ManglerResponse,
            14 => NetCommandType::Progress,
            15 => NetCommandType::LoadComplete,
            16 => NetCommandType::TimeoutStart,
            17 => NetCommandType::Wrapper,
            18 => NetCommandType::File,
            19 => NetCommandType::FileAnnounce,
            20 => NetCommandType::FileProgress,
            21 => NetCommandType::FrameResendRequest,
            22 => NetCommandType::DisconnectStart,
            23 => NetCommandType::DisconnectKeepAlive,
            24 => NetCommandType::DisconnectPlayer,
            25 => NetCommandType::PacketRouterQuery,
            26 => NetCommandType::PacketRouterAck,
            27 => NetCommandType::DisconnectVote,
            28 => NetCommandType::DisconnectFrame,
            29 => NetCommandType::DisconnectScreenOff,
            30 => NetCommandType::DisconnectEnd,
            _ => NetCommandType::Unknown,
        }
    }

    /// Convert to i32
    pub fn to_i32(self) -> i32 {
        self as i32
    }
}

// ============================================================================
// LOCAL STATUS ENUM
// Matches C++ NetworkDefs.h lines 137-143
// ============================================================================

/// Local player network status
/// Matches C++ enum NetLocalStatus
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetLocalStatus {
    Pregame = 0,
    InGame = 1,
    Leaving = 2,
    Left = 3,
    Postgame = 4,
}

// ============================================================================
// PLAYER LEAVE CODE
// Matches C++ NetworkDefs.h lines 145-150
// ============================================================================

/// Reason for player leaving
/// Matches C++ enum PlayerLeaveCode
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerLeaveCode {
    Client = 0,
    Local = 1,
    PacketRouter = 2,
    Unknown = 3,
}

// ============================================================================
// PLACEHOLDER TYPES (will be defined elsewhere)
// ============================================================================

/// Placeholder for GameMessage type
/// Real implementation in game message module
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GameMessage {
    _placeholder: [u8; 32], // Actual size determined by C++ struct
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

impl TransportMessage {
    /// Create a new transport message
    pub fn new() -> Self {
        Self {
            header: TransportMessageHeader {
                crc: 0,
                magic: GENERALS_MAGIC_NUMBER,
            },
            data: [0u8; MAX_MESSAGE_LEN],
            length: 0,
            addr: 0,
            port: 0,
        }
    }

    /// Check if this is a valid Generals packet
    pub fn is_valid_generals_packet(&self) -> bool {
        self.header.magic == GENERALS_MAGIC_NUMBER
            && self.length >= 0
            && self.length <= MAX_MESSAGE_LEN as i32
    }
}

impl Default for TransportMessage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants_match_cpp() {
        // Verify critical constants match C++ values
        assert_eq!(MAX_COMMANDS, 256);
        assert_eq!(MAX_FRAMES_AHEAD, 128);
        assert_eq!(FRAME_DATA_LENGTH, 258);
        assert_eq!(FRAMES_TO_KEEP, 65);
        assert_eq!(MAX_PLAYER, 7);
        assert_eq!(MAX_SLOTS, 8);
        assert_eq!(MAX_PACKET_SIZE, 476);
        assert_eq!(GENERALS_MAGIC_NUMBER, 0xF00D);
        assert_eq!(NETWORK_BASE_PORT_NUMBER, 8088);
    }

    #[test]
    fn test_net_command_type_conversion() {
        assert_eq!(NetCommandType::from_i32(0), NetCommandType::AckBoth);
        assert_eq!(NetCommandType::from_i32(4), NetCommandType::GameCommand);
        assert_eq!(NetCommandType::from_i32(-1), NetCommandType::Unknown);
        assert_eq!(NetCommandType::from_i32(999), NetCommandType::Unknown);
        assert_eq!(NetCommandType::AckBoth.to_i32(), 0);
        assert_eq!(NetCommandType::GameCommand.to_i32(), 4);
    }

    #[test]
    fn test_transport_message_creation() {
        let msg = TransportMessage::new();

        // Packed structs in this module intentionally match C++ layout and can
        // trigger unaligned references on newer Rust compilers.
        let raw = &msg as *const TransportMessage as *const u8;
        const HEADER_MAGIC_OFFSET: usize = 4;
        const MESSAGE_LENGTH_OFFSET: usize = 4 + 2 + MAX_MESSAGE_LEN;

        let header_magic =
            unsafe { std::ptr::read_unaligned(raw.add(HEADER_MAGIC_OFFSET) as *const u16) };
        let length =
            unsafe { std::ptr::read_unaligned(raw.add(MESSAGE_LENGTH_OFFSET) as *const i32) };

        assert_eq!(header_magic, GENERALS_MAGIC_NUMBER);
        assert_eq!(length, 0);
        assert!(msg.is_valid_generals_packet());
    }
}
