//! Render Target System
//!
//! Implements render-to-texture for various effects including:
//! - Minimap rendering
//! - Portal rendering
//! - Picture-in-picture
//! - Post-processing buffers
//!
//! Matches C++ WW3D render target functionality.

use std::collections::HashMap;
use std::sync::Arc;
use wgpu::{CommandEncoder, Device, RenderPass, Texture, TextureView};

/// Render target formats matching C++ capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderTargetFormat {
    /// RGBA8 (most common, 32-bit color)
    Rgba8Unorm,
    /// RGBA16F (HDR, 64-bit color)
    Rgba16Float,
    /// RGBA32F (High precision HDR)
    Rgba32Float,
    /// R32F (Single channel float, for special effects)
    R32Float,
    /// Depth24Stencil8 (depth + stencil)
    Depth24PlusStencil8,
    /// Depth32F (high precision depth)
    Depth32Float,
}

impl RenderTargetFormat {
    /// Convert to WGPU texture format
    pub fn to_wgpu_format(&self) -> wgpu::TextureFormat {
        match self {
            RenderTargetFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
            RenderTargetFormat::Rgba16Float => wgpu::TextureFormat::Rgba16Float,
            RenderTargetFormat::Rgba32Float => wgpu::TextureFormat::Rgba32Float,
            RenderTargetFormat::R32Float => wgpu::TextureFormat::R32Float,
            RenderTargetFormat::Depth24PlusStencil8 => wgpu::TextureFormat::Depth24PlusStencil8,
            RenderTargetFormat::Depth32Float => wgpu::TextureFormat::Depth32Float,
        }
    }

    /// Check if this is a depth format
    pub fn is_depth_format(&self) -> bool {
        matches!(
            self,
            RenderTargetFormat::Depth24PlusStencil8 | RenderTargetFormat::Depth32Float
        )
    }

    /// Get bytes per pixel
    pub fn bytes_per_pixel(&self) -> u32 {
        match self {
            RenderTargetFormat::Rgba8Unorm => 4,
            RenderTargetFormat::Rgba16Float => 8,
            RenderTargetFormat::Rgba32Float => 16,
            RenderTargetFormat::R32Float => 4,
            RenderTargetFormat::Depth24PlusStencil8 => 4,
            RenderTargetFormat::Depth32Float => 4,
        }
    }
}

/// A render target for off-screen rendering
pub struct RenderTarget {
    /// Name/identifier for this render target
    pub name: String,
    /// Color texture (if applicable)
    pub color_texture: Option<Arc<Texture>>,
    /// Depth texture (if applicable)
    pub depth_texture: Option<Arc<Texture>>,
    /// Color texture view
    pub color_view: Option<Arc<TextureView>>,
    /// Depth texture view
    pub depth_view: Option<Arc<TextureView>>,
    /// Target size
    pub size: (u32, u32),
    /// Color format
    pub color_format: Option<RenderTargetFormat>,
    /// Depth format
    pub depth_format: Option<RenderTargetFormat>,
    /// MSAA sample count
    pub sample_count: u32,
}

impl RenderTarget {
    /// Create a new render target
    pub fn new(
        device: &Device,
        name: String,
        size: (u32, u32),
        color_format: Option<RenderTargetFormat>,
        depth_format: Option<RenderTargetFormat>,
        sample_count: u32,
    ) -> Self {
        let mut color_texture = None;
        let mut color_view = None;
        let mut depth_texture = None;
        let mut depth_view = None;

        // Create color texture if needed
        if let Some(format) = color_format {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("{} Color Texture", name)),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count,
                dimension: wgpu::TextureDimension::D2,
                format: format.to_wgpu_format(),
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            color_texture = Some(Arc::new(texture));
            color_view = Some(Arc::new(view));
        }

        // Create depth texture if needed
        if let Some(format) = depth_format {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("{} Depth Texture", name)),
                size: wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count,
                dimension: wgpu::TextureDimension::D2,
                format: format.to_wgpu_format(),
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            depth_texture = Some(Arc::new(texture));
            depth_view = Some(Arc::new(view));
        }

        Self {
            name,
            color_texture,
            depth_texture,
            color_view,
            depth_view,
            size,
            color_format,
            depth_format,
            sample_count,
        }
    }

    /// Begin rendering to this target
    pub fn begin_render_pass<'a>(
        &'a self,
        encoder: &'a mut CommandEncoder,
        clear_color: Option<wgpu::Color>,
        clear_depth: Option<f32>,
    ) -> RenderPass<'a> {
        let color_attachment =
            self.color_view
                .as_ref()
                .map(|view| wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: clear_color
                            .map(wgpu::LoadOp::Clear)
                            .unwrap_or(wgpu::LoadOp::Load),
                        store: wgpu::StoreOp::Store,
                    },
                });

        let depth_attachment =
            self.depth_view
                .as_ref()
                .map(|view| wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations {
                        load: clear_depth
                            .map(wgpu::LoadOp::Clear)
                            .unwrap_or(wgpu::LoadOp::Load),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                });

        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(&format!("{} Render Pass", self.name)),
            color_attachments: &[color_attachment],
            depth_stencil_attachment: depth_attachment,
            occlusion_query_set: None,
            timestamp_writes: None,
        })
    }

    /// Get memory usage in bytes
    pub fn get_memory_usage(&self) -> u64 {
        let mut total = 0u64;
        let pixel_count = (self.size.0 * self.size.1) as u64;

        if let Some(format) = self.color_format {
            total += pixel_count * format.bytes_per_pixel() as u64;
        }

        if let Some(format) = self.depth_format {
            total += pixel_count * format.bytes_per_pixel() as u64;
        }

        // Multiply by MSAA sample count
        total * self.sample_count as u64
    }
}

