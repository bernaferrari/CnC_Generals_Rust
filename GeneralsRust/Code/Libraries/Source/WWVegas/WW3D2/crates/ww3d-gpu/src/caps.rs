//! GPU Capability Detection and Management
//!
//! This module provides GPU capability detection, feature querying,
//! and capability management for different GPU vendors and architectures.

use crate::*;

/// GPU capabilities manager
#[derive(Debug)]
pub struct GpuCapabilitiesManager {
    /// Device capabilities
    device_caps: DeviceCapabilities,
    /// Feature support
    feature_support: FeatureSupport,
    /// Vendor-specific capabilities
    vendor_caps: VendorCapabilities,
    /// Performance capabilities
    performance_caps: PerformanceCapabilities,
}

impl GpuCapabilitiesManager {
    /// Create a new capabilities manager
    pub fn new(adapter: &crate::adapter::GpuAdapter) -> Self {
        let device_caps = DeviceCapabilities::from_adapter(adapter);
        let feature_support = FeatureSupport::from_adapter(adapter);
        let vendor_caps = VendorCapabilities::from_adapter(adapter);
        let performance_caps = PerformanceCapabilities::from_adapter(adapter);

        Self {
            device_caps,
            feature_support,
            vendor_caps,
            performance_caps,
        }
    }

    /// Get device capabilities
    pub fn device_caps(&self) -> &DeviceCapabilities {
        &self.device_caps
    }

    /// Get feature support
    pub fn feature_support(&self) -> &FeatureSupport {
        &self.feature_support
    }

    /// Get vendor capabilities
    pub fn vendor_caps(&self) -> &VendorCapabilities {
        &self.vendor_caps
    }

    /// Get performance capabilities
    pub fn performance_caps(&self) -> &PerformanceCapabilities {
        &self.performance_caps
    }

    /// Check if a specific feature is supported
    pub fn supports_feature(&self, feature: GpuFeature) -> bool {
        self.feature_support.supported_features.contains(&feature)
    }

    /// Get recommended settings based on capabilities
    pub fn get_recommended_settings(&self) -> RecommendedSettings {
        RecommendedSettings::from_capabilities(self)
    }

    /// Check if the GPU meets minimum requirements
    pub fn meets_minimum_requirements(&self) -> bool {
        // Check minimum requirements
        self.device_caps.max_texture_size >= 1024
            && self.device_caps.max_bind_groups >= 4
            && self.device_caps.max_uniform_buffer_binding_size >= 65536
            && self.supports_feature(GpuFeature::Instancing)
    }

    /// Get capability report as string
    pub fn capability_report(&self) -> String {
        format!(
            "GPU Capabilities Report:\n\
             Device: {}\n\
             Vendor: {}\n\
             Max Texture Size: {}x{}\n\
             Max Bind Groups: {}\n\
             Compute Shaders: {}\n\
             Geometry Shaders: {}\n\
             Tessellation: {}\n\
             Anisotropic Filtering: {}\n\
             Performance Score: {}",
            self.device_caps.device_name,
            self.vendor_caps.vendor_name,
            self.device_caps.max_texture_size,
            self.device_caps.max_texture_size,
            self.device_caps.max_bind_groups,
            self.supports_feature(GpuFeature::ComputeShaders),
            self.supports_feature(GpuFeature::GeometryShaders),
            self.supports_feature(GpuFeature::TessellationShaders),
            self.supports_feature(GpuFeature::AnisotropicFiltering),
            self.performance_caps.performance_score
        )
    }
}

/// Device capabilities
#[derive(Debug, Clone)]
pub struct DeviceCapabilities {
    pub device_name: String,
    pub device_type: crate::adapter::DeviceType,
    pub max_texture_size: u32,
    pub max_texture_array_layers: u32,
    pub max_bind_groups: u32,
    pub max_bindings_per_bind_group: u32,
    pub max_uniform_buffer_binding_size: u64,
    pub max_storage_buffer_binding_size: u64,
    pub max_vertex_buffers: u32,
    pub max_buffer_size: u64,
    pub max_vertex_attributes: u32,
    pub max_compute_workgroup_size_x: u32,
    pub max_compute_workgroup_size_y: u32,
    pub max_compute_workgroup_size_z: u32,
    pub max_compute_invocations_per_workgroup: u32,
    pub supports_astc_compression: bool,
    pub supports_bc_compression: bool,
    pub supports_etc2_compression: bool,
}

