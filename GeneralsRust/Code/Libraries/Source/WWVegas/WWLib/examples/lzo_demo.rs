//! LZO Compression Demo
//!
//! This example demonstrates the LZO compression capabilities of the wwlib-rust crate.
//! It shows basic compression and decompression, performance benchmarking, and various
//! usage patterns.

use std::time::Instant;
use wwlib_rust::lzo::{lzo_buffer_size, LzoCompressor, LzoError};

fn main() -> Result<(), LzoError> {
    println!("=== LZO Compression Demo ===\n");

    // Basic compression example
    basic_compression_example()?;

    // Buffer reuse example
    buffer_reuse_example()?;

    // Performance benchmark
    performance_benchmark()?;

    // Compression ratio analysis
    compression_ratio_analysis()?;

    Ok(())
}

fn basic_compression_example() -> Result<(), LzoError> {
    println!("1. Basic Compression Example");
    println!("----------------------------");

    let original_data = b"Hello, World! This is a test of the LZO compression algorithm. \
                          LZO stands for Lempel-Ziv-Oberhumer and is designed for fast \
                          decompression. Hello, World! This is a test of the LZO compression algorithm.";

    println!(
        "Original data: {:?}",
        String::from_utf8_lossy(original_data)
    );
    println!("Original size: {} bytes", original_data.len());

    // Compress the data
    let start = Instant::now();
    let compressed = LzoCompressor::compress(original_data)?;
    let compress_time = start.elapsed();

    println!("Compressed size: {} bytes", compressed.len());
    println!(
        "Compression ratio: {:.2}%",
        (compressed.len() as f64 / original_data.len() as f64) * 100.0
    );
    println!("Compression time: {:?}", compress_time);

    // Decompress the data
    let start = Instant::now();
    let decompressed = LzoCompressor::decompress(&compressed, original_data.len())?;
    let decompress_time = start.elapsed();

    println!("Decompression time: {:?}", decompress_time);
    println!(
        "Speed ratio (decompress/compress): {:.2}x faster",
        compress_time.as_nanos() as f64 / decompress_time.as_nanos() as f64
    );

    // Verify correctness
    if decompressed == original_data {
        println!("✓ Decompression successful - data matches original");
    } else {
        println!("✗ Decompression failed - data mismatch!");
        return Err(LzoError::Error);
    }

    println!();
    Ok(())
}

fn buffer_reuse_example() -> Result<(), LzoError> {
    println!("2. Buffer Reuse Example");
    println!("-----------------------");

    let test_data = b"This example shows how to reuse buffers for better performance when compressing multiple pieces of data.";

    println!("Using buffer reuse for better performance...");

    // Pre-allocate buffers
    let mut compress_buffer = vec![0u8; lzo_buffer_size(test_data.len())];
    let mut decompress_buffer = vec![0u8; test_data.len()];

    let start = Instant::now();

    // Compress using pre-allocated buffer
    let compressed_size = LzoCompressor::compress_to_buffer(test_data, &mut compress_buffer)?;
    let compressed_data = &compress_buffer[..compressed_size];

    // Decompress using pre-allocated buffer
    let decompressed_size =
        LzoCompressor::decompress_to_buffer(compressed_data, &mut decompress_buffer)?;
    let decompressed_data = &decompress_buffer[..decompressed_size];

    let total_time = start.elapsed();

    println!("Original size: {} bytes", test_data.len());
    println!("Compressed size: {} bytes", compressed_size);
    println!("Total time (compress + decompress): {:?}", total_time);

    // Verify correctness
    if decompressed_data == test_data {
        println!("✓ Buffer reuse example successful");
    } else {
        println!("✗ Buffer reuse example failed!");
        return Err(LzoError::Error);
    }

    println!();
    Ok(())
}

