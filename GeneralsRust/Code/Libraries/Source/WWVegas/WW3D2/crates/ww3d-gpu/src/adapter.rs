//! GPU Adapter Management
//!
//! This module provides GPU adapter enumeration, selection, and management.
//! It handles different GPU backends and provides a unified interface for
//! adapter discovery and capability querying.

use crate::*;
use std::fmt;

/// GPU adapter abstraction
#[derive(Debug)]
pub struct GpuAdapter {
    /// WGPU adapter handle
    adapter: wgpu::Adapter,
    /// Adapter information
    info: AdapterInfo,
    /// Adapter capabilities
    capabilities: AdapterCapabilities,
    /// Adapter features
    features: wgpu::Features,
    /// Adapter limits
    limits: wgpu::Limits,
}

impl GpuAdapter {
    /// Create a new GPU adapter wrapper
    pub fn new(adapter: wgpu::Adapter) -> Self {
        let info = AdapterInfo::from_adapter(&adapter);
        let capabilities = AdapterCapabilities::from_adapter(&adapter);
        let features = adapter.features();
        let limits = adapter.limits();

        Self {
            adapter,
            info,
            capabilities,
            features,
            limits,
        }
    }

    /// Get the underlying WGPU adapter
    pub fn wgpu_adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }

    /// Get adapter information
    pub fn info(&self) -> &AdapterInfo {
        &self.info
    }

    /// Get adapter capabilities
    pub fn capabilities(&self) -> &AdapterCapabilities {
        &self.capabilities
    }

    /// Get adapter features
    pub fn features(&self) -> wgpu::Features {
        self.features
    }

    /// Get adapter limits
    pub fn limits(&self) -> wgpu::Limits {
        self.limits.clone()
    }

    /// Check if the adapter is a discrete GPU
    pub fn is_discrete_gpu(&self) -> bool {
        matches!(self.info.device_type, DeviceType::DiscreteGpu)
    }

    /// Check if the adapter is an integrated GPU
    pub fn is_integrated_gpu(&self) -> bool {
        matches!(self.info.device_type, DeviceType::IntegratedGpu)
    }

    /// Check if the adapter is a CPU device
    pub fn is_cpu(&self) -> bool {
        matches!(self.info.device_type, DeviceType::Cpu)
    }

    /// Check if the adapter is a virtual GPU
    pub fn is_virtual_gpu(&self) -> bool {
        matches!(self.info.device_type, DeviceType::VirtualGpu)
    }

    /// Check if the adapter is an other type
    pub fn is_other(&self) -> bool {
        matches!(self.info.device_type, DeviceType::Other)
    }

    /// Get adapter name
    pub fn name(&self) -> &str {
        &self.info.name
    }

    /// Get adapter vendor
    pub fn vendor(&self) -> Vendor {
        self.info.vendor
    }

    /// Get adapter score for automatic selection
    pub fn selection_score(&self) -> u32 {
        let mut score = 0;

        // Prefer discrete GPUs
        if self.is_discrete_gpu() {
            score += 1000;
        } else if self.is_integrated_gpu() {
            score += 100;
        }

        // Prefer higher memory limits
        score += (self.limits.max_buffer_size / 1_000_000) as u32;

        // Prefer more features
        let feature_bits = self.features.bits().0;
        let feature_count: u32 = feature_bits.iter().map(|bits| bits.count_ones()).sum();
        score += feature_count * 10;

        score
    }

    /// Check if adapter supports a specific feature
    pub fn supports_feature(&self, feature: wgpu::Features) -> bool {
        self.features.contains(feature)
    }

    /// Check if adapter supports required limits
    pub fn supports_limits(&self, required_limits: &wgpu::Limits) -> bool {
        self.limits.max_texture_dimension_2d >= required_limits.max_texture_dimension_2d
            && self.limits.max_bind_groups >= required_limits.max_bind_groups
            && self.limits.max_bindings_per_bind_group
                >= required_limits.max_bindings_per_bind_group
            && self.limits.max_uniform_buffer_binding_size
                >= required_limits.max_uniform_buffer_binding_size
            && self.limits.max_storage_buffer_binding_size
                >= required_limits.max_storage_buffer_binding_size
    }

    /// Create a device from this adapter
    pub async fn create_device(
        &self,
        features: wgpu::Features,
        limits: wgpu::Limits,
    ) -> Result<crate::device::GpuDevice, GpuError> {
        let required_features = features & self.features;
        let required_limits = wgpu::Limits {
            max_texture_dimension_2d: limits
                .max_texture_dimension_2d
                .min(self.limits.max_texture_dimension_2d),
            max_bind_groups: limits.max_bind_groups.min(self.limits.max_bind_groups),
            max_bindings_per_bind_group: limits
                .max_bindings_per_bind_group
                .min(self.limits.max_bindings_per_bind_group),
            max_uniform_buffer_binding_size: limits
                .max_uniform_buffer_binding_size
                .min(self.limits.max_uniform_buffer_binding_size),
            max_storage_buffer_binding_size: limits
                .max_storage_buffer_binding_size
                .min(self.limits.max_storage_buffer_binding_size),
            ..limits
        };

        let (device, queue) = self
            .adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some(&format!("WW3D Device ({})", self.info.name)),
                required_features,
                required_limits,
                ..Default::default()
            })
            .await
            .map_err(|_e| GpuError::DeviceLost)?;

        let downlevel = self.adapter.get_downlevel_capabilities();

        Ok(crate::device::GpuDevice::new_with_downlevel(
            device, queue, downlevel,
        ))
    }
}

