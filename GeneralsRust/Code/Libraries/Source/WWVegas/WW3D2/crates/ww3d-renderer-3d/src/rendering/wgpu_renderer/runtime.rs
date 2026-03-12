//! WGPU runtime helpers.
//!
//! This module provides a small builder that captures the boilerplate required to initialise
//! `wgpu` in a way that mirrors the flexibility of the legacy WW3D engine: we can either run
//! headless (for server-side validation/tests) or attach to a window surface for on-screen
//! rendering. The resulting [`RuntimeParts`] can then be fed into [`WgpuWrapper::from_parts`]
//! or any other subsystem that needs direct access to the `Device`, `Queue`, or `Surface`.

use crate::core::error::{Error, Result};
use std::sync::Arc;
use wgpu::{Adapter, Device, Instance, InstanceDescriptor, Queue, Surface, SurfaceConfiguration};

/// Output of the runtime builder – arcs to the WGPU primitives required by the renderer.
#[derive(Debug, Clone)]
pub struct RuntimeParts {
    pub instance: Option<Arc<Instance>>,
    pub adapter: Option<Arc<Adapter>>,
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub surface: Option<Arc<Surface<'static>>>,
    pub surface_config: SurfaceConfiguration,
}

/// Builder for a WGPU runtime. Start with [`RuntimeBuilder::headless`] or
/// [`RuntimeBuilder::with_window`] and tweak the configuration as required before calling
/// [`build`][RuntimeBuilder::build].
#[derive(Debug, Clone)]
pub struct RuntimeBuilder {
    window_size: Option<(u32, u32)>,
    surface_format: Option<wgpu::TextureFormat>,
    present_mode: wgpu::PresentMode,
    power_preference: wgpu::PowerPreference,
    required_features: wgpu::Features,
    required_limits: Option<wgpu::Limits>,
    window_callback: Option<WindowCallback>,
}

/// Internal callback for deferred window surface creation.
#[derive(Debug, Clone)]
enum WindowCallback {
    /// Create a surface from a raw window handle using `create_surface_unsafe`.
    UnsafeSurfaceCreation,
}

impl RuntimeBuilder {
    /// Create a builder for a headless renderer (rendering to an off-screen texture).
    pub fn headless(size: (u32, u32), format: wgpu::TextureFormat) -> Self {
        Self {
            window_size: Some(size),
            surface_format: Some(format),
            present_mode: wgpu::PresentMode::Immediate,
            power_preference: wgpu::PowerPreference::HighPerformance,
            required_features: wgpu::Features::empty(),
            required_limits: None,
            window_callback: None,
        }
    }

    /// Create a builder that will attach to a winit window (or any type that implements the
    /// required handle traits). The actual window reference is supplied to
    /// [`build_with_window`](RuntimeBuilder::build_with_window).
    pub fn with_window() -> Self {
        Self {
            window_size: None,
            surface_format: None,
            present_mode: wgpu::PresentMode::Fifo,
            power_preference: wgpu::PowerPreference::HighPerformance,
            required_features: wgpu::Features::empty(),
            required_limits: None,
            window_callback: Some(WindowCallback::UnsafeSurfaceCreation),
        }
    }

    /// Request additional WGPU features.
    pub fn required_features(mut self, features: wgpu::Features) -> Self {
        self.required_features |= features;
        self
    }

    /// Override the limits passed to `request_device`.
    pub fn required_limits(mut self, limits: wgpu::Limits) -> Self {
        self.required_limits = Some(limits);
        self
    }

    /// Select a presentation mode for surface-backed rendering.
    pub fn present_mode(mut self, present_mode: wgpu::PresentMode) -> Self {
        self.present_mode = present_mode;
        self
    }

    /// Explicitly set the anticipated surface size.
    pub fn window_size(mut self, size: (u32, u32)) -> Self {
        self.window_size = Some(size);
        self
    }

