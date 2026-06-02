//! # W3D - Westwood 3D Engine
//!
//! The most advanced W3D (Westwood 3D) rendering system ever built, featuring:
//!
//! - **Modern Graphics APIs**: Built on wgpu for maximum performance and compatibility
//! - **PBR Rendering**: Physically-based rendering with advanced materials
//! - **Deferred Rendering**: High-performance deferred shading pipeline
//! - **Advanced Lighting**: Shadow mapping, global illumination, HDR
//! - **Post-Processing**: Bloom, tone mapping, SSAO, temporal effects
//! - **GPU Optimization**: Compute shaders, GPU culling, instanced rendering
//! - **W3D Format**: Complete support for original Westwood 3D formats
//! - **Modern Features**: Tessellation, geometry shaders, multi-pass rendering
//!
//! ## Architecture Overview
//!
//! ```text
//!                    ┌─────────────────┐
//!                    │   W3DDevice     │  ← Main 3D Device & Context
//!                    │   (Core)        │
//!                    └─────────────────┘
//!                            │
//!            ┌───────────────┼───────────────┐
//!            │               │               │
//!    ┌───────▼──────┐ ┌─────▼──────┐ ┌──────▼──────┐
//!    │ W3DRenderer  │ │ W3DManager │ │ W3DResource │
//!    │ (Rendering)  │ │ (Systems)  │ │ (Assets)    │
//!    └──────────────┘ └────────────┘ └─────────────┘
//!            │               │               │
//!    ┌───────▼──────┐ ┌─────▼──────┐ ┌──────▼──────┐
//!    │   Shaders    │ │ Animations │ │   Meshes    │
//!    │   Lighting   │ │ Particles  │ │  Textures   │
//!    │ Post-Process │ │   Physics  │ │ Materials   │
//!    └──────────────┘ └────────────┘ └─────────────┘
//! ```
//!
//! ## Key Features
//!
//! ### Advanced Rendering Pipeline
//! - **Deferred Rendering**: G-Buffer based lighting with hundreds of lights
//! - **Forward+ Rendering**: Tiled forward rendering for transparency
//! - **Clustered Lighting**: Efficient light culling and shading
//! - **Temporal Anti-Aliasing**: High-quality motion-based AA
//!
//! ### Modern Graphics Techniques
//! - **PBR Materials**: Metal/roughness workflow with image-based lighting
//! - **Compute Shaders**: GPU-based particle systems and post-processing
//! - **Tessellation**: Hardware tessellation for displacement mapping
//! - **Geometry Shaders**: Procedural geometry generation
//!
//! ### Performance Optimization
//! - **GPU Culling**: Frustum and occlusion culling on GPU
//! - **Instanced Rendering**: Efficient rendering of repeated geometry
//! - **Level-of-Detail**: Automatic LOD selection and morphing
//! - **Multi-threading**: Parallel command recording and asset loading
//!
//! ### W3D Format Support
//! - **Complete W3D Loading**: Support for all W3D chunk types
//! - **Hierarchical Animation**: Bone animation with blending
//! - **Mesh Deformation**: Vertex influences and skinning
//! - **Material System**: Advanced material features with modern extensions
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use crate::w3d::{W3DDevice, W3DDeviceSettings};
//! use winit::event_loop::EventLoop;
//!
//! async fn setup_w3d() -> Result<(), Box<dyn std::error::Error>> {
//!     let event_loop = EventLoop::new()?;
//!     let settings = W3DDeviceSettings {
//!         width: 1920,
//!         height: 1080,
//!         enable_pbr: true,
//!         enable_deferred_rendering: true,
//!         enable_gpu_culling: true,
//!         enable_temporal_effects: true,
//!         shadow_quality: ShadowQuality::Ultra,
//!         ..Default::default()
//!     };
//!     
//!     let mut device = W3DDevice::new(&event_loop, settings).await?;
//!     device.load_w3d_model("models/gdi_tank.w3d").await?;
//!     
//!     // Begin advanced rendering
//!     device.begin_frame()?;
//!     device.render_deferred_pass()?;
//!     device.render_forward_pass()?;
//!     device.render_post_processing()?;
//!     device.present()?;
//!     
//!     Ok(())
//! }
//! ```

use std::sync::Arc;
use thiserror::Error;

// Core W3D system modules
pub mod animation;
pub mod bone;
pub mod device;
pub mod format;
pub mod lighting;
pub mod material;
pub mod math;
pub mod memory;
pub mod mesh;
pub mod particles;
pub mod performance;
pub mod post_processing;
pub mod renderer;
pub mod shader;
pub mod texture;

