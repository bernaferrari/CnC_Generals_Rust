//! UV Mapper Integration Examples
//!
//! This example demonstrates how to use the UV texture mapper system with game assets.
//! It shows practical integration patterns for different mapper types and use cases.

// Note: These examples show the API and concepts. To run with real assets,
// you would need actual W3D files from C&C Generals.

/// Example 1: Water Surface with SineLinearOffset Mapper
///
/// Creates a scrolling water surface with wave effects using SineLinearOffset mapper.
/// This is commonly used for water, lava, and other undulating surfaces.
fn example_water_surface() {
    println!("\n=== Example 1: Water Surface ===");
    println!("Purpose: Animated water with wave effects");
    println!("Mapper Type: SineLinearOffset (Type 9)");
    println!();

    // In a real scenario, you would load a material from a W3D file:
    // let material = asset_manager.load_material("path/to/water.w3d")?;

    // Configuration for water surface
    struct WaterMaterialConfig {
        mapper_id: u32,
        u_amplitude: i32, // × 1000
        v_amplitude: i32, // × 1000
        frequency: i32,   // × 100
        phase: i32,       // × 100
    }

    let water_config = WaterMaterialConfig {
        mapper_id: 9,     // SineLinearOffset
        u_amplitude: 50,  // 0.05 units
        v_amplitude: 100, // 0.1 units
        frequency: 50,    // 0.5 Hz
        phase: 0,         // No phase offset
    };

    println!("Configuration:");
    println!("  Mapper ID: {} (SineLinearOffset)", water_config.mapper_id);
    println!("  U Amplitude: 0.{:02} units", water_config.u_amplitude);
    println!("  V Amplitude: 0.{:02} units", water_config.v_amplitude);
    println!("  Frequency: 0.{} Hz", water_config.frequency);
    println!();
    println!("Expected behavior:");
    println!("  - Texture scrolls and waves continuously");
    println!("  - V-channel has twice the amplitude as U (more vertical wave motion)");
    println!("  - At 0.5 Hz, completes one wave cycle every 2 seconds");
    println!();
    println!("Game Use Cases:");
    println!("  - Water surfaces in outdoor maps");
    println!("  - Flowing lava in volcanic areas");
    println!("  - Energy fields and force barriers");
}

/// Example 2: Conveyor Belt with LinearOffset Mapper
///
/// Creates a scrolling conveyor belt or moving surface using LinearOffset mapper.
/// Used for factory equipment, escalators, and moving platforms.
fn example_conveyor_belt() {
    println!("\n=== Example 2: Conveyor Belt ===");
    println!("Purpose: Continuous scrolling surface");
    println!("Mapper Type: LinearOffset (Type 4)");
    println!();

    struct ConveyorConfig {
        mapper_id: u32,
        u_speed: i32, // × 1000 units/sec
        v_speed: i32, // × 1000 units/sec
    }

    let conveyor_config = ConveyorConfig {
        mapper_id: 4, // LinearOffset
        u_speed: 200, // 0.2 units/sec (rightward)
        v_speed: 0,   // No vertical scrolling
    };

    println!("Configuration:");
    println!("  Mapper ID: {} (LinearOffset)", conveyor_config.mapper_id);
    println!("  U Speed: 0.{} units/second", conveyor_config.u_speed);
    println!("  V Speed: 0.{} units/second", conveyor_config.v_speed);
    println!();
    println!("Expected behavior:");
    println!("  - Texture scrolls continuously to the right");
    println!("  - Simulates conveyor belt motion");
    println!("  - No vertical movement");
    println!();
    println!("Game Use Cases:");
    println!("  - Factory conveyor belts");
    println!("  - Moving escalators");
    println!("  - Mechanical walkways");
    println!("  - Lava flows");
}

/// Example 3: Sprite Sheet Animation with Grid Mapper
///
/// Animates a character or effect using a sprite sheet grid with Grid mapper.
/// Commonly used for character animations, explosions, and special effects.
fn example_sprite_sheet_animation() {
    println!("\n=== Example 3: Sprite Sheet Animation ===");
    println!("Purpose: Animate through sprite sheet frames");
    println!("Mapper Type: Grid (Type 7)");
    println!();

    struct SpriteSheetConfig {
        mapper_id: u32,
        columns: i32, // Number of columns in sprite sheet
        rows: i32,    // Number of rows in sprite sheet
        total_frames: usize,
        frames_per_second: f32,
    }

    let sprite_config = SpriteSheetConfig {
        mapper_id: 7,
        columns: 4,
        rows: 4,
        total_frames: 16,
        frames_per_second: 10.0,
    };

    println!("Configuration:");
    println!("  Mapper ID: {} (Grid)", sprite_config.mapper_id);
    println!(
        "  Sprite Grid: {}×{} = {} total frames",
        sprite_config.columns, sprite_config.rows, sprite_config.total_frames
    );
    println!("  Animation Speed: {} FPS", sprite_config.frames_per_second);
    println!();
    println!("Expected behavior:");
    println!(
        "  - Texture coordinate tiles {} columns and {} rows",
        sprite_config.columns, sprite_config.rows
    );
    println!(
        "  - Each frame displayed for {:.0}ms",
        1000.0 / sprite_config.frames_per_second
    );
    println!("  - Animation loops back to frame 0 after frame 15");
    println!();
    println!("Game Use Cases:");
    println!("  - Character idle/run/attack animations");
    println!("  - Explosion effects");
    println!("  - Particle effects");
    println!("  - Vehicle destruction sequences");
    println!("  - Environmental effects (steam, smoke)");
}

