use generals_main::save_load::{compression, SaveLoadError, SaveLoadResult};
use std::collections::HashMap;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎮 Command & Conquer Generals Zero Hour - Simple Save/Load Demo");
    println!("==============================================================");

    // Demo 1: Basic Compression
    demo_compression()?;

    // Demo 2: Error Handling
    demo_error_handling()?;

    println!("\n✅ Simple save/load demo completed successfully!");
    println!("The basic save/load system functionality is working correctly.");

    Ok(())
}

fn demo_compression() -> SaveLoadResult<()> {
    println!("\n🗜️ Demo 1: Compression System");
    println!("-----------------------------");

    // Test different data patterns
    let test_cases = vec![
        ("Small uniform data", vec![42u8; 100]),
        ("Large uniform data", vec![123u8; 10000]),
        (
            "Random pattern",
            (0..5000).map(|i| (i * 17 % 256) as u8).collect(),
        ),
        (
            "Alternating pattern",
            (0..8000)
                .map(|i| if i % 2 == 0 { 0xFF } else { 0x00 })
                .collect(),
        ),
    ];

    for (name, data) in test_cases {
        println!("\n📊 Testing: {}", name);

        // Compress the data
        let compressed = compression::compress(&data)?;

        // Decompress and verify
        let decompressed = compression::decompress(&compressed)?;

        // Verify integrity
        if data != decompressed {
            return Err(SaveLoadError::Corrupted(
                "Data integrity check failed".to_string(),
            ));
        }

        let ratio = (compressed.len() as f64 / data.len() as f64) * 100.0;

        println!("  📏 Original size: {} bytes", data.len());
        println!(
            "  🗜️ Compressed size: {} bytes ({:.1}%)",
            compressed.len(),
            ratio
        );
        println!("  ✅ Integrity verified");
    }

    Ok(())
}

fn demo_error_handling() -> SaveLoadResult<()> {
    println!("\n❌ Demo 2: Error Handling");
    println!("--------------------------");

    // Test with arbitrary non-compressed bytes (this should pass through unchanged).
    println!("🔍 Testing non-compressed input...");
    let raw_data = vec![0xFF, 0xFF, 0xFF, 0x00, 0x00];

    match compression::decompress(&raw_data) {
        Ok(result) => {
            if result == raw_data {
                println!("  ✅ Non-compressed input passed through unchanged");
            } else {
                println!("  ⚠️ Non-compressed input was altered during pass-through");
            }
        }
        Err(e) => println!("  ❌ Unexpected error for non-compressed input: {}", e),
    }

    // Test with a truly corrupted compressed blob.
    println!("🔍 Testing corrupted compressed data...");
    let base_data = vec![42u8; 4096];
    let mut corrupted_data = compression::compress(&base_data)?;

    if corrupted_data.len() > 8 {
        // Preserve header/magic so `decompress` takes the compressed path.
        corrupted_data[4] = 0xFF;
        corrupted_data[5] = 0xFF;
        corrupted_data[6] = 0xFF;
        corrupted_data[7] = 0x7F;
    } else {
        // Fallback in very small case (should be rare since this path is usually >8 bytes).
        corrupted_data = vec![b'G', b'Z', b'L', b'Z', 0x80, 0x00, 0x00, 0x00, 0x99];
    }

    match compression::decompress(&corrupted_data) {
        Ok(_) => println!("  ❌ Decompression should have failed for corrupted compressed data"),
        Err(SaveLoadError::Corrupted(msg)) => {
            println!("  ✅ Correctly caught corrupted data: {}", msg);
        }
        Err(SaveLoadError::Compression(msg)) => {
            println!("  ✅ Correctly caught compression error: {}", msg);
        }
        Err(e) => println!("  ⚠️ Unexpected error type: {}", e),
    }

    // Test with empty data
    println!("📭 Testing empty data...");
    let empty_data = vec![];
    match compression::compress(&empty_data) {
        Ok(result) => println!("  ✅ Empty data compressed to {} bytes", result.len()),
        Err(e) => println!("  ❌ Failed to compress empty data: {}", e),
    }

    // Test compression detection
    println!("🔍 Testing compression detection...");
    let test_data = vec![42u8; 4096];
    let compressed = compression::compress(&test_data)?;

    match compression::is_compressed(&compressed) {
        Ok(true) => println!("  ✅ Correctly identified compressed data"),
        Ok(false) => println!("  ⚠️ Unexpectedly identified compressed data as uncompressed"),
        Err(e) => println!("  ⚠️ Error checking compression: {}", e),
    }

    match compression::is_compressed(&test_data) {
        Ok(false) => println!("  ✅ Correctly identified uncompressed data"),
        Ok(true) => println!("  ❌ Incorrectly identified as compressed"),
        Err(e) => println!("  ⚠️ Error checking compression: {}", e),
    }

    Ok(())
}

// Helper function to create test game state data
#[allow(dead_code)]
fn create_test_state() -> HashMap<String, Vec<u8>> {
    let mut state = HashMap::new();

    // Simulate various game data
    state.insert("player_resources".to_string(), vec![100, 200, 50, 75]); // Mock resource data
    state.insert(
        "unit_positions".to_string(),
        (0..100).map(|i| (i % 256) as u8).collect(),
    ); // Mock position data
    state.insert("building_states".to_string(), vec![1, 0, 1, 1, 0]); // Mock building states
    state.insert("ai_state".to_string(), vec![42; 200]); // Mock AI state

    state
}
