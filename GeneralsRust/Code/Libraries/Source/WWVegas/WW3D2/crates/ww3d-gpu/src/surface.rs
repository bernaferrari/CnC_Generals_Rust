//! GPU Surface and Swapchain Management
//!
//! This module provides surface creation, configuration, and swapchain management
//! for rendering to windows and other display targets.

use crate::*;

/// GPU surface abstraction
#[derive(Debug)]
pub struct GpuSurface {
    /// WGPU surface handle
    pub surface: wgpu::Surface<'static>,
    /// Surface configuration
    pub config: wgpu::SurfaceConfiguration,
    /// Surface capabilities
    capabilities: wgpu::SurfaceCapabilities,
    /// Current surface format
    format: wgpu::TextureFormat,
    /// Surface size
    size: SurfaceSize,
    /// Present mode
    present_mode: wgpu::PresentMode,
}

impl GpuSurface {
    /// Create a new surface from a window
    pub async fn from_window<W>(
        window: W,
        instance: &wgpu::Instance,
        adapter: &wgpu::Adapter,
        width: u32,
        height: u32,
    ) -> Result<Self, GpuError>
    where
        W: Into<wgpu::SurfaceTarget<'static>> + Send + Sync + 'static,
    {
        let surface = instance
            .create_surface(window)
            .map_err(|_| GpuError::SurfaceError(wgpu::SurfaceError::Lost))?;

        let capabilities = surface.get_capabilities(adapter);

        // Choose the best available format
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(capabilities.formats[0]);

        // Choose the best available present mode
        let present_mode = capabilities
            .present_modes
            .iter()
            .copied()
            .find(|&mode| mode == wgpu::PresentMode::Mailbox)
            .unwrap_or(capabilities.present_modes[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode,
            alpha_mode: capabilities.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        let size = SurfaceSize { width, height };

        Ok(Self {
            surface,
            config,
            capabilities,
            format,
            size,
            present_mode,
        })
    }

    /// Resize the surface
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.size = SurfaceSize { width, height };
            self.surface.configure(device, &self.config);
        }
    }

    /// Get the current frame texture
    pub fn get_current_texture(&self) -> Result<wgpu::SurfaceTexture, wgpu::SurfaceError> {
        self.surface.get_current_texture()
    }

    /// Get surface format
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Get surface size
    pub fn size(&self) -> SurfaceSize {
        self.size
    }

    /// Get surface width
    pub fn width(&self) -> u32 {
        self.size.width
    }

    /// Get surface height
    pub fn height(&self) -> u32 {
        self.size.height
    }

    /// Get present mode
    pub fn present_mode(&self) -> wgpu::PresentMode {
        self.present_mode
    }

    /// Check if surface is configured
    pub fn is_configured(&self) -> bool {
        self.config.width > 0 && self.config.height > 0
    }

    /// Get surface capabilities
    pub fn capabilities(&self) -> &wgpu::SurfaceCapabilities {
        &self.capabilities
    }
}

/// Surface size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceSize {
    pub width: u32,
    pub height: u32,
}

impl SurfaceSize {
    /// Create a new surface size
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Calculate aspect ratio
    pub fn aspect_ratio(&self) -> f32 {
        if self.height == 0 {
            0.0
        } else {
            self.width as f32 / self.height as f32
        }
    }

    /// Check if size is valid
    pub fn is_valid(&self) -> bool {
        self.width > 0 && self.height > 0
    }
}

impl Default for SurfaceSize {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
        }
    }
}

/// Swapchain management
#[derive(Debug)]
pub struct Swapchain {
    /// Surface textures
    frames: Vec<wgpu::SurfaceTexture>,
    /// Current frame index
    current_frame: usize,
    /// Frame count
    frame_count: usize,
}

impl Swapchain {
    /// Create a new swapchain
    pub fn new(frame_count: usize) -> Self {
        Self {
            frames: Vec::with_capacity(frame_count),
            current_frame: 0,
            frame_count,
        }
    }

