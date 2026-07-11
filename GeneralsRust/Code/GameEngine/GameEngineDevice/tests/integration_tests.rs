//! # Integration Tests for GameEngineDevice
//!
//! Comprehensive integration tests for the complete GameEngineDevice system.

use game_engine_device::*;

#[tokio::test]
async fn test_device_system_initialization() {
    let device_system = GameEngineDevice::new().await;
    assert!(
        device_system.is_ok(),
        "Device system should initialize successfully"
    );

    let system = device_system.unwrap();
    let status = system.get_system_status().await.unwrap();

    // Initially, no devices should be active since we haven't initialized any
    assert!(
        status.is_empty() || status.iter().all(|s| !s.active),
        "No devices should be active initially"
    );
}

#[cfg(feature = "audio")]
#[tokio::test]
async fn test_device_config_builder_audio() {
    let audio_config = DeviceConfig::audio()
        .with_parameter("sample_rate", 44100)
        .with_parameter("channels", 2)
        .with_parameter("buffer_size", 1024);

    assert_eq!(audio_config.device_type, DeviceType::Audio);
    assert!(audio_config.parameters.contains_key("sample_rate"));
    assert!(audio_config.parameters.contains_key("channels"));
    assert!(audio_config.parameters.contains_key("buffer_size"));
}

#[cfg(feature = "video")]
#[tokio::test]
async fn test_device_config_builder_video() {
    let video_config = DeviceConfig::video()
        .with_parameter("width", 1920)
        .with_parameter("height", 1080)
        .with_parameter("vsync", true);

    assert_eq!(video_config.device_type, DeviceType::Video);
    assert!(video_config.parameters.contains_key("width"));
    assert!(video_config.parameters.contains_key("height"));
    assert!(video_config.parameters.contains_key("vsync"));
}

#[cfg(feature = "w3d")]
#[tokio::test]
async fn test_device_config_builder_w3d() {
    let w3d_config = DeviceConfig::w3d()
        .with_parameter("max_lights", 8)
        .with_parameter("shader_quality", "High");

    assert_eq!(w3d_config.device_type, DeviceType::W3D);
    assert!(w3d_config.parameters.contains_key("max_lights"));
    assert!(w3d_config.parameters.contains_key("shader_quality"));
}

#[cfg(feature = "audio")]
#[tokio::test]
async fn test_audio_device_initialization() {
    let device_system = GameEngineDevice::new().await.unwrap();

    let audio_config = DeviceConfig::audio()
        .with_parameter("sample_rate", 44100)
        .with_parameter("channels", 2);

    let audio_result = device_system.init_audio_device(audio_config).await;

    // Audio device should initialize successfully
    assert!(
        audio_result.is_ok(),
        "Audio device should initialize: {:?}",
        audio_result.err()
    );

    let audio_device = audio_result.unwrap();
    let capabilities = audio_device.get_capabilities().await;

    // Basic capability checks
    assert!(capabilities.supported_sample_rates.contains(&44100));
    assert!(capabilities.max_channels >= 2);
}

#[cfg(feature = "video")]
#[tokio::test]
async fn test_video_device_initialization() {
    let device_system = GameEngineDevice::new().await.unwrap();

    let video_config = DeviceConfig::video()
        .with_parameter("width", 1920)
        .with_parameter("height", 1080);

    let video_result = device_system.init_video_device(video_config).await;

    // Video device should initialize successfully
    assert!(
        video_result.is_ok(),
        "Video device should initialize: {:?}",
        video_result.err()
    );

    let video_device = video_result.unwrap();
    let status = video_device.get_status().await.unwrap();

    assert_eq!(status.device_type, DeviceType::Video);
    assert!(status.initialized);
}

#[cfg(feature = "w3d")]
#[tokio::test]
async fn test_w3d_device_initialization() {
    let device_system = GameEngineDevice::new().await.unwrap();

    let w3d_config = DeviceConfig::w3d().with_parameter("max_lights", 8);

    let w3d_result = device_system.init_w3d_device(w3d_config).await;

    // W3D device should initialize successfully
    assert!(
        w3d_result.is_ok(),
        "W3D device should initialize: {:?}",
        w3d_result.err()
    );

    let w3d_device = w3d_result.unwrap();
    let status = w3d_device.get_status().await.unwrap();

    assert_eq!(status.device_type, DeviceType::W3D);
    assert!(status.initialized);
}

