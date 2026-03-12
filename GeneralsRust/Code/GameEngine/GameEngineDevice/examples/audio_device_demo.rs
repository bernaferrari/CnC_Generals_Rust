//! # Audio Device Demo
//!
//! Demonstrates the GameEngineDevice audio capabilities with practical examples.

#[cfg(feature = "audio")]
use game_engine_device::{DeviceConfig, GameEngineDevice};
#[cfg(feature = "audio")]
use std::time::Duration;
#[cfg(feature = "audio")]
use tokio::time::sleep;

#[cfg(feature = "audio")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::init();

    println!("🎵 GameEngineDevice Audio Demo");
    println!("===============================\n");

    // Initialize the complete device system
    let device_system = GameEngineDevice::new().await?;

    // Configure audio with high performance settings
    let audio_config = DeviceConfig::audio()
        .with_parameter("sample_rate", 44100)
        .with_parameter("channels", 2)
        .with_parameter("buffer_size", 1024)
        .with_parameter("low_latency", true)
        .with_parameter("hardware_acceleration", true)
        .with_parameter("enable_3d", true);

    println!("🔧 Initializing audio device...");
    let audio_device = device_system.init_audio_device(audio_config).await?;
    println!("✅ Audio device initialized successfully!\n");

    // Get device capabilities
    let capabilities = audio_device.get_capabilities().await;
    println!("📊 Audio Device Capabilities:");
    println!("   Hardware Mixing: {}", capabilities.hardware_mixing);
    println!("   3D Audio: {}", capabilities.hardware_3d);
    println!("   Max Channels: {}", capabilities.max_channels);
    println!("   Latency: {:.1}ms", capabilities.latency_ms);
    println!(
        "   Supported Formats: {} formats\n",
        capabilities.supported_formats.len()
    );

    // Enumerate available audio devices
    println!("🔍 Available Audio Devices:");
    let devices = audio_device.get_available_devices().await;
    for (i, device) in devices.iter().enumerate() {
        println!(
            "   {}. {} {}",
            i + 1,
            device.name,
            if device.is_default { "(Default)" } else { "" }
        );
    }
    println!();

    // Demonstrate audio playback capabilities
    demo_audio_playback(&device_system).await?;

    // Demonstrate 3D audio
    demo_3d_audio(&device_system).await?;

    // Show performance metrics
    demo_performance_metrics(&device_system).await?;

    println!("🎉 Audio demo completed successfully!");
    Ok(())
}

#[cfg(not(feature = "audio"))]
fn main() {
    eprintln!("This example requires the `audio` feature.");
}

async fn demo_audio_playback(device_system: &GameEngineDevice) -> anyhow::Result<()> {
    println!("🔊 Audio Playback Demo");
    println!("----------------------");

    // Note: In a real implementation, these would be actual audio files
    let demo_sounds = [
        ("explosion.wav", "Explosion sound effect"),
        ("music.mp3", "Background music"),
        ("voice.wav", "Character voice"),
    ];

    for (filename, description) in &demo_sounds {
        println!("   Playing: {} - {}", filename, description);

        // In a real implementation, this would actually play audio
        println!("   ▶️  [Simulated playback - {} seconds]", 2);
        sleep(Duration::from_millis(500)).await; // Simulate playback time

        println!("   ✅ Playback completed\n");
    }

    Ok(())
}

async fn demo_3d_audio(device_system: &GameEngineDevice) -> anyhow::Result<()> {
    println!("🌍 3D Audio Demo");
    println!("----------------");

    // Simulate moving audio source
    let positions = [
        ([-10.0, 0.0, 0.0], "Left"),
        ([0.0, 0.0, -10.0], "Front"),
        ([10.0, 0.0, 0.0], "Right"),
        ([0.0, 0.0, 10.0], "Behind"),
    ];

    println!("   Simulating 3D positioned audio source...");

    for (pos, direction) in &positions {
        println!(
            "   🎯 Position: {} [{:.1}, {:.1}, {:.1}]",
            direction, pos[0], pos[1], pos[2]
        );

        // In a real implementation, this would update 3D audio position
        sleep(Duration::from_millis(300)).await;
    }

    println!("   ✅ 3D audio demonstration completed\n");
    Ok(())
}

async fn demo_performance_metrics(device_system: &GameEngineDevice) -> anyhow::Result<()> {
    println!("📈 Performance Metrics");
    println!("----------------------");

    let status = device_system.get_system_status().await?;
    let metrics = device_system.get_performance_metrics().await?;

    for device_status in status {
        println!("   Device: {:?}", device_status.device_type);
        println!(
            "   Status: {}",
            if device_status.active {
                "Active"
            } else {
                "Inactive"
            }
        );

        if let Some(audio_metrics) = metrics.get(&device_status.device_type) {
            println!("   CPU Usage: {:.1}%", audio_metrics.cpu_usage * 100.0);
            println!(
                "   Memory Usage: {:.1} MB",
                audio_metrics.memory_usage as f64 / 1024.0 / 1024.0
            );
            println!("   Latency: {:.1}ms", audio_metrics.latency_ms);
            println!("   Throughput: {:.1} ops/sec", audio_metrics.throughput);
        }
        println!();
    }

    Ok(())
}
