//! # Video Benchmarks
//!
//! Performance benchmarks for the GameEngineDevice video subsystem.

#[cfg(feature = "video")]
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
#[cfg(feature = "video")]
use game_engine_device::video::*;
#[cfg(feature = "video")]
use game_engine_device::*;
#[cfg(feature = "video")]
use tokio::runtime::Runtime;

#[cfg(feature = "video")]
fn benchmark_video_device_initialization(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("video_device_init", |b| {
        b.iter(|| {
            rt.block_on(async {
                let device_system = GameEngineDevice::new().await.unwrap();
                let video_config = DeviceConfig::video()
                    .with_parameter("width", 1920)
                    .with_parameter("height", 1080);

                let _video_device = device_system.init_video_device(video_config).await.unwrap();
            });
        });
    });
}

#[cfg(feature = "video")]
fn benchmark_resolution_operations(c: &mut Criterion) {
    let resolutions = [
        ("720p", Resolution::hd_720p()),
        ("1080p", Resolution::hd_1080p()),
        ("4K", Resolution::uhd_4k()),
        ("8K", Resolution::uhd_8k()),
    ];

    let mut group = c.benchmark_group("resolution_ops");

    for (name, resolution) in &resolutions {
        group.bench_with_input(
            BenchmarkId::new("aspect_ratio", name),
            resolution,
            |b, res| {
                b.iter(|| black_box(res.aspect_ratio()));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("pixel_count", name),
            resolution,
            |b, res| {
                b.iter(|| black_box(res.pixel_count()));
            },
        );
    }

    group.finish();
}

#[cfg(feature = "video")]
fn benchmark_display_adapter_enumeration(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("adapter_enumeration", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _adapters = DisplayAdapter::enumerate().await.unwrap();
            });
        });
    });

    c.bench_function("get_primary_adapter", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _primary = DisplayAdapter::get_primary().await.unwrap();
            });
        });
    });
}

#[cfg(feature = "video")]
fn benchmark_display_mode_operations(c: &mut Criterion) {
    let display_modes = [
        DisplayMode::new(Resolution::hd_720p(), RefreshRate::rate_60hz(), 32),
        DisplayMode::new(Resolution::hd_1080p(), RefreshRate::rate_60hz(), 32),
        DisplayMode::new(Resolution::hd_1080p(), RefreshRate::rate_120hz(), 32),
        DisplayMode::new(Resolution::hd_1080p(), RefreshRate::rate_144hz(), 32),
        DisplayMode::new(Resolution::uhd_4k(), RefreshRate::rate_60hz(), 32),
    ];

    c.bench_function("display_mode_creation", |b| {
        b.iter(|| {
            for mode in &display_modes {
                black_box(*mode);
            }
        });
    });
}

#[cfg(feature = "video")]
fn benchmark_render_device_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let apis = [
        ("Vulkan", GraphicsApi::Vulkan),
        ("DirectX12", GraphicsApi::DirectX12),
        ("Metal", GraphicsApi::Metal),
        ("WebGPU", GraphicsApi::WebGPU),
    ];

    let mut group = c.benchmark_group("render_device");

    for (name, api) in &apis {
        group.bench_with_input(BenchmarkId::new("creation", name), api, |b, api| {
            b.iter(|| {
                rt.block_on(async {
                    // Note: This might fail for unsupported APIs, but that's expected
                    let _result = RenderDevice::new(*api).await;
                });
            });
        });
    }

    group.finish();
}

#[cfg(feature = "video")]
fn benchmark_video_statistics(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("video_statistics_update", |b| {
        b.iter(|| {
            rt.block_on(async {
                let device_system = GameEngineDevice::new().await.unwrap();
                let video_config = DeviceConfig::video();
                let video_device = device_system.init_video_device(video_config).await.unwrap();

                // Simulate updating statistics
                video_device.update_statistics(16.67, 100, 50000).await;
                let _stats = video_device.get_statistics().await;
            });
        });
    });
}

#[cfg(feature = "video")]
fn benchmark_msaa_settings(c: &mut Criterion) {
    let msaa_settings = [
        MsaaSettings::none(),
        MsaaSettings::msaa_2x(),
        MsaaSettings::msaa_4x(),
        MsaaSettings::msaa_8x(),
    ];

    c.bench_function("msaa_is_enabled_check", |b| {
        b.iter(|| {
            for setting in &msaa_settings {
                black_box(setting.is_enabled());
            }
        });
    });
}