#[tokio::test]
async fn test_system_performance_metrics() {
    let device_system = GameEngineDevice::new().await.unwrap();

    let metrics = device_system.get_performance_metrics().await;
    assert!(metrics.is_ok(), "Should be able to get performance metrics");

    let metrics_map = metrics.unwrap();

    // Initially, metrics should be empty since no devices are initialized
    // But the call should succeed
    for (_device_type, perf_metrics) in metrics_map {
        assert!(perf_metrics.cpu_usage >= 0.0 && perf_metrics.cpu_usage <= 1.0);
        assert!(perf_metrics.memory_usage >= 0);
        assert!(perf_metrics.latency_ms >= 0.0);
        assert!(perf_metrics.throughput >= 0.0);
    }
}

#[tokio::test]
async fn test_system_shutdown() {
    let device_system = GameEngineDevice::new().await.unwrap();

    // Initialize some devices
    #[cfg(feature = "audio")]
    {
        let _audio = device_system
            .init_audio_device(DeviceConfig::audio())
            .await
            .ok();
    }

    #[cfg(feature = "video")]
    {
        let _video = device_system
            .init_video_device(DeviceConfig::video())
            .await
            .ok();
    }

    // Shutdown should succeed
    let shutdown_result = device_system.shutdown().await;
    assert!(shutdown_result.is_ok(), "System shutdown should succeed");
}

#[tokio::test]
async fn test_concurrent_device_access() {
    use std::sync::Arc;

    let device_system = Arc::new(GameEngineDevice::new().await.unwrap());

    let mut handles = Vec::new();

    // Test concurrent access to system status
    for _ in 0..10 {
        let system = device_system.clone();
        let handle = tokio::spawn(async move {
            let _status = system.get_system_status().await;
            let _metrics = system.get_performance_metrics().await;
        });
        handles.push(handle);
    }

    // Wait for all concurrent operations to complete
    for handle in handles {
        assert!(handle.await.is_ok(), "Concurrent operations should succeed");
    }
}

#[test]
fn test_version_info() {
    let version = version_info();
    assert!(!version.is_empty(), "Version info should not be empty");
    assert!(
        version.contains('.'),
        "Version should contain dot separator"
    );
}

#[tokio::test]
async fn test_device_capabilities() {
    let device_system = GameEngineDevice::new().await.unwrap();
    let status = device_system.get_system_status().await.unwrap();

    // Test that device capabilities are properly structured
    for device_status in status {
        assert!(
            device_status.capabilities.platform_features.len() >= 0,
            "Platform features should be a valid list"
        );

        // Performance metrics should have valid ranges
        let perf = &device_status.performance;
        assert!(
            perf.cpu_usage >= 0.0 && perf.cpu_usage <= 1.0,
            "CPU usage should be 0-1"
        );
        assert!(
            perf.memory_usage >= 0,
            "Memory usage should be non-negative"
        );
        assert!(perf.latency_ms >= 0.0, "Latency should be non-negative");
        assert!(perf.throughput >= 0.0, "Throughput should be non-negative");
    }
}

#[cfg(feature = "audio")]
mod audio_tests {
    use super::*;
    use game_engine_device::audio::*;

    #[tokio::test]
    async fn test_audio_formats() {
        let cd_quality = AudioFormat::cd_quality();
        assert_eq!(cd_quality.sample_rate, 44100);
        assert_eq!(cd_quality.channels, 2);
        assert_eq!(cd_quality.bits_per_sample, 16);

        let dvd_quality = AudioFormat::dvd_quality();
        assert_eq!(dvd_quality.sample_rate, 48000);

        // Test format compatibility
        assert!(cd_quality.is_compatible_with(cd_quality));
        assert!(!cd_quality.is_compatible_with(dvd_quality)); // Different sample rates
    }

    #[test]
    fn test_audio_handle() {
        let handle1 = AudioHandle::new();
        let handle2 = AudioHandle::new();

        assert_ne!(handle1, handle2, "Handles should be unique");
        assert!(handle1.is_valid(), "Handle should be valid");
        assert!(handle2.is_valid(), "Handle should be valid");

        let invalid = AudioHandle::INVALID;
        assert!(!invalid.is_valid(), "Invalid handle should not be valid");
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical > Priority::High);
        assert!(Priority::High > Priority::Normal);
        assert!(Priority::Normal > Priority::Low);

