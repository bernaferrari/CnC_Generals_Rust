//! Demonstration of the DesyncManager functionality
//!
//! This example shows how to use the DesyncManager to detect and recover from
//! game state desynchronization in a multiplayer game.

use game_network::{DesyncInfo, DesyncManager};

fn main() {
    println!("=== Desync Manager Demo ===\n");

    // Create a new desync manager with a threshold of 5 desyncs
    let mut manager = DesyncManager::new(5);
    println!("Created DesyncManager with max_desyncs=5\n");

    // Simulate normal gameplay with matching CRCs
    println!("--- Scenario 1: Normal gameplay (matching CRCs) ---");
    for frame in 100..105 {
        let crc = 0x12345678 + frame;
        match manager.check_frame_crc(frame, crc, crc, 0) {
            Ok(_) => println!("Frame {}: CRC check passed (0x{:08X})", frame, crc),
            Err(e) => println!("Frame {}: CRC check FAILED - {}", frame, e),
        }
    }
    println!("Desync count: {}\n", manager.desync_count());

    // Simulate a desync event
    println!("--- Scenario 2: Single desync detected ---");
    let frame = 105;
    let expected = 0x11111111;
    let received = 0x22222222;
    match manager.check_frame_crc(frame, expected, received, 1) {
        Ok(_) => println!("Frame {}: Desync detected but within tolerance", frame),
        Err(e) => println!("Frame {}: Critical desync - {}", frame, e),
    }
    println!("Desync count: {}", manager.desync_count());
    println!("Is desynchronized: {}\n", manager.is_desynchronized());

    // Show desync details
    println!("--- Desync Details ---");
    for (i, desync) in manager.get_desyncs().iter().enumerate() {
        println!(
            "Desync #{}: frame={}, player={}, expected=0x{:08X}, received=0x{:08X}",
            i + 1,
            desync.frame_number,
            desync.player_id,
            desync.expected_crc,
            desync.received_crc
        );
    }
    println!();

    // Simulate multiple desyncs from different players
    println!("--- Scenario 3: Multiple desyncs ---");
    for i in 0..4 {
        let frame = 106 + i;
        let player = (i % 3) as u8;
        let expected = 0xAAAA0000 + (i * 0x1111);
        let received = 0xBBBB0000 + (i * 0x1111);
        match manager.check_frame_crc(frame, expected, received, player) {
            Ok(_) => println!(
                "Frame {}: Desync from player {} (within tolerance)",
                frame, player
            ),
            Err(e) => println!("Frame {}: CRITICAL - {}", frame, e),
        }
    }
    println!("Total desync count: {}", manager.desync_count());
    println!("Is desynchronized: {}\n", manager.is_desynchronized());

    // Show metrics
    println!("--- Metrics ---");
    let metrics = manager.metrics();
    println!("Total desyncs: {}", metrics.total_desyncs);
    println!("Desyncs per player:");
    for player_id in 0..8 {
        let count = metrics.player_desyncs(player_id);
        if count > 0 {
            println!("  Player {}: {}", player_id, count);
        }
    }
    if let Some(frame) = metrics.last_desync_frame {
        println!("Last desync frame: {}", frame);
    }
    println!();

    // Demonstrate recovery mode
    println!("--- Scenario 4: Recovery Mode ---");
    let last_good_frame = 100;
    println!("Entering recovery mode from frame {}", last_good_frame);
    manager.enter_recovery_mode(last_good_frame);
    println!("In recovery mode: {}", manager.is_in_recovery_mode());
    println!("Last known good frame: {}", manager.last_known_good_frame());

    // Create a resync request
    let resync_cmd = manager.request_resync(last_good_frame);
    println!("Created resync request: {:?}", resync_cmd.command_type);
    println!();

    // Simulate successful recovery
    println!("--- Scenario 5: Successful Recovery ---");
    manager.exit_recovery_mode();
    println!("In recovery mode: {}", manager.is_in_recovery_mode());
    println!("Is desynchronized: {}", manager.is_desynchronized());
    println!("Desync count after recovery: {}", manager.desync_count());
    println!();

    // Show final metrics
    println!("--- Final Metrics ---");
    let final_metrics = manager.metrics();
    println!("Total desyncs detected: {}", final_metrics.total_desyncs);
    println!("Recovery attempts: {}", final_metrics.recovery_attempts);
    println!(
        "Successful recoveries: {}",
        final_metrics.successful_recoveries
    );
    println!(
        "Recovery success rate: {:.1}%",
        final_metrics.recovery_success_rate()
    );
    println!(
        "Total time in desync state: {}ms",
        final_metrics.total_desync_time_ms
    );
    println!();

    // Demonstrate threshold breach
    println!("--- Scenario 6: Exceeding Threshold ---");
    let mut strict_manager = DesyncManager::new(2);
    println!("Created strict manager with max_desyncs=2");

    for i in 0..4 {
        let frame = 200 + i;
        match strict_manager.check_frame_crc(frame, 0x1111, 0x2222, 0) {
            Ok(_) => println!("Frame {}: Desync {} (within tolerance)", frame, i + 1),
            Err(e) => println!("Frame {}: CRITICAL ERROR - {}", frame, e),
        }
    }
    println!();

    println!("=== Demo Complete ===");
}
