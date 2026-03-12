#![cfg(feature = "dictionary_compression")]

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use lzh_compression::{compress_parallel, CompressionLevel};

fn dictionary_parallel(c: &mut Criterion) {
    let data = vec![0_u8; 256 * 1024];

    c.bench_function("lzh_dictionary_parallel", |b| {
        b.iter_batched(
            || data.clone(),
            |input| {
                let compressed = compress_parallel(&input, CompressionLevel::Best, 32 * 1024)
                    .expect("parallel compression should succeed");
                criterion::black_box(compressed);
            },
            BatchSize::LargeInput,
        );
    });
}

criterion_group!(benches, dictionary_parallel);
criterion_main!(benches);
