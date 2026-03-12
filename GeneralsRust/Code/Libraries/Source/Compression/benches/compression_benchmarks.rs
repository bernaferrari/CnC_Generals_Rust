use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use generals_compression::*;
use rand::prelude::*;

fn generate_test_data(size: usize, pattern: TestPattern) -> Vec<u8> {
    let mut rng = SmallRng::seed_from_u64(42); // Deterministic for consistent benchmarks

    match pattern {
        TestPattern::Random => (0..size).map(|_| rng.gen()).collect(),
        TestPattern::Repetitive => {
            let pattern = b"ABCDEFGH";
            (0..size).map(|i| pattern[i % pattern.len()]).collect()
        }
        TestPattern::Text => {
            let text = b"The quick brown fox jumps over the lazy dog. ";
            (0..size).map(|i| text[i % text.len()]).collect()
        }
        TestPattern::Binary => {
            // Simulate binary file with some structure
            (0..size)
                .map(|i| {
                    if i % 16 < 4 {
                        0x00 // Header-like bytes
                    } else if i % 16 < 8 {
                        (i / 16) as u8 // Sequential data
                    } else {
                        rng.gen() // Random payload
                    }
                })
                .collect()
        }
        TestPattern::Sparse => {
            // Mostly zeros with occasional data
            (0..size)
                .map(|i| if i % 100 < 5 { rng.gen() } else { 0x00 })
                .collect()
        }
    }
}

#[derive(Clone, Copy)]
enum TestPattern {
    Random,
    Repetitive,
    Text,
    Binary,
    Sparse,
}

impl TestPattern {
    fn name(&self) -> &'static str {
        match self {
            Self::Random => "random",
            Self::Repetitive => "repetitive",
            Self::Text => "text",
            Self::Binary => "binary",
            Self::Sparse => "sparse",
        }
    }
}

fn benchmark_compression(c: &mut Criterion) {
    let sizes = [1024, 8192, 65536, 262144, 1048576]; // 1KB to 1MB
    let patterns = [
        TestPattern::Random,
        TestPattern::Repetitive,
        TestPattern::Text,
        TestPattern::Binary,
        TestPattern::Sparse,
    ];
    let algorithms = [
        CompressionType::RefPack,
        CompressionType::BTree,
        CompressionType::Huffman,
        CompressionType::LZH,
        CompressionType::ZLib(1),
        CompressionType::ZLib(6),
        CompressionType::ZLib(9),
    ];

    for &size in &sizes {
        for &pattern in &patterns {
            let data = generate_test_data(size, pattern);

            let mut group = c.benchmark_group(format!("compress_{}_{}", pattern.name(), size));
            group.throughput(Throughput::Bytes(size as u64));

            for &algorithm in &algorithms {
                group.bench_with_input(
                    BenchmarkId::from_parameter(algorithm.name()),
                    &(&data, algorithm),
                    |b, (data, algo)| b.iter(|| compress(data, *algo).unwrap()),
                );
            }
            group.finish();
        }
    }
}

fn benchmark_decompression(c: &mut Criterion) {
    let sizes = [1024, 8192, 65536, 262144, 1048576];
    let patterns = [
        TestPattern::Random,
        TestPattern::Repetitive,
        TestPattern::Text,
        TestPattern::Binary,
        TestPattern::Sparse,
    ];
    let algorithms = [
        CompressionType::RefPack,
        CompressionType::BTree,
        CompressionType::Huffman,
        CompressionType::LZH,
        CompressionType::ZLib(6),
    ];

    for &size in &sizes {
        for &pattern in &patterns {
            let data = generate_test_data(size, pattern);

            // Pre-compress data for all algorithms
            let compressed_data: Vec<_> = algorithms
                .iter()
                .map(|&algo| (algo, compress(&data, algo).unwrap()))
                .collect();

            let mut group = c.benchmark_group(format!("decompress_{}_{}", pattern.name(), size));
            group.throughput(Throughput::Bytes(size as u64));

            for (algorithm, compressed) in compressed_data {
                group.bench_with_input(
                    BenchmarkId::from_parameter(algorithm.name()),
                    &compressed,
                    |b, compressed| b.iter(|| decompress(compressed).unwrap()),
                );
            }
            group.finish();
        }
    }
}

