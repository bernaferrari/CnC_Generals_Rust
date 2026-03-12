//! WGPU Surface Management
//!
//! This module handles surface creation and management for WGPU,
//! equivalent to the DirectX8 surface functionality.

use crate::core::error::{Error, Result};
use std::sync::Arc;
use wgpu::rwh::{HasDisplayHandle, HasWindowHandle, RawWindowHandle};
use wgpu::{Surface, SurfaceConfiguration, SurfaceTexture, TextureView};
use ww3d_gpu::present_surface_texture;

/// WGPU Surface manager
pub struct WgpuSurfaceManager {
    /// WGPU surface
    surface: Option<Arc<Surface<'static>>>,
    /// Surface configuration
    config: Option<wgpu::SurfaceConfiguration>,
    /// Current surface texture
    pub current_texture: Option<SurfaceTexture>,
    /// Current texture view
    current_view: Option<Arc<TextureView>>,
}

impl WgpuSurfaceManager {
    /// Create new surface manager
    /// Attach an existing surface to the manager.
    pub fn set_surface(&mut self, surface: Arc<Surface<'static>>) {
        self.surface = Some(surface);
    }

    pub fn new() -> Self {
        Self {
            surface: None,
            config: None,
            current_texture: None,
            current_view: None,
        }
    }

    /// Create surface from window handle
    pub fn create_surface<W>(
        &mut self,
        instance: &wgpu::Instance,
        window: &W,
        _width: u32,
        _height: u32,
    ) -> Result<()>
    where
        W: HasWindowHandle + HasDisplayHandle + Send + Sync,
    {
        let surface_target = unsafe {
            // SAFETY: window must be valid for the duration of the surface
            wgpu::SurfaceTargetUnsafe::from_window(window)
        }
        .map_err(|e| Error::GenericError(format!("Failed to get window handle: {}", e)))?;
        let surface = unsafe { instance.create_surface_unsafe(surface_target) }
            .map_err(|e| Error::GenericError(format!("Failed to create surface: {}", e)))?;

        self.surface = Some(Arc::new(surface));
        Ok(())
    }

    /// Configure surface
    pub fn configure_surface(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        present_mode: wgpu::PresentMode,
    ) -> Result<()> {
        let surface = self
            .surface
            .as_ref()
            .ok_or_else(|| Error::GenericError("Surface not created".to_string()))?;

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(device, &config);
        self.config = Some(config);

        Ok(())
    }

    /// Get current surface texture
    pub fn get_current_texture(&mut self) -> Result<SurfaceTexture> {
        let surface = self
            .surface
            .as_ref()
            .ok_or_else(|| Error::GenericError("Surface not created".to_string()))?;

        surface
            .get_current_texture()
            .map_err(|e| Error::GenericError(format!("Failed to get current texture: {}", e)))
    }

    /// Create view from surface texture
    pub fn create_view_from_texture(
        &mut self,
        texture: &SurfaceTexture,
    ) -> Result<Arc<TextureView>> {
        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let view_arc = Arc::new(view);
        self.current_view = Some(view_arc.clone());

        Ok(view_arc)
    }

    /// Present current frame
    pub fn present(&mut self) {
        if let Some(texture) = self.current_texture.take() {
            present_surface_texture(texture);
        }
        self.current_view = None;
    }

    /// Get surface
    pub fn surface(&self) -> Option<&Arc<Surface<'_>>> {
        self.surface.as_ref()
    }

    /// Get surface configuration
    pub fn config(&self) -> Option<&SurfaceConfiguration> {
        self.config.as_ref()
    }

    /// Get current texture view
    pub fn current_view(&self) -> Option<&Arc<TextureView>> {
        self.current_view.as_ref()
    }

    /// Resize surface
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) -> Result<()> {
        if let Some(config) = &mut self.config {
            config.width = width;
            config.height = height;

            if let Some(surface) = &self.surface {
                surface.configure(device, config);
            }
        }
        Ok(())
    }

    /// Get surface capabilities
    pub fn get_capabilities(&self, adapter: &wgpu::Adapter) -> Option<wgpu::SurfaceCapabilities> {
        self.surface
            .as_ref()
            .map(|surface| surface.get_capabilities(adapter))
    }

    /// Check if surface is configured
    pub fn is_configured(&self) -> bool {
        self.config.is_some()
    }

    /// Cleanup resources
    pub fn cleanup(&mut self) {
        self.present(); // Present any pending frame
        self.current_view = None;
        self.current_texture = None;
        self.config = None;
        self.surface = None;
    }
}