impl DeviceCapabilities {
    /// Create device capabilities from adapter
    pub fn from_adapter(adapter: &crate::adapter::GpuAdapter) -> Self {
        let limits = adapter.limits();

        Self {
            device_name: adapter.name().to_string(),
            device_type: adapter.info().device_type,
            max_texture_size: limits.max_texture_dimension_2d,
            max_texture_array_layers: limits.max_texture_array_layers,
            max_bind_groups: limits.max_bind_groups,
            max_bindings_per_bind_group: limits.max_bindings_per_bind_group,
            max_uniform_buffer_binding_size: limits.max_uniform_buffer_binding_size as u64,
            max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size as u64,
            max_vertex_buffers: limits.max_vertex_buffers,
            max_buffer_size: limits.max_buffer_size,
            max_vertex_attributes: limits.max_vertex_attributes,
            max_compute_workgroup_size_x: limits.max_compute_workgroup_size_x,
            max_compute_workgroup_size_y: limits.max_compute_workgroup_size_y,
            max_compute_workgroup_size_z: limits.max_compute_workgroup_size_z,
            max_compute_invocations_per_workgroup: limits.max_compute_invocations_per_workgroup,
            supports_astc_compression: false, // Would need to check texture format support
            supports_bc_compression: false,
            supports_etc2_compression: false,
        }
    }
}

/// Feature support
#[derive(Debug, Clone)]
pub struct FeatureSupport {
    pub supported_features: Vec<GpuFeature>,
    pub unsupported_features: Vec<GpuFeature>,
    pub experimental_features: Vec<GpuFeature>,
}

impl FeatureSupport {
    /// Create feature support from adapter
    pub fn from_adapter(adapter: &crate::adapter::GpuAdapter) -> Self {
        let limits = adapter.limits();
        let downlevel = adapter.wgpu_adapter().get_downlevel_capabilities();
        let mut supported_features = Vec::new();
        let mut unsupported_features = Vec::new();

        let mut record = |feature: GpuFeature, condition: bool| {
            if condition {
                supported_features.push(feature);
            } else {
                unsupported_features.push(feature);
            }
        };

        record(GpuFeature::Instancing, limits.max_vertex_buffers > 0);
        record(
            GpuFeature::ComputeShaders,
            downlevel
                .flags
                .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS),
        );
        record(GpuFeature::GeometryShaders, false);
        record(GpuFeature::TessellationShaders, false);
        record(
            GpuFeature::AnisotropicFiltering,
            downlevel
                .flags
                .contains(wgpu::DownlevelFlags::ANISOTROPIC_FILTERING),
        );
        record(
            GpuFeature::Msaa,
            limits.max_sampled_textures_per_shader_stage > 0,
        );

        // Texture operations - modern GPUs support these natively
        // ADD operation: supported on all modern GPUs
        record(GpuFeature::TexOpAdd, true);

        // MODULATE2X: supported on all modern GPUs (simple multiply by 2)
        record(GpuFeature::TexOpModulate2X, true);

        // BUMPENVMAP: bump mapping is emulated in WGSL for compatibility
        // Modern approach uses normal mapping instead
        record(GpuFeature::TexOpBumpEnvMap, false);

        // BUMPENVMAPLUMINANCE: legacy feature, not supported in modern APIs
        record(GpuFeature::TexOpBumpEnvMapLuminance, false);

        // Always supported features
        supported_features.push(GpuFeature::MultiThreading);

        Self {
            supported_features,
            unsupported_features,
            experimental_features: Vec::new(),
        }
    }
}

