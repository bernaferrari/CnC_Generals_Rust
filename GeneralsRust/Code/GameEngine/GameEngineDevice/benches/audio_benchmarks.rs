//! # Audio Benchmarks
//!
//! Performance benchmarks for the GameEngineDevice audio subsystem.

#[cfg(feature = "audio")]
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
#[cfg(feature = "audio")]
use game_engine_device::audio::*;
#[cfg(feature = "audio")]
use game_engine_device::*;
#[cfg(feature = "audio")]
use std::time::Duration;
#[cfg(feature = "audio")]
use tokio::runtime::Runtime;

#[cfg(feature = "audio")]
fn benchmark_audio_device_initialization(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("audio_device_init", |b| {
        b.iter(|| {
            rt.block_on(async {
                let device_system = GameEngineDevice::new().await.unwrap();
                let audio_config = DeviceConfig::audio()
                    .with_parameter("sample_rate", 44100)
                    .with_parameter("channels", 2);

                let _audio_device = device_system.init_audio_device(audio_config).await.unwrap();
            });
        });
    });
}

#[cfg(feature = "audio")]
fn benchmark_miles_audio_device(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("miles_audio_device_creation", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = MilesAudioConfig::default();
                let _device = MilesAudioDevice::new_with_config(config).await.unwrap();
            });
        });
    });
}

#[cfg(feature = "audio")]
fn benchmark_audio_format_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("audio_format");

    let formats = [
        ("CD Quality", AudioFormat::cd_quality()),
        ("DVD Quality", AudioFormat::dvd_quality()),
        ("High Quality", AudioFormat::high_quality()),
    ];

    for (name, format) in &formats {
        group.bench_with_input(
            BenchmarkId::new("bytes_for_duration", name),
            format,
            |b, fmt| {
                let duration = Duration::from_secs(1);
                b.iter(|| black_box(fmt.bytes_for_duration(duration)));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("compatibility_check", name),
            format,
            |b, fmt| {
                let other_format = AudioFormat::cd_quality();
                b.iter(|| black_box(fmt.is_compatible_with(other_format)));
            },
        );
    }

    group.finish();
}

#[cfg(feature = "audio")]
fn benchmark_audio_handle_generation(c: &mut Criterion) {
    c.bench_function("audio_handle_generation", |b| {
        b.iter(|| {
            for _ in 0..1000 {
                black_box(AudioHandle::new());
            }
        });
    });
}

