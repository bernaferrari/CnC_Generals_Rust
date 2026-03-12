//! Compatibility Layer - API-Compatible Factory Functions
//!
//! The original engine swapped between multiple DirectX backends. The Rust port
//! only targets the modern WGPU renderer, but we expose a similar façade so code
//! written against the old API keeps working.

use crate::{
    wgpu_backend::create_wgpu_shader_interface, ShdDefClass, ShdError, ShdInterface, ShdResult,
};
use std::sync::{Arc, OnceLock};

#[derive(Clone)]
pub struct WgpuContext {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub surface_format: wgpu::TextureFormat,
}

static WGPU_CONTEXT: OnceLock<WgpuContext> = OnceLock::new();

/// Initialise the modern renderer. This is effectively a thin wrapper around
/// the standard WGPU device creation flow.
pub async fn initialize_rendering() -> ShdResult<()> {
    let context = initialize_wgpu().await?;
    WGPU_CONTEXT
        .set(context)
        .map_err(|_| ShdError::InvalidConfig("WGPU context already initialised".to_string()))?;
    Ok(())
}

async fn initialize_wgpu() -> ShdResult<WgpuContext> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .map_err(|_| ShdError::HardwareUnsupported("No suitable WGPU adapter found".to_string()))?;

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("WWShade WGPU Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        })
        .await
        .map_err(|e| {
            ShdError::HardwareUnsupported(format!("Failed to create WGPU device: {}", e))
        })?;

    Ok(WgpuContext {
        device: Arc::new(device),
        queue: Arc::new(queue),
        surface_format: wgpu::TextureFormat::Bgra8UnormSrgb,
    })
}

/// Create a shader interface backed by WGPU.
pub fn create_shader_interface(
    class_id: u32,
    definition: Arc<dyn ShdDefClass>,
) -> ShdResult<Box<dyn ShdInterface>> {
    let context = WGPU_CONTEXT.get().ok_or_else(|| {
        ShdError::InvalidConfig(
            "Rendering system not initialised. Call initialize_rendering() first.".to_string(),
        )
    })?;

    create_wgpu_shader_interface(
        class_id,
        definition,
        context.device.clone(),
        context.queue.clone(),
        context.surface_format,
    )
}

/// Convenience helper for callers that previously expected a backend summary.
pub fn get_backend_info() -> String {
    if WGPU_CONTEXT.get().is_some() {
        "Modern WGPU Backend (Windows/macOS/Linux)".to_string()
    } else {
        "WGPU backend not initialised".to_string()
    }
}

/// Check whether the WGPU runtime is ready.
pub fn has_modern_support() -> bool {
    WGPU_CONTEXT.get().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_backend_detection() {
        initialize_rendering().await.unwrap();
        assert!(has_modern_support());
        assert!(get_backend_info().contains("WGPU"));
    }
}
