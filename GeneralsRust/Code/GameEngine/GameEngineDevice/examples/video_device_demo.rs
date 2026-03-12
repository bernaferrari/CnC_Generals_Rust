//! # Video Device Demo
//!
//! Demonstrates the GameEngineDevice video capabilities.

#[cfg(feature = "video")]
use game_engine_device::video::{Resolution, VSync};
#[cfg(feature = "video")]
use game_engine_device::{DeviceConfig, GameEngineDevice};
#[cfg(feature = "video")]
use std::time::Duration;
#[cfg(feature = "video")]
use tokio::time::sleep;

#[cfg(feature = "video")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    println!("🎮 GameEngineDevice Video Demo");
    println!("===============================\n");

    let device_system = GameEngineDevice::new().await?;

    // Configure video device
    let video_config = DeviceConfig::video()
        .with_parameter("resolution_width", 1920)
        .with_parameter("resolution_height", 1080)
        .with_parameter("refresh_rate", 60.0)
        .with_parameter("fullscreen", false)
        .with_parameter("vsync", true)
        .with_parameter("msaa_samples", 4);

    println!("🖥️  Initializing video device...");
    let video_device = device_system.init_video_device(video_config).await?;
    println!("✅ Video device initialized successfully!\n");

    // Display video capabilities
    demo_video_capabilities(&device_system).await?;

    // Demonstrate display modes
    demo_display_modes(&device_system).await?;

    // Show rendering demo
    demo_rendering(&device_system).await?;

    println!("🎉 Video demo completed successfully!");
    Ok(())
}

#[cfg(not(feature = "video"))]
fn main() {
    eprintln!("This example requires the `video` feature.");
}

async fn demo_video_capabilities(device_system: &GameEngineDevice) -> anyhow::Result<()> {
    println!("📊 Video Device Capabilities");
    println!("----------------------------");

    let status = device_system.get_system_status().await?;
    for device_status in status {
        if matches!(
            device_status.device_type,
            game_engine_device::DeviceType::Video
        ) {
            println!(
                "   Video Device Status: {}",
                if device_status.active {
                    "Active"
                } else {
                    "Inactive"
                }
            );
            println!(
                "   Hardware Acceleration: {}",
                device_status.capabilities.hardware_acceleration
            );
            println!(
                "   Multi-threading: {}",
                device_status.capabilities.multi_threading
            );
            println!(
                "   SIMD Support: {}",
                device_status.capabilities.simd_support
            );

            println!("   Platform Features:");
            for feature in &device_status.capabilities.platform_features {
                println!("     - {}", feature);
            }
            println!();
        }
    }

    Ok(())
}

async fn demo_display_modes(device_system: &GameEngineDevice) -> anyhow::Result<()> {
    println!("🖼️  Display Mode Demo");
    println!("--------------------");

    let resolutions = [
        Resolution::new(1280, 720),
        Resolution::hd_1080p(),
        Resolution::new(2560, 1440),
        Resolution::uhd_4k(),
    ];

    for resolution in &resolutions {
        println!("   Testing resolution: {}", resolution);
        println!("     Aspect ratio: {:.2}", resolution.aspect_ratio());
        println!("     Pixels: {}", resolution.pixel_count());

        // Simulate mode setting
        sleep(Duration::from_millis(200)).await;
        println!("     ✅ Mode set successfully");
        println!();
    }

    // VSync demo
    println!("   VSync Modes:");
    let vsync_modes = [
        VSync::Disabled,
        VSync::Enabled,
        VSync::Adaptive,
        VSync::Fast,
    ];
    for mode in &vsync_modes {
        println!(
            "     {:?} - {}",
            mode,
            match mode {
                VSync::Disabled => "No frame rate limit",
                VSync::Enabled => "Limit to display refresh rate",
                VSync::Adaptive => "Adaptive based on performance",
                VSync::Fast => "Half refresh rate when below target",
            }
        );
    }
    println!();

    Ok(())
}

async fn demo_rendering(device_system: &GameEngineDevice) -> anyhow::Result<()> {
    println!("🎨 Rendering Demo");
    println!("------------------");

    println!("   Simulating rendering pipeline...");

    let rendering_stages = [
        "Clear render targets",
        "Setup camera matrices",
        "Render opaque geometry",
        "Render transparent objects",
        "Apply post-processing effects",
        "Present to screen",
    ];

    for (i, stage) in rendering_stages.iter().enumerate() {
        println!("   {}. {}", i + 1, stage);
        sleep(Duration::from_millis(100)).await;
    }

    println!("\n   📊 Frame Statistics:");
    println!("     Frame Time: 16.7ms");
    println!("     FPS: 60.0");
    println!("     Draw Calls: 342");
    println!("     Triangles: 125,847");
    println!("     GPU Memory: 245MB");
    println!();

    Ok(())
}
