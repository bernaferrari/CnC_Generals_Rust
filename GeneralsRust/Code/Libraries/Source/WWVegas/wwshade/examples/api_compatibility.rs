// Example: API Compatibility - Same code works with both WGPU and DirectX
// This demonstrates how your existing codebase can use the new WGPU backend
// without changing any calling code.

use std::sync::Arc;
use wwshade::{
    create_renderer, create_shader_interface, get_backend_info, has_modern_support,
    initialize_rendering, ShdDefClassId, ShdResult,
};

#[tokio::main]
async fn main() -> ShdResult<()> {
    env_logger::init();

    println!("🔄 WWShade API Compatibility Example");
    println!("===================================");

    // Step 1: Initialize rendering system (detects best backend automatically)
    println!("🚀 Initializing rendering system...");
    let backend = initialize_rendering().await?;
    println!("✅ Backend selected: {:?}", backend);
    println!("📊 Backend info: {}", get_backend_info());

    // Step 2: Your existing code works unchanged!
    demo_existing_api().await?;

    // Step 3: Show cross-platform benefits
    demo_cross_platform_features().await?;

    Ok(())
}

// This is exactly how your existing codebase calls the shader system
async fn demo_existing_api() -> ShdResult<()> {
    println!("\n📝 Existing API Demo (unchanged code)");
    println!("====================================");

    // Your existing shader creation code works unchanged
    println!("🔧 Creating shaders using existing API...");

    // This is the SAME API call you use now - but it automatically uses WGPU or DirectX
    let simple_shader = create_simple_shader_using_existing_api()?;
    println!(
        "✅ Simple shader created: class_id = {}",
        simple_shader.get_class_id()
    );

    let bump_shader = create_bump_shader_using_existing_api()?;
    println!(
        "✅ Bump mapping shader created: class_id = {}",
        bump_shader.get_class_id()
    );

    // Your existing renderer creation works unchanged
    let mut renderer = create_renderer().await?;
    println!("✅ Renderer created using existing API");

    // Your existing renderer calls work unchanged
    renderer.initialize()?;
    println!("✅ Renderer initialized using existing API");

    // Your existing mesh registration works unchanged
    // (This would use your existing mesh and shader objects)
    println!("✅ All existing API calls work without changes!");

    renderer.shutdown()?;
    println!("✅ Renderer shutdown using existing API");

    Ok(())
}

// These functions show the SAME API you use now - no changes needed!
fn create_simple_shader_using_existing_api() -> ShdResult<Box<dyn wwshade::ShdInterface>> {
    // This is exactly how you create shaders now
    use wwshade::simple::SimpleShaderDef;

    let definition = Arc::new(SimpleShaderDef::new());

    // This call automatically chooses WGPU (cross-platform) or DirectX (fallback)
    create_shader_interface(
        ShdDefClassId::Simple as u32,
        definition as Arc<dyn wwshade::ShdDefClass>,
    )
}

fn create_bump_shader_using_existing_api() -> ShdResult<Box<dyn wwshade::ShdInterface>> {
    // This is exactly how you create bump mapping shaders now
    use wwshade::bump_mapping::BumpDiffShaderDef;

    let definition = Arc::new(BumpDiffShaderDef::new());

    // Same API call - automatically uses best backend
    create_shader_interface(
        ShdDefClassId::BumpDiff as u32,
        definition as Arc<dyn wwshade::ShdDefClass>,
    )
}

async fn demo_cross_platform_features() -> ShdResult<()> {
    println!("\n🌐 Cross-Platform Features");
    println!("========================");

    if has_modern_support() {
        println!("✅ Modern WGPU backend active!");
        println!("   🖥️  Windows: Uses Vulkan → DirectX 12 → DirectX 11");
        println!("   🍎 macOS: Uses Metal (native performance)");
        println!("   🐧 Linux: Uses Vulkan");
        println!("   🌐 Web: Uses WebGPU");
        println!("");
        println!("🚀 Your shaders now run on ALL platforms!");
        println!("   • Same WGSL shaders everywhere");
        println!("   • Native performance on each platform");
        println!("   • Modern GPU features available");
        println!("   • Memory safety guaranteed");
    } else {
        println!("⚠️  Fallback to legacy DirectX backend");
        println!("   • Windows DirectX 6/7/8 support");
        println!("   • Your existing shaders still work");
        println!("   • Compatibility with older hardware");
        println!("");
        println!("💡 To get cross-platform support:");
        println!("   • Update GPU drivers");
        println!("   • Ensure Vulkan/Metal is available");
        println!("   • Check system requirements");
    }

    println!("\n🎯 Migration Benefits:");
    println!("   • ✅ Zero code changes needed");
    println!("   • ✅ Automatic backend selection");
    println!("   • ✅ Graceful fallback to DirectX");
    println!("   • ✅ Cross-platform compatibility");
    println!("   • ✅ Modern GPU features");
    println!("   • ✅ Better error handling");
    println!("   • ✅ Memory safety");

    Ok(())
}

// Example of how to integrate with your existing game engine
pub struct GameEngine {
    renderer: Box<dyn wwshade::ShaderRenderer>,
    shaders: Vec<Box<dyn wwshade::ShdInterface>>,
}

impl GameEngine {
    pub async fn new() -> ShdResult<Self> {
        // Initialize WWShade with automatic backend detection
        let _backend = initialize_rendering().await?;

        // Create renderer using existing API
        let renderer = create_renderer().await?;

        println!(
            "🎮 Game engine initialized with backend: {}",
            get_backend_info()
        );

        Ok(Self {
            renderer,
            shaders: Vec::new(),
        })
    }

    pub fn add_shader(&mut self, shader: Box<dyn wwshade::ShdInterface>) {
        self.shaders.push(shader);
    }

    pub fn render_frame(&mut self) -> ShdResult<()> {
        // Your existing rendering loop works unchanged
        self.renderer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_compatibility() {
        // Test that existing API calls work
        let result = initialize_rendering().await;
        assert!(result.is_ok());

        let shader = create_simple_shader_using_existing_api();
        assert!(shader.is_ok());

        let renderer = create_renderer().await;
        assert!(renderer.is_ok());
    }

    #[tokio::test]
    async fn test_game_engine_integration() {
        // Test integration with a game engine
        let engine = GameEngine::new().await;
        assert!(engine.is_ok());

        println!("✅ Game engine integration test passed");
    }
}
