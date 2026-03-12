//! Network command system for game synchronization
//!
//! This module implements the command system that ensures deterministic
//! gameplay across all connected players. Commands are serialized,
//! validated, and executed in a synchronized manner.

use crate::error::{NetworkError, NetworkResult};
use crate::file_transfer::FileMetadata;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub mod binary_format;
pub mod cpp_compat_serialization;
pub mod cpp_tag_encoding;
pub mod game_message;
pub mod routing;
pub mod sequence_validator;
pub mod serialization;
pub mod validation;
pub mod wrapper;
pub mod xfer;

// Re-export the correct NetCommandType from command_types module
// Uses i32 representation to match C++ exactly (Unknown = -1)
pub use crate::command_types::NetCommandType;

/// Priority levels for commands
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[repr(u8)]
pub enum CommandPriority {
    /// Critical commands (disconnect, security)
    Critical = 0,
    /// High priority (frame sync, acks)
    High = 1,
    /// Normal priority (game commands)
    Normal = 2,
    /// Low priority (chat, progress)
    Low = 3,
}

/// Command flags for special handling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct CommandFlags {
    /// Requires acknowledgment
    pub needs_ack: bool,
    /// Must be processed in sequence
    pub sequenced: bool,
    /// Can supersede older commands of same type
    pub superseding: bool,
    /// Should be encrypted
    pub encrypted: bool,
}

impl Default for CommandFlags {
    fn default() -> Self {
        Self {
            needs_ack: false,
            sequenced: false,
            superseding: false,
            encrypted: false,
        }
    }
}

/// Base network command structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetCommand {
    /// Unique command identifier
    pub id: Uuid,
    /// Command type
    pub command_type: NetCommandType,
    /// Player who sent this command
    pub player_id: u8,
    /// Frame number for execution
    pub execution_frame: u32,
    /// Timestamp when command was created
    pub timestamp: DateTime<Utc>,
    /// Command priority
    pub priority: CommandPriority,
    /// Command flags
    pub flags: CommandFlags,
    /// Command payload
    pub payload: CommandPayload,
    /// Digital signature for anti-cheat
    pub signature: Option<Vec<u8>>,
    /// Sequence number within frame
    pub sequence: u16,
}

impl NetCommand {
    /// Create a new network command
    pub fn new(
        command_type: NetCommandType,
        player_id: u8,
        execution_frame: u32,
        payload: CommandPayload,
    ) -> Self {
        let command_id = if Self::requires_command_id(command_type) {
            Uuid::from_u128(crate::net_command_messages::generate_next_command_id() as u128)
        } else {
            Uuid::nil()
        };
        Self {
            id: command_id,
            command_type,
            player_id,
            execution_frame,
            timestamp: Utc::now(),
            priority: Self::default_priority(command_type),
            flags: Self::default_flags(command_type),
            payload,
            signature: None,
            sequence: 0,
        }
    }

    /// Create a game command
    pub fn game_command(player_id: u8, execution_frame: u32, game_data: GameCommandData) -> Self {
        Self::new(
            NetCommandType::GameCommand,
            player_id,
            execution_frame,
            CommandPayload::GameCommand(game_data),
        )
    }

    /// Create a chat command
    pub fn chat(player_id: u8, message: String, target_mask: i32) -> Self {
        Self::new(
            NetCommandType::Chat,
            player_id,
            0, // Chat commands don't need frame sync
            CommandPayload::Chat(ChatData {
                message,
                target_mask,
            }),
        )
    }

    /// Create a disconnect chat command shown on the disconnect screen
    pub fn disconnect_chat(player_id: u8, message: String, target_mask: i32) -> Self {
        Self::new(
            NetCommandType::DisconnectChat,
            player_id,
            0,
            CommandPayload::Chat(ChatData {
                message,
                target_mask,
            }),
        )
    }

    /// Create a keep-alive command
    pub fn keep_alive(player_id: u8) -> Self {
        Self::new(
            NetCommandType::KeepAlive,
            player_id,
            0,
            CommandPayload::KeepAlive,
        )
    }