fn benchmark_auto_compression(c: &mut Criterion) {
    let sizes = [1024, 8192, 65536, 262144];
    let patterns = [
        TestPattern::Random,
        TestPattern::Repetitive,
        TestPattern::Text,
        TestPattern::Binary,
        TestPattern::Sparse,
    ];

    let mut group = c.benchmark_group("auto_compression");

    for &size in &sizes {
        for &pattern in &patterns {
            let data = generate_test_data(size, pattern);

            group.throughput(Throughput::Bytes(size as u64));
            group.bench_with_input(
                BenchmarkId::from_parameter(format!("{}_{}", pattern.name(), size)),
                &data,
                |b, data| b.iter(|| compress_auto(data).unwrap()),
            );
        }
    }
    group.finish();
}

fn benchmark_streaming_compression(c: &mut Criterion) {
    let sizes = [65536, 262144, 1048576]; // 64KB to 1MB
    let chunk_sizes = [4096, 16384, 65536]; // 4KB to 64KB chunks

    for &size in &sizes {
        for &chunk_size in &chunk_sizes {
            let data = generate_test_data(size, TestPattern::Binary);

            let mut group = c.benchmark_group(format!("streaming_{}_{}", size, chunk_size));
            group.throughput(Throughput::Bytes(size as u64));

            group.bench_function("refpack", |b| {
                b.iter(|| {
                    let mut compressor =
                        StreamingCompressor::with_chunk_size(CompressionType::RefPack, chunk_size);
                    let mut input = std::io::Cursor::new(&data);
                    let mut output = Vec::new();
                    compressor.compress_stream(&mut input, &mut output).unwrap();
                    output
                })
            });

            group.bench_function("lzh", |b| {
                b.iter(|| {
                    let mut compressor =
                        StreamingCompressor::with_chunk_size(CompressionType::LZH, chunk_size);
                    let mut input = std::io::Cursor::new(&data);
                    let mut output = Vec::new();
                    compressor.compress_stream(&mut input, &mut output).unwrap();
                    output
                })
            });

            group.finish();
        }
    }
}

fn benchmark_compression_ratios(c: &mut Criterion) {
    let size = 65536; // 64KB test size
    let patterns = [
        TestPattern::Random,
        TestPattern::Repetitive,
        TestPattern::Text,
        TestPattern::Binary,
        TestPattern::Sparse,
    ];
    let algorithms = [
        CompressionType::RefPack,
        CompressionType::BTree,
        CompressionType::Huffman,
        CompressionType::LZH,
        CompressionType::ZLib(1),
        CompressionType::ZLib(6),
        CompressionType::ZLib(9),
    ];

    for &pattern in &patterns {
        let data = generate_test_data(size, pattern);

        println!(
            "\n=== Compression Ratios for {} data ({}KB) ===",
            pattern.name(),
            size / 1024
        );
        println!(
            "{:<12} {:<10} {:<12} {:<8}",
            "Algorithm", "Size", "Ratio", "Speed"
        );

        for &algorithm in &algorithms {
            let start = std::time::Instant::now();
            let compressed = compress(&data, algorithm).unwrap();
            let duration = start.elapsed();

            let ratio = compressed.len() as f64 / data.len() as f64;
            let speed = (data.len() as f64 / duration.as_secs_f64()) / 1_048_576.0; // MB/s

            println!(
                "{:<12} {:<10} {:<12.3} {:<8.1}MB/s",
                algorithm.name(),
                compressed.len(),
                ratio,
                speed
            );
        }
    }
}

criterion_group!(
    benches,
    benchmark_compression,
    benchmark_decompression,
    benchmark_auto_compression,
    benchmark_streaming_compression
);
criterion_main!(benches);