/// Render target manager
pub struct RenderTargetManager {
    device: Arc<Device>,
    targets: HashMap<String, RenderTarget>,
    default_size: (u32, u32),
}

impl RenderTargetManager {
    /// Create a new render target manager
    pub fn new(device: Arc<Device>, default_size: (u32, u32)) -> Self {
        Self {
            device,
            targets: HashMap::new(),
            default_size,
        }
    }

    /// Create a standard color+depth render target
    pub fn create_standard_target(
        &mut self,
        name: String,
        size: Option<(u32, u32)>,
    ) -> &RenderTarget {
        let size = size.unwrap_or(self.default_size);
        let target = RenderTarget::new(
            &self.device,
            name.clone(),
            size,
            Some(RenderTargetFormat::Rgba8Unorm),
            Some(RenderTargetFormat::Depth32Float),
            1,
        );
        self.targets.insert(name.clone(), target);
        self.targets.get(&name).unwrap()
    }

    /// Create an HDR render target
    pub fn create_hdr_target(&mut self, name: String, size: Option<(u32, u32)>) -> &RenderTarget {
        let size = size.unwrap_or(self.default_size);
        let target = RenderTarget::new(
            &self.device,
            name.clone(),
            size,
            Some(RenderTargetFormat::Rgba16Float),
            Some(RenderTargetFormat::Depth32Float),
            1,
        );
        self.targets.insert(name.clone(), target);
        self.targets.get(&name).unwrap()
    }

    /// Create a depth-only render target (for shadow maps)
    pub fn create_depth_target(&mut self, name: String, size: Option<(u32, u32)>) -> &RenderTarget {
        let size = size.unwrap_or(self.default_size);
        let target = RenderTarget::new(
            &self.device,
            name.clone(),
            size,
            None,
            Some(RenderTargetFormat::Depth32Float),
            1,
        );
        self.targets.insert(name.clone(), target);
        self.targets.get(&name).unwrap()
    }

    /// Create a minimap render target
    pub fn create_minimap_target(&mut self, size: u32) -> &RenderTarget {
        let name = "Minimap".to_string();
        let target = RenderTarget::new(
            &self.device,
            name.clone(),
            (size, size),
            Some(RenderTargetFormat::Rgba8Unorm),
            None,
            1,
        );
        self.targets.insert(name.clone(), target);
        self.targets.get(&name).unwrap()
    }

    /// Create a portal render target
    pub fn create_portal_target(&mut self, index: usize, size: (u32, u32)) -> &RenderTarget {
        let name = format!("Portal_{}", index);
        let target = RenderTarget::new(
            &self.device,
            name.clone(),
            size,
            Some(RenderTargetFormat::Rgba8Unorm),
            Some(RenderTargetFormat::Depth32Float),
            1,
        );
        self.targets.insert(name.clone(), target);
        self.targets.get(&name).unwrap()
    }

    /// Get a render target by name
    pub fn get_target(&self, name: &str) -> Option<&RenderTarget> {
        self.targets.get(name)
    }

    /// Get a mutable render target by name
    pub fn get_target_mut(&mut self, name: &str) -> Option<&mut RenderTarget> {
        self.targets.get_mut(name)
    }

    /// Remove a render target
    pub fn remove_target(&mut self, name: &str) -> Option<RenderTarget> {
        self.targets.remove(name)
    }

    /// Get all target names
    pub fn get_target_names(&self) -> Vec<String> {
        self.targets.keys().cloned().collect()
    }

    /// Get total memory usage of all render targets
    pub fn get_total_memory_usage(&self) -> u64 {
        self.targets.values().map(|t| t.get_memory_usage()).sum()
    }

    /// Get statistics
    pub fn get_stats(&self) -> RenderTargetManagerStats {
        RenderTargetManagerStats {
            target_count: self.targets.len(),
            total_memory_usage: self.get_total_memory_usage(),
            default_size: self.default_size,
        }
    }
}

/// Render target manager statistics
#[derive(Debug, Clone)]
pub struct RenderTargetManagerStats {
    pub target_count: usize,
    pub total_memory_usage: u64,
    pub default_size: (u32, u32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_target_format() {
        assert_eq!(RenderTargetFormat::Rgba8Unorm.bytes_per_pixel(), 4);
        assert_eq!(RenderTargetFormat::Rgba16Float.bytes_per_pixel(), 8);
        assert_eq!(RenderTargetFormat::Rgba32Float.bytes_per_pixel(), 16);
        assert!(RenderTargetFormat::Depth32Float.is_depth_format());
        assert!(!RenderTargetFormat::Rgba8Unorm.is_depth_format());
    }

    #[test]
    fn test_memory_calculation() {
        // 1024x1024 RGBA8 = 1024 * 1024 * 4 = 4MB
        let size = (1024, 1024);
        let format = RenderTargetFormat::Rgba8Unorm;
        let expected = 1024 * 1024 * 4;
        assert_eq!(size.0 * size.1 * format.bytes_per_pixel(), expected);
    }
}