    /// Create a timeout-start command used during loading watchdog sequences.
    pub fn timeout_start(player_id: u8) -> Self {
        Self::new(
            NetCommandType::TimeoutStart,
            player_id,
            0,
            CommandPayload::Generic(Vec::new()),
        )
    }

    /// Create a progress update command.
    pub fn progress(player_id: u8, progress_type: ProgressType, percentage: u8) -> Self {
        Self::new(
            NetCommandType::Progress,
            player_id,
            0,
            CommandPayload::Progress(ProgressData {
                progress_type,
                percentage: percentage.min(100),
            }),
        )
    }

    /// Create a load-complete command.
    /// C++ Format (NetPacket.cpp:5641-5645): No payload, just the 'D' tag
    pub fn load_complete(player_id: u8) -> Self {
        Self::new(
            NetCommandType::LoadComplete,
            player_id,
            0,
            CommandPayload::KeepAlive, // LoadComplete has no payload
        )
    }

    /// Create a file announcement command containing transfer metadata.
    pub fn file_announce(
        player_id: u8,
        command_id: u16,
        player_mask: u8,
        metadata: FileMetadata,
    ) -> Self {
        Self::new(
            NetCommandType::FileAnnounce,
            player_id,
            0,
            CommandPayload::FileAnnouncement(FileAnnouncementData {
                command_id,
                player_mask,
                metadata,
            }),
        )
    }

    /// Create a file transfer command containing the file payload.
    pub fn file_transfer(player_id: u8, filename: String, data: Vec<u8>, command_id: u16) -> Self {
        let mut command = Self::new(
            NetCommandType::File,
            player_id,
            0,
            CommandPayload::FileTransfer(FileTransferData {
                file_id: command_id as u32,
                filename,
                data,
                chunk_number: 0,
                total_chunks: 1,
                checksum: 0,
            }),
        );
        command.id = Uuid::from_u128(command_id as u128);
        command
    }

    /// Create a file progress command containing the latest completion percentage.
    pub fn file_progress(player_id: u8, file_id: u16, progress: i32) -> Self {
        Self::new(
            NetCommandType::FileProgress,
            player_id,
            0,
            CommandPayload::FileProgress(FileProgressData { file_id, progress }),
        )
    }

    /// Create an acknowledgment command
    pub fn ack(player_id: u8, ack_type: NetCommandType, command_id: Uuid) -> Self {
        Self::new(
            ack_type,
            player_id,
            0,
            CommandPayload::Ack(AckData { command_id }),
        )
    }

    /// Get default priority for command type
    fn default_priority(command_type: NetCommandType) -> CommandPriority {
        match command_type {
            NetCommandType::DisconnectStart | NetCommandType::DisconnectEnd => {
                CommandPriority::Critical
            }

            NetCommandType::FrameInfo
            | NetCommandType::AckBoth
            | NetCommandType::AckStage1
            | NetCommandType::AckStage2 => CommandPriority::High,

            NetCommandType::Chat
            | NetCommandType::DisconnectChat
            | NetCommandType::LoadComplete
            | NetCommandType::Progress
            | NetCommandType::FileProgress
            | NetCommandType::KeepAlive
            | NetCommandType::FileAnnounce => CommandPriority::Low,

            _ => CommandPriority::Normal,
        }
    }