        let mut priorities = vec![
            Priority::Low,
            Priority::Critical,
            Priority::Normal,
            Priority::High,
        ];
        priorities.sort();

        assert_eq!(
            priorities,
            vec![
                Priority::Low,
                Priority::Normal,
                Priority::High,
                Priority::Critical
            ]
        );
    }
}

#[cfg(feature = "video")]
mod video_tests {
    use super::*;
    use game_engine_device::video::*;

    #[test]
    fn test_resolution() {
        let res = Resolution::hd_1080p();
        assert_eq!(res.width, 1920);
        assert_eq!(res.height, 1080);
        assert!((res.aspect_ratio() - 16.0 / 9.0).abs() < 0.01);
        assert_eq!(res.pixel_count(), 1920 * 1080);

        let res_4k = Resolution::uhd_4k();
        assert!(res_4k.pixel_count() > res.pixel_count());
    }

    #[test]
    fn test_refresh_rate() {
        let rate_60 = RefreshRate::rate_60hz();
        let rate_120 = RefreshRate::rate_120hz();

        assert_eq!(rate_60.hz, 60.0);
        assert_eq!(rate_120.hz, 120.0);
    }

    #[test]
    fn test_msaa_settings() {
        let none = MsaaSettings::none();
        assert!(!none.is_enabled());

        let msaa_4x = MsaaSettings::msaa_4x();
        assert!(msaa_4x.is_enabled());
        assert_eq!(msaa_4x.sample_count, 4);
    }
}

#[cfg(feature = "w3d")]
mod w3d_tests {
    use super::*;
    use game_engine_device::w3d::*;

    #[test]
    fn test_bounding_box() {
        let bbox = BoundingBox::new([-1.0, -1.0, -1.0], [1.0, 1.0, 1.0]);

        let center = bbox.center();
        assert_eq!(center, [0.0, 0.0, 0.0]);

        let size = bbox.size();
        assert_eq!(size, [2.0, 2.0, 2.0]);

        assert!(bbox.contains_point([0.0, 0.0, 0.0]));
        assert!(!bbox.contains_point([2.0, 0.0, 0.0]));
    }

    #[test]
    fn test_camera_default() {
        let camera = Camera::default();
        assert_eq!(camera.position, [0.0, 0.0, 0.0]);
        assert_eq!(camera.target, [0.0, 0.0, -1.0]);
        assert_eq!(camera.up, [0.0, 1.0, 0.0]);
        assert!((camera.fov - std::f32::consts::PI / 4.0).abs() < 0.01);
    }

    #[test]
    fn test_material_properties() {
        let props = MaterialProperties::default();
        assert_eq!(props.diffuse_color, [1.0, 1.0, 1.0, 1.0]);
        assert!(!props.transparent);
        assert!(!props.double_sided);
        assert_eq!(props.alpha_cutoff, 0.5);
    }
}

mod platform_tests {
    use super::*;
    use game_engine_device::platform::*;

    #[test]
    fn test_platform_detection() {
        let current = Platform::current();
        assert_ne!(current.name(), "");

        // Test feature support
        let supports_opengl = current.supports_feature(PlatformFeature::OpenGL);
        assert!(supports_opengl, "All platforms should support OpenGL");
    }

    #[test]
    fn test_cpu_architecture() {
        let arch = CpuArchitecture::current();
        assert_ne!(arch.name(), "");

        match arch {
            CpuArchitecture::X86_64 | CpuArchitecture::X86 => {
                assert!(arch.supports_simd(), "x86/x64 should support SIMD");
            }
            CpuArchitecture::Aarch64 | CpuArchitecture::Arm => {
                assert!(arch.supports_simd(), "ARM should support SIMD (NEON)");
            }
            _ => {}
        }
    }

    #[tokio::test]
    async fn test_device_interface_creation() {
        let interface_result = DeviceInterface::new().await;
        assert!(
            interface_result.is_ok(),
            "Device interface should initialize"
        );

        let interface = interface_result.unwrap();
        let capabilities = interface.get_capabilities();
        let system_info = interface.get_system_info();

        assert_eq!(capabilities.platform, Platform::current());
        assert_eq!(capabilities.architecture, CpuArchitecture::current());
        assert!(!system_info.os_name.is_empty());
    }
}
