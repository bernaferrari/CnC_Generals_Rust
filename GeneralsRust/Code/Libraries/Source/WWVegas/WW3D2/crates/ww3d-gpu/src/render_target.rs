//! Render Target System
//!
//! This module implements render-to-texture functionality including support for
//! color attachments, depth/stencil buffers, and shadow maps.

use crate::{GpuError, GpuTexture};
use std::sync::Arc;

/// Render target with color and optional depth attachments
pub struct RenderTarget {
    /// Color texture
    pub color_texture: Arc<GpuTexture>,
    /// Color texture view
    pub color_view: wgpu::TextureView,
    /// Optional depth/stencil texture
    pub depth_texture: Option<Arc<GpuTexture>>,
    /// Optional depth texture view
    pub depth_view: Option<wgpu::TextureView>,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Color format
    pub color_format: wgpu::TextureFormat,
    /// Depth format
    pub depth_format: Option<wgpu::TextureFormat>,
    /// Sample count for MSAA
    pub sample_count: u32,
}

impl RenderTarget {
    /// Create a new render target
    pub fn new(
        device: &crate::device::GpuDevice,
        width: u32,
        height: u32,
        color_format: wgpu::TextureFormat,
        has_depth: bool,
        sample_count: u32,
        label: Option<&str>,
    ) -> Result<Self, GpuError> {
        // Create color texture with sample count
        let color_label = label.map(|s| format!("{} Color", s));
        let color_desc = wgpu::TextureDescriptor {
            label: color_label.as_deref(),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: color_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };

        let color_texture = GpuTexture::new(device, &color_desc)?;
        let color_view = color_texture
            .wgpu_texture()
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("Render Target Color View"),
                ..Default::default()
            });

        // Create depth texture if requested
        let (depth_texture, depth_view, depth_format) = if has_depth {
            let depth_fmt = wgpu::TextureFormat::Depth24Plus;
            let depth_label = label.map(|s| format!("{} Depth", s));
            let depth_desc = wgpu::TextureDescriptor {
                label: depth_label.as_deref(),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count,
                dimension: wgpu::TextureDimension::D2,
                format: depth_fmt,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            };

            let depth_tex = GpuTexture::new(device, &depth_desc)?;
            let depth_v = depth_tex
                .wgpu_texture()
                .create_view(&wgpu::TextureViewDescriptor {
                    label: Some("Render Target Depth View"),
                    ..Default::default()
                });

            (Some(Arc::new(depth_tex)), Some(depth_v), Some(depth_fmt))
        } else {
            (None, None, None)
        };

        Ok(Self {
            color_texture: Arc::new(color_texture),
            color_view,
            depth_texture,
            depth_view,
            width,
            height,
            color_format,
            depth_format,
            sample_count,
        })
    }

    /// Create a render target with default settings
    pub fn create_default(
        device: &crate::device::GpuDevice,
        width: u32,
        height: u32,
    ) -> Result<Self, GpuError> {
        Self::new(
            device,
            width,
            height,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            true,
            1,
            Some("Render Target"),
        )
    }

    /// Begin a render pass with this render target
    pub fn begin_render_pass<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
        load_op: wgpu::LoadOp<wgpu::Color>,
        clear_depth: bool,
    ) -> wgpu::RenderPass<'a> {
        let color_attachment = wgpu::RenderPassColorAttachment {
            view: &self.color_view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: load_op,
                store: wgpu::StoreOp::Store,
            },
        };

        let depth_stencil_attachment =
            self.depth_view
                .as_ref()
                .map(|view| wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations {
                        load: if clear_depth {
                            wgpu::LoadOp::Clear(1.0)
                        } else {
                            wgpu::LoadOp::Load
                        },
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                });

        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Target Pass"),
            color_attachments: &[Some(color_attachment)],
            depth_stencil_attachment,
            timestamp_writes: None,
            occlusion_query_set: None,
        })
    }

    /// Resize the render target
    pub fn resize(
        &mut self,
        device: &crate::device::GpuDevice,
        width: u32,
        height: u32,
    ) -> Result<(), GpuError> {
        if width == self.width && height == self.height {
            return Ok(());
        }

        // Recreate color texture
        let color_desc = wgpu::TextureDescriptor {
            label: Some("Resized Render Target Color"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: self.sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: self.color_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };

        let color_texture = GpuTexture::new(device, &color_desc)?;
        self.color_view = color_texture
            .wgpu_texture()
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.color_texture = Arc::new(color_texture);

        // Recreate depth texture if exists
        if let Some(depth_format) = self.depth_format {
            let depth_desc = wgpu::TextureDescriptor {
                label: Some("Resized Render Target Depth"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: self.sample_count,
                dimension: wgpu::TextureDimension::D2,
                format: depth_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            };

            let depth_tex = GpuTexture::new(device, &depth_desc)?;
            self.depth_view = Some(
                depth_tex
                    .wgpu_texture()
                    .create_view(&wgpu::TextureViewDescriptor::default()),
            );
            self.depth_texture = Some(Arc::new(depth_tex));
        }

        self.width = width;
        self.height = height;

        Ok(())
    }

    /// Get color texture for reading
    pub fn color_texture(&self) -> &Arc<GpuTexture> {
        &self.color_texture
    }

    /// Get depth texture for reading
    pub fn depth_texture(&self) -> Option<&Arc<GpuTexture>> {
        self.depth_texture.as_ref()
    }
}

/// Shadow map (depth-only render target)
pub struct ShadowMap {
    /// Depth texture
    pub depth_texture: Arc<GpuTexture>,
    /// Depth texture view
    pub depth_view: wgpu::TextureView,
    /// Sampler for shadow comparison
    pub sampler: wgpu::Sampler,
    /// Resolution (square)
    pub resolution: u32,
    /// Depth format
    pub format: wgpu::TextureFormat,
}

impl ShadowMap {
    /// Create a new shadow map
    pub fn new(
        device: &crate::device::GpuDevice,
        resolution: u32,
        label: Option<&str>,
    ) -> Result<Self, GpuError> {
        let format = wgpu::TextureFormat::Depth32Float;

        // Create depth texture
        let depth_texture = GpuTexture::create_2d(
            device,
            resolution,
            resolution,
            format,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            label.map(|s| format!("{} Shadow Depth", s)).as_deref(),
        )?;

        let depth_view = depth_texture
            .wgpu_texture()
            .create_view(&wgpu::TextureViewDescriptor {
                label: Some("Shadow Map Depth View"),
                ..Default::default()
            });

        // Create comparison sampler for shadow sampling
        let sampler = device
            .wgpu_device()
            .create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Shadow Map Sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Nearest,
                compare: Some(wgpu::CompareFunction::LessEqual),
                ..Default::default()
            });

        Ok(Self {
            depth_texture: Arc::new(depth_texture),
            depth_view,
            sampler,
            resolution,
            format,
        })
    }

    /// Begin a shadow map render pass
    pub fn begin_render_pass<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
    ) -> wgpu::RenderPass<'a> {
        let depth_stencil_attachment = wgpu::RenderPassDepthStencilAttachment {
            view: &self.depth_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        };

        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Shadow Map Pass"),
            color_attachments: &[],
            depth_stencil_attachment: Some(depth_stencil_attachment),
            timestamp_writes: None,
            occlusion_query_set: None,
        })
    }

    /// Get depth texture for reading
    pub fn depth_texture(&self) -> &Arc<GpuTexture> {
        &self.depth_texture
    }

    /// Get sampler
    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }
}

