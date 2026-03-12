//! Example demonstrating embedded shader access
//!
//! This example shows how to use the embedded shader system to access
//! original DirectX shader source files at runtime.

use wwshade::{DirectXVersion, EmbeddedShaders, ShaderType};

fn main() {
    println!("WWShade Embedded Shader System Demo");
    println!("==================================");

    // Display total shader count
    println!(
        "\nTotal embedded shaders: {}",
        EmbeddedShaders::shader_count()
    );

    // List all available shaders
    println!("\nAvailable Shaders:");
    let all_shaders = EmbeddedShaders::list_all_shaders();
    for shader_key in &all_shaders {
        println!(
            "  - {:?} {:?}: {}",
            shader_key.dx_version, shader_key.shader_type, shader_key.name
        );
    }

    // Demonstrate accessing specific shaders
    println!("\n=== Shader Access Examples ===");

    // Get a DX6 vertex shader
    println!("\n1. Accessing DX6 bump diffuse vertex shader:");
    match EmbeddedShaders::get_vertex_shader(DirectXVersion::DX6, "shd6bumpdiff") {
        Ok(shader) => {
            println!("   Name: {}", shader.name());
            println!("   Size: {} bytes", shader.size());
            println!("   First few lines:");
            for (i, line) in shader.source().lines().take(5).enumerate() {
                println!("   {:2}: {}", i + 1, line);
            }
        }
        Err(e) => println!("   Error: {}", e),
    }

    // Get a DX8 pixel shader
    println!("\n2. Accessing DX8 bump diffuse pixel shader:");
    match EmbeddedShaders::get_pixel_shader(DirectXVersion::DX8, "shd8bumpdiff") {
        Ok(shader) => {
            println!("   Name: {}", shader.name());
            println!("   Size: {} bytes", shader.size());
            println!("   First few lines:");
            for (i, line) in shader.source().lines().take(5).enumerate() {
                println!("   {:2}: {}", i + 1, line);
            }
        }
        Err(e) => println!("   Error: {}", e),
    }

    // Demonstrate using convenience macros
    println!("\n3. Using convenience macros:");
    match wwshade::get_dx7_vertex_shader!("shd7bumpdiffpass0") {
        Ok(shader) => {
            println!(
                "   DX7 shader '{}' loaded successfully ({} bytes)",
                shader.name(),
                shader.size()
            );
        }
        Err(e) => println!("   Error: {}", e),
    }

    // Show shaders by DirectX version
    println!("\n=== Shaders by DirectX Version ===");

    for dx_version in [
        DirectXVersion::DX6,
        DirectXVersion::DX7,
        DirectXVersion::DX8,
    ] {
        let shaders = EmbeddedShaders::list_shaders(dx_version);
        println!("\n{:?} Shaders ({} total):", dx_version, shaders.len());
        for shader_key in shaders {
            println!("  - {:?}: {}", shader_key.shader_type, shader_key.name);
        }
    }

    // Demonstrate error handling
    println!("\n=== Error Handling Example ===");
    match EmbeddedShaders::get_vertex_shader(DirectXVersion::DX8, "nonexistent_shader") {
        Ok(_) => println!("Unexpected success!"),
        Err(e) => println!("Expected error: {}", e),
    }

    println!("\n=== Summary ===");
    println!(
        "The embedded shader system successfully loaded {} shaders",
        all_shaders.len()
    );
    println!("from the original DirectX shader source files.");
    println!("These can now be used for runtime HLSL compilation or");
    println!("future translation to WGSL for modern graphics APIs.");
}