/// Vendor-specific capabilities
#[derive(Debug, Clone)]
pub struct VendorCapabilities {
    pub vendor: crate::adapter::Vendor,
    pub vendor_name: String,
    pub driver_version: String,
    pub optimal_texture_formats: Vec<wgpu::TextureFormat>,
    pub vendor_extensions: Vec<String>,
    pub known_issues: Vec<String>,
}

impl VendorCapabilities {
    /// Create vendor capabilities from adapter
    pub fn from_adapter(adapter: &crate::adapter::GpuAdapter) -> Self {
        let info = adapter.info();
        let vendor = info.vendor;
        let vendor_name = vendor.name().to_string();

        let optimal_texture_formats = match vendor {
            crate::adapter::Vendor::Nvidia => vec![
                wgpu::TextureFormat::Rgba8Unorm,
                wgpu::TextureFormat::Rgba16Float,
                wgpu::TextureFormat::Rgba32Float,
            ],
            crate::adapter::Vendor::Amd => vec![
                wgpu::TextureFormat::Rgba8Unorm,
                wgpu::TextureFormat::Rgba16Float,
            ],
            crate::adapter::Vendor::Intel => vec![
                wgpu::TextureFormat::Rgba8Unorm,
                wgpu::TextureFormat::Rgba16Float,
            ],
            _ => vec![wgpu::TextureFormat::Rgba8Unorm],
        };

        let (vendor_extensions, known_issues) = match vendor {
            crate::adapter::Vendor::Nvidia => (
                vec!["NV_mesh_shader".to_string(), "NV_ray_tracing".to_string()],
                vec!["Some drivers have issues with geometry shaders".to_string()],
            ),
            crate::adapter::Vendor::Amd => (
                vec!["AMD_mesh_shader".to_string()],
                vec!["Older drivers may have shader compilation issues".to_string()],
            ),
            crate::adapter::Vendor::Intel => (
                vec!["INTEL_performance_monitoring".to_string()],
                vec!["Integrated GPUs may have lower performance".to_string()],
            ),
            _ => (vec![], vec![]),
        };

        Self {
            vendor,
            vendor_name,
            driver_version: info.driver_info.clone(),
            optimal_texture_formats,
            vendor_extensions,
            known_issues,
        }
    }
}

/// Performance capabilities
#[derive(Debug, Clone)]
pub struct PerformanceCapabilities {
    pub performance_score: u32,
    pub memory_bandwidth_gb_s: f32,
    pub compute_units: u32,
    pub clock_speed_mhz: u32,
    pub supports_async_compute: bool,
    pub supports_async_transfer: bool,
    pub optimal_workgroup_size: (u32, u32, u32),
}

impl PerformanceCapabilities {
    /// Create performance capabilities from adapter
    pub fn from_adapter(adapter: &crate::adapter::GpuAdapter) -> Self {
        let device_type = adapter.info().device_type;
        let vendor = adapter.info().vendor;

        // Estimate performance score based on device type and vendor
        let mut performance_score = 100; // Base score

        match device_type {
            crate::adapter::DeviceType::DiscreteGpu => performance_score += 1000,
            crate::adapter::DeviceType::IntegratedGpu => performance_score += 100,
            _ => {}
        }

        match vendor {
            crate::adapter::Vendor::Nvidia => performance_score += 200,
            crate::adapter::Vendor::Amd => performance_score += 150,
            _ => {}
        }

        // Estimate other capabilities
        let (memory_bandwidth, compute_units, clock_speed) = match device_type {
            crate::adapter::DeviceType::DiscreteGpu => (200.0, 20, 1500),
            crate::adapter::DeviceType::IntegratedGpu => (50.0, 8, 1000),
            _ => (10.0, 4, 500),
        };

        Self {
            performance_score,
            memory_bandwidth_gb_s: memory_bandwidth,
            compute_units,
            clock_speed_mhz: clock_speed,
            supports_async_compute: device_type == crate::adapter::DeviceType::DiscreteGpu,
            supports_async_transfer: true,
            optimal_workgroup_size: (64, 1, 1),
        }
    }
}

