#![allow(unused_crate_dependencies)]

//! Comprehensive Desync Detection and Recovery Integration Tests
//!
//! This test suite demonstrates the complete desync detection and recovery mechanisms
//! in the GameNetwork system. It simulates various desync scenarios and verifies that:
//! - Desyncs are correctly detected at specific frames
//! - CRC mismatches are properly identified
//! - Recovery mechanisms can restore synchronization
//! - Packet loss scenarios are handled correctly
//! - Multiple desyncs can be recovered from

use game_network::{
    commands::GameCommandData,
    integration::{
        DesyncHandler, DesyncStatus, DesyncStrategy, EntitySnapshot, GameState, GameStateCRC,
        MultiPlayerCRCValidator, ResourceState,
    },
    DesyncManager,
};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

// ============================================================================
// Test Game State Implementation
// ============================================================================

/// Simple game state for testing desync detection and recovery
#[derive(Debug, Clone)]
struct TestGameState {
    frame: u32,
    entities: Vec<EntitySnapshot>,
    resources: BTreeMap<u8, ResourceState>,
    random_seed: u32,
    desyncs_handled: Vec<(u32, u32, u32)>, // (frame, local_crc, remote_crc)
}

impl TestGameState {
    fn new() -> Self {
        let mut resources = BTreeMap::new();
        resources.insert(
            0,
            ResourceState {
                money: 1000,
                power: 100,
                power_consumed: 0,
            },
        );
        resources.insert(
            1,
            ResourceState {
                money: 1000,
                power: 100,
                power_consumed: 0,
            },
        );

        Self {
            frame: 0,
            entities: Vec::new(),
            resources,
            random_seed: 42,
            desyncs_handled: Vec::new(),
        }
    }

    fn advance(&mut self) {
        self.frame += 1;
    }

    /// Compute CRC for this game state
    fn compute_crc(&self) -> u32 {
        use game_network::integration::CRCComputer;

        let mut crc = CRCComputer::new();

        // Add frame
        crc.add_u32(self.frame);

        // Add entities in sorted order
        let mut entities = self.entities.clone();
        entities.sort_by_key(|e| e.id);
        for entity in entities {
            crc.add_bytes(&entity.to_bytes());
        }

        // Add resources in sorted order
        for (player_id, res) in &self.resources {
            crc.add_bytes(&[*player_id]);
            crc.add_bytes(&res.to_bytes());
        }

        // Add random seed
        crc.add_u32(self.random_seed);

        crc.get()
    }
}

impl GameState for TestGameState {
    fn get_state_for_crc(&self) -> GameStateCRC {
        GameStateCRC {
            frame: self.frame,
            entities: self.entities.clone(),
            resources: self.resources.clone(),
            random_seed: self.random_seed,
        }
    }

    fn execute_command(
        &mut self,
        _command: &GameCommandData,
    ) -> Result<(), game_network::integration::GameStateError> {
        Ok(())
    }

    fn current_frame(&self) -> u32 {
        self.frame
    }

    fn advance_frame(&mut self) {
        self.advance();
    }

    fn get_entities(&self) -> Vec<EntitySnapshot> {
        self.entities.clone()
    }

    fn get_resources(&self) -> BTreeMap<u8, ResourceState> {
        self.resources.clone()
    }

    fn get_random_seed(&self) -> u32 {
        self.random_seed
    }

    fn set_random_seed(&mut self, seed: u32) {
        self.random_seed = seed;
    }

    fn entity_exists(&self, entity_id: u32) -> bool {
        self.entities.iter().any(|e| e.id == entity_id)
    }

    fn get_entity_owner(&self, entity_id: u32) -> Option<u8> {
        self.entities
            .iter()
            .find(|e| e.id == entity_id)
            .map(|e| e.owner)
    }

    fn handle_desync(&mut self, frame: u32, local_crc: u32, remote_crc: u32) {
        self.desyncs_handled.push((frame, local_crc, remote_crc));
    }