#[cfg(feature = "audio")]
fn benchmark_sound_buffer_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("sound_buffer");

    // Test different buffer sizes
    let buffer_sizes = [1024, 4096, 16384, 65536];

    for &size in &buffer_sizes {
        group.bench_with_input(
            BenchmarkId::new("create_and_load", size),
            &size,
            |b, &size| {
                let format = AudioFormat::cd_quality();
                let test_data = vec![0u8; size];

                b.iter(|| {
                    let mut buffer = SoundBuffer::new(format, BufferFormat::Interleaved);
                    black_box(buffer.load_from_bytes(&test_data).unwrap());
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("get_samples_f32", size),
            &size,
            |b, &size| {
                let format = AudioFormat::cd_quality();
                let test_data = vec![0u8; size];
                let mut buffer = SoundBuffer::new(format, BufferFormat::Interleaved);
                buffer.load_from_bytes(&test_data).unwrap();

                b.iter(|| {
                    black_box(buffer.get_samples_f32().unwrap());
                });
            },
        );
    }

    group.finish();
}

#[cfg(feature = "audio")]
fn benchmark_streaming_sound(c: &mut Criterion) {
    c.bench_function("streaming_sound_creation", |b| {
        b.iter(|| {
            let format = AudioFormat::cd_quality();
            let config = StreamConfig::default();
            black_box(StreamingSound::new(format, config));
        });
    });
}

#[cfg(feature = "audio")]
fn benchmark_device_enumeration(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("device_enumeration", |b| {
        b.iter(|| {
            rt.block_on(async {
                let device_system = GameEngineDevice::new().await.unwrap();
                let audio_config = DeviceConfig::audio();
                let audio_device = device_system.init_audio_device(audio_config).await.unwrap();

                let _devices = audio_device.get_available_devices().await;
            });
        });
    });
}

#[cfg(feature = "audio")]
fn benchmark_audio_statistics(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("get_audio_statistics", |b| {
        b.iter(|| {
            rt.block_on(async {
                let device_system = GameEngineDevice::new().await.unwrap();
                let audio_config = DeviceConfig::audio();
                let audio_device = device_system.init_audio_device(audio_config).await.unwrap();

                let _stats = audio_device.get_statistics().await;
            });
        });
    });
}

#[cfg(feature = "audio")]
fn benchmark_concurrent_audio_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("concurrent_audio_stats", |b| {
        b.iter(|| {
            rt.block_on(async {
                let device_system = std::sync::Arc::new(GameEngineDevice::new().await.unwrap());
                let audio_config = DeviceConfig::audio();
                let audio_device = std::sync::Arc::new(
                    device_system.init_audio_device(audio_config).await.unwrap(),
                );

                let mut handles = Vec::new();

                // Spawn multiple concurrent operations
                for _ in 0..10 {
                    let device = audio_device.clone();
                    let handle = tokio::spawn(async move { device.get_statistics().await });
                    handles.push(handle);
                }

                // Wait for all to complete
                for handle in handles {
                    let _ = handle.await.unwrap();
                }
            });
        });
    });
}

#[cfg(feature = "audio")]
fn benchmark_memory_usage_calculation(c: &mut Criterion) {
    let formats = [
        AudioFormat::cd_quality(),
        AudioFormat::dvd_quality(),
        AudioFormat::high_quality(),
    ];

    let durations = [
        Duration::from_millis(100),
        Duration::from_secs(1),
        Duration::from_secs(10),
        Duration::from_secs(60),
    ];

    let mut group = c.benchmark_group("memory_calculation");

    for (format_name, format) in [
        ("CD", &formats[0]),
        ("DVD", &formats[1]),
        ("High", &formats[2]),
    ] {
        for duration in &durations {
            let bench_name = format!("{}_{}ms", format_name, duration.as_millis());
            group.bench_with_input(
                BenchmarkId::new("bytes_for_duration", bench_name),
                &(format, duration),
                |b, (fmt, dur)| {
                    b.iter(|| black_box(fmt.bytes_for_duration(**dur)));
                },
            );
        }
    }

    group.finish();
}

#[cfg(feature = "audio")]
fn benchmark_priority_operations(c: &mut Criterion) {
    let priorities = [
        Priority::Low,
        Priority::Normal,
        Priority::High,
        Priority::Critical,
    ];

    c.bench_function("priority_sorting", |b| {
        b.iter(|| {
            let mut test_priorities = priorities.clone();
            test_priorities.sort();
            black_box(test_priorities);
        });
    });

    c.bench_function("priority_comparison", |b| {
        b.iter(|| {
            for i in 0..priorities.len() {
                for j in 0..priorities.len() {
                    black_box(priorities[i] > priorities[j]);
                }
            }
        });
    });
}

#[cfg(feature = "audio")]
criterion_group!(
    audio_benches,
    benchmark_audio_device_initialization,
    benchmark_miles_audio_device,
    benchmark_audio_format_operations,
    benchmark_audio_handle_generation,
    benchmark_sound_buffer_operations,
    benchmark_streaming_sound,
    benchmark_device_enumeration,
    benchmark_audio_statistics,
    benchmark_concurrent_audio_operations,
    benchmark_memory_usage_calculation,
    benchmark_priority_operations
);

#[cfg(feature = "audio")]
criterion_main!(audio_benches);

#[cfg(not(feature = "audio"))]
fn main() {}
