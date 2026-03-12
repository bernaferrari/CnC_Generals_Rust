use criterion::{black_box, criterion_group, criterion_main, Criterion};
use wp_audio::{AudioSystemConfig, Priority, Volume};

fn benchmark_volume_conversion(c: &mut Criterion) {
    c.bench_function("volume_to_linear", |b| {
        b.iter(|| {
            for volume in 0..=100u8 {
                black_box(wp_audio::level::VolumeUtils::volume_to_linear(volume));
            }
        })
    });
}

fn benchmark_priority_comparison(c: &mut Criterion) {
    let priorities = vec![
        Priority::Low,
        Priority::Normal,
        Priority::High,
        Priority::Critical,
    ];

    c.bench_function("priority_ordering", |b| {
        b.iter(|| {
            for p1 in &priorities {
                for p2 in &priorities {
                    black_box(p1.cmp(p2));
                }
            }
        })
    });
}

fn benchmark_audio_config_creation(c: &mut Criterion) {
    c.bench_function("config_default", |b| {
        b.iter(|| {
            black_box(AudioSystemConfig::default());
        })
    });
}

criterion_group!(
    benches,
    benchmark_volume_conversion,
    benchmark_priority_comparison,
    benchmark_audio_config_creation
);

criterion_main!(benches);