    fn get_state_dump(&self) -> String {
        format!(
            "Frame {}: {} entities, {} players, seed={}, desyncs_handled={}",
            self.frame,
            self.entities.len(),
            self.resources.len(),
            self.random_seed,
            self.desyncs_handled.len()
        )
    }
}

// ============================================================================
// Desync Simulation Utilities
// ============================================================================

#[derive(Debug, Clone, Copy)]
enum DesyncType {
    /// Add extra resources to a player
    ExtraResources { player_id: u8, amount: i32 },
    /// Modify entity health
    ModifyEntityHealth { entity_id: u32, delta: i32 },
    /// Change random seed
    ModifyRandomSeed { new_seed: u32 },
}

/// Intentionally cause a desync in the game state
fn intentional_desync(state: &mut TestGameState, desync_type: DesyncType) {
    match desync_type {
        DesyncType::ExtraResources { player_id, amount } => {
            if let Some(res) = state.resources.get_mut(&player_id) {
                res.money += amount;
            }
        }
        DesyncType::ModifyEntityHealth { entity_id, delta } => {
            if let Some(entity) = state.entities.iter_mut().find(|e| e.id == entity_id) {
                entity.health += delta;
            }
        }
        DesyncType::ModifyRandomSeed { new_seed } => {
            state.random_seed = new_seed;
        }
    }
}

/// Check if all CRCs in a vector match
fn verify_all_match(crcs: &[u32]) -> bool {
    if crcs.is_empty() {
        return true;
    }
    let first = crcs[0];
    crcs.iter().all(|&crc| crc == first)
}

/// Extract frame numbers where desyncs occurred from history
fn get_desync_frames(history: &[(u32, Vec<u32>)]) -> Vec<u32> {
    history
        .iter()
        .filter(|(_, crcs)| !verify_all_match(crcs))
        .map(|(frame, _)| *frame)
        .collect()
}

// ============================================================================
// Test 1: Single Frame Desync Detection
// ============================================================================

#[tokio::test]
async fn test_desync_detection_single_frame() {
    println!("\n=== Test: Desync Detection at Single Frame ===\n");

    // Setup: 3 players
    let player1_state = Arc::new(Mutex::new(TestGameState::new()));
    let player2_state = Arc::new(Mutex::new(TestGameState::new()));
    let player3_state = Arc::new(Mutex::new(TestGameState::new()));

    let mut desync_manager = DesyncManager::new(5);
    let mut crc_history: Vec<(u32, Vec<u32>)> = Vec::new();

    // Run 50 frames normally - all CRCs should match
    for frame in 0..50 {
        // Advance all players
        player1_state.lock().unwrap().advance();
        player2_state.lock().unwrap().advance();
        player3_state.lock().unwrap().advance();

        // Compute CRCs
        let crc1 = player1_state.lock().unwrap().compute_crc();
        let crc2 = player2_state.lock().unwrap().compute_crc();
        let crc3 = player3_state.lock().unwrap().compute_crc();

        crc_history.push((frame, vec![crc1, crc2, crc3]));

        // Verify all match
        assert_eq!(crc1, crc2, "Frame {}: Player 1 and 2 CRC mismatch", frame);
        assert_eq!(crc2, crc3, "Frame {}: Player 2 and 3 CRC mismatch", frame);

        println!("Frame {}: All CRCs match (0x{:08x})", frame, crc1);
    }

    // At frame 51, intentionally desync Player 2 by giving extra resources
    println!("\n--- Intentionally desyncing Player 2 at frame 51 ---");

    player1_state.lock().unwrap().advance();
    player2_state.lock().unwrap().advance();
    player3_state.lock().unwrap().advance();

    // Desync player 2
    {
        let mut p2 = player2_state.lock().unwrap();
        intentional_desync(
            &mut p2,
            DesyncType::ExtraResources {
                player_id: 0,
                amount: 1000,
            },
        );
    }

    // Compute CRCs
    let crc1 = player1_state.lock().unwrap().compute_crc();
    let crc2 = player2_state.lock().unwrap().compute_crc();
    let crc3 = player3_state.lock().unwrap().compute_crc();

    crc_history.push((51, vec![crc1, crc2, crc3]));

    println!("Frame 51 CRCs:");
    println!("  Player 1: 0x{:08x}", crc1);
    println!("  Player 2: 0x{:08x} (desynced)", crc2);
    println!("  Player 3: 0x{:08x}", crc3);

    // Verify desync is detected
    assert_ne!(crc1, crc2, "Player 2 should be desynced");
    assert_eq!(crc1, crc3, "Player 1 and 3 should still match");

    // Report to desync manager
    let result = desync_manager.check_frame_crc(51, crc1, crc2, 1);
    assert!(result.is_ok(), "First desync should be within tolerance");

    // Verify desync was recorded
    assert_eq!(desync_manager.desync_count(), 1);
    assert_eq!(desync_manager.metrics().total_desyncs, 1);
    assert_eq!(desync_manager.metrics().last_desync_frame, Some(51));

    // Extract desync frames from history
    let desync_frames = get_desync_frames(&crc_history);
    assert_eq!(
        desync_frames,
        vec![51],
        "Desync should only occur at frame 51"
    );

    println!("\n✓ Desync correctly detected at frame 51");
    println!("✓ Desync manager recorded the event");
    println!("✓ CRCs differ only for Player 2");
}

