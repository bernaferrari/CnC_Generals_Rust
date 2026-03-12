//! Simple CRC Test - Validates CRC functionality without dependencies
//!
//! This test verifies CRC32 deterministic behavior for multiplayer synchronization
//! without depending on complex game engine modules.

/// Test basic CRC32 functionality  
#[test]
fn test_crc32_basic() {
    // Test data that represents game state
    let data1 =
        b"Frame:150|Player1:100.5,200.3,0.0,100,1|Player2:300.0,400.0,5.0,85,2|Seed:0x12345678";
    let data2 =
        b"Frame:150|Player1:100.5,200.3,0.0,100,1|Player2:300.0,400.0,5.0,85,2|Seed:0x12345678";

    let crc1 = crc32fast::hash(data1);
    let crc2 = crc32fast::hash(data2);

    assert_eq!(crc1, crc2, "Identical data should produce identical CRC");

    // Test different data
    let data3 =
        b"Frame:151|Player1:100.5,200.3,0.0,100,1|Player2:300.0,400.0,5.0,85,2|Seed:0x12345678";
    let crc3 = crc32fast::hash(data3);

    assert_ne!(crc1, crc3, "Different data should produce different CRC");

    println!(
        "✓ Basic CRC test passed: Same={:08X}, Different={:08X}",
        crc1, crc3
    );
}

/// Test CRC sensitivity to tiny changes - Critical for desync detection
#[test]
fn test_crc_sensitivity() {
    let position1 = 100.0f32;
    let position2 = 100.01f32; // Tiny difference

    let mut data1 = Vec::new();
    data1.extend_from_slice(&position1.to_le_bytes());

    let mut data2 = Vec::new();
    data2.extend_from_slice(&position2.to_le_bytes());

    let crc1 = crc32fast::hash(&data1);
    let crc2 = crc32fast::hash(&data2);

    assert_ne!(crc1, crc2, "Tiny differences should be detected");

    println!(
        "✓ CRC sensitivity test passed: {:08X} != {:08X}",
        crc1, crc2
    );
}

/// Test game state CRC calculation - Simulates actual game logic
#[test]
fn test_game_state_crc() {
    use crc32fast::Hasher;

    let frame = 300u32;
    let player1_pos = [1024.5f32, 768.2f32, 12.0f32];
    let player1_health = 95u32;
    let player1_id = 1u32;

    // Build game state CRC
    let mut hasher = Hasher::new();

    // Add markers like C++ implementation
    hasher.update(b"MARKER:Objects");
    hasher.update(&player1_pos[0].to_le_bytes());
    hasher.update(&player1_pos[1].to_le_bytes());
    hasher.update(&player1_pos[2].to_le_bytes());
    hasher.update(&player1_health.to_le_bytes());
    hasher.update(&player1_id.to_le_bytes());

    hasher.update(b"MARKER:Frame");
    hasher.update(&frame.to_le_bytes());

    hasher.update(b"MARKER:End");

    let game_crc = hasher.finalize();

    // Test that same state produces same CRC
    let mut hasher2 = Hasher::new();
    hasher2.update(b"MARKER:Objects");
    hasher2.update(&player1_pos[0].to_le_bytes());
    hasher2.update(&player1_pos[1].to_le_bytes());
    hasher2.update(&player1_pos[2].to_le_bytes());
    hasher2.update(&player1_health.to_le_bytes());
    hasher2.update(&player1_id.to_le_bytes());
    hasher2.update(b"MARKER:Frame");
    hasher2.update(&frame.to_le_bytes());
    hasher2.update(b"MARKER:End");
    let game_crc2 = hasher2.finalize();

    assert_eq!(
        game_crc, game_crc2,
        "Same game state should produce same CRC"
    );

    // Test that different health produces different CRC
    let mut hasher3 = Hasher::new();
    hasher3.update(b"MARKER:Objects");
    hasher3.update(&player1_pos[0].to_le_bytes());
    hasher3.update(&player1_pos[1].to_le_bytes());
    hasher3.update(&player1_pos[2].to_le_bytes());
    hasher3.update(&(player1_health - 1).to_le_bytes()); // Different health
    hasher3.update(&player1_id.to_le_bytes());
    hasher3.update(b"MARKER:Frame");
    hasher3.update(&frame.to_le_bytes());
    hasher3.update(b"MARKER:End");
    let game_crc3 = hasher3.finalize();

    assert_ne!(
        game_crc, game_crc3,
        "Different game state should produce different CRC"
    );

    println!(
        "✓ Game state CRC test passed: Same={:08X}, Different={:08X}",
        game_crc, game_crc3
    );
}