impl fmt::Display for GpuAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.info.name, self.info.device_type)
    }
}

/// Adapter information
#[derive(Debug, Clone)]
pub struct AdapterInfo {
    pub name: String,
    pub vendor: Vendor,
    pub device: u32,
    pub device_type: DeviceType,
    pub driver: String,
    pub driver_info: String,
    pub backend: Backend,
}

impl AdapterInfo {
    /// Create adapter info from WGPU adapter
    pub fn from_adapter(adapter: &wgpu::Adapter) -> Self {
        let info = adapter.get_info();

        Self {
            name: info.name.clone(),
            vendor: Vendor::from_id(info.vendor),
            device: info.device,
            device_type: DeviceType::from_wgpu(info.device_type),
            driver: info.driver.clone(),
            driver_info: info.driver_info.clone(),
            backend: Backend::from_wgpu(info.backend),
        }
    }
}

/// GPU device types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    /// Other or unknown
    Other,
    /// Integrated GPU
    IntegratedGpu,
    /// Discrete GPU
    DiscreteGpu,
    /// Virtual GPU
    VirtualGpu,
    /// CPU device
    Cpu,
}

impl DeviceType {
    /// Convert from WGPU device type
    pub fn from_wgpu(device_type: wgpu::DeviceType) -> Self {
        match device_type {
            wgpu::DeviceType::Other => Self::Other,
            wgpu::DeviceType::IntegratedGpu => Self::IntegratedGpu,
            wgpu::DeviceType::DiscreteGpu => Self::DiscreteGpu,
            wgpu::DeviceType::VirtualGpu => Self::VirtualGpu,
            wgpu::DeviceType::Cpu => Self::Cpu,
        }
    }
}

impl fmt::Display for DeviceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Other => write!(f, "Other"),
            Self::IntegratedGpu => write!(f, "Integrated GPU"),
            Self::DiscreteGpu => write!(f, "Discrete GPU"),
            Self::VirtualGpu => write!(f, "Virtual GPU"),
            Self::Cpu => write!(f, "CPU"),
        }
    }
}

/// GPU vendors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vendor {
    /// Unknown vendor
    Unknown,
    /// Google (SwiftShader)
    Google,
    /// AMD
    Amd,
    /// Apple
    Apple,
    /// ARM
    Arm,
    /// Broadcom
    Broadcom,
    /// Imagination Technologies
    Imagination,
    /// Intel
    Intel,
    /// Microsoft
    Microsoft,
    /// NVIDIA
    Nvidia,
    /// Qualcomm
    Qualcomm,
}

impl Vendor {
    /// Create vendor from vendor ID
    pub fn from_id(vendor_id: u32) -> Self {
        match vendor_id {
            0x1002 => Self::Amd,
            0x1010 => Self::Imagination,
            0x106B => Self::Apple,
            0x10DE => Self::Nvidia,
            0x1414 => Self::Microsoft,
            0x1AE0 => Self::Google,
            0x8086 => Self::Intel,
            0x5143 => Self::Qualcomm,
            _ => Self::Unknown,
        }
    }

