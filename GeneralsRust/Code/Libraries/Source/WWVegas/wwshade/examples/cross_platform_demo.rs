// Cross-Platform Demo: Same API, Works Everywhere
// This shows how your existing WWShade code gets cross-platform support automatically

use wwshade::{ShdDefClassId, WWShadeApiCompatibility, YourExistingGameEngine};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🌍 WWShade Cross-Platform Demo");
    println!("==============================");
    println!("Platform: {}", std::env::consts::OS);
    println!("Architecture: {}", std::env::consts::ARCH);

    // Step 1: Initialize WWShade (detects best backend for this platform)
    println!("\n🚀 Initializing WWShade...");
    WWShadeApiCompatibility::initialize().await?;
    println!("Backend: {}", WWShadeApiCompatibility::get_platform_info());

    // Step 2: Your existing game engine code works unchanged!
    println!("\n🎮 Creating game engine...");
    let mut engine = YourExistingGameEngine::new().await?;

    // Step 3: Add materials using the same API you always use
    println!("\n🎨 Adding materials (same API as always)...");
    engine.add_material(ShdDefClassId::Simple)?;
    engine.add_material(ShdDefClassId::BumpDiff)?;
    // engine.add_material(ShdDefClassId::Cubemap)?; // Would work too

    // Step 4: Render frames using the same API
    println!("\n🖼️  Rendering frames...");
    for frame in 0..3 {
        engine.render_frame()?;
        println!("   Frame {} rendered", frame + 1);
    }

    // Step 5: Show what this means for your codebase
    println!("\n✨ What this means for your codebase:");

    if cfg!(target_os = "macos") {
        println!("   🍎 macOS: Using Metal backend (native performance)");
        println!("   • Same WWShade API calls");
        println!("   • Native macOS Metal rendering");
        println!("   • No DirectX dependencies");
        println!("   • Full shader compatibility");
    } else if cfg!(target_os = "linux") {
        println!("   🐧 Linux: Using Vulkan backend (high performance)");
        println!("   • Same WWShade API calls");
        println!("   • Native Linux Vulkan rendering");
        println!("   • No DirectX dependencies");
        println!("   • Full shader compatibility");
    } else if cfg!(target_os = "windows") {
        println!("   🪟 Windows: Can use Vulkan, DX12, or fallback to legacy DX8");
        println!("   • Same WWShade API calls");
        println!("   • Modern Vulkan/DX12 for new hardware");
        println!("   • Legacy DirectX 6/7/8 for old hardware");
        println!("   • Automatic best backend selection");
    } else {
        println!("   🌐 Other platform: WebGPU or custom backend");
    }

    println!("\n🎯 Key Benefits:");
    println!("   ✅ Zero code changes to your existing WWShade calls");
    println!("   ✅ Automatic platform detection");
    println!("   ✅ Modern GPU performance where available");
    println!("   ✅ Graceful fallback to legacy DirectX");
    println!("   ✅ Cross-platform compatibility");
    println!("   ✅ Same shader functionality everywhere");

    println!("\n🔮 Future: WGSL Shaders");
    println!("   Your existing HLSL shaders (DX6/7/8) still work, but now you can also use:");
    println!("   • WGSL shaders (modern, cross-platform)");
    println!("   • Automatic HLSL → WGSL conversion");
    println!("   • WebGPU compatibility for web games");

    Ok(())
}

// Example: How your existing rendering code stays the same
fn your_existing_rendering_loop() {
    println!("\n📝 Your existing rendering loop (unchanged):");
    println!(
        r#"
    // This is exactly how you use WWShade now:
    
    // Initialize (same API)
    WWShade::initialize();
    
    // Create shaders (same API)
    let simple_shader = create_shader(SIMPLE_SHADER);
    let bump_shader = create_shader(BUMP_MAPPING_SHADER);
    
    // Rendering loop (same API)
    for pass in 0..shader.get_pass_count() {{
        shader.apply_shared(pass);
        shader.apply_instance(pass);
        // Your mesh rendering...
    }}
    
    // BUT NOW IT WORKS ON MAC AND LINUX TOO! 🎉
    "#
    );
}