    /// Get default flags for command type
    fn default_flags(command_type: NetCommandType) -> CommandFlags {
        match command_type {
            NetCommandType::GameCommand => CommandFlags {
                needs_ack: true,
                sequenced: true,
                superseding: false,
                encrypted: false,
            },
            NetCommandType::FrameInfo => CommandFlags {
                needs_ack: true,
                sequenced: true,
                superseding: true,
                encrypted: false,
            },
            NetCommandType::PlayerLeave
            | NetCommandType::DestroyPlayer
            | NetCommandType::RunAhead => CommandFlags {
                needs_ack: true,
                sequenced: true,
                superseding: false,
                encrypted: false,
            },
            NetCommandType::RunAheadMetrics => CommandFlags {
                needs_ack: true,
                sequenced: false,
                superseding: false,
                encrypted: false,
            },
            NetCommandType::Chat => CommandFlags {
                needs_ack: true,
                sequenced: false,
                superseding: false,
                encrypted: false,
            },
            NetCommandType::DisconnectChat => CommandFlags {
                needs_ack: false,
                sequenced: false,
                superseding: false,
                encrypted: false,
            },
            NetCommandType::DisconnectVote
            | NetCommandType::DisconnectPlayer
            | NetCommandType::DisconnectFrame
            | NetCommandType::DisconnectScreenOff
            | NetCommandType::TimeoutStart
            | NetCommandType::FrameResendRequest
            | NetCommandType::File
            | NetCommandType::FileAnnounce
            | NetCommandType::FileProgress
            | NetCommandType::Wrapper => CommandFlags {
                needs_ack: true,
                sequenced: false,
                superseding: false,
                encrypted: false,
            },
            NetCommandType::Progress => CommandFlags {
                needs_ack: false,
                sequenced: false,
                superseding: false,
                encrypted: false,
            },
            NetCommandType::LoadComplete => CommandFlags {
                needs_ack: true,
                sequenced: false,
                superseding: false,
                encrypted: false,
            },
            NetCommandType::KeepAlive => CommandFlags {
                needs_ack: false,
                sequenced: false,
                superseding: true,
                encrypted: false,
            },
            _ => CommandFlags::default(),
        }
    }

    fn requires_command_id(command_type: NetCommandType) -> bool {
        matches!(
            command_type,
            NetCommandType::GameCommand
                | NetCommandType::FrameInfo
                | NetCommandType::PlayerLeave
                | NetCommandType::DestroyPlayer
                | NetCommandType::RunAheadMetrics
                | NetCommandType::RunAhead
                | NetCommandType::Chat
                | NetCommandType::DisconnectVote
                | NetCommandType::LoadComplete
                | NetCommandType::TimeoutStart
                | NetCommandType::Wrapper
                | NetCommandType::File
                | NetCommandType::FileAnnounce
                | NetCommandType::FileProgress
                | NetCommandType::DisconnectPlayer
                | NetCommandType::DisconnectFrame
                | NetCommandType::DisconnectScreenOff
                | NetCommandType::FrameResendRequest
        )
    }

    /// Calculate command size in bytes
    pub fn size(&self) -> usize {
        // This is a rough estimate - real implementation would use actual serialization
        std::mem::size_of::<Self>() + self.payload.size()
    }

    /// Check if command needs acknowledgment
    pub fn needs_acknowledgment(&self) -> bool {
        self.flags.needs_ack
    }

    /// Check if command should be processed sequentially
    pub fn is_sequenced(&self) -> bool {
        self.flags.sequenced
    }

    /// Check if command can supersede others
    pub fn is_superseding(&self) -> bool {
        self.flags.superseding
    }

    /// Set sequence number
    pub fn with_sequence(mut self, sequence: u16) -> Self {
        self.sequence = sequence;
        self
    }

    /// Set signature for anti-cheat
    pub fn with_signature(mut self, signature: Vec<u8>) -> Self {
        self.signature = Some(signature);
        self
    }

    /// Validate command integrity and permissions
    pub fn validate(&self) -> NetworkResult<()> {
        // Basic validation
        if self.player_id >= crate::config::MAX_PLAYERS {
            return Err(NetworkError::invalid_command("invalid player ID"));
        }

        // Type-specific validation
        self.payload.validate(self.player_id, self.command_type)?;

        Ok(())
    }

    /// Validate sequence number for basic sanity checks
    ///
    /// This performs basic validation to detect obviously corrupted sequence numbers.
    /// For full wraparound detection and gap analysis, use SequenceValidator.
    ///
    /// # Returns
    ///
    /// `true` if the sequence appears valid, `false` if it appears corrupted
    pub fn validate_sequence(&self) -> bool {
        // For sequenced commands, the sequence number should be reasonable
        if !self.is_sequenced() {
            return true; // Non-sequenced commands don't need sequence validation
        }

        // Basic sanity check: sequence number exists (always true for u16)
        // In the future, could add checks like:
        // - Sequence not too far in the future (if we track expected range)
        // - Sequence not drastically in the past
        // For now, all u16 values are valid as this is a basic sanity check
        true
    }
}

