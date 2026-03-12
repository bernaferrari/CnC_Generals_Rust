//! WGPU Device Management
//!
//! This module provides device management functionality for WGPU,
//! replacing DirectX8 device management.

use crate::core::error::{Error, RendererResult};
use std::sync::Arc;
use wgpu::{Adapter, Device, Instance, Queue, Surface};

/// WGPU device manager
pub struct WgpuDeviceManager {
    device: Arc<Device>,
    queue: Arc<Queue>,
    adapter: Arc<Adapter>,
    instance: Arc<Instance>,
}

impl WgpuDeviceManager {
    /// Create new device manager
    pub fn new(_window_handle: Option<*mut std::ffi::c_void>) -> RendererResult<Self> {
        let instance = Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .map_err(|e| Error::Generic(format!("Failed to find suitable adapter: {e}")))?;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            label: Some("WW3D Device"),
            ..Default::default()
        }))
        .map_err(|e| Error::Generic(format!("Failed to request device: {}", e)))?;

        Ok(Self {
            device: Arc::new(device),
            queue: Arc::new(queue),
            adapter: Arc::new(adapter),
            instance: Arc::new(instance),
        })
    }

    /// Get device reference
    pub fn device(&self) -> &Arc<Device> {
        &self.device
    }

    /// Get queue reference
    pub fn queue(&self) -> &Arc<Queue> {
        &self.queue
    }

    /// Get adapter reference
    pub fn adapter(&self) -> &Arc<Adapter> {
        &self.adapter
    }

    /// Create a surface for the provided window.
    pub fn create_surface_from_window<W>(&self, window: &W) -> RendererResult<Arc<Surface<'static>>>
    where
        W: wgpu::rwh::HasWindowHandle + wgpu::rwh::HasDisplayHandle + Send + Sync,
    {
        let unsafe_target = unsafe { wgpu::SurfaceTargetUnsafe::from_window(window) }
            .map_err(|e| Error::Generic(format!("Failed to get window handle: {e}")))?;
        // SAFETY: the returned surface target keeps the window alive for the duration of the surface.
        let surface = unsafe { self.instance.create_surface_unsafe(unsafe_target) }
            .map_err(|e| Error::Generic(format!("Failed to create surface: {e}")))?;
        Ok(Arc::new(surface))
    }

    /// Create surface from window handle
    pub fn create_surface(
        &self,
        window_handle: *mut std::ffi::c_void,
    ) -> RendererResult<Surface<'_>> {
        let _ = window_handle;
        Err(Error::InvalidOperation(
            "Raw window handles are not supported on this path; use create_surface_from_window"
                .to_string(),
        ))
    }
}