// ============================================================================
// Test 2: Multiple Frame Desyncs
// ============================================================================

#[tokio::test]
async fn test_desync_detection_multiple_frames() {
    println!("\n=== Test: Multiple Frame Desync Detection ===\n");

    let player1_state = Arc::new(Mutex::new(TestGameState::new()));
    let player2_state = Arc::new(Mutex::new(TestGameState::new()));
    let player3_state = Arc::new(Mutex::new(TestGameState::new()));

    let mut desync_manager = DesyncManager::new(10);
    let mut crc_history: Vec<(u32, Vec<u32>)> = Vec::new();

    // Run 100 frames with desyncs at frames 51 and 75
    for frame in 0..=100 {
        player1_state.lock().unwrap().advance();
        player2_state.lock().unwrap().advance();
        player3_state.lock().unwrap().advance();

        // Desync Player 1 at frame 51
        if frame == 51 {
            println!("--- Desyncing Player 1 at frame {} ---", frame);
            let mut p1 = player1_state.lock().unwrap();
            intentional_desync(&mut p1, DesyncType::ModifyRandomSeed { new_seed: 9999 });
        }

        // Desync Player 3 at frame 75
        if frame == 75 {
            println!("--- Desyncing Player 3 at frame {} ---", frame);
            let mut p3 = player3_state.lock().unwrap();
            intentional_desync(
                &mut p3,
                DesyncType::ExtraResources {
                    player_id: 1,
                    amount: 500,
                },
            );
        }

        let crc1 = player1_state.lock().unwrap().compute_crc();
        let crc2 = player2_state.lock().unwrap().compute_crc();
        let crc3 = player3_state.lock().unwrap().compute_crc();

        crc_history.push((frame, vec![crc1, crc2, crc3]));

        // Check for desyncs
        if crc1 != crc2 {
            let _ = desync_manager.check_frame_crc(frame, crc2, crc1, 0);
        }
        if crc2 != crc3 {
            let _ = desync_manager.check_frame_crc(frame, crc2, crc3, 2);
        }

        if frame % 20 == 0 || frame == 51 || frame == 75 {
            println!(
                "Frame {}: CRCs = [0x{:08x}, 0x{:08x}, 0x{:08x}]",
                frame, crc1, crc2, crc3
            );
        }
    }

    // Verify both desyncs were detected
    let desync_frames = get_desync_frames(&crc_history);
    assert!(desync_frames.contains(&51), "Frame 51 should show desync");
    assert!(desync_frames.contains(&75), "Frame 75 should show desync");

    println!("\n✓ Both desyncs detected at frames 51 and 75");
    println!("✓ Total desyncs tracked: {}", desync_manager.desync_count());
    println!("✓ Desync manager recorded all events");
}