    /// Get vendor name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::Google => "Google",
            Self::Amd => "AMD",
            Self::Apple => "Apple",
            Self::Arm => "ARM",
            Self::Broadcom => "Broadcom",
            Self::Imagination => "Imagination",
            Self::Intel => "Intel",
            Self::Microsoft => "Microsoft",
            Self::Nvidia => "NVIDIA",
            Self::Qualcomm => "Qualcomm",
        }
    }
}

/// GPU backends
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// No-op backend
    Noop,
    /// Vulkan
    Vulkan,
    /// DirectX 12
    Dx12,
    /// DirectX 11
    Dx11,
    /// Metal
    Metal,
    /// OpenGL
    Gl,
    /// WebGPU
    WebGpu,
    /// Browser WebGPU
    BrowserWebGpu,
}

impl Backend {
    /// Convert from WGPU backend
    pub fn from_wgpu(backend: wgpu::Backend) -> Self {
        match backend {
            wgpu::Backend::Noop => Self::Noop,
            wgpu::Backend::Vulkan => Self::Vulkan,
            wgpu::Backend::Dx12 => Self::Dx12,
            // wgpu::Backend::Dx11 => Self::Dx11, // Dx11 not available in current wgpu
            wgpu::Backend::Metal => Self::Metal,
            wgpu::Backend::Gl => Self::Gl,
            // wgpu::Backend::WebGpu => Self::WebGpu, // WebGpu not available in current wgpu
            wgpu::Backend::BrowserWebGpu => Self::BrowserWebGpu,
        }
    }
}

impl fmt::Display for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Noop => write!(f, "Noop"),
            Self::Vulkan => write!(f, "Vulkan"),
            Self::Dx12 => write!(f, "DirectX 12"),
            Self::Dx11 => write!(f, "DirectX 11"),
            Self::Metal => write!(f, "Metal"),
            Self::Gl => write!(f, "OpenGL"),
            Self::WebGpu => write!(f, "WebGPU"),
            Self::BrowserWebGpu => write!(f, "Browser WebGPU"),
        }
    }
}

/// Adapter capabilities
#[derive(Debug, Clone)]
pub struct AdapterCapabilities {
    pub supports_compute: bool,
    pub supports_graphics: bool,
    pub supports_transfer: bool,
    pub max_texture_size: u32,
    pub max_buffer_size: u64,
    pub shader_model: ShaderModel,
    pub supported_texture_formats: Vec<wgpu::TextureFormat>,
    pub backend: Backend,
}

impl AdapterCapabilities {
    /// Create capabilities from WGPU adapter
    pub fn from_adapter(adapter: &wgpu::Adapter) -> Self {
        let features = adapter.features();
        let limits = adapter.limits();
        let info = adapter.get_info();

        Self {
            supports_compute: features.contains(wgpu::Features::PUSH_CONSTANTS),
            supports_graphics: true, // All WGPU adapters support graphics
            supports_transfer: true, // All WGPU adapters support transfer
            max_texture_size: limits.max_texture_dimension_2d,
            max_buffer_size: limits.max_buffer_size,
            shader_model: ShaderModel::Sm50, // WGPU supports SM 5.0+ equivalent
            supported_texture_formats: Vec::new(), // Would need to query supported formats
            backend: Backend::from_wgpu(info.backend),
        }
    }
}

/// Shader model support
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ShaderModel {
    /// Shader Model 3.0
    Sm30,
    /// Shader Model 4.0
    Sm40,
    /// Shader Model 5.0
    Sm50,
    /// Shader Model 6.0
    Sm60,
}

/// GPU adapter manager for enumeration and selection
#[derive(Debug)]
pub struct AdapterManager {
    adapters: Vec<GpuAdapter>,
}

impl AdapterManager {
    /// Create a new adapter manager and enumerate adapters
    pub async fn new() -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapters = instance
            .enumerate_adapters(wgpu::Backends::all())
            .into_iter()
            .map(GpuAdapter::new)
            .collect();