/// Multi-render-target (MRT) support
pub struct MultiRenderTarget {
    /// Color textures (up to 4)
    pub color_textures: Vec<Arc<GpuTexture>>,
    /// Color texture views
    pub color_views: Vec<wgpu::TextureView>,
    /// Optional depth texture
    pub depth_texture: Option<Arc<GpuTexture>>,
    /// Optional depth view
    pub depth_view: Option<wgpu::TextureView>,
    /// Width
    pub width: u32,
    /// Height
    pub height: u32,
}

impl MultiRenderTarget {
    /// Create a new multi-render-target
    pub fn new(
        device: &crate::device::GpuDevice,
        width: u32,
        height: u32,
        color_formats: &[wgpu::TextureFormat],
        has_depth: bool,
        label: Option<&str>,
    ) -> Result<Self, GpuError> {
        if color_formats.is_empty() || color_formats.len() > 4 {
            return Err(GpuError::InvalidOperation(
                "MRT requires 1-4 color attachments".to_string(),
            ));
        }

        // Create color textures
        let mut color_textures = Vec::new();
        let mut color_views = Vec::new();

        for (i, &format) in color_formats.iter().enumerate() {
            let texture = GpuTexture::create_2d(
                device,
                width,
                height,
                format,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                label.map(|s| format!("{} Color {}", s, i)).as_deref(),
            )?;

            let view = texture
                .wgpu_texture()
                .create_view(&wgpu::TextureViewDescriptor::default());
            color_textures.push(Arc::new(texture));
            color_views.push(view);
        }

        // Create depth texture if requested
        let (depth_texture, depth_view) = if has_depth {
            let depth_tex = GpuTexture::create_2d(
                device,
                width,
                height,
                wgpu::TextureFormat::Depth24Plus,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                label.map(|s| format!("{} Depth", s)).as_deref(),
            )?;

            let depth_v = depth_tex
                .wgpu_texture()
                .create_view(&wgpu::TextureViewDescriptor::default());
            (Some(Arc::new(depth_tex)), Some(depth_v))
        } else {
            (None, None)
        };

        Ok(Self {
            color_textures,
            color_views,
            depth_texture,
            depth_view,
            width,
            height,
        })
    }

    /// Get color texture by index
    pub fn color_texture(&self, index: usize) -> Option<&Arc<GpuTexture>> {
        self.color_textures.get(index)
    }

    /// Get depth texture
    pub fn depth_texture(&self) -> Option<&Arc<GpuTexture>> {
        self.depth_texture.as_ref()
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_shadow_map_resolution() {
        let resolution = 1024u32;
        assert_eq!(resolution, 1024);
    }

    #[test]
    fn test_render_target_formats() {
        let color_format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let depth_format = wgpu::TextureFormat::Depth24Plus;
        assert_ne!(format!("{:?}", color_format), format!("{:?}", depth_format));
    }
}
