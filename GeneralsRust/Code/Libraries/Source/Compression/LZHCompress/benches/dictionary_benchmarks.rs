#![cfg(feature = "dictionary_compression")]

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use lzh_compression::{CompressionLevel, compress_parallel};

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