    /// Get the next frame
    pub fn next_frame(&mut self, surface: &GpuSurface) -> Result<&wgpu::SurfaceTexture, GpuError> {
        let frame = surface
            .get_current_texture()
            .map_err(GpuError::SurfaceError)?;

        // Keep track of frames for synchronization
        if self.frames.len() < self.frame_count {
            self.frames.push(frame);
        } else {
            // Replace the oldest frame
            self.frames[self.current_frame] = frame;
        }

        self.current_frame = (self.current_frame + 1) % self.frame_count;

        Ok(&self.frames[self.current_frame])
    }

    /// Present the current frame
    pub fn present(&mut self) {
        if !self.frames.is_empty() && self.current_frame < self.frames.len() {
            let frame = self.frames.swap_remove(self.current_frame);
            crate::present_surface_texture(frame);
            // Reset current frame to 0 after presenting
            self.current_frame = 0;
        }
    }

    /// Get current frame index
    pub fn current_frame_index(&self) -> usize {
        self.current_frame
    }

    /// Get frame count
    pub fn frame_count(&self) -> usize {
        self.frame_count
    }

    /// Check if swapchain has frames
    pub fn has_frames(&self) -> bool {
        !self.frames.is_empty()
    }
}

/// Surface manager for handling multiple surfaces
#[derive(Debug)]
pub struct SurfaceManager {
    /// Primary surface
    primary_surface: Option<GpuSurface>,
    /// Additional surfaces
    surfaces: Vec<GpuSurface>,
    /// Active surface index
    active_surface: Option<usize>,
}

impl Default for SurfaceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SurfaceManager {
    /// Create a new surface manager
    pub fn new() -> Self {
        Self {
            primary_surface: None,
            surfaces: Vec::new(),
            active_surface: None,
        }
    }

    /// Add a primary surface
    pub fn set_primary_surface(&mut self, surface: GpuSurface) {
        self.primary_surface = Some(surface);
        self.active_surface = Some(0);
    }

    /// Add an additional surface
    pub fn add_surface(&mut self, surface: GpuSurface) -> usize {
        self.surfaces.push(surface);
        self.surfaces.len() - 1
    }

    /// Get the primary surface
    pub fn primary_surface(&self) -> Option<&GpuSurface> {
        self.primary_surface.as_ref()
    }

    /// Get the primary surface mutably
    pub fn primary_surface_mut(&mut self) -> Option<&mut GpuSurface> {
        self.primary_surface.as_mut()
    }

    /// Get a surface by index
    pub fn get_surface(&self, index: usize) -> Option<&GpuSurface> {
        if index == 0 {
            self.primary_surface.as_ref()
        } else {
            self.surfaces.get(index - 1)
        }
    }

    /// Get a surface mutably by index
    pub fn get_surface_mut(&mut self, index: usize) -> Option<&mut GpuSurface> {
        if index == 0 {
            self.primary_surface.as_mut()
        } else {
            self.surfaces.get_mut(index - 1)
        }
    }

    /// Get the active surface
    pub fn active_surface(&self) -> Option<&GpuSurface> {
        if let Some(index) = self.active_surface {
            self.get_surface(index)
        } else {
            None
        }
    }

    /// Set the active surface
    pub fn set_active_surface(&mut self, index: usize) {
        let valid = (index == 0 && self.primary_surface.is_some())
            || (index > 0 && index - 1 < self.surfaces.len());
        if valid {
            self.active_surface.replace(index);
        }
    }

    /// Get surface count
    pub fn surface_count(&self) -> usize {
        (if self.primary_surface.is_some() { 1 } else { 0 }) + self.surfaces.len()
    }

    /// Resize all surfaces
    pub fn resize_all(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if let Some(ref mut surface) = self.primary_surface {
            surface.resize(device, width, height);
        }

        for surface in &mut self.surfaces {
            surface.resize(device, width, height);
        }
    }