/// Command payload types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CommandPayload {
    /// Game command (unit orders, building, etc.)
    GameCommand(GameCommandData),
    /// Chat message
    Chat(ChatData),
    /// Acknowledgment
    Ack(AckData),
    /// Frame information
    FrameInfo(FrameInfoData),
    /// Player leave notification
    PlayerLeave(PlayerLeaveData),
    /// Keep-alive (no data)
    KeepAlive,
    /// File transfer data
    FileTransfer(FileTransferData),
    /// File transfer progress status
    FileProgress(FileProgressData),
    /// Progress update
    Progress(ProgressData),
    /// Run-ahead metrics
    RunAheadMetrics(RunAheadMetricsData),
    /// Run-ahead adjustment command
    RunAhead(RunAheadData),
    /// Disconnect vote
    DisconnectVote(DisconnectVoteData),
    /// File transfer announcement metadata
    FileAnnouncement(FileAnnouncementData),
    /// Frame resend request for packet loss recovery
    FrameResendRequest(FrameResendRequestData),
    /// Wrapper command for large messages
    Wrapper(wrapper::WrapperCommand),
    /// Disconnect player command
    DisconnectPlayer(DisconnectPlayerData),
    /// Disconnect frame command
    DisconnectFrame(DisconnectFrameData),
    /// Disconnect screen off command
    DisconnectScreenOff(DisconnectScreenOffData),
    /// Generic data for extensibility
    Generic(Vec<u8>),
}

impl CommandPayload {
    /// Get payload size estimate
    pub fn size(&self) -> usize {
        match self {
            Self::GameCommand(data) => data.size(),
            Self::Chat(data) => data.message.len() + 1,
            Self::Ack(_) => 16, // UUID size
            Self::FrameInfo(data) => std::mem::size_of_val(data),
            Self::PlayerLeave(_) => 1,
            Self::KeepAlive => 0,
            Self::FileTransfer(data) => data.filename.len() + 1 + 4 + data.data.len(),
            Self::FileProgress(_) => 6, // u16 + i32
            Self::Progress(_) => 4,
            Self::RunAheadMetrics(_) => 12,
            Self::RunAhead(_) => 3, // u16 + u8
            Self::DisconnectVote(_) => 8,
            Self::FileAnnouncement(data) => data.metadata.filename.len() + 1 + 2 + 1,
            Self::FrameResendRequest(_) => 4, // u32 frame number
            Self::Wrapper(data) => 22 + data.data.len(), // Header + chunk data
            Self::DisconnectPlayer(_) => 5,   // u8 + u32
            Self::DisconnectFrame(_) => 4,    // u32
            Self::DisconnectScreenOff(_) => 4, // u32
            Self::Generic(data) => data.len(),
        }
    }

    /// Validate payload
    pub fn validate(&self, player_id: u8, command_type: NetCommandType) -> NetworkResult<()> {
        match (self, command_type) {
            (Self::GameCommand(data), NetCommandType::GameCommand) => data.validate(player_id),
            (Self::Chat(data), NetCommandType::Chat)
            | (Self::Chat(data), NetCommandType::DisconnectChat) => {
                if data.message.len() > 256 {
                    return Err(NetworkError::invalid_command("chat message too long"));
                }
                Ok(())
            }
            (Self::Progress(data), NetCommandType::Progress)
            | (Self::Progress(data), NetCommandType::LoadComplete) => {
                if data.percentage > 100 {
                    return Err(NetworkError::invalid_command(
                        "progress percentage out of range",
                    ));
                }
                Ok(())
            }
            (Self::FileProgress(_), NetCommandType::FileProgress) => {
                // Progress is i32 in C++, can be any value including negative for errors
                Ok(())
            }
            (Self::KeepAlive, NetCommandType::KeepAlive) => Ok(()),
            (Self::RunAhead(data), NetCommandType::RunAhead) => {
                if data.run_ahead == 0 || data.frame_rate == 0 {
                    return Err(NetworkError::invalid_command(
                        "run_ahead and frame_rate must be non-zero",
                    ));
                }
                Ok(())
            }
            (Self::FileAnnouncement(_), NetCommandType::FileAnnounce) => Ok(()),
            (Self::FrameResendRequest(_), NetCommandType::FrameResendRequest) => Ok(()),
            _ => Ok(()), // Basic validation for other types
        }
    }
}