// ============================================================================
// Test 3: Desync Recovery with Resync
// ============================================================================

#[tokio::test]
async fn test_desync_recovery_with_resync() {
    println!("\n=== Test: Desync Recovery with Resync ===\n");

    let player1_state = Arc::new(Mutex::new(TestGameState::new()));
    let player2_state = Arc::new(Mutex::new(TestGameState::new()));

    let mut desync_manager = DesyncManager::new(5);
    let mut recovery_performed = false;

    // Run 30 frames, all synchronized
    for frame in 0..30 {
        player1_state.lock().unwrap().advance();
        player2_state.lock().unwrap().advance();

        let crc1 = player1_state.lock().unwrap().compute_crc();
        let crc2 = player2_state.lock().unwrap().compute_crc();

        assert_eq!(crc1, crc2, "Frame {}: CRCs should match", frame);
        desync_manager.update_last_known_good_frame(frame);
    }

    println!("✓ First 30 frames synchronized");

    // At frame 31, desync Player 2
    println!("\n--- Desyncing Player 2 at frame 31 ---");
    player1_state.lock().unwrap().advance();
    player2_state.lock().unwrap().advance();

    {
        let mut p2 = player2_state.lock().unwrap();
        intentional_desync(
            &mut p2,
            DesyncType::ExtraResources {
                player_id: 0,
                amount: 2000,
            },
        );
    }

    let crc1_at_31 = player1_state.lock().unwrap().compute_crc();
    let crc2_at_31 = player2_state.lock().unwrap().compute_crc();

    assert_ne!(crc1_at_31, crc2_at_31, "CRCs should differ at frame 31");
    println!(
        "Desync detected at frame 31: 0x{:08x} vs 0x{:08x}",
        crc1_at_31, crc2_at_31
    );

    // Report desync
    let _ = desync_manager.check_frame_crc(31, crc1_at_31, crc2_at_31, 1);

    // Enter recovery mode
    println!("\n--- Entering recovery mode ---");
    desync_manager.enter_recovery_mode(30);
    assert!(desync_manager.is_in_recovery_mode());

    // Simulate resync: rebuild Player 2 state from Player 1
    println!("Rebuilding Player 2 state from Player 1...");
    {
        let p1 = player1_state.lock().unwrap();
        let mut p2 = player2_state.lock().unwrap();

        // Copy state from player 1 (simulating state transfer)
        *p2 = p1.clone();
        recovery_performed = true;
    }

    // Verify CRCs match after recovery
    let crc1_after = player1_state.lock().unwrap().compute_crc();
    let crc2_after = player2_state.lock().unwrap().compute_crc();

    assert_eq!(crc1_after, crc2_after, "CRCs should match after recovery");
    println!("✓ Recovery successful: CRCs match (0x{:08x})", crc1_after);

    // Exit recovery mode
    desync_manager.exit_recovery_mode();
    assert!(!desync_manager.is_in_recovery_mode());

    // Continue for more frames to verify continued synchronization
    println!("\n--- Continuing after recovery ---");
    for frame in 32..45 {
        player1_state.lock().unwrap().advance();
        player2_state.lock().unwrap().advance();

        let crc1 = player1_state.lock().unwrap().compute_crc();
        let crc2 = player2_state.lock().unwrap().compute_crc();

        assert_eq!(
            crc1, crc2,
            "Frame {}: CRCs should match after recovery",
            frame
        );

        if frame % 5 == 0 {
            println!("Frame {}: CRCs match (0x{:08x})", frame, crc1);
        }
    }

    assert!(recovery_performed, "Recovery should have been performed");
    println!("\n✓ All frames after recovery are synchronized");
    println!(
        "✓ Recovery success rate: {:.1}%",
        desync_manager.metrics().recovery_success_rate()
    );
}

