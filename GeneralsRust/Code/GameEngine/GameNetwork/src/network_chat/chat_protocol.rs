//! Chat Protocol Module
//!
//! Defines the chat message protocol and serialization format

use crate::error::{NetworkError, NetworkResult};
use crate::network_chat::{ChatChannel, ChatMessageType, UnifiedChatMessage};
use byteorder::{ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read, Write};

/// Chat packet types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ChatPacketType {
    /// Regular chat message
    Message = 0,
    /// Emote message
    Emote = 1,
    /// Private message
    Private = 2,
    /// System notification
    System = 3,
    /// Typing indicator
    Typing = 4,
    /// Player joined
    PlayerJoined = 5,
    /// Player left
    PlayerLeft = 6,
    /// Channel change
    ChannelChange = 7,
    /// Moderation action
    Moderation = 8,
}

impl ChatPacketType {
    pub fn from_u8(value: u8) -> NetworkResult<Self> {
        match value {
            0 => Ok(ChatPacketType::Message),
            1 => Ok(ChatPacketType::Emote),
            2 => Ok(ChatPacketType::Private),
            3 => Ok(ChatPacketType::System),
            4 => Ok(ChatPacketType::Typing),
            5 => Ok(ChatPacketType::PlayerJoined),
            6 => Ok(ChatPacketType::PlayerLeft),
            7 => Ok(ChatPacketType::ChannelChange),
            8 => Ok(ChatPacketType::Moderation),
            _ => Err(NetworkError::invalid_command(format!("Unknown packet type: {}", value))),
        }
    }
}

/// Chat packet header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatPacketHeader {
    /// Packet type
    pub packet_type: ChatPacketType,
    /// Protocol version
    pub protocol_version: u32,
    /// Packet ID
    pub packet_id: u64,
    /// Sender player ID
    pub sender_id: u32,
    /// Data length
    pub data_length: u16,
}

impl ChatPacketHeader {
    /// Header size in bytes
    pub const SIZE: usize = 1 + 4 + 8 + 4 + 2; // = 19

    /// Serialize header to bytes
    pub fn to_bytes(&self) -> NetworkResult<Vec<u8>> {
        let mut buffer = Vec::with_capacity(Self::SIZE);

        buffer.write_u8(self.packet_type as u8)
            .map_err(|e| NetworkError::serialization(e.to_string()))?;
        buffer.write_u32::<byteorder::LittleEndian>(self.protocol_version)
            .map_err(|e| NetworkError::serialization(e.to_string()))?;
        buffer.write_u64::<byteorder::LittleEndian>(self.packet_id)
            .map_err(|e| NetworkError::serialization(e.to_string()))?;
        buffer.write_u32::<byteorder::LittleEndian>(self.sender_id)
            .map_err(|e| NetworkError::serialization(e.to_string()))?;
        buffer.write_u16::<byteorder::LittleEndian>(self.data_length)
            .map_err(|e| NetworkError::serialization(e.to_string()))?;

        Ok(buffer)
    }

    /// Deserialize header from bytes
    pub fn from_bytes(data: &[u8]) -> NetworkResult<Self> {
        if data.len() < Self::SIZE {
            return Err(NetworkError::invalid_command(
                format!("Header too short: {} < {}", data.len(), Self::SIZE)
            ));
        }

        let mut cursor = Cursor::new(data);

        let packet_type_byte = cursor.read_u8()
            .map_err(|e| NetworkError::serialization(e.to_string()))?;
        let packet_type = ChatPacketType::from_u8(packet_type_byte)?;

        let protocol_version = cursor.read_u32::<byteorder::LittleEndian>()
            .map_err(|e| NetworkError::serialization(e.to_string()))?;
        let packet_id = cursor.read_u64::<byteorder::LittleEndian>()
            .map_err(|e| NetworkError::serialization(e.to_string()))?;
        let sender_id = cursor.read_u32::<byteorder::LittleEndian>()
            .map_err(|e| NetworkError::serialization(e.to_string()))?;
        let data_length = cursor.read_u16::<byteorder::LittleEndian>()
            .map_err(|e| NetworkError::serialization(e.to_string()))?;

        Ok(Self {
            packet_type,
            protocol_version,
            packet_id,
            sender_id,
            data_length,
        })
    }
}