/// Game command data for unit orders, building, etc.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameCommandData {
    /// Command type (move, attack, build, etc.)
    pub command_type: u32,
    /// Target object ID (if applicable)
    pub target_id: Option<u32>,
    /// Position data
    pub position: Option<(f32, f32, f32)>,
    /// Additional parameters
    pub parameters: HashMap<String, CommandParameter>,
    /// Command checksum for validation
    pub checksum: u32,
}

impl GameCommandData {
    /// Calculate size estimate
    pub fn size(&self) -> usize {
        let base_size = std::mem::size_of::<u32>() * 2 + 12; // command_type + checksum + position
        let params_size = self
            .parameters
            .iter()
            .map(|(k, v)| k.len() + v.size())
            .sum::<usize>();
        base_size + params_size
    }

    /// Validate game command
    pub fn validate(&self, _player_id: u8) -> NetworkResult<()> {
        // Implement game-specific validation logic
        // This would check things like:
        // - Player owns the units being commanded
        // - Command is valid for current game state
        // - No obvious cheating attempts

        // For now, just basic checks
        if self.parameters.len() > 16 {
            return Err(NetworkError::invalid_command("too many parameters"));
        }

        Ok(())
    }
}

/// Command parameter types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CommandParameter {
    /// Integer parameter
    Int(i32),
    /// Floating point parameter
    Float(f32),
    /// String parameter
    String(String),
    /// Boolean parameter
    Bool(bool),
    /// Object ID reference
    ObjectId(u32),
    /// Drawable ID reference
    DrawableId(u32),
    /// Team ID reference
    TeamId(u32),
    /// Position coordinate
    Position(f32, f32, f32),
    /// Pixel coordinate
    Pixel(i32, i32),
    /// Pixel region (x1, y1, x2, y2)
    PixelRegion(i32, i32, i32, i32),
    /// Timestamp value
    Timestamp(u32),
    /// Wide character value
    WideChar(u16),
}

impl CommandParameter {
    /// Get parameter size estimate
    pub fn size(&self) -> usize {
        match self {
            Self::Int(_) => 4,
            Self::Float(_) => 4,
            Self::String(s) => s.len() + 4,
            Self::Bool(_) => 1,
            Self::ObjectId(_) => 4,
            Self::DrawableId(_) => 4,
            Self::TeamId(_) => 4,
            Self::Position(_, _, _) => 12,
            Self::Pixel(_, _) => 8,
            Self::PixelRegion(_, _, _, _) => 16,
            Self::Timestamp(_) => 4,
            Self::WideChar(_) => 4,
        }
    }
}

/// Chat command data
/// C++ Format (NetPacket.cpp:5583-5610):
/// - text_length: u8 (max 255 chars)
/// - text: u16[] (UTF-16 chars)
/// - player_mask: i32 (signed 4-byte int)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatData {
    /// Chat message text (max 255 characters)
    pub message: String,
    /// Target player mask (i32 bitfield, matches C++)
    pub target_mask: i32,
}

/// Acknowledgment data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AckData {
    /// ID of command being acknowledged
    pub command_id: Uuid,
}

/// Frame information data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FrameInfoData {
    /// Frame number
    pub frame: u32,
    /// Number of commands in frame
    pub command_count: u16,
    /// Frame checksum
    pub checksum: u32,
}

/// Player leave data
/// C++ Format (NetPacket.cpp:5437-5451): encodes which player is leaving, not the reason
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerLeaveData {
    /// ID of the player who is leaving (0-7)
    pub leaving_player_id: u8,
}