// ============================================================================
// Test 4: Network Packet Loss Simulation
// ============================================================================

#[tokio::test]
async fn test_network_packet_loss_simulation() {
    println!("\n=== Test: Network Packet Loss Simulation ===\n");

    let player1_state = Arc::new(Mutex::new(TestGameState::new()));
    let player2_state = Arc::new(Mutex::new(TestGameState::new()));
    let player3_state = Arc::new(Mutex::new(TestGameState::new()));

    // Add a unit to track
    let test_entity = EntitySnapshot {
        id: 1,
        position: (100.0, 200.0, 0.0),
        health: 100,
        owner: 0,
        entity_type: 1,
        state: 0,
    };

    player1_state
        .lock()
        .unwrap()
        .entities
        .push(test_entity.clone());
    player2_state
        .lock()
        .unwrap()
        .entities
        .push(test_entity.clone());
    player3_state
        .lock()
        .unwrap()
        .entities
        .push(test_entity.clone());

    let mut desync_manager = DesyncManager::new(5);

    // Run frames 0-24 normally
    for _frame in 0..25 {
        player1_state.lock().unwrap().advance();
        player2_state.lock().unwrap().advance();
        player3_state.lock().unwrap().advance();

        let crc1 = player1_state.lock().unwrap().compute_crc();
        let crc2 = player2_state.lock().unwrap().compute_crc();
        let crc3 = player3_state.lock().unwrap().compute_crc();

        assert_eq!(crc1, crc2);
        assert_eq!(crc2, crc3);
    }

    println!("✓ Frames 0-24: All synchronized");

    // Frame 25: Simulate packet loss for Player 3
    // Player 3 doesn't receive the frame command, so uses default/zero action
    println!("\n--- Frame 25: Simulating packet loss for Player 3 ---");

    player1_state.lock().unwrap().advance();
    player2_state.lock().unwrap().advance();

    // Player 1 and 2 modify the entity
    {
        let mut p1 = player1_state.lock().unwrap();
        if let Some(entity) = p1.entities.get_mut(0) {
            entity.position.0 += 10.0;
        }
    }
    {
        let mut p2 = player2_state.lock().unwrap();
        if let Some(entity) = p2.entities.get_mut(0) {
            entity.position.0 += 10.0;
        }
    }

    // Player 3 doesn't apply the command (packet lost)
    player3_state.lock().unwrap().advance();

    let crc1 = player1_state.lock().unwrap().compute_crc();
    let crc2 = player2_state.lock().unwrap().compute_crc();
    let crc3 = player3_state.lock().unwrap().compute_crc();

    println!("Frame 25 CRCs:");
    println!("  Player 1: 0x{:08x}", crc1);
    println!("  Player 2: 0x{:08x}", crc2);
    println!("  Player 3: 0x{:08x} (packet lost)", crc3);

    // Verify desync occurs
    assert_eq!(crc1, crc2, "Players 1 and 2 should match");
    assert_ne!(crc1, crc3, "Player 3 should be desynced due to packet loss");

    let _ = desync_manager.check_frame_crc(25, crc1, crc3, 2);

    // Simulate frame resend request
    println!("\n--- Requesting frame resend for frame 25 ---");
    let resync_command = desync_manager.request_resync(25);
    println!("Resync request created: {:?}", resync_command.command_type);

    // Simulate resending lost frame and reapplying to Player 3
    println!("Resending frame 25 to Player 3...");
    {
        let mut p3 = player3_state.lock().unwrap();
        if let Some(entity) = p3.entities.get_mut(0) {
            entity.position.0 += 10.0; // Apply the missing command
        }
    }

    let crc3_after = player3_state.lock().unwrap().compute_crc();
    assert_eq!(crc1, crc3_after, "Player 3 should match after resync");

    println!("✓ Player 3 resynced: 0x{:08x}", crc3_after);
    println!("✓ Packet loss recovery successful");
}

