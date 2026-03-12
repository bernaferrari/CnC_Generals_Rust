//! Network command types matching C++ NetworkDefs.h exactly
//!
//! This module provides a direct mapping of the C++ NetCommandType enum
//! to Rust, using i32 representation to match the original implementation.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Network command types matching the original C++ implementation
///
/// These command types are used for network synchronization and game state
/// management across all connected players.
///
/// # Representation
///
/// Uses `#[repr(i32)]` to match the C++ enum which has a value of -1 for UNKNOWN.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NetCommandType {
    /// Unknown command type (-1 in C++)
    Unknown = -1,
    /// Acknowledgment for both stages
    AckBoth = 0,
    /// Stage 1 acknowledgment
    AckStage1 = 1,
    /// Stage 2 acknowledgment
    AckStage2 = 2,
    /// Frame information
    FrameInfo = 3,
    /// Game command (unit orders, building, etc.)
    GameCommand = 4,
    /// Player leaving notification
    PlayerLeave = 5,
    /// Run-ahead metrics
    RunAheadMetrics = 6,
    /// Run-ahead adjustment
    RunAhead = 7,
    /// Destroy player
    DestroyPlayer = 8,
    /// Keep-alive packet
    KeepAlive = 9,
    /// Disconnect chat message
    DisconnectChat = 10,
    /// Chat message
    Chat = 11,
    /// Mangler query (for NAT traversal)
    ManglerQuery = 12,
    /// Mangler response
    ManglerResponse = 13,
    /// Progress update
    Progress = 14,
    /// Load complete notification
    LoadComplete = 15,
    /// Timeout start
    TimeoutStart = 16,
    /// Wrapper for large commands
    Wrapper = 17,
    /// File transfer
    File = 18,
    /// File transfer announcement
    FileAnnounce = 19,
    /// File transfer progress
    FileProgress = 20,
    /// Frame resend request
    FrameResendRequest = 21,
    /// Disconnect process start
    DisconnectStart = 22,
    /// Disconnect keep-alive
    DisconnectKeepAlive = 23,
    /// Disconnect player
    DisconnectPlayer = 24,
    /// Packet router query
    PacketRouterQuery = 25,
    /// Packet router acknowledgment
    PacketRouterAck = 26,
    /// Disconnect vote
    DisconnectVote = 27,
    /// Disconnect frame
    DisconnectFrame = 28,
    /// Disconnect screen off
    DisconnectScreenOff = 29,
    /// Disconnect end
    DisconnectEnd = 30,
}

impl NetCommandType {
    /// Convert from i32 to NetCommandType
    ///
    /// # Arguments
    ///
    /// * `value` - The i32 value to convert
    ///
    /// # Returns
    ///
    /// The corresponding `NetCommandType`, or `NetCommandType::Unknown` if the value is invalid
    ///
    /// # Examples
    ///
    /// ```
    /// use game_network::NetCommandType;
    ///
    /// assert_eq!(NetCommandType::from_i32(0), NetCommandType::AckBoth);
    /// assert_eq!(NetCommandType::from_i32(4), NetCommandType::GameCommand);
    /// assert_eq!(NetCommandType::from_i32(-1), NetCommandType::Unknown);
    /// assert_eq!(NetCommandType::from_i32(999), NetCommandType::Unknown);
    /// ```
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

    /// Convert NetCommandType to i32
    ///
    /// # Returns
    ///
    /// The i32 representation of this command type
    ///
    /// # Examples
    ///
    /// ```
    /// use game_network::NetCommandType;
    ///
    /// assert_eq!(NetCommandType::AckBoth.as_i32(), 0);
    /// assert_eq!(NetCommandType::GameCommand.as_i32(), 4);
    /// assert_eq!(NetCommandType::Unknown.as_i32(), -1);
    /// ```
    pub fn as_i32(&self) -> i32 {
        *self as i32
    }

