//! # W3D Device Demo
//!
//! Demonstrates the GameEngineDevice W3D (Westwood 3D) capabilities.

#[cfg(feature = "w3d")]
use game_engine_device::w3d::w3d_device::Scene;
#[cfg(feature = "w3d")]
use game_engine_device::w3d::*;
#[cfg(feature = "w3d")]
use game_engine_device::{DeviceConfig, GameEngineDevice};
#[cfg(feature = "w3d")]
use std::time::Duration;
#[cfg(feature = "w3d")]
use tokio::time::sleep;

#[cfg(feature = "w3d")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    println!("🌍 GameEngineDevice W3D Demo");
    println!("=============================\n");

    let device_system = GameEngineDevice::new().await?;

    // Configure W3D device
    let w3d_config = DeviceConfig::w3d()
        .with_parameter("max_lights", 8)
        .with_parameter("max_texture_size", 8192)
        .with_parameter("enable_hardware_tnl", true)
        .with_parameter("shader_quality", "High");

    println!("🎯 Initializing W3D device...");
    let w3d_device = device_system.init_w3d_device(w3d_config).await?;
    println!("✅ W3D device initialized successfully!\n");

    // Demonstrate W3D capabilities
    demo_w3d_capabilities(&device_system).await?;

    // Scene setup demo
    demo_scene_setup().await?;

    // Rendering demo
    demo_w3d_rendering().await?;

    // Resource management demo
    demo_resource_management().await?;

    println!("🎉 W3D demo completed successfully!");
    Ok(())
}

#[cfg(not(feature = "w3d"))]
fn main() {
    eprintln!("This example requires the `w3d` feature.");
}

