use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use lzh_compression::{compress, decompress, CompressionLevel};

fn lzh_round_trip(c: &mut Criterion) {
    let data = vec![0_u8; 64 * 1024];

    c.bench_function("lzh_round_trip", |b| {
        b.iter_batched(
            || data.clone(),
            |input| {
                let compressed = compress(&input, CompressionLevel::Default)
                    .expect("compression should succeed");
                let decompressed = decompress(&compressed).expect("decompression should succeed");
                assert_eq!(input, decompressed);
            },
            BatchSize::LargeInput,
        );
    });
}

criterion_group!(benches, lzh_round_trip);
criterion_main!(benches);