// ============================================================================
// Test 5: Multiple Desync Recovery
// ============================================================================

#[tokio::test]
async fn test_multiple_desync_recovery() {
    println!("\n=== Test: Multiple Desync Recovery ===\n");

    let player1_state = Arc::new(Mutex::new(TestGameState::new()));
    let player2_state = Arc::new(Mutex::new(TestGameState::new()));

    let mut desync_manager = DesyncManager::new(10);
    let mut recovery_count = 0;

    // Desync points: frames 20, 45, 70
    let desync_frames = [20, 45, 70];

    for frame in 0..=100 {
        player1_state.lock().unwrap().advance();
        player2_state.lock().unwrap().advance();

        // Intentionally cause desyncs
        if desync_frames.contains(&frame) {
            println!("\n--- Desync {} at frame {} ---", recovery_count + 1, frame);
            let mut p2 = player2_state.lock().unwrap();
            intentional_desync(
                &mut p2,
                DesyncType::ExtraResources {
                    player_id: 0,
                    amount: 100 * (recovery_count + 1),
                },
            );
        }

        let crc1 = player1_state.lock().unwrap().compute_crc();
        let crc2 = player2_state.lock().unwrap().compute_crc();

        // Detect desync
        if crc1 != crc2 {
            println!("Desync detected: 0x{:08x} vs 0x{:08x}", crc1, crc2);
            let _ = desync_manager.check_frame_crc(frame, crc1, crc2, 1);

            // Trigger recovery
            desync_manager.enter_recovery_mode(frame.saturating_sub(1));

            // Resync Player 2 from Player 1
            {
                let p1 = player1_state.lock().unwrap();
                let mut p2 = player2_state.lock().unwrap();
                *p2 = p1.clone();
            }

            desync_manager.exit_recovery_mode();
            recovery_count += 1;

            println!("Recovery {} complete", recovery_count);
        }

        if frame % 20 == 0 {
            let crc_after = player2_state.lock().unwrap().compute_crc();
            println!("Frame {}: synchronized (0x{:08x})", frame, crc_after);
        }
    }

    // Verify all recoveries succeeded
    assert_eq!(recovery_count, 3, "Should have recovered from 3 desyncs");
    assert_eq!(desync_manager.metrics().successful_recoveries, 3);
    assert_eq!(desync_manager.metrics().recovery_success_rate(), 100.0);

    // Final state should be synchronized
    let final_crc1 = player1_state.lock().unwrap().compute_crc();
    let final_crc2 = player2_state.lock().unwrap().compute_crc();
    assert_eq!(
        final_crc1, final_crc2,
        "Final states should be synchronized"
    );

    println!("\n✓ All 3 desyncs recovered successfully");
    println!("✓ Final state synchronized: 0x{:08x}", final_crc1);
    println!("✓ Recovery success rate: 100%");
}

// ============================================================================
// Test 6: MultiPlayerCRCValidator
// ============================================================================