    /// Override the format used when configuring the surface (or headless target).
    pub fn surface_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.surface_format = Some(format);
        self
    }

    /// Configure the power preference used when requesting an adapter.
    pub fn power_preference(mut self, preference: wgpu::PowerPreference) -> Self {
        self.power_preference = preference;
        self
    }

    /// Build a headless runtime.
    pub fn build_headless(self) -> Result<RuntimeParts> {
        let Some((width, height)) = self.window_size else {
            return Err(Error::InvalidParameter(
                "headless runtime requires explicit size".into(),
            ));
        };
        let format = self
            .surface_format
            .ok_or_else(|| Error::InvalidParameter("headless runtime requires a format".into()))?;

        let instance = Arc::new(Instance::new(&InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        }));
        let adapter = Arc::new(
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: self.power_preference,
                compatible_surface: None,
                force_fallback_adapter: false,
            }))
            .map_err(|e| Error::AdapterNotFound(format!("No compatible adapter found: {e}")))?,
        );

        let (device, queue) = pollster::block_on(
            adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("WW3D Headless Device"),
                required_features: self.required_features,
                required_limits: self
                    .required_limits
                    .unwrap_or_else(wgpu::Limits::downlevel_defaults),
                ..Default::default()
            }),
        )
        .map_err(|e| Error::Generic(format!("Failed to request device: {e}")))?;

        let surface_config = SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: self.present_mode,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };

        Ok(RuntimeParts {
            instance: Some(instance),
            adapter: Some(adapter),
            device: Arc::new(device),
            queue: Arc::new(queue),
            surface: None,
            surface_config,
        })
    }

    /// Build a runtime that renders to an on-screen surface using the provided window.
    pub fn build_with_window<W>(self, window: &W) -> Result<RuntimeParts>
    where
        W: wgpu::rwh::HasWindowHandle + wgpu::rwh::HasDisplayHandle + Send + Sync + 'static,
    {
        let Some(WindowCallback::UnsafeSurfaceCreation) = self.window_callback else {
            return Err(Error::InvalidOperation(
                "builder was not configured for window rendering".into(),
            ));
        };

        let instance = Arc::new(Instance::new(&InstanceDescriptor::default()));
        let surface_target = unsafe { wgpu::SurfaceTargetUnsafe::from_window(window) }
            .map_err(|e| Error::Generic(format!("Failed to get window handle: {e}")))?;
        let surface = unsafe { instance.create_surface_unsafe(surface_target) }
            .map_err(|e| Error::Generic(format!("Failed to create surface: {e}")))?;
        let surface = Arc::new(surface);

        let adapter = Arc::new(
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: self.power_preference,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            }))
            .map_err(|e| Error::AdapterNotFound(format!("No compatible adapter found: {e}")))?,
        );

        let adapter_features = adapter.features();
        let mut required_features = self.required_features;
        if adapter_features.contains(wgpu::Features::TEXTURE_COMPRESSION_BC) {
            required_features |= wgpu::Features::TEXTURE_COMPRESSION_BC;
        }

        let (device, queue) = pollster::block_on(
            adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("WW3D Surface Device"),
                required_features,
                required_limits: self
                    .required_limits
                    .unwrap_or_else(|| wgpu::Limits::downlevel_defaults()),
                ..Default::default()
            }),
        )
        .map_err(|e| Error::Generic(format!("Failed to request device: {e}")))?;

        let capabilities = surface.get_capabilities(&adapter);
        let format = self
            .surface_format
            .or_else(|| capabilities.formats.first().copied())
            .unwrap_or(wgpu::TextureFormat::Bgra8Unorm);
        let present_mode = if capabilities.present_modes.contains(&self.present_mode) {
            self.present_mode
        } else {
            capabilities
                .present_modes
                .first()
                .copied()
                .unwrap_or(wgpu::PresentMode::Fifo)
        };
        let alpha_mode = capabilities
            .alpha_modes
            .first()
            .copied()
            .unwrap_or(wgpu::CompositeAlphaMode::Opaque);
        let (width, height) = self.window_size.unwrap_or((1, 1));

        let surface_config = SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_config);

        Ok(RuntimeParts {
            instance: Some(instance),
            adapter: Some(adapter),
            device: Arc::new(device),
            queue: Arc::new(queue),
            surface: Some(surface),
            surface_config,
        })
    }
}