/// Reasons for player leaving
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[repr(u8)]
pub enum PlayerLeaveReason {
    /// Player chose to leave
    PlayerQuit = 0,
    /// Connection lost
    ConnectionLost = 1,
    /// Kicked by host
    Kicked = 2,
    /// Network error
    NetworkError = 3,
    /// Anti-cheat violation
    AntiCheat = 4,
}

/// File transfer data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileTransferData {
    /// File identifier
    pub file_id: u32,
    /// File name
    pub filename: String,
    /// File data chunk
    pub data: Vec<u8>,
    /// Chunk number
    pub chunk_number: u32,
    /// Total chunks
    pub total_chunks: u32,
    /// File checksum
    pub checksum: u32,
}

/// File transfer progress update
/// C++ Format (NetPacket.cpp:5754-5769): file_id (u16) + progress (i32)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileProgressData {
    /// File/Command identifier (u16 in C++)
    pub file_id: u16,
    /// Transfer progress as i32 (NOT u8 - matches C++ Int type)
    pub progress: i32,
}

/// Progress update data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProgressData {
    /// Progress type
    pub progress_type: ProgressType,
    /// Progress percentage (0-100)
    pub percentage: u8,
}

/// File announcement data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileAnnouncementData {
    /// Command identifier shared between announce and transfer
    pub command_id: u16,
    /// Bitmask of intended recipients
    pub player_mask: u8,
    /// Metadata describing the file to be transferred
    pub metadata: FileMetadata,
}

/// Progress types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[repr(u8)]
pub enum ProgressType {
    /// Loading progress
    Loading = 0,
    /// Connection progress
    Connection = 1,
    /// File transfer progress
    FileTransfer = 2,
}

/// Run-ahead metrics data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunAheadMetricsData {
    /// Average latency in milliseconds
    pub average_latency: f32,
    /// Average FPS
    pub average_fps: u32,
    /// Recommended run-ahead frames
    pub recommended_frames: u16,
}

/// Run-ahead adjustment data
/// C++ Format (NetPacket.cpp:5471-5489): Dynamically adjust runahead distance and frame rate
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunAheadData {
    /// New runahead distance (frames to run ahead)
    pub run_ahead: u16,
    /// New frame rate
    pub frame_rate: u8,
}

/// Disconnect vote data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisconnectVoteData {
    /// Player slot to disconnect
    pub target_slot: u8,
    /// Vote frame number
    pub vote_frame: u32,
    /// Vote type (kick, timeout, etc.)
    pub vote_type: DisconnectVoteType,
}

/// Disconnect vote types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[repr(u8)]
pub enum DisconnectVoteType {
    /// Kick player vote
    Kick = 0,
    /// Timeout disconnect
    Timeout = 1,
    /// Network issues
    NetworkIssues = 2,
}

/// Frame resend request data for packet loss recovery
/// C++ Format (NetPacket.cpp:725-736, 5794-5805)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FrameResendRequestData {
    /// Frame number to resend
    pub frame_number: u32,
}

/// Disconnect player data (NETCOMMANDTYPE_DISCONNECTPLAYER, type 24)
/// C++ Format: disconnect_slot (u8) + disconnect_frame (u32)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisconnectPlayerData {
    /// Player slot being disconnected
    pub disconnect_slot: u8,
    /// Frame at which disconnect occurs
    pub disconnect_frame: u32,
}

/// Disconnect frame data (NETCOMMANDTYPE_DISCONNECTFRAME, type 28)
/// C++ Format: disconnect_frame (u32)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisconnectFrameData {
    /// Frame at which disconnect occurs
    pub disconnect_frame: u32,
}

/// Disconnect screen off data (NETCOMMANDTYPE_DISCONNECTSCREENOFF, type 29)
/// C++ Format: new_frame (u32)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisconnectScreenOffData {
    /// New frame number after screen off
    pub new_frame: u32,
}