    /// Check if any surface needs resizing
    pub fn needs_resize(&self, new_size: SurfaceSize) -> bool {
        if let Some(surface) = &self.primary_surface {
            if surface.size() != new_size {
                return true;
            }
        }

        for surface in &self.surfaces {
            if surface.size() != new_size {
                return true;
            }
        }

        false
    }
}

/// Surface configuration helper
#[derive(Debug, Clone)]
pub struct SurfaceConfig {
    pub format: Option<wgpu::TextureFormat>,
    pub present_mode: Option<wgpu::PresentMode>,
    pub alpha_mode: Option<wgpu::CompositeAlphaMode>,
    pub usage: wgpu::TextureUsages,
}

impl Default for SurfaceConfig {
    fn default() -> Self {
        Self {
            format: None, // Will be chosen automatically
            present_mode: Some(wgpu::PresentMode::Fifo),
            alpha_mode: Some(wgpu::CompositeAlphaMode::Auto),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        }
    }
}

impl SurfaceConfig {
    /// Create a default surface configuration
    pub fn default_config() -> Self {
        Self::default()
    }

    /// Create a configuration optimized for gaming
    pub fn gaming() -> Self {
        Self {
            format: None,
            present_mode: Some(wgpu::PresentMode::Mailbox),
            alpha_mode: Some(wgpu::CompositeAlphaMode::Opaque),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        }
    }

    /// Create a configuration for compute operations
    pub fn compute() -> Self {
        Self {
            format: None,
            present_mode: Some(wgpu::PresentMode::Immediate),
            alpha_mode: Some(wgpu::CompositeAlphaMode::Opaque),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::STORAGE_BINDING,
        }
    }
}

/// Surface error handling
#[derive(Debug, thiserror::Error)]
pub enum SurfaceError {
    #[error("Surface is lost")]
    SurfaceLost,
    #[error("Surface is outdated")]
    SurfaceOutdated,
    #[error("Surface timeout")]
    SurfaceTimeout,
    #[error("Surface is out of memory")]
    SurfaceOutOfMemory,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surface_size() {
        let size = SurfaceSize::new(1920, 1080);
        assert_eq!(size.width, 1920);
        assert_eq!(size.height, 1080);
        assert_eq!(size.aspect_ratio(), 1920.0 / 1080.0);
        assert!(size.is_valid());
    }

    #[test]
    fn test_surface_size_default() {
        let size = SurfaceSize::default();
        assert_eq!(size.width, 800);
        assert_eq!(size.height, 600);
    }

    #[test]
    fn test_surface_size_invalid() {
        let size = SurfaceSize::new(0, 0);
        assert!(!size.is_valid());

        let size = SurfaceSize::new(1920, 0);
        assert!(!size.is_valid());
    }

    #[test]
    fn test_surface_config() {
        let config = SurfaceConfig::default();
        assert_eq!(config.present_mode, Some(wgpu::PresentMode::Fifo));
        assert_eq!(config.alpha_mode, Some(wgpu::CompositeAlphaMode::Auto));

        let gaming_config = SurfaceConfig::gaming();
        assert_eq!(gaming_config.present_mode, Some(wgpu::PresentMode::Mailbox));

        let compute_config = SurfaceConfig::compute();
        assert_eq!(
            compute_config.present_mode,
            Some(wgpu::PresentMode::Immediate)
        );
    }

    #[test]
    fn test_swapchain() {
        let swapchain = Swapchain::new(3);
        assert_eq!(swapchain.frame_count(), 3);
        assert_eq!(swapchain.current_frame_index(), 0);
        assert!(!swapchain.has_frames());
    }

    #[test]
    fn test_surface_manager() {
        let manager = SurfaceManager::new();
        assert_eq!(manager.surface_count(), 0);
        assert!(manager.primary_surface().is_none());
        assert!(manager.active_surface().is_none());

        // Note: We can't actually create surfaces without a real window/device
        // These tests verify the management logic
    }
}
