//! Rendering benchmark placeholder.
//!
//! The real benchmark will mirror the C++ renderer micro-benchmarks once the
//! wgpu backend reaches feature parity.

use criterion::{criterion_group, criterion_main, Criterion};

fn placeholder_benchmark(c: &mut Criterion) {
    c.bench_function("rendering_stub", |b| b.iter(|| ()));
}

criterion_group!(benches, placeholder_benchmark);
criterion_main!(benches);
