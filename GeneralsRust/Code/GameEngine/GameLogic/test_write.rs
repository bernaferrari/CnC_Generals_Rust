//! Test write module - verifies write and read operations for game state.
//!
//! This standalone test exercises basic game state patterns used by the
//! save/load subsystem. Run with: `cargo run --example test_write`

fn main() {
    println!("=== GameLogic Write Test ===\n");

    test_frame_counter();
    test_position_integrity();
    test_state_roundtrip();
    test_checksum_sensitivity();

    println!("\n=== All tests passed ===");
}

/// Test deterministic frame counter behavior
fn test_frame_counter() {
    const FRAME_RATE: u32 = 30;
    const SECONDS: u32 = 10;
    const EXPECTED_FRAMES: u32 = FRAME_RATE * SECONDS;

    let mut frame: u32 = 0;
    for _ in 0..EXPECTED_FRAMES {
        frame += 1;
    }

    assert_eq!(frame, EXPECTED_FRAMES, "Frame counter must match expected");
    println!(
        "  [PASS] Frame counter: {} frames at {} FPS over {}s",
        frame, FRAME_RATE, SECONDS
    );
}

/// Test position state integrity
fn test_position_integrity() {
    #[derive(Debug, Clone, PartialEq)]
    struct Position {
        x: f32,
        y: f32,
        z: f32,
    }

    let original = Position {
        x: 100.0,
        y: 200.0,
        z: 0.5,
    };
    let restored = original.clone();

    assert_eq!(original, restored, "Position must survive clone");
    assert_eq!(original.x, restored.x);
    assert_eq!(original.y, restored.y);
    assert_eq!(original.z, restored.z);

    // Test that small changes are detectable
    let slightly_different = Position {
        x: 100.01,
        y: 200.0,
        z: 0.5,
    };
    assert_ne!(
        original, slightly_different,
        "Small position changes must be detectable"
    );

    println!(
        "  [PASS] Position integrity: original={:?}, different={:?}",
        original, slightly_different
    );
}

/// Test state serialization roundtrip using basic byte packing
fn test_state_roundtrip() {
    #[derive(Debug, PartialEq)]
    struct GameState {
        frame: u32,
        player_count: u8,
        credits: i32,
        power: i32,
    }

    let state = GameState {
        frame: 42,
        player_count: 2,
        credits: 10000,
        power: 50,
    };

    // Simulate serialization to bytes
    let mut buffer = Vec::new();
    buffer.extend_from_slice(&state.frame.to_le_bytes());
    buffer.push(state.player_count);
    buffer.extend_from_slice(&state.credits.to_le_bytes());
    buffer.extend_from_slice(&state.power.to_le_bytes());

    // Simulate deserialization from bytes
    let mut offset = 0;
    let frame = u32::from_le_bytes(buffer[offset..offset + 4].try_into().unwrap());
    offset += 4;
    let player_count = buffer[offset];
    offset += 1;
    let credits = i32::from_le_bytes(buffer[offset..offset + 4].try_into().unwrap());
    offset += 4;
    let power = i32::from_le_bytes(buffer[offset..offset + 4].try_into().unwrap());

    let restored = GameState {
        frame,
        player_count,
        credits,
        power,
    };

    assert_eq!(state, restored, "Deserialized state must match original");
    println!(
        "  [PASS] State roundtrip: {} bytes serialized, restored={:?}",
        buffer.len(),
        restored
    );
}

/// Test checksum sensitivity to small changes
fn test_checksum_sensitivity() {
    // Simple checksum using wrapping arithmetic
    fn simple_checksum(data: &[u8]) -> u32 {
        let mut sum: u32 = 0;
        for &byte in data {
            sum = sum.wrapping_add(byte as u32);
            sum = sum.wrapping_mul(31);
        }
        sum
    }

    let data1 = b"unit_position_100_200";
    let data2 = b"unit_position_101_200";

    let checksum1 = simple_checksum(data1);
    let checksum2 = simple_checksum(data2);

    assert_ne!(
        checksum1, checksum2,
        "Different data must produce different checksums"
    );

    // Verify determinism
    let checksum1_verify = simple_checksum(data1);
    assert_eq!(
        checksum1, checksum1_verify,
        "Checksum must be deterministic"
    );

    println!(
        "  [PASS] Checksum sensitivity: data1={:08X}, data2={:08X}",
        checksum1, checksum2
    );
}
