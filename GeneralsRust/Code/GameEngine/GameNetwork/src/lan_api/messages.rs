//! LAN message structures and protocols
//!
//! This module defines the message structures used for LAN communication,
//! closely matching the original C++ implementation for compatibility.

use crate::lan_api::{ChatType, GameOptions, LanResult};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use uuid::Uuid;

/// Maximum lengths matching the C++ implementation
pub const MAX_PLAYER_NAME_LENGTH: usize = 12;
pub const MAX_LOGIN_NAME_LENGTH: usize = 1;
pub const MAX_HOST_NAME_LENGTH: usize = 1;
pub const MAX_GAME_NAME_LENGTH: usize = 16;
pub const MAX_CHAT_LENGTH: usize = 100;
pub const MAX_SERIAL_LENGTH: usize = 23;
pub const MAX_OPTIONS_LENGTH: usize = 400; // Calculated from C++ MAX_PACKET_SIZE

/// LAN message types matching the C++ MSG_* constants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum LanMessageType {
    // Location discovery
    RequestLocations = 0,
    GameAnnounce = 1,
    LobbyAnnounce = 2,

    // Game joining
    RequestJoin = 3,
    JoinAccept = 4,
    JoinDeny = 5,

    // Game leaving
    RequestGameLeave = 6,
    RequestLobbyLeave = 7,

    // Game management
    SetAccept = 8,
    MapAvailability = 9,
    Chat = 10,
    GameStart = 11,
    GameStartTimer = 12,
    GameOptions = 13,
    Inactive = 14,

    // Direct connect
    RequestGameInfo = 15,

    // Name updates
    NameChange = 16,
}

impl std::fmt::Display for LanMessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LanMessageType::RequestLocations => write!(f, "RequestLocations"),
            LanMessageType::GameAnnounce => write!(f, "GameAnnounce"),
            LanMessageType::LobbyAnnounce => write!(f, "LobbyAnnounce"),
            LanMessageType::RequestJoin => write!(f, "RequestJoin"),
            LanMessageType::JoinAccept => write!(f, "JoinAccept"),
            LanMessageType::JoinDeny => write!(f, "JoinDeny"),
            LanMessageType::RequestGameLeave => write!(f, "RequestGameLeave"),
            LanMessageType::RequestLobbyLeave => write!(f, "RequestLobbyLeave"),
            LanMessageType::SetAccept => write!(f, "SetAccept"),
            LanMessageType::MapAvailability => write!(f, "MapAvailability"),
            LanMessageType::Chat => write!(f, "Chat"),
            LanMessageType::GameStart => write!(f, "GameStart"),
            LanMessageType::GameStartTimer => write!(f, "GameStartTimer"),
            LanMessageType::GameOptions => write!(f, "GameOptions"),
            LanMessageType::Inactive => write!(f, "Inactive"),
            LanMessageType::RequestGameInfo => write!(f, "RequestGameInfo"),
            LanMessageType::NameChange => write!(f, "NameChange"),
        }
    }
}

/// Message payload types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePayload {
    /// No additional data (used for simple messages like RequestLocations)
    None,

    /// Game start timer (seconds)
    StartTimer { seconds: u32 },

    /// Game to leave
    GameToLeave { game_name: String },

    /// Game information announcement
    GameInfo {
        game_id: Uuid,
        game_name: String,
        in_progress: bool,
        options: String, // Serialized game options
        is_direct_connect: bool,
        player_count: u8,
        max_players: u8,
        is_public: bool,
        has_password: bool,
        version_hash: u32,
        map_crc: Option<u32>,
    },

    /// Player information for direct connect
    PlayerInfo { ip: IpAddr, player_name: String },

    /// Join request information
    GameToJoin {
        game_ip: IpAddr,
        exe_crc: u32,
        ini_crc: u32,
        serial_hash: String,
        player_name: String,
    },

    /// Join acceptance response
    GameJoined {
        game_name: String,
        game_ip: IpAddr,
        player_ip: IpAddr,
        slot_position: u8,
        game_id: Uuid,
    },

    /// Join denial response
    GameNotJoined {
        game_name: String,
        game_ip: IpAddr,
        player_ip: IpAddr,
        reason: LanResult,
    },

    /// Accept status
    Accept {
        game_name: String,
        is_accepted: bool,
    },

    /// Map availability status
    MapStatus {
        game_name: String,
        map_crc: u32,
        has_map: bool,
    },

    /// Chat message
    Chat {
        game_name: String,
        chat_type: ChatType,
        message: String,
    },

    /// Game options update
    GameOptions {
        options: String, // Serialized GameOptions
        is_public: bool,
    },

    /// Name change notification
    NameChange { old_name: String, new_name: String },
}

