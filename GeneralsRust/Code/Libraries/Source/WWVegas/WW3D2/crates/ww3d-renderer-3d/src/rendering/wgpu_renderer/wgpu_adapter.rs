//! WGPU Adapter Management
//!
//! This module handles WGPU adapter selection and management,
//! equivalent to the DirectX8 device enumeration functionality.

use crate::core::error::{Error, Result};
use pollster::block_on;
use std::sync::Arc;
use wgpu::{Adapter, Instance, PowerPreference, RequestAdapterOptions};

/// WGPU Adapter manager
pub struct WgpuAdapterManager {
    /// Available adapters
    adapters: Vec<Arc<Adapter>>,
    /// Selected adapter
    selected_adapter: Option<Arc<Adapter>>,
    /// Instance reference
    instance: Option<Arc<Instance>>,
}

impl WgpuAdapterManager {
    /// Create new adapter manager
    pub fn new() -> Self {
        Self {
            adapters: Vec::new(),
            selected_adapter: None,
            instance: None,
        }
    }

    /// Set WGPU instance
    pub fn set_instance(&mut self, instance: Arc<Instance>) {
        self.instance = Some(instance);
    }

    /// Enumerate available adapters
    pub fn enumerate_adapters(&mut self) -> Result<()> {
        let instance = self
            .instance
            .as_ref()
            .ok_or_else(|| Error::GenericError("Instance not created".to_string()))?;

        // Request high-performance adapter
        let adapter = block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }));

        if let Ok(adapter) = adapter {
            self.adapters.push(Arc::new(adapter));
        }

        // Request low-power adapter as fallback
        let adapter = block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        }));

        if let Ok(adapter) = adapter {
            // Only add if different from the high-performance adapter
            if !self
                .adapters
                .iter()
                .any(|a| a.get_info().device == adapter.get_info().device)
            {
                self.adapters.push(Arc::new(adapter));
            }
        }

        Ok(())
    }

    /// Get adapter count
    pub fn adapter_count(&self) -> usize {
        self.adapters.len()
    }

    /// Get adapter by index
    pub fn get_adapter(&self, index: usize) -> Option<&Arc<Adapter>> {
        self.adapters.get(index)
    }

    /// Select adapter by index
    pub fn select_adapter(&mut self, index: usize) -> Result<()> {
        if let Some(adapter) = self.adapters.get(index) {
            self.selected_adapter = Some(adapter.clone());
            Ok(())
        } else {
            Err(Error::AdapterNotFound(
                "Adapter index out of range".to_string(),
            ))
        }
    }

    /// Get selected adapter
    pub fn selected_adapter(&self) -> Option<&Arc<Adapter>> {
        self.selected_adapter.as_ref()
    }

    /// Auto-select best adapter
    pub fn select_best_adapter(&mut self) -> Result<()> {
        if self.adapters.is_empty() {
            return Err(Error::AdapterNotFound("No adapters available".to_string()));
        }

        // Select the first adapter (already ordered by preference)
        self.selected_adapter = Some(self.adapters[0].clone());
        Ok(())
    }

    /// Get adapter information
    pub fn get_adapter_info(&self, index: usize) -> Option<AdapterInfo> {
        self.adapters.get(index).map(|adapter| {
            let info = adapter.get_info();
            AdapterInfo {
                name: info.name,
                vendor: info.vendor,
                device: info.device,
                device_type: info.device_type,
                backend: info.backend,
                features: adapter.features(),
                limits: adapter.limits(),
            }
        })
    }

    /// Check if adapter supports feature
    pub fn adapter_supports_feature(&self, index: usize, feature: wgpu::Features) -> bool {
        self.adapters
            .get(index)
            .map(|adapter| adapter.features().contains(feature))
            .unwrap_or(false)
    }

    /// Get selected adapter features
    pub fn selected_adapter_features(&self) -> Option<wgpu::Features> {
        self.selected_adapter
            .as_ref()
            .map(|adapter| adapter.features())
    }

    /// Get selected adapter limits
    pub fn selected_adapter_limits(&self) -> Option<wgpu::Limits> {
        self.selected_adapter
            .as_ref()
            .map(|adapter| adapter.limits())
    }

    /// Check if selected adapter supports surface
    pub fn selected_adapter_supports_surface(&self, surface: &wgpu::Surface) -> bool {
        if let Some(adapter) = &self.selected_adapter {
            !surface.get_capabilities(adapter).formats.is_empty()
        } else {
            false
        }
    }

    /// Clear adapters
    pub fn clear(&mut self) {
        self.selected_adapter = None;
        self.adapters.clear();
    }

    /// Cleanup resources
    pub fn cleanup(&mut self) {
        self.clear();
        self.instance = None;
    }
}

impl Default for WgpuAdapterManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Extended adapter information
#[derive(Debug, Clone)]
pub struct AdapterInfo {
    pub name: String,
    pub vendor: u32,
    pub device: u32,
    pub device_type: wgpu::DeviceType,
    pub backend: wgpu::Backend,
    pub features: wgpu::Features,
    pub limits: wgpu::Limits,
}

