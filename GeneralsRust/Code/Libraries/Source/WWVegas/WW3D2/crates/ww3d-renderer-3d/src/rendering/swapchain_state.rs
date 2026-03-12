//! Swapchain management for the modernised renderer.
//!
//! This module provides a lightweight owner for the WGPU surface configuration and the extra
//! attachments (depth/MSAA) that the renderer needs each frame. It is intentionally small for now,
//! allowing the higher level `Renderer` to opt-in while the legacy code is incrementally replaced.

use crate::core::error::{Error, Result};
use std::sync::Arc;
use wgpu::{
    CompositeAlphaMode, Extent3d, Operations, PresentMode, RenderPassColorAttachment,
    RenderPassDepthStencilAttachment, Surface, SurfaceConfiguration, TextureDescriptor,
    TextureDimension, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
};
use ww3d_gpu::device::GpuDevice;

/// Preferred swapchain formats. The renderer always has an SDR fallback, with an optional HDR
/// format for displays that support it.
#[derive(Debug, Clone)]
pub struct SwapchainFormatSet {
    sdr: TextureFormat,
    hdr: Option<TextureFormat>,
}

impl SwapchainFormatSet {
    /// Construct a format set with only an SDR format.
    pub fn new(sdr: TextureFormat) -> Self {
        Self { sdr, hdr: None }
    }

    /// Attach an HDR-capable format to the set.
    pub fn with_hdr(mut self, hdr: TextureFormat) -> Self {
        self.hdr = Some(hdr);
        self
    }

    /// Resolve which format should be used for the current HDR toggle.
    fn resolve(&self, hdr_enabled: bool) -> Result<TextureFormat> {
        if hdr_enabled {
            self.hdr.ok_or_else(|| {
                Error::InvalidOperation(
                    "HDR requested but no HDR-capable format was provided".into(),
                )
            })
        } else {
            Ok(self.sdr)
        }
    }

    /// Set of formats that need to be advertised as compatible with the surface.
    fn view_formats(&self) -> Vec<TextureFormat> {
        let mut formats = vec![self.sdr];
        if let Some(hdr) = self.hdr {
            if hdr != self.sdr {
                formats.push(hdr);
            }
        }
        formats
    }
}

/// Attachment bundle (texture + default view).
struct Attachment {
    _texture: wgpu::Texture,
    view: TextureView,
}

impl Attachment {
    fn new(device: &GpuDevice, desc: &TextureDescriptor<'_>) -> Result<Self> {
        let texture = device.wgpu_device().create_texture(desc);
        let view = texture.create_view(&TextureViewDescriptor::default());
        Ok(Self {
            _texture: texture,
            view,
        })
    }
}

/// Renderer-owned swapchain state, responsible for reacting to resize/HDR/MSAA updates and keeping
/// depth/MSAA attachments in sync with the surface configuration.
pub struct RendererSwapchainState {
    gpu_device: Arc<GpuDevice>,
    surface: Option<Arc<Surface<'static>>>,
    surface_config: SurfaceConfiguration,
    formats: SwapchainFormatSet,
    depth_format: Option<TextureFormat>,
    msaa_samples: u32,
    hdr_enabled: bool,
    color_msaa: Option<Attachment>,
    depth: Option<Attachment>,
}

