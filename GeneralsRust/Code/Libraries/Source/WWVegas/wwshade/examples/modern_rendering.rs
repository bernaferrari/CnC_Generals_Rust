// Example: Modern 2025 Cross-Platform Rendering with WWShade
// Demonstrates how to use both legacy DX6/7/8 and modern WGPU backends

use glam::{Mat4, Vec3, Vec4};
use wwshade::{
    modern_shaders::{
        CameraUniform, HybridShaderSystem, LightUniform, MaterialUniform, ModernShaderSystem,
    },
    ShdResult,
};

#[tokio::main]
async fn main() -> ShdResult<()> {
    env_logger::init();

    println!("🚀 WWShade Modern Rendering Example");
    println!("===================================");

    // Initialize hybrid system with both legacy and modern support
    let mut hybrid_system = HybridShaderSystem::new().await?;

    println!(
        "📊 Available backends: {}",
        hybrid_system.get_backend_info()
    );

    if hybrid_system.has_modern_support() {
        println!("✅ Modern WGPU backend available!");
        demo_modern_rendering(&mut hybrid_system).await?;
    } else {
        println!("⚠️  Modern backend not available, using legacy DirectX fallback");
        demo_legacy_rendering(&hybrid_system)?;
    }

    Ok(())
}

async fn demo_modern_rendering(hybrid_system: &mut HybridShaderSystem) -> ShdResult<()> {
    println!("\n🔥 Modern WGPU Cross-Platform Rendering Demo");
    println!("============================================");

    if let Some(modern_system) = &mut hybrid_system.modern_system {
        // This would work on:
        // - Windows: Vulkan, DirectX 12, DirectX 11
        // - macOS: Metal
        // - Linux: Vulkan
        // - Web: WebGPU

        println!("🖥️  Platform detection:");
        println!("   • Windows: Vulkan/DX12/DX11 backends");
        println!("   • macOS: Metal backend");
        println!("   • Linux: Vulkan backend");
        println!("   • Web: WebGPU backend");

        // Update camera (modern matrix math with glam)
        let view_projection = Mat4::perspective_rh(45.0_f32.to_radians(), 16.0 / 9.0, 0.1, 1000.0);
        let camera_position = Vec3::new(0.0, 5.0, 10.0);

        modern_system.update_camera(view_projection, camera_position);
        println!("📷 Camera updated with modern math (SIMD optimized)");

        // Setup modern lighting
        let lights = vec![LightUniform {
            position: [10.0, 10.0, 10.0],
            _padding1: 0.0,
            color: [1.0, 1.0, 1.0],
            intensity: 1.0,
            direction: [-1.0, -1.0, -1.0],
            _padding2: 0.0,
        }];
        modern_system.update_lights(&lights);
        println!("💡 Modern lighting system configured");

        // Create modern bump mapping pipeline
        let surface_format = wgpu::TextureFormat::Bgra8UnormSrgb; // Common format
        let pipeline_id = modern_system.create_bump_mapping_pipeline(surface_format)?;
        println!(
            "🔧 Modern WGSL bump mapping pipeline created: {}",
            pipeline_id
        );

        println!("✨ Modern shader features:");
        println!("   • WGSL shading language (modern, type-safe)");
        println!("   • Physically-based rendering ready");
        println!("   • Multi-light support (8 lights)");
        println!("   • Tangent-space normal mapping");
        println!("   • Automatic backend selection");
        println!("   • Memory-safe uniform buffers");
    }

    Ok(())
}

fn demo_legacy_rendering(hybrid_system: &HybridShaderSystem) -> ShdResult<()> {
    println!("\n🔧 Legacy DirectX Fallback Demo");
    println!("==============================");

    // Your existing DX6/7/8 system would still work
    if hybrid_system.legacy_dx8 {
        println!("✅ DirectX 8: Programmable vertex/pixel shaders");
        println!("   • Bump mapping with DOT3 + programmable shaders");
        println!("   • Multi-pass rendering");
        println!("   • Hardware vertex processing");
    }

    if hybrid_system.legacy_dx7 {
        println!("✅ DirectX 7: DOT3 bump mapping");
        println!("   • Two-pass DOT3 bump mapping");
        println!("   • Texture stage operations");
    }

    if hybrid_system.legacy_dx6 {
        println!("✅ DirectX 6: Basic fixed-function");
        println!("   • Software vertex processing fallback");
        println!("   • Basic texture blending");
    }

    Ok(())
}

// Example of how to integrate with a game engine
pub struct ModernWWShadeRenderer {
    hybrid_system: HybridShaderSystem,
    current_backend: String,
}

impl ModernWWShadeRenderer {
    pub async fn new() -> ShdResult<Self> {
        let hybrid_system = HybridShaderSystem::new().await?;
        let current_backend = hybrid_system.get_backend_info();

        Ok(Self {
            hybrid_system,
            current_backend,
        })
    }

    pub fn get_platform_info(&self) -> &str {
        &self.current_backend
    }

    // This method would automatically choose the best rendering path
    pub fn render_mesh(&mut self /* mesh data */) -> ShdResult<()> {
        if self.hybrid_system.has_modern_support() {
            // Use modern WGPU pipeline
            self.render_modern_mesh(/* mesh data */)?;
        } else {
            // Fallback to legacy DirectX
            self.render_legacy_mesh(/* mesh data */)?;
        }

        Ok(())
    }

    fn render_modern_mesh(&mut self /* mesh data */) -> ShdResult<()> {
        // Modern cross-platform rendering using WGSL shaders
        println!("🚀 Rendering with modern WGPU backend");
        Ok(())
    }

    fn render_legacy_mesh(&mut self /* mesh data */) -> ShdResult<()> {
        // Legacy DirectX rendering using your converted shaders
        println!("🔧 Rendering with legacy DirectX backend");
        Ok(())
    }
}