/// Recommended settings based on capabilities
#[derive(Debug, Clone)]
pub struct RecommendedSettings {
    pub texture_filter_mode: wgpu::FilterMode,
    pub texture_address_mode: wgpu::AddressMode,
    pub anisotropy_level: u16,
    pub shadow_map_resolution: u32,
    pub max_render_targets: u32,
    pub enable_msaa: bool,
    pub msaa_sample_count: u32,
    pub enable_compute_shaders: bool,
    pub max_particles: usize,
}

impl RecommendedSettings {
    /// Create recommended settings from capabilities
    pub fn from_capabilities(caps: &GpuCapabilitiesManager) -> Self {
        let supports_anisotropic = caps.supports_feature(GpuFeature::AnisotropicFiltering);
        let supports_msaa = caps.supports_feature(GpuFeature::Msaa);
        let supports_compute = caps.supports_feature(GpuFeature::ComputeShaders);

        let anisotropy_level = if supports_anisotropic { 16 } else { 1 };
        let msaa_sample_count = if supports_msaa { 4 } else { 1 };
        let shadow_map_resolution = if caps.device_caps().max_texture_size >= 4096 {
            2048
        } else {
            1024
        };

        Self {
            texture_filter_mode: wgpu::FilterMode::Linear,
            texture_address_mode: wgpu::AddressMode::ClampToEdge,
            anisotropy_level,
            shadow_map_resolution,
            max_render_targets: caps.device_caps().max_bind_groups.min(8),
            enable_msaa: supports_msaa,
            msaa_sample_count,
            enable_compute_shaders: supports_compute,
            max_particles: if supports_compute { 100000 } else { 10000 },
        }
    }
}

/// Capability requirement checker
#[derive(Debug)]
pub struct CapabilityRequirements {
    pub required_features: Vec<GpuFeature>,
    pub minimum_texture_size: u32,
    pub minimum_buffer_size: u64,
    pub minimum_bind_groups: u32,
    pub requires_compute: bool,
    pub requires_geometry_shaders: bool,
    pub requires_tessellation: bool,
}

impl CapabilityRequirements {
    /// Create minimum requirements
    pub fn minimum() -> Self {
        Self {
            required_features: vec![GpuFeature::Instancing],
            minimum_texture_size: 1024,
            minimum_buffer_size: 65536,
            minimum_bind_groups: 4,
            requires_compute: false,
            requires_geometry_shaders: false,
            requires_tessellation: false,
        }
    }

    /// Create high-end requirements
    pub fn high_end() -> Self {
        Self {
            required_features: vec![
                GpuFeature::Instancing,
                GpuFeature::ComputeShaders,
                GpuFeature::AnisotropicFiltering,
                GpuFeature::Msaa,
            ],
            minimum_texture_size: 4096,
            minimum_buffer_size: 134217728, // 128MB
            minimum_bind_groups: 8,
            requires_compute: true,
            requires_geometry_shaders: false,
            requires_tessellation: false,
        }
    }

    /// Check if capabilities meet requirements
    pub fn check(&self, caps: &GpuCapabilitiesManager) -> RequirementCheckResult {
        let mut missing_features = Vec::new();
        let mut unmet_requirements = Vec::new();

        // Check features
        for feature in &self.required_features {
            if !caps.supports_feature(*feature) {
                missing_features.push(*feature);
            }
        }

        // Check limits
        let device_caps = caps.device_caps();
        if device_caps.max_texture_size < self.minimum_texture_size {
            unmet_requirements.push(format!(
                "Texture size: {} < {}",
                device_caps.max_texture_size, self.minimum_texture_size
            ));
        }

        if device_caps.max_buffer_size < self.minimum_buffer_size {
            unmet_requirements.push(format!(
                "Buffer size: {} < {}",
                device_caps.max_buffer_size, self.minimum_buffer_size
            ));
        }

        if device_caps.max_bind_groups < self.minimum_bind_groups {
            unmet_requirements.push(format!(
                "Bind groups: {} < {}",
                device_caps.max_bind_groups, self.minimum_bind_groups
            ));
        }

        // Check specific feature requirements
        if self.requires_compute && !caps.supports_feature(GpuFeature::ComputeShaders) {
            missing_features.push(GpuFeature::ComputeShaders);
        }

        if self.requires_geometry_shaders && !caps.supports_feature(GpuFeature::GeometryShaders) {
            missing_features.push(GpuFeature::GeometryShaders);
        }

        if self.requires_tessellation && !caps.supports_feature(GpuFeature::TessellationShaders) {
            missing_features.push(GpuFeature::TessellationShaders);
        }

        let meets_requirements = missing_features.is_empty() && unmet_requirements.is_empty();

        RequirementCheckResult {
            meets_requirements,
            missing_features,
            unmet_requirements,
        }
    }
}