    /// Returns true if this command type is related to the disconnect process
    ///
    /// # Returns
    ///
    /// `true` if this is a disconnect-related command, `false` otherwise
    pub fn is_disconnect_command(&self) -> bool {
        matches!(
            self,
            NetCommandType::DisconnectStart
                | NetCommandType::DisconnectKeepAlive
                | NetCommandType::DisconnectPlayer
                | NetCommandType::DisconnectVote
                | NetCommandType::DisconnectFrame
                | NetCommandType::DisconnectScreenOff
                | NetCommandType::DisconnectEnd
                | NetCommandType::DisconnectChat
        )
    }

    /// Returns true if this command type requires acknowledgment
    ///
    /// # Returns
    ///
    /// `true` if this command requires acknowledgment, `false` otherwise
    pub fn needs_acknowledgment(&self) -> bool {
        matches!(
            self,
            NetCommandType::AckBoth
                | NetCommandType::AckStage1
                | NetCommandType::AckStage2
                | NetCommandType::FrameInfo
                | NetCommandType::GameCommand
        )
    }

    /// Returns true if this is a file transfer related command
    ///
    /// # Returns
    ///
    /// `true` if this is a file transfer command, `false` otherwise
    pub fn is_file_transfer_command(&self) -> bool {
        matches!(
            self,
            NetCommandType::File | NetCommandType::FileAnnounce | NetCommandType::FileProgress
        )
    }
}

impl fmt::Display for NetCommandType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            NetCommandType::Unknown => "Unknown",
            NetCommandType::AckBoth => "AckBoth",
            NetCommandType::AckStage1 => "AckStage1",
            NetCommandType::AckStage2 => "AckStage2",
            NetCommandType::FrameInfo => "FrameInfo",
            NetCommandType::GameCommand => "GameCommand",
            NetCommandType::PlayerLeave => "PlayerLeave",
            NetCommandType::RunAheadMetrics => "RunAheadMetrics",
            NetCommandType::RunAhead => "RunAhead",
            NetCommandType::DestroyPlayer => "DestroyPlayer",
            NetCommandType::KeepAlive => "KeepAlive",
            NetCommandType::DisconnectChat => "DisconnectChat",
            NetCommandType::Chat => "Chat",
            NetCommandType::ManglerQuery => "ManglerQuery",
            NetCommandType::ManglerResponse => "ManglerResponse",
            NetCommandType::Progress => "Progress",
            NetCommandType::LoadComplete => "LoadComplete",
            NetCommandType::TimeoutStart => "TimeoutStart",
            NetCommandType::Wrapper => "Wrapper",
            NetCommandType::File => "File",
            NetCommandType::FileAnnounce => "FileAnnounce",
            NetCommandType::FileProgress => "FileProgress",
            NetCommandType::FrameResendRequest => "FrameResendRequest",
            NetCommandType::DisconnectStart => "DisconnectStart",
            NetCommandType::DisconnectKeepAlive => "DisconnectKeepAlive",
            NetCommandType::DisconnectPlayer => "DisconnectPlayer",
            NetCommandType::PacketRouterQuery => "PacketRouterQuery",
            NetCommandType::PacketRouterAck => "PacketRouterAck",
            NetCommandType::DisconnectVote => "DisconnectVote",
            NetCommandType::DisconnectFrame => "DisconnectFrame",
            NetCommandType::DisconnectScreenOff => "DisconnectScreenOff",
            NetCommandType::DisconnectEnd => "DisconnectEnd",
        };
        write!(f, "{}", name)
    }
}

impl From<i32> for NetCommandType {
    fn from(value: i32) -> Self {
        Self::from_i32(value)
    }
}