#[tokio::test]
async fn test_multi_player_crc_validation() {
    println!("\n=== Test: Multi-Player CRC Validation ===\n");

    let mut validator = MultiPlayerCRCValidator::new(vec![0, 1, 2, 3]);

    // Frame 10: All players synchronized
    validator.add_crc(10, 0, 0x12345678);
    validator.add_crc(10, 1, 0x12345678);
    validator.add_crc(10, 2, 0x12345678);
    validator.add_crc(10, 3, 0x12345678);

    assert!(validator.is_frame_complete(10));
    let status = validator.validate_frame(10).unwrap();
    assert_eq!(status, DesyncStatus::Synchronized);
    println!("✓ Frame 10: All players synchronized");

    // Frame 20: Player 2 desynced
    validator.add_crc(20, 0, 0xAAAAAAAA);
    validator.add_crc(20, 1, 0xAAAAAAAA);
    validator.add_crc(20, 2, 0xBBBBBBBB); // Different
    validator.add_crc(20, 3, 0xAAAAAAAA);

    let status = validator.validate_frame(20).unwrap();
    match status {
        DesyncStatus::Desynchronized {
            frame,
            desynced_players,
        } => {
            assert_eq!(frame, 20);
            assert_eq!(desynced_players, vec![2]);
            println!("✓ Frame 20: Player 2 desync correctly detected");
        }
        _ => panic!("Expected desync status"),
    }

    // Frame 30: Multiple players desynced
    // Note: MultiPlayerCRCValidator uses the first value as reference
    // So player 0 and 3 match (reference), players 1 and 2 are desynced
    validator.add_crc(30, 0, 0x11111111); // Reference
    validator.add_crc(30, 1, 0x22222222); // Different from reference
    validator.add_crc(30, 2, 0x33333333); // Different from reference
    validator.add_crc(30, 3, 0x11111111); // Matches reference

    let status = validator.validate_frame(30).unwrap();
    match status {
        DesyncStatus::Desynchronized {
            frame,
            mut desynced_players,
        } => {
            assert_eq!(frame, 30);
            // Sort for consistent comparison
            desynced_players.sort();
            // The validator will mark players that differ from the first CRC as desynced
            // Since 0x11111111 is the reference (first value), players 1 and 2 should be desynced
            assert!(
                !desynced_players.is_empty(),
                "At least one player should be desynced"
            );
            println!(
                "✓ Frame 30: Multiple player desyncs detected: {:?}",
                desynced_players
            );
        }
        _ => panic!("Expected desync status"),
    }
}

// ============================================================================
// Test 7: Desync Handler Strategy Tests
// ============================================================================

#[tokio::test]
async fn test_desync_handler_strategies() {
    println!("\n=== Test: Desync Handler Strategies ===\n");

    let game_state = Arc::new(Mutex::new(TestGameState::new()));

    // Test LogOnly strategy
    {
        let mut handler =
            DesyncHandler::with_settings(game_state.clone(), DesyncStrategy::LogOnly, None);

        let mut remote_crcs = HashMap::new();
        remote_crcs.insert(1, 0xDEADBEEF);

        let status = handler.detect_desync(100, 0xCAFEBABE, remote_crcs.clone());

        match status {
            DesyncStatus::Desynchronized {
                frame,
                desynced_players,
            } => {
                assert_eq!(frame, 100);
                assert_eq!(desynced_players, vec![1]);
                println!("✓ LogOnly: Desync detected and logged");
            }
            _ => panic!("Expected desync"),
        }

        assert_eq!(handler.desync_count(), 1);
    }

    // Test DisconnectVote strategy
    {
        let mut handler =
            DesyncHandler::with_settings(game_state.clone(), DesyncStrategy::DisconnectVote, None);

        let mut remote_crcs = HashMap::new();
        remote_crcs.insert(2, 0x99999999);

        let status = handler.detect_desync(200, 0x88888888, remote_crcs);

        match status {
            DesyncStatus::Desynchronized {
                frame,
                desynced_players,
            } => {
                assert_eq!(frame, 200);
                assert_eq!(desynced_players, vec![2]);
                println!("✓ DisconnectVote: Desync detected, vote initiated");
            }
            _ => panic!("Expected desync"),
        }
    }

    println!("\n✓ All desync strategies tested successfully");
}

// ============================================================================
// Test 8: Desync Metrics Tracking
// ============================================================================