impl NetCommand {
    /// Create a run-ahead adjustment command
    ///
    /// # Arguments
    ///
    /// * `player_id` - Player sending the adjustment
    /// * `run_ahead` - New runahead distance in frames
    /// * `frame_rate` - New frame rate
    ///
    /// # Example
    ///
    /// ```
    /// use game_network::commands::NetCommand;
    ///
    /// let cmd = NetCommand::run_ahead(0, 5, 30);
    /// ```
    pub fn run_ahead(player_id: u8, run_ahead: u16, frame_rate: u8) -> Self {
        Self::new(
            NetCommandType::RunAhead,
            player_id,
            0, // RunAhead commands don't need execution frame
            CommandPayload::RunAhead(RunAheadData {
                run_ahead,
                frame_rate,
            }),
        )
    }

    /// Create a frame resend request command
    ///
    /// # Arguments
    ///
    /// * `player_id` - Player requesting the resend
    /// * `frame_number` - Frame number that needs to be resent
    ///
    /// # Example
    ///
    /// ```
    /// use game_network::commands::NetCommand;
    ///
    /// let cmd = NetCommand::frame_resend_request(0, 95);
    /// ```
    pub fn frame_resend_request(player_id: u8, frame_number: u32) -> Self {
        Self::new(
            NetCommandType::FrameResendRequest,
            player_id,
            0, // Frame resend requests don't need execution frame
            CommandPayload::FrameResendRequest(FrameResendRequestData { frame_number }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_type_conversion() {
        assert_eq!(NetCommandType::from(4), NetCommandType::GameCommand);
        assert_eq!(NetCommandType::from(11), NetCommandType::Chat);
        assert_eq!(NetCommandType::from(255), NetCommandType::Unknown);
    }

    #[test]
    fn test_command_creation() {
        let cmd = NetCommand::keep_alive(1);
        assert_eq!(cmd.command_type, NetCommandType::KeepAlive);
        assert_eq!(cmd.player_id, 1);
        assert_eq!(cmd.priority, CommandPriority::Low);
        assert!(matches!(cmd.payload, CommandPayload::KeepAlive));
    }

    #[test]
    fn test_game_command_creation() {
        let mut params = HashMap::new();
        params.insert("target".to_string(), CommandParameter::ObjectId(123));

        let game_data = GameCommandData {
            command_type: 1,
            target_id: Some(123),
            position: Some((10.0, 20.0, 0.0)),
            parameters: params,
            checksum: 0,
        };

        let cmd = NetCommand::game_command(0, 100, game_data);
        assert_eq!(cmd.command_type, NetCommandType::GameCommand);
        assert_eq!(cmd.execution_frame, 100);
        assert!(cmd.needs_acknowledgment());
    }

    #[test]
    fn test_command_validation() {
        let cmd = NetCommand::keep_alive(0);
        assert!(cmd.validate().is_ok());

        let invalid_cmd = NetCommand::keep_alive(255); // Invalid player ID
        assert!(invalid_cmd.validate().is_err());
    }

    #[test]
    fn test_command_flags() {
        let game_cmd = NetCommand::game_command(
            0,
            100,
            GameCommandData {
                command_type: 1,
                target_id: None,
                position: None,
                parameters: HashMap::new(),
                checksum: 0,
            },
        );

        assert!(game_cmd.needs_acknowledgment());
        assert!(game_cmd.is_sequenced());
        assert!(!game_cmd.is_superseding());

        let keep_alive = NetCommand::keep_alive(0);
        assert!(!keep_alive.needs_acknowledgment());
        assert!(!keep_alive.is_sequenced());
        assert!(keep_alive.is_superseding());
    }

    #[test]
    fn test_command_priority() {
        assert_eq!(
            NetCommand::default_priority(NetCommandType::DisconnectStart),
            CommandPriority::Critical
        );
        assert_eq!(
            NetCommand::default_priority(NetCommandType::FrameInfo),
            CommandPriority::High
        );
        assert_eq!(
            NetCommand::default_priority(NetCommandType::GameCommand),
            CommandPriority::Normal
        );
        assert_eq!(
            NetCommand::default_priority(NetCommandType::Chat),
            CommandPriority::Low
        );
    }
}