        Ok(Self { adapters })
    }

    /// Get all available adapters
    pub fn adapters(&self) -> &[GpuAdapter] {
        &self.adapters
    }

    /// Find the best adapter for rendering
    pub fn select_best_adapter(&self) -> Option<&GpuAdapter> {
        self.adapters
            .iter()
            .max_by_key(|adapter| adapter.selection_score())
    }

    /// Find adapter by name
    pub fn find_adapter_by_name(&self, name: &str) -> Option<&GpuAdapter> {
        self.adapters
            .iter()
            .find(|adapter| adapter.name().contains(name))
    }

    /// Find discrete GPU adapters
    pub fn discrete_gpus(&self) -> Vec<&GpuAdapter> {
        self.adapters
            .iter()
            .filter(|adapter| adapter.is_discrete_gpu())
            .collect()
    }

    /// Find integrated GPU adapters
    pub fn integrated_gpus(&self) -> Vec<&GpuAdapter> {
        self.adapters
            .iter()
            .filter(|adapter| adapter.is_integrated_gpu())
            .collect()
    }

    /// Get adapter count
    pub fn adapter_count(&self) -> usize {
        self.adapters.len()
    }

    /// Print adapter information
    pub fn print_adapter_info(&self) {
        println!("Available GPU Adapters:");
        println!("========================");

        for (i, adapter) in self.adapters.iter().enumerate() {
            println!("Adapter {}: {}", i, adapter);
            println!("  Vendor: {}", adapter.vendor().name());
            println!("  Backend: {}", adapter.capabilities().backend);
            println!("  Score: {}", adapter.selection_score());
            println!();
        }

        if let Some(best) = self.select_best_adapter() {
            println!("Recommended adapter: {}", best);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vendor_from_id() {
        assert_eq!(Vendor::from_id(0x10DE), Vendor::Nvidia);
        assert_eq!(Vendor::from_id(0x1002), Vendor::Amd);
        assert_eq!(Vendor::from_id(0x8086), Vendor::Intel);
        assert_eq!(Vendor::from_id(0x106B), Vendor::Apple);
        assert_eq!(Vendor::from_id(0x1234), Vendor::Unknown);
    }

    #[test]
    fn test_device_type_from_wgpu() {
        assert_eq!(
            DeviceType::from_wgpu(wgpu::DeviceType::DiscreteGpu),
            DeviceType::DiscreteGpu
        );
        assert_eq!(
            DeviceType::from_wgpu(wgpu::DeviceType::IntegratedGpu),
            DeviceType::IntegratedGpu
        );
        assert_eq!(
            DeviceType::from_wgpu(wgpu::DeviceType::Cpu),
            DeviceType::Cpu
        );
    }

    #[test]
    fn test_backend_from_wgpu() {
        assert_eq!(Backend::from_wgpu(wgpu::Backend::Vulkan), Backend::Vulkan);
        assert_eq!(Backend::from_wgpu(wgpu::Backend::Dx12), Backend::Dx12);
        assert_eq!(Backend::from_wgpu(wgpu::Backend::Metal), Backend::Metal);
    }

    #[test]
    fn test_vendor_name() {
        assert_eq!(Vendor::Nvidia.name(), "NVIDIA");
        assert_eq!(Vendor::Amd.name(), "AMD");
        assert_eq!(Vendor::Intel.name(), "Intel");
        assert_eq!(Vendor::Unknown.name(), "Unknown");
    }

    #[test]
    fn test_device_type_display() {
        assert_eq!(format!("{}", DeviceType::DiscreteGpu), "Discrete GPU");
        assert_eq!(format!("{}", DeviceType::IntegratedGpu), "Integrated GPU");
        assert_eq!(format!("{}", DeviceType::Cpu), "CPU");
    }

    #[test]
    fn test_backend_display() {
        assert_eq!(format!("{}", Backend::Vulkan), "Vulkan");
        assert_eq!(format!("{}", Backend::Dx12), "DirectX 12");
        assert_eq!(format!("{}", Backend::Metal), "Metal");
    }

    #[test]
    fn test_shader_model_ordering() {
        assert!(ShaderModel::Sm30 < ShaderModel::Sm40);
        assert!(ShaderModel::Sm50 < ShaderModel::Sm60);
        assert!(ShaderModel::Sm40 < ShaderModel::Sm50);
    }
}