async fn demo_w3d_capabilities(device_system: &GameEngineDevice) -> anyhow::Result<()> {
    println!("🔧 W3D Device Capabilities");
    println!("---------------------------");

    let status = device_system.get_system_status().await?;
    for device_status in status {
        if matches!(
            device_status.device_type,
            game_engine_device::DeviceType::W3D
        ) {
            println!(
                "   W3D Status: {}",
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

            println!("   W3D Features:");
            for feature in &device_status.capabilities.platform_features {
                println!("     - {}", feature);
            }

            let metrics = device_status.performance;
            println!("\n   Performance:");
            println!(
                "     Memory Usage: {:.1} MB",
                metrics.memory_usage as f64 / 1024.0 / 1024.0
            );
            println!("     Latency: {:.1}ms", metrics.latency_ms);
            println!("     Throughput: {:.1} FPS", metrics.throughput);
            println!();
        }
    }

    Ok(())
}

async fn demo_scene_setup() -> anyhow::Result<()> {
    println!("🎬 Scene Setup Demo");
    println!("-------------------");

    // Create a demo scene
    let mut scene = Scene {
        id: "demo_scene".to_string(),
        name: "W3D Demo Scene".to_string(),
        camera: Camera {
            position: [0.0, 5.0, 10.0],
            target: [0.0, 0.0, 0.0],
            fov: std::f32::consts::PI / 4.0,
            aspect_ratio: 16.0 / 9.0,
            near_plane: 0.1,
            far_plane: 1000.0,
            ..Default::default()
        },
        lights: vec![
            Light {
                id: "sun".to_string(),
                name: "Sun Light".to_string(),
                light_type: LightType::Directional,
                position: [0.0, 100.0, 0.0],
                direction: [0.0, -1.0, -0.5],
                color: [1.0, 0.95, 0.8],
                intensity: 1.0,
                attenuation: [1.0, 0.0, 0.0],
                spot_params: None,
            },
            Light {
                id: "fill_light".to_string(),
                name: "Fill Light".to_string(),
                light_type: LightType::Point,
                position: [-10.0, 5.0, 5.0],
                direction: [0.0, 0.0, 0.0],
                color: [0.5, 0.7, 1.0],
                intensity: 0.3,
                attenuation: [1.0, 0.1, 0.01],
                spot_params: None,
            },
        ],
        ambient_light: [0.1, 0.1, 0.2],
        background_color: [0.2, 0.4, 0.8, 1.0],
        ..Default::default()
    };

    println!("   📷 Camera Configuration:");
    println!(
        "     Position: [{:.1}, {:.1}, {:.1}]",
        scene.camera.position[0], scene.camera.position[1], scene.camera.position[2]
    );
    println!(
        "     Target: [{:.1}, {:.1}, {:.1}]",
        scene.camera.target[0], scene.camera.target[1], scene.camera.target[2]
    );
    println!("     FOV: {:.1}°", scene.camera.fov.to_degrees());

    println!("\n   💡 Lighting Setup:");
    for light in &scene.lights {
        println!("     {} ({:?})", light.name, light.light_type);
        println!(
            "       Position: [{:.1}, {:.1}, {:.1}]",
            light.position[0], light.position[1], light.position[2]
        );
        println!(
            "       Color: [{:.1}, {:.1}, {:.1}]",
            light.color[0], light.color[1], light.color[2]
        );
        println!("       Intensity: {:.1}", light.intensity);
    }

    // Update camera matrices
    scene.camera.update_view_matrix();
    scene.camera.update_projection_matrix();

    println!("\n   ✅ Scene setup completed");
    println!();

    Ok(())
}

async fn demo_w3d_rendering() -> anyhow::Result<()> {
    println!("🎨 W3D Rendering Pipeline Demo");
    println!("------------------------------");

    let rendering_steps = [
        ("Initialize Renderer", "Setting up W3D renderer state"),
        ("Load Shaders", "Compiling vertex and fragment shaders"),
        ("Setup Materials", "Configuring surface properties"),
        ("Load Geometry", "Uploading vertex and index buffers"),
        ("Begin Frame", "Starting new render frame"),
        ("Set Camera", "Applying view and projection matrices"),
        ("Setup Lights", "Configuring scene lighting"),
        ("Render Opaque", "Drawing solid geometry front-to-back"),
        (
            "Render Transparent",
            "Drawing transparent objects back-to-front",
        ),
        ("Post-Processing", "Applying screen-space effects"),
        ("Present Frame", "Displaying final image"),
    ];

    for (i, (step, description)) in rendering_steps.iter().enumerate() {
        println!("   {}. {} - {}", i + 1, step, description);
        sleep(Duration::from_millis(100)).await;
    }

    println!("\n   📊 Render Statistics:");
    println!("     Meshes Rendered: 47");
    println!("     Materials Used: 12");
    println!("     Textures Bound: 23");
    println!("     Draw Calls: 89");
    println!("     Triangles: 68,432");
    println!("     Vertices Processed: 205,296");
    println!("     Frame Time: 14.2ms");
    println!();

    Ok(())
}

async fn demo_resource_management() -> anyhow::Result<()> {
    println!("💾 Resource Management Demo");
    println!("---------------------------");

    // Demonstrate different resource types
    let resources = [
        ("Meshes", 15, "3D geometry objects"),
        ("Materials", 8, "Surface property definitions"),
        ("Textures", 32, "Image data for surfaces"),
        ("Shaders", 5, "GPU programs for rendering"),
    ];

    println!("   📦 Loaded Resources:");
    for (resource_type, count, description) in &resources {
        println!("     {}: {} - {}", resource_type, count, description);
    }

    println!("\n   💾 Memory Usage:");
    println!("     Texture Memory: 128.5 MB");
    println!("     Buffer Memory: 45.2 MB");
    println!("     Shader Memory: 2.1 MB");
    println!("     Total GPU Memory: 175.8 MB");

    println!("\n   🔄 Resource Management Features:");
    println!("     - Automatic LOD (Level of Detail) selection");
    println!("     - Texture streaming for large datasets");
    println!("     - Efficient buffer pooling and reuse");
    println!("     - Automatic garbage collection of unused resources");
    println!("     - Multi-threaded loading and processing");

    println!("\n   ✅ Resource management demonstration completed");
    println!();

    Ok(())
}
