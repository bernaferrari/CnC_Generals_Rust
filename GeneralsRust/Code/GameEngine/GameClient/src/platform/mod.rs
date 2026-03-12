/*
**  Command & Conquer Generals Zero Hour™
**  Minimal cross-platform bootstrap built on winit/wgpu/kira.
*/

#![allow(missing_docs)]

//! Lightweight platform bootstrap built on top of the modern Rust ecosystem.
//!
//! The original codebase shipped bespoke implementations for every operating
//! system.  For the purposes of the Rust re-launch we rely on well supported
//! crates instead:
//!
//! * [`winit`] for windowing and input.
//! * [`wgpu`] for cross-platform graphics.
//! * [`kira`] for high level audio playback.

use std::mem;
use std::sync::Arc;

use thiserror::Error;
use wgpu::{Queue, SurfaceConfiguration};
use winit::{
    dpi::LogicalSize,
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

/// Platform level errors.
#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("Window creation failed: {0}")]
    WindowCreation(String),

    #[error("Graphics adapter not found")]
    AdapterNotFound,

    #[error("Graphics device creation failed: {0}")]
    DeviceCreation(String),

    #[error("Audio initialization failed: {0}")]
    AudioInitialization(String),
}

/// Encapsulates the state required to render with wgpu.
pub struct GraphicsContext {
    surface: Arc<wgpu::Surface<'static>>,
    device: Arc<wgpu::Device>,
    queue: Arc<Queue>,
    config: SurfaceConfiguration,
}

impl GraphicsContext {
    fn new(window: &Window) -> Result<Self, PlatformError> {
        let size = window.inner_size();

        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window)
            .map_err(|e| PlatformError::DeviceCreation(e.to_string()))?;
        let surface: wgpu::Surface<'static> = unsafe { mem::transmute(surface) };
        let surface = Arc::new(surface);

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(surface.as_ref()),
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
        }))
        .map_err(|_| PlatformError::AdapterNotFound)?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("GameClient Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .map_err(|e| PlatformError::DeviceCreation(e.to_string()))?;
        let device: Arc<wgpu::Device> = Arc::new(device);
        let queue = Arc::new(queue);

        let surface_format = surface
            .get_capabilities(&adapter)
            .formats
            .first()
            .cloned()
            .ok_or(PlatformError::DeviceCreation(
                "No compatible surface format found".to_string(),
            ))?;

        let config = SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };

        surface.configure(device.as_ref(), &config);

        Ok(Self {
            surface,
            device,
            queue,
            config,
        })
    }

    /// Resize the swap chain to match the window.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        self.config.width = width;
        self.config.height = height;
        self.surface.configure(self.device.as_ref(), &self.config);
    }

    pub fn device(&self) -> &wgpu::Device {
        self.device.as_ref()
    }

    pub fn queue(&self) -> &Queue {
        self.queue.as_ref()
    }

    pub fn device_arc(&self) -> Arc<wgpu::Device> {
        Arc::clone(&self.device)
    }

    pub fn queue_arc(&self) -> Arc<Queue> {
        Arc::clone(&self.queue)
    }

    pub fn surface(&self) -> &wgpu::Surface<'_> {
        self.surface.as_ref()
    }

    pub fn config(&self) -> &SurfaceConfiguration {
        &self.config
    }
}

impl Clone for GraphicsContext {
    fn clone(&self) -> Self {
        Self {
            surface: Arc::clone(&self.surface),
            device: Arc::clone(&self.device),
            queue: Arc::clone(&self.queue),
            config: self.config.clone(),
        }
    }
}

/// Minimal audio context using Kira.
pub struct AudioContext {
    manager: kira::manager::AudioManager<kira::manager::backend::DefaultBackend>,
}

impl AudioContext {
    fn new() -> Result<Self, PlatformError> {
        let manager = kira::manager::AudioManager::<kira::manager::backend::DefaultBackend>::new(
            kira::manager::AudioManagerSettings::default(),
        )
        .map_err(|e| PlatformError::AudioInitialization(e.to_string()))?;
        Ok(Self { manager })
    }

    /// Obtain a mutable handle to the underlying audio manager.
    pub fn manager(
        &mut self,
    ) -> &mut kira::manager::AudioManager<kira::manager::backend::DefaultBackend> {
        &mut self.manager
    }
}

/// Bundles together the window, graphics, and audio systems.
pub struct PlatformContext {
    pub event_loop: EventLoop<()>,
    pub window: Window,
    pub graphics: GraphicsContext,
    pub audio: AudioContext,
}

impl PlatformContext {
    /// Create a new window, graphics device, and audio manager.
    pub fn new(title: &str, width: u32, height: u32) -> Result<Self, PlatformError> {
        let event_loop =
            EventLoop::new().map_err(|e| PlatformError::WindowCreation(e.to_string()))?;
        let window = WindowBuilder::new()
            .with_title(title)
            .with_inner_size(LogicalSize::new(width as f64, height as f64))
            .build(&event_loop)
            .map_err(|e| PlatformError::WindowCreation(e.to_string()))?;

        let graphics = GraphicsContext::new(&window)?;
        let audio = AudioContext::new()?;

        Ok(Self {
            event_loop,
            window,
            graphics,
            audio,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_platform_context() {
        let result = PlatformContext::new("Test", 800, 600);
        assert!(result.is_ok());
    }
}
