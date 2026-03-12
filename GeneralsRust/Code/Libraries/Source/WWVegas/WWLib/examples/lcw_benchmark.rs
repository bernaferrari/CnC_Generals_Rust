//! LCW Performance Benchmark
//!
//! This example benchmarks the performance of LCW compression and decompression
//! with various data sizes and patterns.

use std::time::Instant;
use wwlib_rust::lcw::{compress, decompress};

fn main() {
    println!("LCW Performance Benchmark");
    println!("=========================");

    // Test different data sizes - start smaller to avoid issues
    let sizes = vec![1024, 4096, 16384]; // Start with smaller sizes

    for size in sizes {
        println!("\n--- Testing with {} bytes ---", size);

        // Test with repetitive data (best case for compression)
        benchmark_data_type("Repetitive data", &vec![0x42; size]);

        // Test with simple patterns instead of complex ones
        let simple_pattern = b"ABC".repeat(size / 3);
        benchmark_data_type("Simple pattern", &simple_pattern);

        // Test with pseudo-random data (worst case for compression)
        let random_data = create_pseudo_random_data(size);
        benchmark_data_type("Pseudo-random", &random_data);
    }

    println!("\nBenchmark completed!");
}

fn benchmark_data_type(name: &str, data: &[u8]) {
    let iterations = if data.len() >= 65536 { 10 } else { 100 };

    // Benchmark compression
    let start = Instant::now();
    let mut compressed_data = Vec::new();

    for _ in 0..iterations {
        compressed_data = compress(data).expect("Compression failed");
    }

    let compress_duration = start.elapsed();
    let compress_throughput =
        (data.len() as f64 * iterations as f64) / compress_duration.as_secs_f64() / 1_048_576.0;

    // Benchmark decompression
    let start = Instant::now();
    let mut decompressed_data = Vec::new();

    for _ in 0..iterations {
        decompressed_data = decompress(&compressed_data).expect("Decompression failed");
    }

    let decompress_duration = start.elapsed();
    let decompress_throughput =
        (data.len() as f64 * iterations as f64) / decompress_duration.as_secs_f64() / 1_048_576.0;

    // Verify correctness
    assert_eq!(
        data,
        &decompressed_data[..],
        "Round-trip verification failed for {}",
        name
    );

    let ratio = (compressed_data.len() as f64 / data.len() as f64) * 100.0;

    println!(
        "  {}: {:.1}% ratio, {:.1} MB/s compress, {:.1} MB/s decompress",
        name, ratio, compress_throughput, decompress_throughput
    );
}

fn create_mixed_pattern_data(size: usize) -> Vec<u8> {
    let pattern = b"The quick brown fox jumps over the lazy dog. ";
    let mut data = Vec::with_capacity(size);

    while data.len() < size {
        let remaining = size - data.len();
        let chunk_size = std::cmp::min(pattern.len(), remaining);
        data.extend_from_slice(&pattern[..chunk_size]);
    }

    data
}

fn create_pseudo_random_data(size: usize) -> Vec<u8> {
    // Simple linear congruential generator for reproducible "random" data
    let mut data = Vec::with_capacity(size);
    let mut state = 12345u32;

    for _ in 0..size {
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        data.push((state >> 24) as u8);
    }

    data
}