impl RendererSwapchainState {
    /// Create an instance bound to a device and surface configuration.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        gpu_device: Arc<GpuDevice>,
        surface: Option<Arc<Surface<'static>>>,
        mut surface_config: SurfaceConfiguration,
        formats: SwapchainFormatSet,
        depth_format: Option<TextureFormat>,
        msaa_samples: u32,
        hdr_enabled: bool,
    ) -> Result<Self> {
        surface_config.width = surface_config.width.max(1);
        surface_config.height = surface_config.height.max(1);
        surface_config.view_formats = formats.view_formats();
        surface_config.format = formats.resolve(hdr_enabled)?;

        let mut state = Self {
            gpu_device,
            surface,
            surface_config,
            formats,
            depth_format,
            msaa_samples: msaa_samples.max(1),
            hdr_enabled,
            color_msaa: None,
            depth: None,
        };

        state.configure_surface()?;
        state.recreate_attachments()?;
        Ok(state)
    }

    /// Query the current surface configuration.
    pub fn surface_config(&self) -> &SurfaceConfiguration {
        &self.surface_config
    }

    /// Return depth attachment view if available.
    pub fn depth_view(&self) -> Option<&TextureView> {
        self.depth.as_ref().map(|attachment| &attachment.view)
    }

    /// Return MSAA resolve target if one is allocated.
    pub fn msaa_view(&self) -> Option<&TextureView> {
        self.color_msaa.as_ref().map(|attachment| &attachment.view)
    }

    /// Current surface dimensions.
    pub fn size(&self) -> (u32, u32) {
        (self.surface_config.width, self.surface_config.height)
    }

    /// Whether HDR output is enabled.
    pub fn hdr_enabled(&self) -> bool {
        self.hdr_enabled
    }

    /// Current MSAA sample count.
    pub fn msaa_samples(&self) -> u32 {
        self.msaa_samples
    }

    /// Update the tracked surface handle (used when the windowing layer recreates the surface).
    pub fn set_surface(&mut self, surface: Option<Arc<Surface<'static>>>) -> Result<()> {
        self.surface = surface;
        self.configure_surface()
    }

    /// Update the surface size and rebuild attachments.
    pub fn resize(&mut self, new_size: (u32, u32)) -> Result<()> {
        if new_size.0 == 0 || new_size.1 == 0 {
            return Err(Error::InvalidParameter(
                "swapchain resize requires non-zero dimensions".into(),
            ));
        }
        if self.size() == new_size {
            return Ok(());
        }
        self.surface_config.width = new_size.0;
        self.surface_config.height = new_size.1;
        self.configure_surface()?;
        self.recreate_attachments()
    }

    /// Toggle HDR mode and rebuild the attachments if the format changes.
    pub fn set_hdr_enabled(&mut self, hdr_enabled: bool) -> Result<()> {
        if self.hdr_enabled == hdr_enabled {
            return Ok(());
        }
        self.surface_config.format = self.formats.resolve(hdr_enabled)?;
        self.hdr_enabled = hdr_enabled;
        self.configure_surface()?;
        self.recreate_attachments()
    }

    /// Update the MSAA sample count and rebuild attachments.
    pub fn set_msaa_samples(&mut self, samples: u32) -> Result<()> {
        let samples = samples.max(1);
        if self.msaa_samples == samples {
            return Ok(());
        }
        self.msaa_samples = samples;
        self.recreate_attachments()
    }

    /// Update the depth format and rebuild if the format changed.
    pub fn set_depth_format(&mut self, format: Option<TextureFormat>) -> Result<()> {
        if self.depth_format == format {
            return Ok(());
        }
        self.depth_format = format;
        self.recreate_attachments()
    }

    /// Helper to create a colour attachment for the current frame.
    pub fn make_color_attachment<'a>(
        &'a self,
        frame_view: &'a TextureView,
        clear_color: Option<wgpu::Color>,
    ) -> RenderPassColorAttachment<'a> {
        let ops = Operations {
            load: if let Some(color) = clear_color {
                wgpu::LoadOp::Clear(color)
            } else {
                wgpu::LoadOp::Load
            },
            store: wgpu::StoreOp::Store,
        };

        if let Some(msaa) = &self.color_msaa {
            RenderPassColorAttachment {
                view: &msaa.view,
                depth_slice: None,
                resolve_target: Some(frame_view),
                ops,
            }
        } else {
            RenderPassColorAttachment {
                view: frame_view,
                depth_slice: None,
                resolve_target: None,
                ops,
            }
        }
    }

    /// Helper to create a depth/stencil attachment descriptor for the current frame.
    pub fn make_depth_attachment(
        &self,
        clear_depth: Option<f32>,
        clear_stencil: Option<u32>,
    ) -> Option<RenderPassDepthStencilAttachment<'_>> {
        let depth = self.depth.as_ref()?;
        let depth_ops = clear_depth.map_or(
            Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
            |value| Operations {
                load: wgpu::LoadOp::Clear(value),
                store: wgpu::StoreOp::Store,
            },
        );
        let stencil_ops = clear_stencil.map(|value| Operations {
            load: wgpu::LoadOp::Clear(value),
            store: wgpu::StoreOp::Store,
        });
        Some(RenderPassDepthStencilAttachment {
            view: &depth.view,
            depth_ops: Some(depth_ops),
            stencil_ops,
        })
    }

    fn configure_surface(&self) -> Result<()> {
        if let Some(surface) = &self.surface {
            surface.configure(self.gpu_device.wgpu_device(), &self.surface_config);
        }
        Ok(())
    }

    fn recreate_attachments(&mut self) -> Result<()> {
        self.color_msaa = None;
        self.depth = None;

        let (width, height) = self.size();
        let extent = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        if self.msaa_samples > 1 {
            let desc = TextureDescriptor {
                label: Some("Renderer MSAA Target"),
                size: extent,
                mip_level_count: 1,
                sample_count: self.msaa_samples,
                dimension: TextureDimension::D2,
                format: self.surface_config.format,
                usage: TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            };
            self.color_msaa = Some(Attachment::new(&self.gpu_device, &desc)?);
        }

        if let Some(depth_format) = self.depth_format {
            let desc = TextureDescriptor {
                label: Some("Renderer Depth Target"),
                size: extent,
                mip_level_count: 1,
                sample_count: self.msaa_samples,
                dimension: TextureDimension::D2,
                format: depth_format,
                usage: TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            };
            self.depth = Some(Attachment::new(&self.gpu_device, &desc)?);
        }

        Ok(())
    }
}

impl Default for SwapchainFormatSet {
    fn default() -> Self {
        Self::new(TextureFormat::Bgra8UnormSrgb)
    }
}

/// Convenience builder for creating a baseline surface configuration.
pub fn make_surface_config(
    size: (u32, u32),
    format: TextureFormat,
    present_mode: PresentMode,
) -> SurfaceConfiguration {
    SurfaceConfiguration {
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
        format,
        width: size.0.max(1),
        height: size.1.max(1),
        present_mode,
        alpha_mode: CompositeAlphaMode::Opaque,
        view_formats: vec![format],
        desired_maximum_frame_latency: 2,
    }
}
