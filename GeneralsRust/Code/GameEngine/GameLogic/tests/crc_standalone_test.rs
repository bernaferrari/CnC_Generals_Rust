//! Standalone CRC Test - Tests CRC functionality independently
//!
//! This test verifies that the CRC32 system works correctly for
//! multiplayer synchronization without depending on the full GameLogic system.

use crc32fast::Hasher;

/// Test basic CRC32 functionality
#[test]
fn test_crc32_basic_functionality() {
    let data = b"Hello, Multiplayer!";
    let crc1 = crc32fast::hash(data);
    let crc2 = crc32fast::hash(data);

    assert_eq!(crc1, crc2, "Same data should produce same CRC");

    let different_data = b"Hello, Singleplayer!";
    let crc3 = crc32fast::hash(different_data);

    assert_ne!(crc1, crc3, "Different data should produce different CRC");

    println!(
        "✓ CRC32 basic test passed: Same: {:08X}, Different: {:08X}",
        crc1, crc3
    );
}

/// Test CRC sensitivity to small changes - CRITICAL for desync detection
#[test]
fn test_crc_sensitivity() {
    let position_1 = [100.0f32, 200.0f32, 0.0f32];
    let position_2 = [100.01f32, 200.0f32, 0.0f32]; // Tiny difference

    let mut hasher1 = Hasher::new();
    hasher1.update(&position_1[0].to_le_bytes());
    hasher1.update(&position_1[1].to_le_bytes());
    hasher1.update(&position_1[2].to_le_bytes());
    let crc1 = hasher1.finalize();

    let mut hasher2 = Hasher::new();
    hasher2.update(&position_2[0].to_le_bytes());
    hasher2.update(&position_2[1].to_le_bytes());
    hasher2.update(&position_2[2].to_le_bytes());
    let crc2 = hasher2.finalize();

    assert_ne!(
        crc1, crc2,
        "Tiny position differences should produce different CRCs"
    );

    println!(
        "✓ CRC sensitivity test passed: {:08X} != {:08X}",
        crc1, crc2
    );
}

/// Test CRC for game frame numbers - ensures frame progression is detected
#[test]
fn test_frame_number_crc() {
    let mut hasher1 = Hasher::new();
    hasher1.update(b"FRAME:");
    hasher1.update(&0u32.to_le_bytes());
    let frame_0_crc = hasher1.finalize();

    let mut hasher2 = Hasher::new();
    hasher2.update(b"FRAME:");
    hasher2.update(&1u32.to_le_bytes());
    let frame_1_crc = hasher2.finalize();

    assert_ne!(
        frame_0_crc, frame_1_crc,
        "Different frames should have different CRCs"
    );

    println!(
        "✓ Frame CRC test passed: Frame 0: {:08X}, Frame 1: {:08X}",
        frame_0_crc, frame_1_crc
    );
}

/// Test random seed CRC - critical for RNG synchronization
#[test]
fn test_random_seed_crc() {
    let seed1 = [
        0x12345678u32,
        0x9ABCDEF0,
        0x13579BDF,
        0x2468ACE0,
        0xFEDCBA98,
        0x76543210,
    ];
    let seed2 = [
        0x12345679u32,
        0x9ABCDEF0,
        0x13579BDF,
        0x2468ACE0,
        0xFEDCBA98,
        0x76543210,
    ]; // One bit different

    // Convert seed arrays to bytes for CRC
    let seed1_bytes: &[u8] =
        unsafe { std::slice::from_raw_parts(seed1.as_ptr() as *const u8, 6 * 4) };
    let seed2_bytes: &[u8] =
        unsafe { std::slice::from_raw_parts(seed2.as_ptr() as *const u8, 6 * 4) };

    let crc1 = crc32fast::hash(seed1_bytes);
    let crc2 = crc32fast::hash(seed2_bytes);

    assert_ne!(
        crc1, crc2,
        "Different random seeds should have different CRCs"
    );

    // Test same seed produces same CRC
    let crc1_repeat = crc32fast::hash(seed1_bytes);
    assert_eq!(
        crc1, crc1_repeat,
        "Same random seed should produce same CRC"
    );

    println!(
        "✓ Random seed CRC test passed: Seed1: {:08X}, Seed2: {:08X}",
        crc1, crc2
    );
}

