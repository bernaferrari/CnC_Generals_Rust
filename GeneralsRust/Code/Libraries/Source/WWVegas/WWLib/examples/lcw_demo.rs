//! LCW Compression/Decompression Demo
//!
//! This example demonstrates the LCW compression and decompression functionality
//! from the WWLib Rust implementation.

use wwlib_rust::lcw::{compress, decompress, LcwError};

fn main() -> Result<(), LcwError> {
    println!("LCW Compression/Decompression Demo");
    println!("==================================");

    // Example 1: Simple text compression
    println!("\nExample 1: Simple text");
    let text = b"Hello, World! This is a test of LCW compression.";
    demo_compression("Simple text", text)?;

    // Example 2: Repetitive data (should compress well)
    println!("\nExample 2: Repetitive data");
    let repetitive = b"AAABBBCCCAAABBBCCCAAABBBCCC".repeat(5);
    demo_compression("Repetitive data", &repetitive)?;

    // Example 3: Highly repetitive data
    println!("\nExample 3: Highly repetitive data");
    let very_repetitive = vec![0x42; 1000]; // 1000 'B' characters
    demo_compression("Highly repetitive data", &very_repetitive)?;

    // Example 4: Random-looking data (should not compress well)
    println!("\nExample 4: Binary data (all byte values)");
    let binary_data: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
    demo_compression("Binary data", &binary_data)?;

    // Example 5: Mixed patterns
    println!("\nExample 5: Mixed patterns");
    let mixed = b"The quick brown fox jumps over the lazy dog. ".repeat(10);
    demo_compression("Mixed patterns", &mixed)?;

    // Example 6: Empty data edge case
    println!("\nExample 6: Empty data");
    let empty: &[u8] = &[];
    match compress(empty) {
        Ok(compressed) => {
            println!("Empty data compressed to {} bytes", compressed.len());
            match decompress(&compressed) {
                Ok(decompressed) => {
                    println!("Decompressed back to {} bytes", decompressed.len());
                    assert_eq!(empty, &decompressed[..]);
                    println!("✓ Round-trip successful");
                }
                Err(e) => println!("✗ Decompression failed: {}", e),
            }
        }
        Err(e) => println!("✗ Compression failed: {}", e),
    }

    println!("\nDemo completed successfully!");
    Ok(())
}

fn demo_compression(name: &str, data: &[u8]) -> Result<(), LcwError> {
    println!("Testing: {}", name);
    println!("Original size: {} bytes", data.len());

    // Compress the data
    let compressed = compress(data)?;
    println!("Compressed size: {} bytes", compressed.len());

    // Calculate compression ratio
    let ratio = if data.is_empty() {
        0.0
    } else {
        (compressed.len() as f64 / data.len() as f64) * 100.0
    };
    println!("Compression ratio: {:.1}%", ratio);

    if ratio < 100.0 {
        println!("Space saved: {:.1}%", 100.0 - ratio);
    } else {
        println!("No space saved (data expanded)");
    }

    // Decompress and verify
    let decompressed = decompress(&compressed)?;

    if data == &decompressed[..] {
        println!("✓ Round-trip verification successful");
    } else {
        println!("✗ Round-trip verification failed!");
        println!(
            "Expected {} bytes, got {} bytes",
            data.len(),
            decompressed.len()
        );

        // Show first few bytes for debugging
        if !data.is_empty() && !decompressed.is_empty() {
            let show_bytes = std::cmp::min(16, std::cmp::min(data.len(), decompressed.len()));
            print!("Original first {} bytes: ", show_bytes);
            for &b in &data[..show_bytes] {
                print!("{:02x} ", b);
            }
            println!();

            print!("Decompressed first {} bytes: ", show_bytes);
            for &b in &decompressed[..show_bytes] {
                print!("{:02x} ", b);
            }
            println!();
        }

        return Err(LcwError::CorruptData(
            "Round-trip verification failed".to_string(),
        ));
    }

    Ok(())
}