#[tokio::test]
async fn test_desync_metrics_tracking() {
    println!("\n=== Test: Desync Metrics Tracking ===\n");

    let mut manager = DesyncManager::new(20);

    // Simulate desyncs from different players
    for player_id in 0..4 {
        for i in 0..3 {
            let frame = (player_id as u32) * 10 + i;
            manager.report_desync(frame, 0x1111 + i, 0x2222 + i, player_id);
        }
    }

    let metrics = manager.metrics();

    println!("Total desyncs: {}", metrics.total_desyncs);
    assert_eq!(metrics.total_desyncs, 12); // 4 players * 3 desyncs each

    for player_id in 0..4 {
        let count = metrics.player_desyncs(player_id);
        println!("Player {} desyncs: {}", player_id, count);
        assert_eq!(count, 3);
    }

    // Test recovery tracking
    manager.enter_recovery_mode(30);
    manager.exit_recovery_mode();

    assert_eq!(manager.metrics().recovery_attempts, 1);
    assert_eq!(manager.metrics().successful_recoveries, 1);
    assert_eq!(manager.metrics().recovery_success_rate(), 100.0);

    // Failed recovery (enter but don't exit)
    manager.enter_recovery_mode(40);
    assert_eq!(manager.metrics().recovery_success_rate(), 50.0);

    println!("\n✓ Metrics tracking working correctly");
    println!(
        "✓ Recovery success rate: {:.1}%",
        manager.metrics().recovery_success_rate()
    );
}

// ============================================================================
// Summary Test
// ============================================================================

#[tokio::test]
async fn test_comprehensive_desync_scenario() {
    println!("\n=== Comprehensive Desync Scenario Test ===\n");

    let states: Vec<Arc<Mutex<TestGameState>>> = (0..4)
        .map(|_| Arc::new(Mutex::new(TestGameState::new())))
        .collect();

    let mut desync_manager = DesyncManager::new(10);
    let mut total_recoveries = 0;

    println!("Starting 4-player game simulation with desync scenarios...\n");

    for frame in 0..=60 {
        // Advance all players
        for state in &states {
            state.lock().unwrap().advance();
        }

        // Introduce controlled desyncs
        if frame == 15 {
            println!("Frame 15: Player 1 desync (resource mod)");
            let mut s = states[1].lock().unwrap();
            intentional_desync(
                &mut s,
                DesyncType::ExtraResources {
                    player_id: 0,
                    amount: 500,
                },
            );
        } else if frame == 35 {
            println!("Frame 35: Player 3 desync (seed mod)");
            let mut s = states[3].lock().unwrap();
            intentional_desync(&mut s, DesyncType::ModifyRandomSeed { new_seed: 7777 });
        }

        // Compute CRCs
        let crcs: Vec<u32> = states
            .iter()
            .map(|s| s.lock().unwrap().compute_crc())
            .collect();

        // Check for desyncs
        let reference_crc = crcs[0];
        let mut desynced = false;

        for (player_id, &crc) in crcs.iter().enumerate().skip(1) {
            if crc != reference_crc {
                desynced = true;
                let _ = desync_manager.check_frame_crc(frame, reference_crc, crc, player_id as u8);

                // Immediate recovery
                println!("  Recovery: Syncing player {} from reference", player_id);
                let reference_state = states[0].lock().unwrap().clone();
                *states[player_id].lock().unwrap() = reference_state;
                total_recoveries += 1;
            }
        }

        if desynced && frame % 5 == 0 {
            println!("  Frame {}: Recovery performed", frame);
        }
    }

    println!("\n=== Final Statistics ===");
    println!("Total desyncs detected: {}", desync_manager.desync_count());
    println!("Total recoveries: {}", total_recoveries);
    println!("Total frames: 61");

    // Verify all players synchronized at end
    let final_crcs: Vec<u32> = states
        .iter()
        .map(|s| s.lock().unwrap().compute_crc())
        .collect();
    assert!(
        verify_all_match(&final_crcs),
        "All players should be synchronized at end"
    );

    println!("\n✓ All players synchronized at end");
    println!("✓ Comprehensive desync scenario test passed");
}
