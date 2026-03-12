use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use zlib_compression::{deflate_raw, CompressionLevel};

fn deflate_only(c: &mut Criterion) {
    let data = vec![0_u8; 128 * 1024];

    c.bench_function("deflate_raw", |b| {
        b.iter_batched(
            || data.clone(),
            |input| {
                let compressed =
                    deflate_raw(&input, CompressionLevel::Fast4).expect("deflate should succeed");
                criterion::black_box(compressed);
            },
            BatchSize::LargeInput,
        );
    });
}

criterion_group!(benches, deflate_only);
criterion_main!(benches);
