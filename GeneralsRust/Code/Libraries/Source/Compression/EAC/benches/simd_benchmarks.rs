#![cfg(feature = "simd")]

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use eac_compression::{compress_parallel, CompressionType};

fn simd_parallel(c: &mut Criterion) {
    let data = vec![0_u8; 256 * 1024];

    c.bench_function("eac_parallel_refpack", |b| {
        b.iter_batched(
            || data.clone(),
            |input| {
                let compressed = compress_parallel(&input, CompressionType::RefPack, 64 * 1024)
                    .expect("parallel compression should succeed");
                criterion::black_box(compressed);
            },
            BatchSize::LargeInput,
        );
    });
}

criterion_group!(benches, simd_parallel);
criterion_main!(benches);