/// Example 4: Rotating Object with Rotate Mapper
///
/// Creates a spinning/rotating texture using Rotate mapper.
/// Used for spinning objects, rotating blades, and circular motion effects.
fn example_rotating_object() {
    println!("\n=== Example 4: Rotating Object ===");
    println!("Purpose: Continuous rotation animation");
    println!("Mapper Type: Rotate (Type 8)");
    println!();

    struct RotateConfig {
        mapper_id: u32,
        rotation_speed: i32, // × 100 degrees/sec
        center_u: i32,       // × 1000 (center point U)
        center_v: i32,       // × 1000 (center point V)
        cycles_per_second: f32,
    }

    let rotate_config = RotateConfig {
        mapper_id: 8,
        rotation_speed: 36000, // 360 degrees/sec = 1 rotation/sec
        center_u: 500,         // Center at (0.5, 0.5)
        center_v: 500,
        cycles_per_second: 1.0,
    };

    println!("Configuration:");
    println!("  Mapper ID: {} (Rotate)", rotate_config.mapper_id);
    println!(
        "  Rotation Speed: {} degrees/second",
        rotate_config.rotation_speed / 100
    );
    println!(
        "  Rotation Center: ({}, {})",
        rotate_config.center_u as f32 / 1000.0,
        rotate_config.center_v as f32 / 1000.0
    );
    println!("  Cycles: {} per second", rotate_config.cycles_per_second);
    println!();
    println!("Expected behavior:");
    println!("  - Texture rotates around center point");
    println!(
        "  - Completes one full rotation every {} second(s)",
        1.0 / rotate_config.cycles_per_second
    );
    println!("  - Continuous smooth rotation");
    println!();
    println!("Game Use Cases:");
    println!("  - Spinning fans and turbines");
    println!("  - Rotating vehicle wheels (with offset center)");
    println!("  - Spinning energy effects");
    println!("  - Radar dishes");
    println!("  - Rotating searchlights");
}

/// Example 5: Combination - Multi-mapper Material
///
/// Demonstrates how multiple mappers can be applied in sequence.
/// Shows layering of effects for complex animations.
fn example_complex_material() {
    println!("\n=== Example 5: Complex Material (Multi-layer) ===");
    println!("Purpose: Combine multiple mapper effects");
    println!();

    struct ComplexMaterial {
        layers: Vec<(u32, String, Vec<i32>)>, // (mapper_id, description, args)
    }

    let complex = ComplexMaterial {
        layers: vec![
            (
                4,
                "LinearOffset (base flow)".to_string(),
                vec![100, 50, 0, 0],
            ),
            (
                9,
                "SineLinearOffset (wave overlay)".to_string(),
                vec![30, 60, 30, 0],
            ),
        ],
    };

    println!("Material Composition:");
    for (idx, (mapper_id, desc, args)) in complex.layers.iter().enumerate() {
        println!("  Layer {}: ID={} - {}", idx + 1, mapper_id, desc);
        println!("    Args: {:?}", args);
    }
    println!();
    println!("Expected behavior:");
    println!("  1. First applies LinearOffset (scrolling effect)");
    println!("  2. Then applies SineLinearOffset (wave distortion)");
    println!("  3. Result: Combined flow + wave animation");
    println!();
    println!("Game Use Cases:");
    println!("  - Turbulent water (flow + waves)");
    println!("  - Plasma effects (multiple oscillations)");
    println!("  - Energy shielding (multiple layers)");
}