fn performance_benchmark() -> Result<(), LzoError> {
    println!("3. Performance Benchmark");
    println!("------------------------");

    // Generate test data of various sizes
    let test_sizes = vec![1024, 4096, 16384, 65536];

    for &size in &test_sizes {
        // Create repetitive test data (should compress well)
        let mut test_data = Vec::with_capacity(size);
        let pattern = b"Performance test data with some repetition. ";
        while test_data.len() < size {
            let remaining = size - test_data.len();
            if remaining >= pattern.len() {
                test_data.extend_from_slice(pattern);
            } else {
                test_data.extend_from_slice(&pattern[..remaining]);
            }
        }

        println!("\nTesting with {} bytes of data:", size);

        // Compression benchmark
        let start = Instant::now();
        let compressed = LzoCompressor::compress(&test_data)?;
        let compress_time = start.elapsed();

        let compress_throughput = size as f64 / compress_time.as_secs_f64() / 1024.0 / 1024.0;

        // Decompression benchmark
        let start = Instant::now();
        let _decompressed = LzoCompressor::decompress(&compressed, test_data.len())?;
        let decompress_time = start.elapsed();

        let decompress_throughput = size as f64 / decompress_time.as_secs_f64() / 1024.0 / 1024.0;

        println!(
            "  Compression:   {:?} ({:.1} MB/s)",
            compress_time, compress_throughput
        );
        println!(
            "  Decompression: {:?} ({:.1} MB/s)",
            decompress_time, decompress_throughput
        );
        println!(
            "  Ratio: {:.1}% ({} -> {} bytes)",
            (compressed.len() as f64 / size as f64) * 100.0,
            size,
            compressed.len()
        );
    }

    println!();
    Ok(())
}

fn compression_ratio_analysis() -> Result<(), LzoError> {
    println!("4. Compression Ratio Analysis");
    println!("-----------------------------");

    // Test with different types of data
    let test_cases = vec![
        ("Highly repetitive", generate_repetitive_data(4096)),
        ("Text-like data", generate_text_like_data(4096)),
        ("Semi-random data", generate_semi_random_data(4096)),
        ("Random data", generate_random_data(4096)),
    ];

    for (name, data) in test_cases {
        let compressed = LzoCompressor::compress(&data)?;
        let ratio = (compressed.len() as f64 / data.len() as f64) * 100.0;

        println!(
            "{:18}: {:4} bytes -> {:4} bytes ({:5.1}%)",
            name,
            data.len(),
            compressed.len(),
            ratio
        );

        // Verify decompression works
        let decompressed = LzoCompressor::decompress(&compressed, data.len())?;
        if decompressed != data {
            println!("  ✗ Decompression failed for {}", name);
            return Err(LzoError::Error);
        }
    }

    println!("\n✓ All compression ratio tests passed");
    println!();
    Ok(())
}

fn generate_repetitive_data(size: usize) -> Vec<u8> {
    let pattern = b"ABCD";
    let mut data = Vec::with_capacity(size);
    while data.len() < size {
        let remaining = size - data.len();
        if remaining >= pattern.len() {
            data.extend_from_slice(pattern);
        } else {
            data.extend_from_slice(&pattern[..remaining]);
        }
    }
    data
}

fn generate_text_like_data(size: usize) -> Vec<u8> {
    let words = [
        b"the".as_slice(),
        b"quick",
        b"brown",
        b"fox",
        b"jumps",
        b"over",
        b"lazy",
        b"dog",
        b"and",
        b"runs",
        b"away",
        b"into",
        b"forest",
        b"where",
        b"many",
        b"trees",
        b"grow",
        b"tall",
        b"under",
        b"blue",
        b"sky",
        b"with",
        b"white",
        b"clouds",
    ];

    let mut data = Vec::with_capacity(size);
    let mut word_index = 0;

    while data.len() < size {
        if data.len() > 0 {
            data.push(b' ');
        }

        let word = words[word_index % words.len()];
        let remaining = size - data.len();

        if remaining >= word.len() {
            data.extend_from_slice(word);
        } else {
            data.extend_from_slice(&word[..remaining]);
            break;
        }

        word_index += 1;
    }

    data
}

fn generate_semi_random_data(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let mut seed = 12345u32;

    for i in 0..size {
        // Generate semi-predictable data with some patterns
        let value = if i % 16 == 0 {
            0xFF // Regular pattern
        } else if i % 8 == 0 {
            0x00 // Another pattern
        } else {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            (seed >> 16) as u8
        };
        data.push(value);
    }

    data
}

fn generate_random_data(size: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(size);
    let mut seed = 54321u32;

    for _ in 0..size {
        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
        data.push((seed >> 16) as u8);
    }

    data
}