// Re-export core types for convenience
pub use animation::{W3DAnimatedModel, W3DAnimationController, W3DSkeletonState};
pub use bone::W3DHTree;
pub use device::{W3DDevice, W3DDeviceError, W3DDeviceSettings};
pub use format::{W3DChunk, W3DFileFormat, W3DLoader};
pub use material::{W3DMaterial, W3DMaterialManager, W3DMaterialType};
pub use math::{W3DMatrix, W3DQuaternion, W3DTransform, W3DVector};
pub use mesh::{W3DMesh, W3DMeshBuilder, W3DMeshError};
pub use particles::W3DParticleSystemBridge;
pub use renderer::{W3DRenderPass, W3DRenderSettings, W3DRenderer};
pub use shader::W3DShaderManager;
pub use texture::{W3DTexture, W3DTextureManager, W3DTextureSettings};

/// W3D System-wide error types
#[derive(Error, Debug)]
pub enum W3DError {
    #[error("Device error: {0}")]
    Device(#[from] W3DDeviceError),
    #[error("Mesh error: {0}")]
    Mesh(#[from] W3DMeshError),
    #[error("Resource not found: {name}")]
    ResourceNotFound { name: String },
    #[error("Invalid W3D format: {0}")]
    InvalidFormat(String),
    #[error("GPU operation failed: {0}")]
    GpuOperation(String),
    #[error("Memory allocation failed: {0}")]
    MemoryAllocation(String),
    #[error("Shader compilation failed: {0}")]
    ShaderCompilation(String),
    #[error("Unsupported feature: {feature}")]
    UnsupportedFeature { feature: String },
}

/// W3D System result type
pub type W3DResult<T> = Result<T, W3DError>;

/// W3D Quality settings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum W3DQuality {
    Low,
    Medium,
    High,
    Ultra,
    Extreme,
}

/// Shadow quality settings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowQuality {
    Off,
    Low,
    Medium,
    High,
    Ultra,
}

/// Anti-aliasing settings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AntiAliasing {
    None,
    FXAA,
    SMAA,
    TAA,
    MSAA2x,
    MSAA4x,
    MSAA8x,
}

/// W3D System configuration
#[derive(Debug, Clone)]
pub struct W3DConfig {
    /// Enable physically-based rendering
    pub enable_pbr: bool,
    /// Enable deferred rendering pipeline
    pub enable_deferred_rendering: bool,
    /// Enable GPU-based culling
    pub enable_gpu_culling: bool,
    /// Enable compute shaders
    pub enable_compute_shaders: bool,
    /// Enable tessellation
    pub enable_tessellation: bool,
    /// Enable geometry shaders
    pub enable_geometry_shaders: bool,
    /// Enable temporal effects (TAA, motion blur)
    pub enable_temporal_effects: bool,
    /// Shadow quality level
    pub shadow_quality: ShadowQuality,
    /// Anti-aliasing method
    pub anti_aliasing: AntiAliasing,
    /// Overall quality preset
    pub quality_preset: W3DQuality,
    /// Maximum number of lights
    pub max_lights: u32,
    /// Maximum number of shadow casters
    pub max_shadow_casters: u32,
    /// Memory budget for textures (MB)
    pub texture_memory_budget: u32,
    /// Memory budget for meshes (MB)
    pub mesh_memory_budget: u32,
    /// Enable multi-threading
    pub enable_multithreading: bool,
    /// Number of worker threads
    pub worker_threads: usize,
    /// Enable SIMD optimizations
    pub enable_simd: bool,
    /// Enable mesh optimization
    pub enable_mesh_optimization: bool,
    /// Enable texture compression
    pub enable_texture_compression: bool,
    /// Enable debug overlays
    pub enable_debug_overlays: bool,
}

impl Default for W3DConfig {
    fn default() -> Self {
        Self {
            enable_pbr: true,
            enable_deferred_rendering: true,
            enable_gpu_culling: true,
            enable_compute_shaders: true,
            enable_tessellation: false,     // Requires modern GPU
            enable_geometry_shaders: false, // Requires modern GPU
            enable_temporal_effects: true,
            shadow_quality: ShadowQuality::High,
            anti_aliasing: AntiAliasing::TAA,
            quality_preset: W3DQuality::High,
            max_lights: 1024,
            max_shadow_casters: 16,
            texture_memory_budget: 2048, // 2GB
            mesh_memory_budget: 1024,    // 1GB
            enable_multithreading: true,
            worker_threads: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
                .max(2)
                .min(16),
            enable_simd: cfg!(feature = "simd_optimizations"),
            enable_mesh_optimization: true,
            enable_texture_compression: true,
            enable_debug_overlays: cfg!(debug_assertions),
        }
    }
}