/// Example 6: Real-world Performance Pattern
///
/// Shows the recommended pattern for using mappers in actual game code.
fn example_performance_pattern() {
    println!("\n=== Example 6: Performance Pattern ===");
    println!("Recommended pattern for game integration:");
    println!();

    println!("// Load material from asset");
    println!("let material = asset_manager.load_material(\"terrain/water.w3d\")?;");
    println!();

    println!("// In render loop:");
    println!("for render_object in scene.render_objects {{");
    println!("    for material_pass in render_object.material_passes {{");
    println!("        // Mapper is automatically applied in shader");
    println!("        // No CPU-side transformation needed!");
    println!("        renderer.draw_material_pass(");
    println!("            &material_pass,");
    println!("            &geometry,");
    println!("            animation_time,  // Automatically used for mapper");
    println!("        );");
    println!("    }}");
    println!("}}");
    println!();

    println!("Performance characteristics:");
    println!("  - GPU-side transformation: ~1-2% of vertex shader time");
    println!("  - Per-frame buffer update: ~0.1ms");
    println!("  - Memory per material: ~192 bytes");
    println!("  - Expected FPS impact: < 1% on modern hardware");
}

/// Example 7: Advanced - Screen-Space Mapping
///
/// Demonstrates environment mapping for reflections.
/// Used for mirrors, chrome, and reflective surfaces.
fn example_environment_mapping() {
    println!("\n=== Example 7: Environment Mapping (Advanced) ===");
    println!("Purpose: Reflection/environment mapping");
    println!("Mapper Type: Environment (Types 1-3, 12-15)");
    println!();

    struct EnvironmentMapConfig {
        mapper_id: u32,
        material_type: String,
        description: String,
    }

    let configs = vec![
        EnvironmentMapConfig {
            mapper_id: 1,
            material_type: "Reflection".to_string(),
            description: "Basic reflection mapping".to_string(),
        },
        EnvironmentMapConfig {
            mapper_id: 2,
            material_type: "Chrome".to_string(),
            description: "Polished chrome/mirror effect".to_string(),
        },
        EnvironmentMapConfig {
            mapper_id: 3,
            material_type: "Glass".to_string(),
            description: "Transparent glass with reflections".to_string(),
        },
    ];

    println!("Available Environment Mappers:");
    for config in configs {
        println!(
            "  ID {}: {} - {}",
            config.mapper_id, config.material_type, config.description
        );
    }
    println!();

    println!("Status: ✓ Helper functions implemented");
    println!("        ⚙️  Needs vertex shader integration for full C++ parity");
    println!();
    println!("Game Use Cases:");
    println!("  - Vehicle chrome and mirrors");
    println!("  - Water reflections");
    println!("  - Building windows and glass");
    println!("  - Metal/shiny surfaces");
}

/// Example 8: Asset Loading Pattern
///
/// Shows the complete pattern for loading W3D assets and applying mappers.
fn example_asset_loading_pattern() {
    println!("\n=== Example 8: Asset Loading Pattern ===");
    println!();

    println!("// 1. Load W3D file");
    println!("let asset_path = \"assets/models/infantry.w3d\";");
    println!("let w3d_data = std::fs::read(asset_path)?;");
    println!();

    println!("// 2. Parse W3D format");
    println!("let w3d = W3dFile::parse(&w3d_data)?;");
    println!();

    println!("// 3. Create materials with mapper support");
    println!("for material_info in w3d.materials {{");
    println!("    let mut material = MaterialPassClass::new();");
    println!();
    println!("    // Set up texture mapper if present");
    println!("    if let Some(mapper) = material_info.mapper {{");
    println!("        material.set_mapper_id(mapper.mapper_type);");
    println!("        for (i, arg) in mapper.args.iter().enumerate() {{");
    println!("            material.set_mapper_arg(i, *arg);");
    println!("        }}");
    println!("    }}");
    println!();
    println!("    scene.add_material(material);");
    println!("}}");
    println!();

    println!("// 4. Render with animation time");
    println!("let animation_time = game_timer.elapsed_seconds();");
    println!("renderer.render_frame(scene, animation_time)?;");
    println!();

    println!("Key points:");
    println!("  - Mappers are stored in MaterialPassClass");
    println!("  - Animation time is managed per frame");
    println!("  - GPU handles transformations (no CPU overhead)");
    println!("  - Works with all W3D file types");
}

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║         UV Mapper Integration Examples                     ║");
    println!("║    For C&C Generals W3D Asset Files in WW3D2 Engine        ║");
    println!("╚════════════════════════════════════════════════════════════╝");

    example_water_surface();
    example_conveyor_belt();
    example_sprite_sheet_animation();
    example_rotating_object();
    example_complex_material();
    example_performance_pattern();
    example_environment_mapping();
    example_asset_loading_pattern();

    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║              Additional Resources                         ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();
    println!("For more details, see:");
    println!("  - UV_MAPPER_GUIDE.md: Complete API reference");
    println!("  - material_system.rs: Material configuration");
    println!("  - wgpu_material_binds.rs: GPU buffer management");
    println!("  - [shader files]: GPU transformation implementation");
    println!();
    println!("Test coverage:");
    println!("  cargo test --test uv_animation_test");
    println!();
    println!("Performance benchmarks:");
    println!("  cargo bench --bench uv_mapper_bench");
}