/// Main LAN message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanMessage {
    /// Message type
    pub message_type: LanMessageType,
    /// Message ID for tracking/deduplication
    pub message_id: Uuid,
    /// Sender information
    pub sender: PlayerInfo,
    /// Message payload
    pub payload: MessagePayload,
    /// Timestamp when message was created
    pub timestamp: u64,
    /// Message sequence number for ordering
    pub sequence: u32,
}

/// Player information in messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInfo {
    /// Player display name
    pub name: String,
    /// Login name (legacy)
    pub login_name: String,
    /// Host name (legacy)
    pub host_name: String,
    /// Player IP address
    pub ip: IpAddr,
    /// Player port
    pub port: u16,
}

impl PlayerInfo {
    /// Create new player info
    pub fn new(name: String, ip: IpAddr, port: u16) -> Self {
        Self {
            name,
            login_name: String::new(),
            host_name: String::new(),
            ip,
            port,
        }
    }

    /// Validate player name length
    pub fn validate_name(&self) -> Result<(), String> {
        if self.name.len() > MAX_PLAYER_NAME_LENGTH {
            return Err(format!(
                "Player name too long: {} > {}",
                self.name.len(),
                MAX_PLAYER_NAME_LENGTH
            ));
        }
        Ok(())
    }
}

impl LanMessage {
    /// Create a new message
    pub fn new(message_type: LanMessageType, sender: PlayerInfo, payload: MessagePayload) -> Self {
        Self {
            message_type,
            message_id: Uuid::new_v4(),
            sender,
            payload,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            sequence: 0, // Will be set by sender
        }
    }

    /// Create a request locations message
    pub fn request_locations(sender: PlayerInfo) -> Self {
        Self::new(
            LanMessageType::RequestLocations,
            sender,
            MessagePayload::None,
        )
    }

    /// Create a lobby announce message
    pub fn lobby_announce(sender: PlayerInfo) -> Self {
        Self::new(LanMessageType::LobbyAnnounce, sender, MessagePayload::None)
    }

    /// Create a game announce message
    pub fn game_announce(
        sender: PlayerInfo,
        game_id: Uuid,
        game_name: String,
        in_progress: bool,
        options: GameOptions,
        is_direct_connect: bool,
        player_count: u8,
        max_players: u8,
        is_public: bool,
        has_password: bool,
        version_hash: u32,
        map_crc: Option<u32>,
    ) -> Result<Self, String> {
        if game_name.len() > MAX_GAME_NAME_LENGTH {
            return Err(format!(
                "Game name too long: {} > {}",
                game_name.len(),
                MAX_GAME_NAME_LENGTH
            ));
        }

        let options_str = options
            .to_string()
            .map_err(|e| format!("Failed to serialize options: {}", e))?;

        if options_str.len() > MAX_OPTIONS_LENGTH {
            return Err(format!(
                "Options too long: {} > {}",
                options_str.len(),
                MAX_OPTIONS_LENGTH
            ));
        }

        let payload = MessagePayload::GameInfo {
            game_id,
            game_name,
            in_progress,
            options: options_str,
            is_direct_connect,
            player_count,
            max_players,
            is_public,
            has_password,
            version_hash,
            map_crc,
        };

        Ok(Self::new(LanMessageType::GameAnnounce, sender, payload))
    }

    /// Create a join request message
    pub fn request_join(
        sender: PlayerInfo,
        game_ip: IpAddr,
        exe_crc: u32,
        ini_crc: u32,
        serial_hash: String,
    ) -> Result<Self, String> {
        if sender.name.len() > MAX_PLAYER_NAME_LENGTH {
            return Err(format!(
                "Player name too long: {} > {}",
                sender.name.len(),
                MAX_PLAYER_NAME_LENGTH
            ));
        }

        if serial_hash.len() > MAX_SERIAL_LENGTH {
            return Err(format!(
                "Serial hash too long: {} > {}",
                serial_hash.len(),
                MAX_SERIAL_LENGTH
            ));
        }

        let payload = MessagePayload::GameToJoin {
            game_ip,
            exe_crc,
            ini_crc,
            serial_hash,
            player_name: sender.name.clone(),
        };

        Ok(Self::new(LanMessageType::RequestJoin, sender, payload))
    }

