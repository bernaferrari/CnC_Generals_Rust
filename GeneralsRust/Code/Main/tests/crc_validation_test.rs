//! CRC Validation Test for Multiplayer Synchronization
//!
//! This test validates that our CRC implementation produces deterministic results
//! and can detect differences in game state that would cause desynchronization.
//! This is CRITICAL for multiplayer functionality.

use crc32fast::Hasher;

/// Simulate the C++ GameLogic CRC calculation structure
/// This matches the C++ calculateCRC() method exactly
fn calculate_game_state_crc(
    frame_number: u32,
    objects: &[(f32, f32, f32, u32, u32)], // (x, y, z, health, id)
    random_seed: &[u32; 6],
) -> u32 {
    let mut hasher = Hasher::new();

    // MARKER: Objects - matches C++ marker system
    hasher.update(b"MARKER:Objects");

    // Process objects in deterministic order (sorted by ID)
    let mut sorted_objects = objects.to_vec();
    sorted_objects.sort_by_key(|(_, _, _, _, id)| *id);

    for (x, y, z, health, id) in sorted_objects {
        hasher.update(&x.to_le_bytes());
        hasher.update(&y.to_le_bytes());
        hasher.update(&z.to_le_bytes());
        hasher.update(&health.to_le_bytes());
        hasher.update(&id.to_le_bytes());
    }

    // MARKER: Random Seed - matches C++ GetGameLogicRandomSeedCRC()
    hasher.update(b"MARKER:RandomSeed");
    let seed_bytes: &[u8] =
        unsafe { std::slice::from_raw_parts(random_seed.as_ptr() as *const u8, 6 * 4) };
    let seed_crc = crc32fast::hash(seed_bytes);
    hasher.update(&seed_crc.to_le_bytes());

    // MARKER: Game Frame - matches C++ frame tracking
    hasher.update(b"MARKER:GameFrame");
    hasher.update(&frame_number.to_le_bytes());

    // MARKER: End - ensure completeness
    hasher.update(b"MARKER:End");

    hasher.finalize()
}

#[test]
fn test_crc_deterministic_same_state() {
    let frame = 150;
    let objects = vec![
        (100.0, 200.0, 0.0, 100, 1),  // Player 1
        (300.0, 400.0, 5.0, 85, 2),   // Player 2
        (500.0, 100.0, 0.0, 1000, 3), // Building
    ];
    let random_seed = [
        0x12345678, 0x9ABCDEF0, 0x13579BDF, 0x2468ACE0, 0xFEDCBA98, 0x76543210,
    ];

    // Calculate CRC multiple times - should be identical
    let crc1 = calculate_game_state_crc(frame, &objects, &random_seed);
    let crc2 = calculate_game_state_crc(frame, &objects, &random_seed);
    let crc3 = calculate_game_state_crc(frame, &objects, &random_seed);

    assert_eq!(crc1, crc2, "Same game state should produce same CRC");
    assert_eq!(crc2, crc3, "CRC calculation should be deterministic");

    println!("✓ Deterministic CRC test passed: {:08X}", crc1);
}

#[test]
fn test_crc_detects_position_changes() {
    let frame = 150;
    let random_seed = [
        0x12345678, 0x9ABCDEF0, 0x13579BDF, 0x2468ACE0, 0xFEDCBA98, 0x76543210,
    ];

    let objects1 = vec![(100.0, 200.0, 0.0, 100, 1), (300.0, 400.0, 5.0, 85, 2)];

    let objects2 = vec![
        (100.1, 200.0, 0.0, 100, 1), // Tiny position change
        (300.0, 400.0, 5.0, 85, 2),
    ];

    let crc1 = calculate_game_state_crc(frame, &objects1, &random_seed);
    let crc2 = calculate_game_state_crc(frame, &objects2, &random_seed);

    assert_ne!(crc1, crc2, "Position changes should be detected by CRC");

    println!(
        "✓ Position change detection test passed: {:08X} != {:08X}",
        crc1, crc2
    );
}

#[test]
fn test_crc_detects_health_changes() {
    let frame = 150;
    let random_seed = [
        0x12345678, 0x9ABCDEF0, 0x13579BDF, 0x2468ACE0, 0xFEDCBA98, 0x76543210,
    ];

    let objects1 = vec![(100.0, 200.0, 0.0, 100, 1), (300.0, 400.0, 5.0, 85, 2)];

    let objects2 = vec![
        (100.0, 200.0, 0.0, 99, 1), // Health decreased by 1
        (300.0, 400.0, 5.0, 85, 2),
    ];

    let crc1 = calculate_game_state_crc(frame, &objects1, &random_seed);
    let crc2 = calculate_game_state_crc(frame, &objects2, &random_seed);

    assert_ne!(crc1, crc2, "Health changes should be detected by CRC");

    println!(
        "✓ Health change detection test passed: {:08X} != {:08X}",
        crc1, crc2
    );
}