/// Complete chat packet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatPacket {
    /// Packet header
    pub header: ChatPacketHeader,
    /// Packet data
    pub data: Vec<u8>,
}

impl ChatPacket {
    /// Serialize complete packet
    pub fn to_bytes(&self) -> NetworkResult<Vec<u8>> {
        let header_bytes = self.header.to_bytes()?;
        let mut buffer = header_bytes;
        buffer.extend_from_slice(&self.data);
        Ok(buffer)
    }

    /// Deserialize packet from bytes
    pub fn from_bytes(data: &[u8]) -> NetworkResult<Self> {
        let header = ChatPacketHeader::from_bytes(data)?;
        let data_start = ChatPacketHeader::SIZE;
        let data_end = data_start + header.data_length as usize;

        if data.len() < data_end {
            return Err(NetworkError::invalid_command(
                format!("Packet too short: {} < {}", data.len(), data_end)
            ));
        }

        let packet_data = data[data_start..data_end].to_vec();

        Ok(Self {
            header,
            data: packet_data,
        })
    }

    /// Create message packet
    pub fn create_message(
        sender_id: u32,
        message: &UnifiedChatMessage,
    ) -> NetworkResult<Self> {
        let message_data = bincode::serialize(message)
            .map_err(|e| NetworkError::serialization(e.to_string()))?;

        let header = ChatPacketHeader {
            packet_type: ChatPacketType::Message,
            protocol_version: crate::network_chat::CHAT_PROTOCOL_VERSION,
            packet_id: uuid::Uuid::new_v4().as_u128() as u64,
            sender_id,
            data_length: message_data.len() as u16,
        };

        Ok(Self {
            header,
            data: message_data,
        })
    }

    /// Extract message from packet
    pub fn extract_message(&self) -> NetworkResult<UnifiedChatMessage> {
        bincode::deserialize(&self.data)
            .map_err(|e| NetworkError::serialization(e.to_string()))
    }
}

/// Chat protocol constants
pub const CHAT_PROTOCOL_MAX_PACKET_SIZE: usize = 1400; // Safe UDP size
pub const CHAT_PROTOCOL_MAX_MESSAGE_SIZE: usize = 512;
pub const CHAT_PROTOCOL_MAX_EMOTICON_SIZE: usize = 4096;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network_chat::CHAT_PROTOCOL_VERSION;

    #[test]
    fn test_header_serialization() {
        let header = ChatPacketHeader {
            packet_type: ChatPacketType::Message,
            protocol_version: CHAT_PROTOCOL_VERSION,
            packet_id: 12345,
            sender_id: 1,
            data_length: 100,
        };

        let bytes = header.to_bytes().unwrap();
        let decoded = ChatPacketHeader::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.packet_type, ChatPacketType::Message);
        assert_eq!(decoded.protocol_version, CHAT_PROTOCOL_VERSION);
        assert_eq!(decoded.packet_id, 12345);
        assert_eq!(decoded.sender_id, 1);
        assert_eq!(decoded.data_length, 100);
    }

    #[test]
    fn test_packet_type_roundtrip() {
        let types = vec![
            ChatPacketType::Message,
            ChatPacketType::Emote,
            ChatPacketType::Private,
            ChatPacketType::System,
            ChatPacketType::Typing,
            ChatPacketType::PlayerJoined,
            ChatPacketType::PlayerLeft,
            ChatPacketType::ChannelChange,
            ChatPacketType::Moderation,
        ];

        for packet_type in types {
            let byte = packet_type as u8;
            let decoded = ChatPacketType::from_u8(byte).unwrap();
            assert_eq!(decoded, packet_type);
        }
    }

    #[test]
    fn test_invalid_packet_type() {
        let result = ChatPacketType::from_u8(255);
        assert!(result.is_err());
    }
}