    /// Create a join accept message
    pub fn join_accept(
        sender: PlayerInfo,
        game_name: String,
        player_ip: IpAddr,
        slot_position: u8,
        game_id: Uuid,
    ) -> Result<Self, String> {
        if game_name.len() > MAX_GAME_NAME_LENGTH {
            return Err(format!(
                "Game name too long: {} > {}",
                game_name.len(),
                MAX_GAME_NAME_LENGTH
            ));
        }

        let payload = MessagePayload::GameJoined {
            game_name,
            game_ip: sender.ip,
            player_ip,
            slot_position,
            game_id,
        };

        Ok(Self::new(LanMessageType::JoinAccept, sender, payload))
    }

    /// Create a join deny message
    pub fn join_deny(
        sender: PlayerInfo,
        game_name: String,
        player_ip: IpAddr,
        reason: LanResult,
    ) -> Result<Self, String> {
        if game_name.len() > MAX_GAME_NAME_LENGTH {
            return Err(format!(
                "Game name too long: {} > {}",
                game_name.len(),
                MAX_GAME_NAME_LENGTH
            ));
        }

        let payload = MessagePayload::GameNotJoined {
            game_name,
            game_ip: sender.ip,
            player_ip,
            reason,
        };

        Ok(Self::new(LanMessageType::JoinDeny, sender, payload))
    }

    /// Create a chat message
    pub fn chat(
        sender: PlayerInfo,
        game_name: String,
        message: String,
        chat_type: ChatType,
    ) -> Result<Self, String> {
        if game_name.len() > MAX_GAME_NAME_LENGTH {
            return Err(format!(
                "Game name too long: {} > {}",
                game_name.len(),
                MAX_GAME_NAME_LENGTH
            ));
        }

        if message.len() > MAX_CHAT_LENGTH {
            return Err(format!(
                "Chat message too long: {} > {}",
                message.len(),
                MAX_CHAT_LENGTH
            ));
        }

        let payload = MessagePayload::Chat {
            game_name,
            chat_type,
            message,
        };

        Ok(Self::new(LanMessageType::Chat, sender, payload))
    }

    /// Create an accept status message
    pub fn set_accept(
        sender: PlayerInfo,
        game_name: String,
        is_accepted: bool,
    ) -> Result<Self, String> {
        if game_name.len() > MAX_GAME_NAME_LENGTH {
            return Err(format!(
                "Game name too long: {} > {}",
                game_name.len(),
                MAX_GAME_NAME_LENGTH
            ));
        }

        let payload = MessagePayload::Accept {
            game_name,
            is_accepted,
        };

        Ok(Self::new(LanMessageType::SetAccept, sender, payload))
    }

    /// Create a map availability message
    pub fn map_availability(
        sender: PlayerInfo,
        game_name: String,
        map_crc: u32,
        has_map: bool,
    ) -> Result<Self, String> {
        if game_name.len() > MAX_GAME_NAME_LENGTH {
            return Err(format!(
                "Game name too long: {} > {}",
                game_name.len(),
                MAX_GAME_NAME_LENGTH
            ));
        }

        let payload = MessagePayload::MapStatus {
            game_name,
            map_crc,
            has_map,
        };

        Ok(Self::new(LanMessageType::MapAvailability, sender, payload))
    }

    /// Create a game start message
    pub fn game_start(sender: PlayerInfo) -> Self {
        Self::new(LanMessageType::GameStart, sender, MessagePayload::None)
    }

    /// Create a game start timer message
    pub fn game_start_timer(sender: PlayerInfo, seconds: u32) -> Self {
        let payload = MessagePayload::StartTimer { seconds };
        Self::new(LanMessageType::GameStartTimer, sender, payload)
    }