/// Requirement check result
#[derive(Debug, Clone)]
pub struct RequirementCheckResult {
    pub meets_requirements: bool,
    pub missing_features: Vec<GpuFeature>,
    pub unmet_requirements: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_requirements_minimum() {
        let reqs = CapabilityRequirements::minimum();
        assert_eq!(reqs.minimum_texture_size, 1024);
        assert_eq!(reqs.minimum_bind_groups, 4);
        assert!(!reqs.requires_compute);
    }

    #[test]
    fn test_capability_requirements_high_end() {
        let reqs = CapabilityRequirements::high_end();
        assert_eq!(reqs.minimum_texture_size, 4096);
        assert!(reqs.requires_compute);
        assert_eq!(reqs.required_features.len(), 4);
    }

    #[test]
    fn test_recommended_settings() {
        // Mock capabilities for testing
        let device_caps = DeviceCapabilities {
            device_name: "Test GPU".to_string(),
            device_type: crate::adapter::DeviceType::DiscreteGpu,
            max_texture_size: 8192,
            max_texture_array_layers: 256,
            max_bind_groups: 8,
            max_bindings_per_bind_group: 16,
            max_uniform_buffer_binding_size: 65536,
            max_storage_buffer_binding_size: 134217728,
            max_vertex_buffers: 8,
            max_buffer_size: 268435456,
            max_vertex_attributes: 16,
            max_compute_workgroup_size_x: 1024,
            max_compute_workgroup_size_y: 1024,
            max_compute_workgroup_size_z: 64,
            max_compute_invocations_per_workgroup: 1024,
            supports_astc_compression: false,
            supports_bc_compression: false,
            supports_etc2_compression: false,
        };

        let feature_support = FeatureSupport {
            supported_features: vec![
                GpuFeature::AnisotropicFiltering,
                GpuFeature::Msaa,
                GpuFeature::ComputeShaders,
            ],
            unsupported_features: vec![],
            experimental_features: vec![],
        };

        let vendor_caps = VendorCapabilities {
            vendor: crate::adapter::Vendor::Nvidia,
            vendor_name: "NVIDIA".to_string(),
            driver_version: "1.0".to_string(),
            optimal_texture_formats: vec![],
            vendor_extensions: vec![],
            known_issues: vec![],
        };

        let performance_caps = PerformanceCapabilities {
            performance_score: 1200,
            memory_bandwidth_gb_s: 200.0,
            compute_units: 20,
            clock_speed_mhz: 1500,
            supports_async_compute: true,
            supports_async_transfer: true,
            optimal_workgroup_size: (64, 1, 1),
        };

        let caps_manager = GpuCapabilitiesManager {
            device_caps,
            feature_support,
            vendor_caps,
            performance_caps,
        };

        let settings = RecommendedSettings::from_capabilities(&caps_manager);
        assert_eq!(settings.anisotropy_level, 16);
        assert!(settings.enable_msaa);
        assert_eq!(settings.msaa_sample_count, 4);
        assert!(settings.enable_compute_shaders);
    }

    #[test]
    fn test_requirement_check_result() {
        let result = RequirementCheckResult {
            meets_requirements: false,
            missing_features: vec![GpuFeature::ComputeShaders],
            unmet_requirements: vec!["Texture size too small".to_string()],
        };

        assert!(!result.meets_requirements);
        assert_eq!(result.missing_features.len(), 1);
        assert_eq!(result.unmet_requirements.len(), 1);
    }
}