impl AdapterInfo {
    /// Get vendor name
    pub fn vendor_name(&self) -> &'static str {
        match self.vendor {
            0x1002 => "AMD",
            0x10DE => "NVIDIA",
            0x8086 => "Intel",
            0x13B5 => "ARM",
            _ => "Unknown",
        }
    }

    /// Get device type name
    pub fn device_type_name(&self) -> &'static str {
        match self.device_type {
            wgpu::DeviceType::DiscreteGpu => "Discrete GPU",
            wgpu::DeviceType::IntegratedGpu => "Integrated GPU",
            wgpu::DeviceType::VirtualGpu => "Virtual GPU",
            wgpu::DeviceType::Cpu => "CPU",
            _ => "Unknown",
        }
    }

    /// Get backend name
    pub fn backend_name(&self) -> &'static str {
        match self.backend {
            wgpu::Backend::Vulkan => "Vulkan",
            wgpu::Backend::Metal => "Metal",
            wgpu::Backend::Dx12 => "DirectX 12",
            // Note: Dx11 backend was removed in recent WGPU versions
            wgpu::Backend::Gl => "OpenGL",
            wgpu::Backend::BrowserWebGpu => "WebGPU",
            _ => "Unknown",
        }
    }

    /// Check if adapter supports common features
    pub fn supports_common_features(&self) -> bool {
        self.features
            .contains(wgpu::Features::TEXTURE_BINDING_ARRAY)
            && self.features.contains(
                wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
            )
    }

    /// Get maximum texture size
    pub fn max_texture_size(&self) -> u32 {
        self.limits.max_texture_dimension_2d
    }

    /// Get maximum buffer size
    pub fn max_buffer_size(&self) -> u64 {
        self.limits.max_buffer_size
    }

    /// Get maximum bind groups
    pub fn max_bind_groups(&self) -> u32 {
        self.limits.max_bind_groups
    }
}

/// Adapter selection utilities
pub struct AdapterSelectionUtils;

impl AdapterSelectionUtils {
    /// Select best adapter based on criteria
    pub fn select_best_adapter(
        adapters: &[Arc<Adapter>],
        prefer_discrete: bool,
        require_features: Option<wgpu::Features>,
    ) -> Option<usize> {
        let mut best_score = -1i32;
        let mut best_index = None;

        for (index, adapter) in adapters.iter().enumerate() {
            let info = adapter.get_info();
            let features = adapter.features();
            let limits = adapter.limits();

            // Check required features
            if let Some(required) = require_features {
                if !features.contains(required) {
                    continue;
                }
            }

            // Calculate score based on device type and capabilities
            let mut score = 0i32;

            // Prefer discrete GPUs if requested
            if prefer_discrete && info.device_type == wgpu::DeviceType::DiscreteGpu {
                score += 1000;
            }

            // Prefer better device types
            match info.device_type {
                wgpu::DeviceType::DiscreteGpu => score += 100,
                wgpu::DeviceType::IntegratedGpu => score += 50,
                wgpu::DeviceType::VirtualGpu => score += 25,
                wgpu::DeviceType::Cpu => score += 10,
                _ => score += 1,
            }

            // Prefer higher texture limits
            score += (limits.max_texture_dimension_2d / 1000) as i32;

            // Prefer more features
            let feature_bits = features.bits().0;
            let feature_count: i32 = feature_bits.iter().map(|b| b.count_ones() as i32).sum();
            score += feature_count;

            if score > best_score {
                best_score = score;
                best_index = Some(index);
            }
        }

        best_index
    }

    /// Filter adapters by minimum requirements
    pub fn filter_adapters(
        adapters: &[Arc<Adapter>],
        min_texture_size: u32,
        required_features: Option<wgpu::Features>,
    ) -> Vec<usize> {
        adapters
            .iter()
            .enumerate()
            .filter_map(|(index, adapter)| {
                let limits = adapter.limits();
                let features = adapter.features();

                // Check minimum texture size
                if limits.max_texture_dimension_2d < min_texture_size {
                    return None;
                }

                // Check required features
                if let Some(required) = required_features {
                    if !features.contains(required) {
                        return None;
                    }
                }

                Some(index)
            })
            .collect()
    }

    /// Get adapter compatibility score with surface
    pub fn get_surface_compatibility_score(adapter: &Adapter, surface: &wgpu::Surface) -> i32 {
        let capabilities = surface.get_capabilities(adapter);

        if capabilities.formats.is_empty() {
            return 0; // Not compatible
        }

        let mut score = 100; // Base compatibility score

        // Prefer certain formats
        if capabilities
            .formats
            .contains(&wgpu::TextureFormat::Bgra8Unorm)
        {
            score += 50;
        }

        // Prefer certain present modes
        if capabilities
            .present_modes
            .contains(&wgpu::PresentMode::Fifo)
        {
            score += 25;
        }

        if capabilities
            .present_modes
            .contains(&wgpu::PresentMode::Mailbox)
        {
            score += 10;
        }

        score
    }
}

/// Adapter enumeration result
#[derive(Debug)]
pub struct AdapterEnumerationResult {
    pub adapters: Vec<AdapterInfo>,
    pub recommended_index: Option<usize>,
}

impl AdapterEnumerationResult {
    /// Create new enumeration result
    pub fn new(adapters: Vec<AdapterInfo>, recommended_index: Option<usize>) -> Self {
        Self {
            adapters,
            recommended_index,
        }
    }

    /// Get recommended adapter info
    pub fn recommended_adapter(&self) -> Option<&AdapterInfo> {
        self.recommended_index
            .and_then(|index| self.adapters.get(index))
    }

    /// Check if any adapters are available
    pub fn has_adapters(&self) -> bool {
        !self.adapters.is_empty()
    }

    /// Get adapter count
    pub fn adapter_count(&self) -> usize {
        self.adapters.len()
    }
}