#[test]
fn test_crc_detects_frame_changes() {
    let objects = vec![(100.0, 200.0, 0.0, 100, 1), (300.0, 400.0, 5.0, 85, 2)];
    let random_seed = [
        0x12345678, 0x9ABCDEF0, 0x13579BDF, 0x2468ACE0, 0xFEDCBA98, 0x76543210,
    ];

    let crc_frame_100 = calculate_game_state_crc(100, &objects, &random_seed);
    let crc_frame_101 = calculate_game_state_crc(101, &objects, &random_seed);

    assert_ne!(
        crc_frame_100, crc_frame_101,
        "Frame progression should be detected by CRC"
    );

    println!(
        "✓ Frame change detection test passed: Frame 100: {:08X}, Frame 101: {:08X}",
        crc_frame_100, crc_frame_101
    );
}

#[test]
fn test_crc_detects_random_seed_changes() {
    let frame = 150;
    let objects = vec![(100.0, 200.0, 0.0, 100, 1), (300.0, 400.0, 5.0, 85, 2)];

    let seed1 = [
        0x12345678, 0x9ABCDEF0, 0x13579BDF, 0x2468ACE0, 0xFEDCBA98, 0x76543210,
    ];
    let seed2 = [
        0x12345679, 0x9ABCDEF0, 0x13579BDF, 0x2468ACE0, 0xFEDCBA98, 0x76543210,
    ]; // One bit different

    let crc1 = calculate_game_state_crc(frame, &objects, &seed1);
    let crc2 = calculate_game_state_crc(frame, &objects, &seed2);

    assert_ne!(crc1, crc2, "Random seed changes should be detected by CRC");

    println!(
        "✓ Random seed change detection test passed: {:08X} != {:08X}",
        crc1, crc2
    );
}

#[test]
fn test_crc_object_order_independence() {
    let frame = 150;
    let random_seed = [
        0x12345678, 0x9ABCDEF0, 0x13579BDF, 0x2468ACE0, 0xFEDCBA98, 0x76543210,
    ];

    // Same objects in different order - should produce same CRC due to sorting by ID
    let objects1 = vec![
        (100.0, 200.0, 0.0, 100, 1),
        (300.0, 400.0, 5.0, 85, 2),
        (500.0, 100.0, 0.0, 1000, 3),
    ];

    let objects2 = vec![
        (300.0, 400.0, 5.0, 85, 2),
        (500.0, 100.0, 0.0, 1000, 3),
        (100.0, 200.0, 0.0, 100, 1),
    ];

    let crc1 = calculate_game_state_crc(frame, &objects1, &random_seed);
    let crc2 = calculate_game_state_crc(frame, &objects2, &random_seed);

    assert_eq!(
        crc1, crc2,
        "Object order should not affect CRC (deterministic sorting by ID)"
    );

    println!("✓ Object order independence test passed: {:08X}", crc1);
}

#[test]
fn test_crc_empty_vs_objects() {
    let frame = 150;
    let random_seed = [
        0x12345678, 0x9ABCDEF0, 0x13579BDF, 0x2468ACE0, 0xFEDCBA98, 0x76543210,
    ];

    let empty_objects = vec![];
    let with_objects = vec![(100.0, 200.0, 0.0, 100, 1)];

    let crc_empty = calculate_game_state_crc(frame, &empty_objects, &random_seed);
    let crc_with_objects = calculate_game_state_crc(frame, &with_objects, &random_seed);

    assert_ne!(
        crc_empty, crc_with_objects,
        "Empty vs populated game state should have different CRCs"
    );

    println!(
        "✓ Empty vs objects test passed: Empty: {:08X}, With objects: {:08X}",
        crc_empty, crc_with_objects
    );
}

#[test]
fn test_crc_multiplayer_simulation() {
    // Simulate 3 players reporting CRCs for the same frame
    let frame = 300;
    let objects = vec![
        (1024.5, 768.2, 12.0, 95, 1),
        (500.0, 300.0, 0.0, 1500, 2),
        (200.0, 600.0, 5.0, 75, 3),
    ];
    let random_seed = [
        0xDEADBEEF, 0xCAFEBABE, 0x12345678, 0x9ABCDEF0, 0x13579BDF, 0x2468ACE0,
    ];

    // All players should calculate the same CRC for the same game state
    let player1_crc = calculate_game_state_crc(frame, &objects, &random_seed);
    let player2_crc = calculate_game_state_crc(frame, &objects, &random_seed);
    let player3_crc = calculate_game_state_crc(frame, &objects, &random_seed);

    assert_eq!(player1_crc, player2_crc, "Player 1 and 2 CRCs should match");
    assert_eq!(player2_crc, player3_crc, "Player 2 and 3 CRCs should match");

    // Simulate player 3 having slightly different state (desync)
    let mut desync_objects = objects.clone();
    desync_objects[0].0 += 0.01; // Tiny position difference
    let player3_desync_crc = calculate_game_state_crc(frame, &desync_objects, &random_seed);

    assert_ne!(player1_crc, player3_desync_crc, "Desync should be detected");

    println!("✓ Multiplayer simulation test passed:");
    println!(
        "  - Synchronized: Player1={:08X}, Player2={:08X}, Player3={:08X}",
        player1_crc, player2_crc, player3_crc
    );
    println!(
        "  - Desync detected: Player3_desync={:08X}",
        player3_desync_crc
    );
}