#[cfg(feature = "video")]
fn benchmark_color_format_operations(c: &mut Criterion) {
    let formats = [
        ColorFormat::Rgba8,
        ColorFormat::Bgra8,
        ColorFormat::Rgba16,
        ColorFormat::Rgba32Float,
        ColorFormat::Rgb10A2,
        ColorFormat::Hdr10,
    ];

    c.bench_function("color_format_enum_operations", |b| {
        b.iter(|| {
            for format in &formats {
                black_box(*format == ColorFormat::Rgba8);
                black_box(format.clone());
            }
        });
    });
}

#[cfg(feature = "video")]
fn benchmark_vsync_mode_operations(c: &mut Criterion) {
    let vsync_modes = [
        VSync::Disabled,
        VSync::Enabled,
        VSync::Adaptive,
        VSync::Fast,
    ];

    c.bench_function("vsync_mode_operations", |b| {
        b.iter(|| {
            for mode in &vsync_modes {
                black_box(*mode == VSync::Enabled);
                black_box(mode.clone());
            }
        });
    });
}

#[cfg(feature = "video")]
fn benchmark_concurrent_video_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("concurrent_video_stats", |b| {
        b.iter(|| {
            rt.block_on(async {
                let device_system = std::sync::Arc::new(GameEngineDevice::new().await.unwrap());
                let video_config = DeviceConfig::video();
                let video_device = std::sync::Arc::new(
                    device_system.init_video_device(video_config).await.unwrap(),
                );

                let mut handles = Vec::new();

                // Spawn multiple concurrent operations
                for _ in 0..10 {
                    let device = video_device.clone();
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

#[cfg(feature = "video")]
fn benchmark_display_mode_sorting(c: &mut Criterion) {
    let modes = vec![
        DisplayMode::new(Resolution::hd_1080p(), RefreshRate::rate_60hz(), 32),
        DisplayMode::new(Resolution::hd_720p(), RefreshRate::rate_120hz(), 32),
        DisplayMode::new(Resolution::uhd_4k(), RefreshRate::rate_60hz(), 32),
        DisplayMode::new(Resolution::hd_1080p(), RefreshRate::rate_144hz(), 32),
        DisplayMode::new(Resolution::new(2560, 1440), RefreshRate::rate_60hz(), 32),
        DisplayMode::new(Resolution::hd_720p(), RefreshRate::rate_60hz(), 32),
        DisplayMode::new(Resolution::hd_1080p(), RefreshRate::rate_120hz(), 32),
    ];

    c.bench_function("display_mode_sorting", |b| {
        b.iter(|| {
            let mut test_modes = modes.clone();
            // Sort by resolution first, then refresh rate
            test_modes.sort_by(|a, b| {
                let res_cmp = (a.resolution.width * a.resolution.height)
                    .cmp(&(b.resolution.width * b.resolution.height));
                if res_cmp == std::cmp::Ordering::Equal {
                    a.refresh_rate
                        .hz
                        .partial_cmp(&b.refresh_rate.hz)
                        .unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    res_cmp
                }
            });
            black_box(test_modes);
        });
    });
}

#[cfg(feature = "video")]
fn benchmark_adapter_memory_calculations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("adapter_memory_budget", |b| {
        b.iter(|| {
            rt.block_on(async {
                let adapters = DisplayAdapter::enumerate().await.unwrap();
                for adapter in adapters {
                    black_box(adapter.get_texture_memory_budget());
                }
            });
        });
    });
}

#[cfg(feature = "video")]
criterion_group!(
    video_benches,
    benchmark_video_device_initialization,
    benchmark_resolution_operations,
    benchmark_display_adapter_enumeration,
    benchmark_display_mode_operations,
    benchmark_render_device_operations,
    benchmark_video_statistics,
    benchmark_msaa_settings,
    benchmark_color_format_operations,
    benchmark_vsync_mode_operations,
    benchmark_concurrent_video_operations,
    benchmark_display_mode_sorting,
    benchmark_adapter_memory_calculations
);

#[cfg(feature = "video")]
criterion_main!(video_benches);

#[cfg(not(feature = "video"))]
fn main() {}