/// Test multiplayer synchronization scenario
#[test]
fn test_multiplayer_sync_scenario() {
    use crc32fast::Hasher;

    // Simulate game state that all players should have
    let frame = 500u32;
    let objects = [
        // (x, y, z, health, id)
        (1024.5f32, 768.2f32, 12.0f32, 95u32, 1u32),
        (500.0f32, 300.0f32, 0.0f32, 1500u32, 2u32),
        (200.0f32, 600.0f32, 5.0f32, 75u32, 3u32),
    ];

    // Function to calculate CRC for this game state
    let calculate_multiplayer_crc = |objects: &[(f32, f32, f32, u32, u32)], frame: u32| {
        let mut hasher = Hasher::new();
        hasher.update(b"MP_SYNC:");

        // Process objects in sorted order for consistency
        let mut sorted_objects = objects.to_vec();
        sorted_objects.sort_by_key(|(_, _, _, _, id)| *id);

        for (x, y, z, health, id) in sorted_objects {
            hasher.update(&x.to_le_bytes());
            hasher.update(&y.to_le_bytes());
            hasher.update(&z.to_le_bytes());
            hasher.update(&health.to_le_bytes());
            hasher.update(&id.to_le_bytes());
        }

        hasher.update(b"|FRAME:");
        hasher.update(&frame.to_le_bytes());

        hasher.finalize()
    };

    // All players calculate CRC for the same state
    let player1_crc = calculate_multiplayer_crc(&objects, frame);
    let player2_crc = calculate_multiplayer_crc(&objects, frame);
    let player3_crc = calculate_multiplayer_crc(&objects, frame);

    assert_eq!(
        player1_crc, player2_crc,
        "Player 1 and 2 should be synchronized"
    );
    assert_eq!(
        player2_crc, player3_crc,
        "Player 2 and 3 should be synchronized"
    );

    // Simulate desync - player 3 has slightly different state
    let mut desync_objects = objects.clone();
    desync_objects[0].0 += 0.1; // Tiny position difference
    let player3_desync_crc = calculate_multiplayer_crc(&desync_objects, frame);

    assert_ne!(player1_crc, player3_desync_crc, "Desync should be detected");

    println!("✓ Multiplayer sync test passed:");
    println!("  - Synchronized players: {:08X}", player1_crc);
    println!("  - Desync detected: {:08X}", player3_desync_crc);
}

/// Test determinism across multiple runs
#[test]
fn test_crc_determinism() {
    let test_data = b"DETERMINISM_TEST_DATA_FOR_MULTIPLAYER_SYNC";

    // Calculate CRC 100 times
    let mut crcs = Vec::new();
    for _ in 0..100 {
        crcs.push(crc32fast::hash(test_data));
    }

    // All should be identical
    let first = crcs[0];
    for (i, &crc) in crcs.iter().enumerate() {
        assert_eq!(crc, first, "CRC calculation {} differs from first", i);
    }

    println!(
        "✓ CRC determinism test passed: All 100 calculations = {:08X}",
        first
    );
}

/// Test random seed CRC calculation
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
    ]; // 1 bit different

    // Convert to bytes for CRC calculation
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

    // Same seed should produce same CRC
    let crc1_repeat = crc32fast::hash(seed1_bytes);
    assert_eq!(crc1, crc1_repeat, "Same seed should produce same CRC");

    println!(
        "✓ Random seed CRC test passed: Seed1={:08X}, Seed2={:08X}",
        crc1, crc2
    );
}
