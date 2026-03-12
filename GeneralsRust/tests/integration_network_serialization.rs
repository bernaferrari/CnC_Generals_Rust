//! Integration Test: Network Message Serialization
//!
//! This test verifies that network messages can be serialized and deserialized correctly:
//! - Command serialization
//! - State synchronization messages
//! - Frame data serialization
//! - Binary protocol compatibility
//! - Byte order handling
//!
//! Tests should pass on all platforms (Windows, Linux, macOS)

#![cfg(test)]

use std::io::{Cursor, Read, Write};

/// Command types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum CommandType {
    Move = 1,
    Attack = 2,
    Build = 3,
    Stop = 4,
}

/// Network message
#[derive(Debug, Clone, PartialEq)]
struct NetworkMessage {
    message_type: u8,
    player_id: u8,
    frame_number: u32,
    data: Vec<u8>,
}

impl NetworkMessage {
    fn new(message_type: u8, player_id: u8, frame_number: u32) -> Self {
        Self {
            message_type,
            player_id,
            frame_number,
            data: Vec::new(),
        }
    }

    fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::new();

        buffer.push(self.message_type);
        buffer.push(self.player_id);
        buffer.extend_from_slice(&self.frame_number.to_le_bytes());
        buffer.extend_from_slice(&(self.data.len() as u32).to_le_bytes());
        buffer.extend_from_slice(&self.data);

        buffer
    }

    fn deserialize(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 10 {
            return Err("Data too short");
        }

        let message_type = data[0];
        let player_id = data[1];

        let frame_number = u32::from_le_bytes([data[2], data[3], data[4], data[5]]);
        let data_len = u32::from_le_bytes([data[6], data[7], data[8], data[9]]) as usize;

        if data.len() < 10 + data_len {
            return Err("Incomplete data");
        }

        let payload = data[10..10 + data_len].to_vec();

        Ok(Self {
            message_type,
            player_id,
            frame_number,
            data: payload,
        })
    }
}

#[test]
fn test_basic_serialization() {
    println!("Testing basic message serialization...");

    let msg = NetworkMessage::new(1, 0, 100);
    let serialized = msg.serialize();

    assert!(serialized.len() >= 10);
    assert_eq!(serialized[0], 1);
    assert_eq!(serialized[1], 0);

    log::info!("Basic serialization test passed");
}

#[test]
fn test_serialization_roundtrip() {
    println!("Testing serialization roundtrip...");

    let original = NetworkMessage::new(2, 1, 500);
    let serialized = original.serialize();
    let deserialized = NetworkMessage::deserialize(&serialized).unwrap();

    assert_eq!(original, deserialized);

    log::info!("Serialization roundtrip test passed");
}

#[test]
fn test_with_payload() {
    println!("Testing serialization with payload...");

    let mut msg = NetworkMessage::new(3, 2, 1000);
    msg.data = vec![1, 2, 3, 4, 5];

    let serialized = msg.serialize();
    let deserialized = NetworkMessage::deserialize(&serialized).unwrap();

    assert_eq!(msg, deserialized);
    assert_eq!(deserialized.data, vec![1, 2, 3, 4, 5]);

    log::info!("Payload serialization test passed");
}

#[test]
fn test_byte_order() {
    println!("Testing byte order handling...");

    let msg = NetworkMessage::new(1, 0, 0x12345678);
    let serialized = msg.serialize();

    // Check little-endian byte order
    let frame_bytes = &serialized[2..6];
    assert_eq!(frame_bytes, &[0x78, 0x56, 0x34, 0x12]);

    log::info!("Byte order test passed");
}

#[test]
fn test_invalid_deserialization() {
    println!("Testing invalid deserialization...");

    // Too short
    let short_data = vec![1, 2, 3];
    assert!(NetworkMessage::deserialize(&short_data).is_err());

    // Incomplete payload
    let incomplete = vec![1, 0, 0, 0, 0, 1, 10, 0, 0, 0, 1, 2]; // Claims 10 bytes, only has 2
    assert!(NetworkMessage::deserialize(&incomplete).is_err());

    log::info!("Invalid deserialization test passed");
}

#[test]
fn test_large_payload() {
    println!("Testing large payload serialization...");

    let mut msg = NetworkMessage::new(4, 3, 2000);
    msg.data = vec![0x42; 10000]; // 10KB payload

    let serialized = msg.serialize();
    let deserialized = NetworkMessage::deserialize(&serialized).unwrap();

    assert_eq!(msg, deserialized);
    assert_eq!(deserialized.data.len(), 10000);

    log::info!("Large payload test passed");
}

#[cfg(test)]
mod stress_tests {
    use super::*;

    #[test]
    #[ignore]
    fn test_many_serializations() {
        println!("Stress test: Many serializations...");

        const NUM_MESSAGES: usize = 100000;
        let start = std::time::Instant::now();

        for i in 0..NUM_MESSAGES {
            let msg = NetworkMessage::new((i % 255) as u8, (i % 8) as u8, i as u32);
            let _serialized = msg.serialize();
        }

        let elapsed = start.elapsed();
        let msgs_per_sec = NUM_MESSAGES as f64 / elapsed.as_secs_f64();

        println!("Serialized {} messages in {:?} ({:.0} msg/sec)",
            NUM_MESSAGES, elapsed, msgs_per_sec);

        assert!(msgs_per_sec > 100000.0);

        log::info!("Many serializations stress test passed");
    }
}