impl From<u8> for NetCommandType {
    fn from(value: u8) -> Self {
        Self::from_i32(value as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_i32_all_values() {
        // Test Unknown
        assert_eq!(NetCommandType::from_i32(-1), NetCommandType::Unknown);

        // Test all valid command types (0-30)
        assert_eq!(NetCommandType::from_i32(0), NetCommandType::AckBoth);
        assert_eq!(NetCommandType::from_i32(1), NetCommandType::AckStage1);
        assert_eq!(NetCommandType::from_i32(2), NetCommandType::AckStage2);
        assert_eq!(NetCommandType::from_i32(3), NetCommandType::FrameInfo);
        assert_eq!(NetCommandType::from_i32(4), NetCommandType::GameCommand);
        assert_eq!(NetCommandType::from_i32(5), NetCommandType::PlayerLeave);
        assert_eq!(NetCommandType::from_i32(6), NetCommandType::RunAheadMetrics);
        assert_eq!(NetCommandType::from_i32(7), NetCommandType::RunAhead);
        assert_eq!(NetCommandType::from_i32(8), NetCommandType::DestroyPlayer);
        assert_eq!(NetCommandType::from_i32(9), NetCommandType::KeepAlive);
        assert_eq!(NetCommandType::from_i32(10), NetCommandType::DisconnectChat);
        assert_eq!(NetCommandType::from_i32(11), NetCommandType::Chat);
        assert_eq!(NetCommandType::from_i32(12), NetCommandType::ManglerQuery);
        assert_eq!(
            NetCommandType::from_i32(13),
            NetCommandType::ManglerResponse
        );
        assert_eq!(NetCommandType::from_i32(14), NetCommandType::Progress);
        assert_eq!(NetCommandType::from_i32(15), NetCommandType::LoadComplete);
        assert_eq!(NetCommandType::from_i32(16), NetCommandType::TimeoutStart);
        assert_eq!(NetCommandType::from_i32(17), NetCommandType::Wrapper);
        assert_eq!(NetCommandType::from_i32(18), NetCommandType::File);
        assert_eq!(NetCommandType::from_i32(19), NetCommandType::FileAnnounce);
        assert_eq!(NetCommandType::from_i32(20), NetCommandType::FileProgress);
        assert_eq!(
            NetCommandType::from_i32(21),
            NetCommandType::FrameResendRequest
        );
        assert_eq!(
            NetCommandType::from_i32(22),
            NetCommandType::DisconnectStart
        );
        assert_eq!(
            NetCommandType::from_i32(23),
            NetCommandType::DisconnectKeepAlive
        );
        assert_eq!(
            NetCommandType::from_i32(24),
            NetCommandType::DisconnectPlayer
        );
        assert_eq!(
            NetCommandType::from_i32(25),
            NetCommandType::PacketRouterQuery
        );
        assert_eq!(
            NetCommandType::from_i32(26),
            NetCommandType::PacketRouterAck
        );
        assert_eq!(NetCommandType::from_i32(27), NetCommandType::DisconnectVote);
        assert_eq!(
            NetCommandType::from_i32(28),
            NetCommandType::DisconnectFrame
        );
        assert_eq!(
            NetCommandType::from_i32(29),
            NetCommandType::DisconnectScreenOff
        );
        assert_eq!(NetCommandType::from_i32(30), NetCommandType::DisconnectEnd);

        // Test invalid values
        assert_eq!(NetCommandType::from_i32(31), NetCommandType::Unknown);
        assert_eq!(NetCommandType::from_i32(100), NetCommandType::Unknown);
        assert_eq!(NetCommandType::from_i32(-2), NetCommandType::Unknown);
        assert_eq!(NetCommandType::from_i32(999), NetCommandType::Unknown);
    }

    #[test]
    fn test_as_i32_round_trip() {
        // Test that converting to i32 and back works correctly
        let command_types = vec![
            NetCommandType::Unknown,
            NetCommandType::AckBoth,
            NetCommandType::AckStage1,
            NetCommandType::AckStage2,
            NetCommandType::FrameInfo,
            NetCommandType::GameCommand,
            NetCommandType::PlayerLeave,
            NetCommandType::RunAheadMetrics,
            NetCommandType::RunAhead,
            NetCommandType::DestroyPlayer,
            NetCommandType::KeepAlive,
            NetCommandType::DisconnectChat,
            NetCommandType::Chat,
            NetCommandType::ManglerQuery,
            NetCommandType::ManglerResponse,
            NetCommandType::Progress,
            NetCommandType::LoadComplete,
            NetCommandType::TimeoutStart,
            NetCommandType::Wrapper,
            NetCommandType::File,
            NetCommandType::FileAnnounce,
            NetCommandType::FileProgress,
            NetCommandType::FrameResendRequest,
            NetCommandType::DisconnectStart,
            NetCommandType::DisconnectKeepAlive,
            NetCommandType::DisconnectPlayer,
            NetCommandType::PacketRouterQuery,
            NetCommandType::PacketRouterAck,
            NetCommandType::DisconnectVote,
            NetCommandType::DisconnectFrame,
            NetCommandType::DisconnectScreenOff,
            NetCommandType::DisconnectEnd,
        ];

        for cmd_type in command_types {
            let value = cmd_type.as_i32();
            let converted = NetCommandType::from_i32(value);
            assert_eq!(cmd_type, converted, "Round trip failed for {:?}", cmd_type);
        }
    }

    #[test]
    fn test_display_implementation() {
        // Test Display trait for all command types
        assert_eq!(format!("{}", NetCommandType::Unknown), "Unknown");
        assert_eq!(format!("{}", NetCommandType::AckBoth), "AckBoth");
        assert_eq!(format!("{}", NetCommandType::GameCommand), "GameCommand");
        assert_eq!(format!("{}", NetCommandType::Chat), "Chat");
        assert_eq!(format!("{}", NetCommandType::KeepAlive), "KeepAlive");
        assert_eq!(format!("{}", NetCommandType::FileAnnounce), "FileAnnounce");
        assert_eq!(
            format!("{}", NetCommandType::DisconnectStart),
            "DisconnectStart"
        );
        assert_eq!(
            format!("{}", NetCommandType::DisconnectEnd),
            "DisconnectEnd"
        );
    }

    #[test]
    fn test_from_trait() {
        // Test the From<i32> trait implementation
        assert_eq!(NetCommandType::from(0), NetCommandType::AckBoth);
        assert_eq!(NetCommandType::from(4), NetCommandType::GameCommand);
        assert_eq!(NetCommandType::from(-1), NetCommandType::Unknown);
        assert_eq!(NetCommandType::from(999), NetCommandType::Unknown);
    }

    #[test]
    fn test_is_disconnect_command() {
        assert!(NetCommandType::DisconnectStart.is_disconnect_command());
        assert!(NetCommandType::DisconnectChat.is_disconnect_command());
        assert!(NetCommandType::DisconnectEnd.is_disconnect_command());
        assert!(!NetCommandType::GameCommand.is_disconnect_command());
        assert!(!NetCommandType::Chat.is_disconnect_command());
        assert!(!NetCommandType::KeepAlive.is_disconnect_command());
    }

    #[test]
    fn test_needs_acknowledgment() {
        assert!(NetCommandType::AckBoth.needs_acknowledgment());
        assert!(NetCommandType::AckStage1.needs_acknowledgment());
        assert!(NetCommandType::GameCommand.needs_acknowledgment());
        assert!(!NetCommandType::Chat.needs_acknowledgment());
        assert!(!NetCommandType::KeepAlive.needs_acknowledgment());
    }

    #[test]
    fn test_is_file_transfer_command() {
        assert!(NetCommandType::File.is_file_transfer_command());
        assert!(NetCommandType::FileAnnounce.is_file_transfer_command());
        assert!(NetCommandType::FileProgress.is_file_transfer_command());
        assert!(!NetCommandType::GameCommand.is_file_transfer_command());
        assert!(!NetCommandType::Chat.is_file_transfer_command());
    }

    #[test]
    fn test_repr_i32_values() {
        // Verify that the repr(i32) gives us the correct values
        assert_eq!(NetCommandType::Unknown as i32, -1);
        assert_eq!(NetCommandType::AckBoth as i32, 0);
        assert_eq!(NetCommandType::GameCommand as i32, 4);
        assert_eq!(NetCommandType::DisconnectEnd as i32, 30);
    }
}