/// Test comprehensive game state CRC - simulates full game state
#[test]
fn test_comprehensive_game_state_crc() {
    let frame = 150u32;
    let player1_position = [1024.5f32, 768.2f32, 12.0f32];
    let player1_health = 100u32;
    let player1_id = 1u32;
    let random_seed = [
        0x12345678u32,
        0x9ABCDEF0,
        0x13579BDF,
        0x2468ACE0,
        0xFEDCBA98,
        0x76543210,
    ];

    // Build comprehensive CRC that matches C++ structure
    let mut hasher = Hasher::new();

    // Game objects
    hasher.update(b"MARKER:Objects");
    hasher.update(&player1_position[0].to_le_bytes());
    hasher.update(&player1_position[1].to_le_bytes());
    hasher.update(&player1_position[2].to_le_bytes());
    hasher.update(&player1_health.to_le_bytes());
    hasher.update(&player1_id.to_le_bytes());

    // Random seed
    hasher.update(b"MARKER:RandomSeed");
    let seed_bytes: &[u8] =
        unsafe { std::slice::from_raw_parts(random_seed.as_ptr() as *const u8, 6 * 4) };
    let seed_crc = crc32fast::hash(seed_bytes);
    hasher.update(&seed_crc.to_le_bytes());

    // Frame number
    hasher.update(b"MARKER:GameFrame");
    hasher.update(&frame.to_le_bytes());

    // End marker
    hasher.update(b"MARKER:End");

    let comprehensive_crc = hasher.finalize();

    // Test that changing any component changes the CRC
    let mut hasher2 = Hasher::new();
    hasher2.update(b"MARKER:Objects");
    hasher2.update(&player1_position[0].to_le_bytes());
    hasher2.update(&player1_position[1].to_le_bytes());
    hasher2.update(&player1_position[2].to_le_bytes());
    hasher2.update(&(player1_health - 1).to_le_bytes()); // Different health
    hasher2.update(&player1_id.to_le_bytes());
    hasher2.update(b"MARKER:RandomSeed");
    hasher2.update(&seed_crc.to_le_bytes());
    hasher2.update(b"MARKER:GameFrame");
    hasher2.update(&frame.to_le_bytes());
    hasher2.update(b"MARKER:End");
    let different_crc = hasher2.finalize();

    assert_ne!(
        comprehensive_crc, different_crc,
        "Different game states should have different CRCs"
    );

    println!(
        "✓ Comprehensive game state CRC test passed: Original: {:08X}, Modified: {:08X}",
        comprehensive_crc, different_crc
    );
}

/// Test CRC determinism across multiple calculations
#[test]
fn test_crc_determinism() {
    let test_data = [
        b"MARKER:Objects".as_slice(),
        &100.5f32.to_le_bytes(),
        &200.3f32.to_le_bytes(),
        &0.0f32.to_le_bytes(),
        b"MARKER:Frame",
        &42u32.to_le_bytes(),
        b"MARKER:End",
    ];

    // Calculate CRC multiple times
    let mut crcs = Vec::new();
    for _ in 0..10 {
        let mut hasher = Hasher::new();
        for data in &test_data {
            hasher.update(data);
        }
        crcs.push(hasher.finalize());
    }

    // All CRCs should be identical
    let first_crc = crcs[0];
    for (i, &crc) in crcs.iter().enumerate() {
        assert_eq!(crc, first_crc, "CRC calculation {} differs from first", i);
    }

    println!(
        "✓ CRC determinism test passed: All 10 calculations produced {:08X}",
        first_crc
    );
}