impl Default for WgpuSurfaceManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Surface utilities
pub struct SurfaceUtils;

impl SurfaceUtils {
    /// Get preferred surface format
    pub fn get_preferred_format(capabilities: &wgpu::SurfaceCapabilities) -> wgpu::TextureFormat {
        capabilities
            .formats
            .iter()
            .find(|format| {
                matches!(
                    format,
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
                )
            })
            .copied()
            .unwrap_or(capabilities.formats[0])
    }

    /// Get preferred present mode
    pub fn get_preferred_present_mode(
        capabilities: &wgpu::SurfaceCapabilities,
    ) -> wgpu::PresentMode {
        capabilities
            .present_modes
            .iter()
            .find(|mode| {
                matches!(
                    mode,
                    wgpu::PresentMode::Fifo | wgpu::PresentMode::FifoRelaxed
                )
            })
            .copied()
            .unwrap_or(wgpu::PresentMode::Fifo)
    }

    /// Create surface from raw window handle (platform-specific)
    pub fn create_surface_from_raw_handle(
        _instance: &wgpu::Instance,
        _window_handle: RawWindowHandle,
        _width: u32,
        _height: u32,
    ) -> Result<Surface<'_>> {
        // Note: This would require platform-specific window handle conversion
        // For now, return an error indicating platform-specific implementation needed
        Err(Error::PlatformNotSupported(
            "Surface creation not supported on this platform".to_string(),
        ))
    }

    /// Get surface format name for debugging
    pub fn format_name(format: wgpu::TextureFormat) -> &'static str {
        match format {
            wgpu::TextureFormat::Rgba8Unorm => "RGBA8 Unorm",
            wgpu::TextureFormat::Bgra8Unorm => "BGRA8 Unorm",
            wgpu::TextureFormat::Rgba16Float => "RGBA16 Float",
            _ => "Unknown",
        }
    }

    /// Get present mode name for debugging
    pub fn present_mode_name(mode: wgpu::PresentMode) -> &'static str {
        match mode {
            wgpu::PresentMode::Fifo => "FIFO (VSync)",
            wgpu::PresentMode::FifoRelaxed => "FIFO Relaxed",
            wgpu::PresentMode::Immediate => "Immediate",
            wgpu::PresentMode::Mailbox => "Mailbox",
            wgpu::PresentMode::AutoVsync => "Auto VSync",
            wgpu::PresentMode::AutoNoVsync => "Auto No VSync",
        }
    }
}

/// Surface statistics
#[derive(Debug, Clone)]
pub struct SurfaceStats {
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    pub present_mode: wgpu::PresentMode,
    pub frames_presented: u64,
}

impl SurfaceStats {
    /// Create new surface stats
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            format: wgpu::TextureFormat::Rgba8Unorm,
            present_mode: wgpu::PresentMode::Fifo,
            frames_presented: 0,
        }
    }

    /// Update dimensions
    pub fn update_dimensions(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    /// Update format
    pub fn update_format(&mut self, format: wgpu::TextureFormat) {
        self.format = format;
    }

    /// Update present mode
    pub fn update_present_mode(&mut self, mode: wgpu::PresentMode) {
        self.present_mode = mode;
    }

    /// Increment frame count
    pub fn increment_frames(&mut self) {
        self.frames_presented += 1;
    }
}

impl Default for SurfaceStats {
    fn default() -> Self {
        Self::new()
    }
}