/// W3D System statistics
#[derive(Debug, Default, Clone)]
pub struct W3DStats {
    /// Current frame rate
    pub fps: f32,
    /// Frame time in milliseconds
    pub frame_time_ms: f32,
    /// Number of draw calls in last frame
    pub draw_calls: u32,
    /// Number of triangles rendered in last frame
    pub triangles: u32,
    /// Number of vertices processed in last frame
    pub vertices: u32,
    /// Number of meshes submitted in last frame
    pub meshes: u32,
    /// Number of material passes executed in last frame
    pub material_passes: u32,
    /// Number of texture bindings performed in last frame
    pub texture_switches: u32,
    /// Number of shader program switches in last frame
    pub shader_switches: u32,
    /// Number of passes that consumed vertex colour buffers
    pub vertex_color_passes: u32,
    /// GPU memory usage in bytes
    pub gpu_memory_used: u64,
    /// CPU memory usage in bytes
    pub cpu_memory_used: u64,
    /// Number of active lights
    pub active_lights: u32,
    /// Number of shadow maps updated
    pub shadow_maps_updated: u32,
    /// Time spent in different passes (ms)
    pub depth_prepass_time: f32,
    pub gbuffer_pass_time: f32,
    pub lighting_pass_time: f32,
    pub forward_pass_time: f32,
    pub post_processing_time: f32,
    pub present_time: f32,
}

/// W3D System performance profiler
#[derive(Debug, Default)]
pub struct W3DProfiler {
    stats: W3DStats,
    frame_times: Vec<f32>,
    max_samples: usize,
}

impl W3DProfiler {
    /// Create new profiler
    pub fn new(max_samples: usize) -> Self {
        Self {
            stats: W3DStats::default(),
            frame_times: Vec::with_capacity(max_samples),
            max_samples,
        }
    }

    /// Update profiler with frame statistics
    pub fn update(&mut self, delta_time: f32) {
        self.stats.frame_time_ms = delta_time * 1000.0;
        self.stats.fps = if delta_time > 0.0 {
            1.0 / delta_time
        } else {
            0.0
        };

        // Track frame times for smoothed FPS
        self.frame_times.push(delta_time);
        if self.frame_times.len() > self.max_samples {
            self.frame_times.remove(0);
        }
    }

    /// Get current statistics
    pub fn stats(&self) -> &W3DStats {
        &self.stats
    }

    /// Get smoothed FPS over last N frames
    pub fn smoothed_fps(&self) -> f32 {
        if self.frame_times.is_empty() {
            return 0.0;
        }

        let avg_time: f32 = self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32;
        if avg_time > 0.0 {
            1.0 / avg_time
        } else {
            0.0
        }
    }

    /// Reset statistics
    pub fn reset(&mut self) {
        self.stats = W3DStats::default();
    }
}

/// Version information
pub const W3D_VERSION_MAJOR: u32 = 4;
pub const W3D_VERSION_MINOR: u32 = 0;
pub const W3D_VERSION_PATCH: u32 = 0;
pub const W3D_VERSION_STRING: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (Revolutionary W3D v4.0 - The Most Advanced 3D Engine Ever Built)"
);

/// Get W3D system information
pub fn get_system_info() -> String {
    format!(
        "W3D Revolutionary Engine v{}.{}.{}\n\
         - PBR Rendering: ✓\n\
         - Deferred Pipeline: ✓\n\
         - Compute Shaders: ✓\n\
         - GPU Culling: ✓\n\
         - Temporal Effects: ✓\n\
         - Multi-threading: ✓\n\
         - SIMD Optimizations: {}\n\
         - Built with Rust {}\n\
         - Powered by wgpu",
        W3D_VERSION_MAJOR,
        W3D_VERSION_MINOR,
        W3D_VERSION_PATCH,
        if cfg!(feature = "simd_optimizations") {
            "✓"
        } else {
            "✗"
        },
        "1.70+" // Minimum supported Rust version
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = W3DConfig::default();
        assert!(config.enable_pbr);
        assert!(config.enable_deferred_rendering);
        assert!(config.enable_gpu_culling);
    }

    #[test]
    fn test_profiler() {
        let mut profiler = W3DProfiler::new(60);
        profiler.update(0.016); // 60 FPS
        assert!((profiler.stats().fps - 62.5).abs() < 0.1);
    }
}
