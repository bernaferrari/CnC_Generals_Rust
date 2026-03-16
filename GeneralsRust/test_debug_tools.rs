// Test script to verify debug tools implementation
// This file demonstrates the usage of all debug tools

use std::sync::{Arc, RwLock};

fn main() {
    println!("=== Debug Tools Implementation Test ===\n");

    // Test 1: DebugMessage
    println!("1. Testing DebugMessage...");
    let message = gamelogic::scripting::debug_tools::DebugMessageAction::new(
        gamelogic::common::AsciiString::from("Test debug message"),
        false,
    );
    match message.execute() {
        Ok(_) => println!("   ✓ DebugMessage executed successfully"),
        Err(e) => println!("   ✗ DebugMessage failed: {}", e),
    }

    // Test 2: DisplayCounter
    println!("\n2. Testing DisplayCounter...");
    let counter = gamelogic::scripting::debug_tools::DisplayCounterAction::new(
        gamelogic::common::AsciiString::from("test_counter"),
        gamelogic::common::AsciiString::from("Test Counter: {0}"),
    );
    match counter.execute() {
        Ok(_) => println!("   ✓ DisplayCounter executed successfully"),
        Err(e) => println!("   ✗ DisplayCounter failed: {}", e),
    }

    // Test 3: DisplayCountdownTimer
    println!("\n3. Testing DisplayCountdownTimer...");
    let timer = gamelogic::scripting::debug_tools::DisplayCountdownTimerAction::new(
        gamelogic::common::AsciiString::from("test_timer"),
        gamelogic::common::AsciiString::from("Countdown: {0}"),
        Some(60.0), // 60 seconds
    );
    match timer.execute() {
        Ok(_) => println!("   ✓ DisplayCountdownTimer executed successfully"),
        Err(e) => println!("   ✗ DisplayCountdownTimer failed: {}", e),
    }

    // Test 4: DoShowStats
    println!("\n4. Testing DoShowStats...");
    let stats = gamelogic::scripting::debug_tools::DoShowStatsAction::new(
        gamelogic::common::AsciiString::from("all"),
        false,
    );
    match stats.execute() {
        Ok(_) => println!("   ✓ DoShowStats executed successfully"),
        Err(e) => println!("   ✗ DoShowStats failed: {}", e),
    }

    // Test 5: StartStopwatch
    println!("\n5. Testing StartStopwatch...");
    let start = gamelogic::scripting::debug_tools::StartStopwatchAction::new(
        gamelogic::common::AsciiString::from("test_stopwatch"),
    );
    match start.execute() {
        Ok(_) => println!("   ✓ StartStopwatch executed successfully"),
        Err(e) => println!("   ✗ StartStopwatch failed: {}", e),
    }

    // Simulate some work
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Test 6: StopStopwatch
    println!("\n6. Testing StopStopwatch...");
    let stop = gamelogic::scripting::debug_tools::StopStopwatchAction::new(
        gamelogic::common::AsciiString::from("test_stopwatch"),
        Some(gamelogic::common::AsciiString::from("Test timing")),
    );
    match stop.execute() {
        Ok(_) => println!("   ✓ StopStopwatch executed successfully"),
        Err(e) => println!("   ✗ StopStopwatch failed: {}", e),
    }

    // Test 7: AssertCondition (should pass)
    println!("\n7. Testing AssertCondition (true)...");
    let assert_pass = gamelogic::scripting::debug_tools::AssertConditionAction::new(
        gamelogic::common::AsciiString::from("true"),
        gamelogic::common::AsciiString::from("This should pass"),
        false,
    );
    match assert_pass.execute() {
        Ok(_) => println!("   ✓ AssertCondition (true) passed as expected"),
        Err(e) => println!("   ✗ AssertCondition (true) failed unexpectedly: {}", e),
    }

    // Test 8: AssertCondition (should fail)
    println!("\n8. Testing AssertCondition (false)...");
    let assert_fail = gamelogic::scripting::debug_tools::AssertConditionAction::new(
        gamelogic::common::AsciiString::from("false"),
        gamelogic::common::AsciiString::from("This should fail"),
        false,
    );
    match assert_fail.execute() {
        Ok(_) => println!("   ✗ AssertCondition (false) passed unexpectedly"),
        Err(_) => println!("   ✓ AssertCondition (false) failed as expected"),
    }

    // Test 9: EnableVTune
    println!("\n9. Testing EnableVTune...");
    let vtune = gamelogic::scripting::debug_tools::EnableVTuneAction::new(true);
    match vtune.execute() {
        Ok(_) => println!("   ✓ EnableVTune executed successfully"),
        Err(e) => println!("   ✗ EnableVTune failed: {}", e),
    }

    // Test 10: DumpGameState
    println!("\n10. Testing DumpGameState...");
    let dump = gamelogic::scripting::debug_tools::DumpGameStateAction::new(
        gamelogic::common::AsciiString::from("all"),
        None,
    );
    match dump.execute() {
        Ok(_) => println!("   ✓ DumpGameState executed successfully"),
        Err(e) => println!("   ✗ DumpGameState failed: {}", e),
    }

    println!("\n=== All Debug Tools Tests Complete ===");
}
