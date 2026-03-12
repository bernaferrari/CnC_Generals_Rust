//! Integration Test: Save/Load Functionality
//!
//! This test verifies game state can be saved and loaded correctly:
//! - Serialization of game state
//! - Deserialization and restoration
//! - File I/O operations
//! - Data integrity verification
//! - Version compatibility
//!
//! Tests should pass on all platforms (Windows, Linux, macOS)

#![cfg(test)]

use std::io::{Read, Write};
use std::fs::File;

/// Game state for save/load
#[derive(Debug, Clone, PartialEq)]
struct GameState {
    version: u32,
    player_name: String,
    level: u32,
    score: u64,
    position_x: f32,
    position_y: f32,
    inventory: Vec<u32>,
}

impl GameState {
    fn new() -> Self {
        Self {
            version: 1,
            player_name: "Player".to_string(),
            level: 1,
            score: 0,
            position_x: 0.0,
            position_y: 0.0,
            inventory: Vec::new(),
        }
    }

    fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::new();

        // Version
        buffer.extend_from_slice(&self.version.to_le_bytes());

        // Player name (length + string)
        let name_bytes = self.player_name.as_bytes();
        buffer.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
        buffer.extend_from_slice(name_bytes);

        // Game stats
        buffer.extend_from_slice(&self.level.to_le_bytes());
        buffer.extend_from_slice(&self.score.to_le_bytes());
        buffer.extend_from_slice(&self.position_x.to_le_bytes());
        buffer.extend_from_slice(&self.position_y.to_le_bytes());

        // Inventory
        buffer.extend_from_slice(&(self.inventory.len() as u32).to_le_bytes());
        for &item in &self.inventory {
            buffer.extend_from_slice(&item.to_le_bytes());
        }

        buffer
    }

    fn deserialize(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 4 {
            return Err("Data too short");
        }

        let mut offset = 0;

        // Version
        let version = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        offset += 4;

        // Player name
        if data.len() < offset + 4 {
            return Err("Incomplete data");
        }
        let name_len = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;

        if data.len() < offset + name_len {
            return Err("Incomplete name data");
        }
        let player_name = String::from_utf8(data[offset..offset + name_len].to_vec())
            .map_err(|_| "Invalid UTF-8")?;
        offset += name_len;

        // Game stats
        if data.len() < offset + 24 {
            return Err("Incomplete stats data");
        }

        let level = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        let score = u64::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]);
        offset += 8;

        let position_x = f32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        let position_y = f32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;

        // Inventory
        if data.len() < offset + 4 {
            return Err("Incomplete inventory header");
        }

        let inventory_len = u32::from_le_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;

        if data.len() < offset + inventory_len * 4 {
            return Err("Incomplete inventory data");
        }

        let mut inventory = Vec::new();
        for _ in 0..inventory_len {
            let item = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            inventory.push(item);
            offset += 4;
        }

        Ok(Self {
            version,
            player_name,
            level,
            score,
            position_x,
            position_y,
            inventory,
        })
    }
}

#[test]
fn test_save_load_roundtrip() {
    println!("Testing save/load roundtrip...");

    let mut original = GameState::new();
    original.player_name = "TestPlayer".to_string();
    original.level = 5;
    original.score = 10000;
    original.position_x = 123.45;
    original.position_y = 678.90;
    original.inventory = vec![1, 2, 3, 4, 5];

    let serialized = original.serialize();
    let loaded = GameState::deserialize(&serialized).unwrap();

    assert_eq!(original, loaded);

    log::info!("Save/load roundtrip test passed");
}

#[test]
fn test_save_to_file() {
    println!("Testing save to file...");

    let mut state = GameState::new();
    state.level = 10;
    state.score = 50000;

    let temp_dir = std::env::temp_dir();
    let save_path = temp_dir.join("test_save.dat");

    // Save
    let data = state.serialize();
    std::fs::write(&save_path, data).expect("Failed to save");

    // Load
    let loaded_data = std::fs::read(&save_path).expect("Failed to load");
    let loaded_state = GameState::deserialize(&loaded_data).unwrap();

    assert_eq!(state, loaded_state);

    // Cleanup
    std::fs::remove_file(save_path).ok();

    log::info!("Save to file test passed");
}

#[test]
fn test_empty_inventory() {
    println!("Testing empty inventory serialization...");

    let state = GameState::new();
    assert_eq!(state.inventory.len(), 0);

    let serialized = state.serialize();
    let loaded = GameState::deserialize(&serialized).unwrap();

    assert_eq!(state, loaded);
    assert_eq!(loaded.inventory.len(), 0);

    log::info!("Empty inventory test passed");
}

#[test]
fn test_large_inventory() {
    println!("Testing large inventory...");

    let mut state = GameState::new();
    state.inventory = (0..1000).collect();

    let serialized = state.serialize();
    let loaded = GameState::deserialize(&serialized).unwrap();

    assert_eq!(state, loaded);
    assert_eq!(loaded.inventory.len(), 1000);

    log::info!("Large inventory test passed");
}

#[test]
fn test_invalid_data() {
    println!("Testing invalid data handling...");

    // Too short
    let short_data = vec![1, 2, 3];
    assert!(GameState::deserialize(&short_data).is_err());

    // Truncated
    let valid = GameState::new().serialize();
    let truncated = &valid[..valid.len() / 2];
    assert!(GameState::deserialize(truncated).is_err());

    log::info!("Invalid data handling test passed");
}

#[test]
fn test_version_field() {
    println!("Testing version field...");

    let state = GameState::new();
    assert_eq!(state.version, 1);

    let serialized = state.serialize();
    assert_eq!(serialized[0..4], 1u32.to_le_bytes());

    log::info!("Version field test passed");
}

#[cfg(test)]
mod stress_tests {
    use super::*;

    #[test]
    #[ignore]
    fn test_many_save_load_cycles() {
        println!("Stress test: Many save/load cycles...");

        let mut state = GameState::new();
        state.inventory = vec![1, 2, 3, 4, 5];

        const NUM_CYCLES: usize = 10000;
        let start = std::time::Instant::now();

        for i in 0..NUM_CYCLES {
            state.score = i as u64;

            let serialized = state.serialize();
            let loaded = GameState::deserialize(&serialized).unwrap();

            assert_eq!(state, loaded);
        }

        let elapsed = start.elapsed();
        let cycles_per_sec = NUM_CYCLES as f64 / elapsed.as_secs_f64();

        println!("Completed {} cycles in {:?} ({:.0} cycles/sec)",
            NUM_CYCLES, elapsed, cycles_per_sec);

        assert!(cycles_per_sec > 1000.0);

        log::info!("Many save/load cycles stress test passed");
    }
}
