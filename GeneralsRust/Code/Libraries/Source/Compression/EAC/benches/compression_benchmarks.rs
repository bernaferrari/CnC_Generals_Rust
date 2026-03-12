use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use eac_compression::{compress, decompress, CompressionType};

fn refpack_round_trip(c: &mut Criterion) {
    let data = vec![0_u8; 128 * 1024];

    c.bench_function("eac_refpack_round_trip", |b| {
        b.iter_batched(
            || data.clone(),
            |input| {
                let compressed = compress(&input, CompressionType::RefPack)
                    .expect("refpack compression should succeed");
                let decompressed =
                    decompress(&compressed).expect("refpack decompress should succeed");
                assert_eq!(input, decompressed);
            },
            BatchSize::LargeInput,
        );
    });
}

criterion_group!(benches, refpack_round_trip);
criterion_main!(benches);