    /// Create a game options message
    pub fn game_options(
        sender: PlayerInfo,
        options: GameOptions,
        is_public: bool,
    ) -> Result<Self, String> {
        let options_str = options
            .to_string()
            .map_err(|e| format!("Failed to serialize options: {}", e))?;

        if options_str.len() > MAX_OPTIONS_LENGTH {
            return Err(format!(
                "Options too long: {} > {}",
                options_str.len(),
                MAX_OPTIONS_LENGTH
            ));
        }

        let payload = MessagePayload::GameOptions {
            options: options_str,
            is_public,
        };

        Ok(Self::new(LanMessageType::GameOptions, sender, payload))
    }

    /// Create a leave game message
    pub fn request_game_leave(sender: PlayerInfo, game_name: String) -> Result<Self, String> {
        if game_name.len() > MAX_GAME_NAME_LENGTH {
            return Err(format!(
                "Game name too long: {} > {}",
                game_name.len(),
                MAX_GAME_NAME_LENGTH
            ));
        }

        let payload = MessagePayload::GameToLeave { game_name };
        Ok(Self::new(LanMessageType::RequestGameLeave, sender, payload))
    }

    /// Create a leave lobby message
    pub fn request_lobby_leave(sender: PlayerInfo) -> Self {
        Self::new(
            LanMessageType::RequestLobbyLeave,
            sender,
            MessagePayload::None,
        )
    }

    /// Create an inactive message
    pub fn inactive(sender: PlayerInfo) -> Self {
        Self::new(LanMessageType::Inactive, sender, MessagePayload::None)
    }

    /// Create a request game info message (for direct connect)
    pub fn request_game_info(sender: PlayerInfo, target_ip: IpAddr) -> Self {
        let payload = MessagePayload::PlayerInfo {
            ip: target_ip,
            player_name: sender.name.clone(),
        };
        Self::new(LanMessageType::RequestGameInfo, sender, payload)
    }

    /// Create a name change message
    pub fn name_change(sender: PlayerInfo, old_name: String, new_name: String) -> Self {
        let payload = MessagePayload::NameChange { old_name, new_name };
        Self::new(LanMessageType::NameChange, sender, payload)
    }

    /// Serialize message to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(self).map_err(|e| format!("Failed to serialize message: {}", e))
    }

    /// Deserialize message from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(data).map_err(|e| format!("Failed to deserialize message: {}", e))
    }

    /// Get the expected response message type for this message
    pub fn expected_response(&self) -> Option<LanMessageType> {
        match self.message_type {
            LanMessageType::RequestLocations => Some(LanMessageType::LobbyAnnounce),
            LanMessageType::RequestJoin => Some(LanMessageType::JoinAccept), // or JoinDeny
            LanMessageType::RequestGameInfo => Some(LanMessageType::GameAnnounce),
            _ => None,
        }
    }

    /// Check if this is a response to another message type
    pub fn is_response_to(&self, other_type: LanMessageType) -> bool {
        match (other_type, self.message_type) {
            (LanMessageType::RequestLocations, LanMessageType::LobbyAnnounce) => true,
            (LanMessageType::RequestLocations, LanMessageType::GameAnnounce) => true,
            (LanMessageType::RequestJoin, LanMessageType::JoinAccept) => true,
            (LanMessageType::RequestJoin, LanMessageType::JoinDeny) => true,
            (LanMessageType::RequestGameInfo, LanMessageType::GameAnnounce) => true,
            _ => false,
        }
    }

    /// Get message priority for network transmission
    pub fn get_priority(&self) -> u8 {
        match self.message_type {
            // High priority - game critical
            LanMessageType::GameStart => 0,
            LanMessageType::GameStartTimer => 0,
            LanMessageType::JoinAccept => 0,
            LanMessageType::JoinDeny => 0,

            // Medium priority - game management
            LanMessageType::RequestJoin => 1,
            LanMessageType::GameOptions => 1,
            LanMessageType::SetAccept => 1,
            LanMessageType::MapAvailability => 1,
            LanMessageType::RequestGameLeave => 1,

            // Low priority - discovery and chat
            LanMessageType::RequestLocations => 2,
            LanMessageType::GameAnnounce => 2,
            LanMessageType::LobbyAnnounce => 2,
            LanMessageType::Chat => 2,
            LanMessageType::Inactive => 2,
            LanMessageType::NameChange => 2,

            // Lowest priority - misc
            LanMessageType::RequestLobbyLeave => 3,
            LanMessageType::RequestGameInfo => 3,
        }
    }

    /// Set sequence number
    pub fn set_sequence(&mut self, sequence: u32) {
        self.sequence = sequence;
    }

    /// Check if message requires reliable delivery
    pub fn requires_reliable_delivery(&self) -> bool {
        match self.message_type {
            LanMessageType::RequestJoin => true,
            LanMessageType::JoinAccept => true,
            LanMessageType::JoinDeny => true,
            LanMessageType::GameStart => true,
            LanMessageType::GameOptions => true,
            LanMessageType::RequestGameLeave => true,
            LanMessageType::RequestLobbyLeave => true,
            _ => false,
        }
    }

    /// Get message size estimate
    pub fn estimated_size(&self) -> usize {
        // Base message overhead
        let mut size = 64;

        // Add payload size estimates
        match &self.payload {
            MessagePayload::None => {}
            MessagePayload::StartTimer { .. } => size += 4,
            MessagePayload::GameToLeave { game_name } => size += game_name.len(),
            MessagePayload::GameInfo {
                game_name, options, ..
            } => {
                size += game_name.len() + options.len() + 20;
            }
            MessagePayload::PlayerInfo { player_name, .. } => size += player_name.len() + 16,
            MessagePayload::GameToJoin {
                player_name,
                serial_hash,
                ..
            } => {
                size += player_name.len() + serial_hash.len() + 20;
            }
            MessagePayload::GameJoined { game_name, .. } => size += game_name.len() + 32,
            MessagePayload::GameNotJoined { game_name, .. } => size += game_name.len() + 20,
            MessagePayload::Accept { game_name, .. } => size += game_name.len() + 1,
            MessagePayload::MapStatus { game_name, .. } => size += game_name.len() + 5,
            MessagePayload::Chat {
                game_name, message, ..
            } => {
                size += game_name.len() + message.len() + 4;
            }
            MessagePayload::GameOptions { options, .. } => size += options.len() + 1,
            MessagePayload::NameChange { old_name, new_name } => {
                size += old_name.len() + new_name.len();
            }
        }

        size
    }
}

/// Message builder for convenient message creation
pub struct MessageBuilder {
    sender: PlayerInfo,
    sequence: u32,
}

impl MessageBuilder {
    /// Create a new message builder
    pub fn new(sender: PlayerInfo) -> Self {
        Self {
            sender,
            sequence: 0,
        }
    }

    /// Set sequence number
    pub fn with_sequence(mut self, sequence: u32) -> Self {
        self.sequence = sequence;
        self
    }

    /// Build a request locations message
    pub fn request_locations(self) -> LanMessage {
        let mut msg = LanMessage::request_locations(self.sender);
        msg.set_sequence(self.sequence);
        msg
    }

    /// Build a lobby announce message
    pub fn lobby_announce(self) -> LanMessage {
        let mut msg = LanMessage::lobby_announce(self.sender);
        msg.set_sequence(self.sequence);
        msg
    }

    // Build other message types...
    // (Additional builder methods can be added as needed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_player_info_validation() {
        let mut player = PlayerInfo::new("Test".to_string(), IpAddr::V4(Ipv4Addr::LOCALHOST), 8088);
        assert!(player.validate_name().is_ok());

        // Test name too long
        player.name = "ThisNameIsTooLong".to_string();
        assert!(player.validate_name().is_err());
    }

    #[test]
    fn test_message_creation() {
        let player = PlayerInfo::new(
            "TestPlayer".to_string(),
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            8088,
        );

        let msg = LanMessage::request_locations(player.clone());
        assert_eq!(msg.message_type, LanMessageType::RequestLocations);
        assert!(matches!(msg.payload, MessagePayload::None));

        let msg = LanMessage::lobby_announce(player.clone());
        assert_eq!(msg.message_type, LanMessageType::LobbyAnnounce);
    }

    #[test]
    fn test_game_announce_message() {
        let player = PlayerInfo::new("Host".to_string(), IpAddr::V4(Ipv4Addr::LOCALHOST), 8088);
        let options = GameOptions::default();

        let msg = LanMessage::game_announce(
            player,
            Uuid::new_v4(),
            "Test Game".to_string(),
            false,
            options,
            false,
            1,
            8,
            true,
            false,
            12345,
            Some(67890),
        )
        .unwrap();

        assert_eq!(msg.message_type, LanMessageType::GameAnnounce);
        if let MessagePayload::GameInfo {
            game_name,
            player_count,
            max_players,
            ..
        } = msg.payload
        {
            assert_eq!(game_name, "Test Game");
            assert_eq!(player_count, 1);
            assert_eq!(max_players, 8);
        } else {
            panic!("Wrong payload type");
        }
    }

    #[test]
    fn test_join_request_message() {
        let player = PlayerInfo::new("Player".to_string(), IpAddr::V4(Ipv4Addr::LOCALHOST), 8088);
        let host_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        let msg =
            LanMessage::request_join(player, host_ip, 12345, 67890, "serial_hash".to_string())
                .unwrap();

        assert_eq!(msg.message_type, LanMessageType::RequestJoin);
        if let MessagePayload::GameToJoin {
            game_ip,
            exe_crc,
            ini_crc,
            ..
        } = msg.payload
        {
            assert_eq!(game_ip, host_ip);
            assert_eq!(exe_crc, 12345);
            assert_eq!(ini_crc, 67890);
        } else {
            panic!("Wrong payload type");
        }
    }

    #[test]
    fn test_chat_message() {
        let player = PlayerInfo::new("Player".to_string(), IpAddr::V4(Ipv4Addr::LOCALHOST), 8088);

        let msg = LanMessage::chat(
            player,
            "Test Game".to_string(),
            "Hello world!".to_string(),
            ChatType::Normal,
        )
        .unwrap();

        assert_eq!(msg.message_type, LanMessageType::Chat);
        if let MessagePayload::Chat {
            message, chat_type, ..
        } = msg.payload
        {
            assert_eq!(message, "Hello world!");
            assert_eq!(chat_type, ChatType::Normal);
        } else {
            panic!("Wrong payload type");
        }
    }

    #[test]
    fn test_message_validation() {
        let player = PlayerInfo::new("Player".to_string(), IpAddr::V4(Ipv4Addr::LOCALHOST), 8088);

        // Test game name too long
        let long_name = "A".repeat(MAX_GAME_NAME_LENGTH + 1);
        let result = LanMessage::chat(
            player.clone(),
            long_name,
            "Hi".to_string(),
            ChatType::Normal,
        );
        assert!(result.is_err());

        // Test chat message too long
        let long_message = "A".repeat(MAX_CHAT_LENGTH + 1);
        let result = LanMessage::chat(player, "Game".to_string(), long_message, ChatType::Normal);
        assert!(result.is_err());
    }

    #[test]
    fn test_message_serialization() {
        let player = PlayerInfo::new("Player".to_string(), IpAddr::V4(Ipv4Addr::LOCALHOST), 8088);
        let msg = LanMessage::request_locations(player);

        let bytes = msg.to_bytes().unwrap();
        assert!(!bytes.is_empty());

        let deserialized = LanMessage::from_bytes(&bytes).unwrap();
        assert_eq!(deserialized.message_type, msg.message_type);
        assert_eq!(deserialized.message_id, msg.message_id);
    }

    #[test]
    fn test_message_responses() {
        assert_eq!(
            LanMessage::request_locations(PlayerInfo::new(
                "Test".to_string(),
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                8088
            ))
            .expected_response(),
            Some(LanMessageType::LobbyAnnounce)
        );

        let lobby_msg = LanMessage::lobby_announce(PlayerInfo::new(
            "Test".to_string(),
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            8088,
        ));
        assert!(lobby_msg.is_response_to(LanMessageType::RequestLocations));
    }

    #[test]
    fn test_message_priorities() {
        let player = PlayerInfo::new("Test".to_string(), IpAddr::V4(Ipv4Addr::LOCALHOST), 8088);

        let start_msg = LanMessage::game_start(player.clone());
        let chat_msg = LanMessage::chat(
            player,
            "Game".to_string(),
            "Hi".to_string(),
            ChatType::Normal,
        )
        .unwrap();

        assert!(start_msg.get_priority() < chat_msg.get_priority());
    }

    #[test]
    fn test_message_builder() {
        let player = PlayerInfo::new("Builder".to_string(), IpAddr::V4(Ipv4Addr::LOCALHOST), 8088);
        let builder = MessageBuilder::new(player).with_sequence(42);

        let msg = builder.request_locations();
        assert_eq!(msg.sequence, 42);
        assert_eq!(msg.message_type, LanMessageType::RequestLocations);
    }
}
