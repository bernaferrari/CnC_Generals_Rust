//! # Display Adapter Management
//!
//! Handles graphics adapter enumeration, capabilities detection, and selection using wgpu.

use super::{ColorFormat, DisplayMode, Resolution, Result, VideoDeviceError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(feature = "video")]
use wgpu::{Adapter, Backends, DeviceType, Instance, PowerPreference};

/// Graphics adapter information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayAdapter {
    /// Adapter ID
    pub id: String,
    /// Adapter name
    pub name: String,
    /// Vendor name
    pub vendor: String,
    /// Device ID
    pub device_id: u32,
    /// Vendor ID
    pub vendor_id: u32,
    /// Revision ID
    pub revision_id: u32,
    /// Device type (Discrete, Integrated, Virtual, CPU)
    pub device_type: AdapterDeviceType,
    /// Backend type (Vulkan, DX12, Metal, etc.)
    pub backend: BackendType,
    /// Is this the primary adapter
    pub is_primary: bool,
    /// Adapter capabilities
    pub capabilities: AdapterCapabilities,
    /// Supported display modes
    pub supported_modes: Vec<DisplayMode>,
    /// Connected displays
    pub displays: Vec<DisplayInfo>,
    /// Performance score (higher is better)
    pub performance_score: u32,
}

/// Adapter device type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdapterDeviceType {
    /// Discrete GPU (dedicated graphics card)
    DiscreteGpu,
    /// Integrated GPU (built into CPU)
    IntegratedGpu,
    /// Virtual GPU (software rendering)
    VirtualGpu,
    /// CPU-based software renderer
    Cpu,
    /// Unknown device type
    Unknown,
}

/// Graphics backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendType {
    /// Vulkan API
    Vulkan,
    /// DirectX 12 (Windows)
    Dx12,
    /// DirectX 11 (Windows)
    Dx11,
    /// Metal (macOS, iOS)
    Metal,
    /// OpenGL
    OpenGL,
    /// WebGPU
    WebGpu,
    /// Software fallback
    Software,
}

/// Display adapter capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterCapabilities {
    /// Dedicated video memory in bytes
    pub dedicated_video_memory: u64,
    /// Dedicated system memory in bytes  
    pub dedicated_system_memory: u64,
    /// Shared system memory in bytes
    pub shared_system_memory: u64,
    /// Maximum texture size (1D and 2D)
    pub max_texture_size_1d: u32,
    pub max_texture_size_2d: u32,
    pub max_texture_size_3d: u32,
    /// Maximum texture array layers
    pub max_texture_array_layers: u32,
    /// Maximum bind groups
    pub max_bind_groups: u32,
    /// Maximum bindings per bind group
    pub max_bindings_per_bind_group: u32,
    /// Maximum dynamic uniform buffers per pipeline layout
    pub max_dynamic_uniform_buffers_per_pipeline_layout: u32,
    /// Maximum dynamic storage buffers per pipeline layout
    pub max_dynamic_storage_buffers_per_pipeline_layout: u32,
    /// Maximum sampled textures per shader stage
    pub max_sampled_textures_per_shader_stage: u32,
    /// Maximum samplers per shader stage
    pub max_samplers_per_shader_stage: u32,
    /// Maximum storage buffers per shader stage
    pub max_storage_buffers_per_shader_stage: u32,
    /// Maximum storage textures per shader stage
    pub max_storage_textures_per_shader_stage: u32,
    /// Maximum uniform buffers per shader stage
    pub max_uniform_buffers_per_shader_stage: u32,
    /// Maximum uniform buffer binding size
    pub max_uniform_buffer_binding_size: u64,
    /// Maximum storage buffer binding size
    pub max_storage_buffer_binding_size: u64,
    /// Maximum vertex buffers
    pub max_vertex_buffers: u32,
    /// Maximum vertex attributes
    pub max_vertex_attributes: u32,
    /// Maximum vertex buffer array stride
    pub max_vertex_buffer_array_stride: u32,
    /// Maximum inter-stage shader components
    pub max_inter_stage_shader_components: u32,
    /// Maximum compute workgroup storage size
    pub max_compute_workgroup_storage_size: u32,
    /// Maximum compute invocations per workgroup
    pub max_compute_invocations_per_workgroup: u32,
    /// Maximum compute workgroup size X/Y/Z
    pub max_compute_workgroup_size_x: u32,
    pub max_compute_workgroup_size_y: u32,
    pub max_compute_workgroup_size_z: u32,
    /// Maximum compute workgroups per dimension
    pub max_compute_workgroups_per_dimension: u32,
    /// Minimum uniform buffer offset alignment
    pub min_uniform_buffer_offset_alignment: u32,
    /// Minimum storage buffer offset alignment
    pub min_storage_buffer_offset_alignment: u32,
    /// Supported features
    pub features: AdapterFeatures,
    /// Supported texture formats
    pub supported_texture_formats: Vec<ColorFormat>,
    /// Hardware features
    pub hardware_features: HashMap<String, bool>,
}

/// Advanced adapter features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterFeatures {
    /// Depth clamping
    pub depth_clip_control: bool,
    /// Depth32Float-stencil8 format
    pub depth32float_stencil8: bool,
    /// Timestamp queries
    pub timestamp_query: bool,
    /// Pipeline statistics queries  
    pub pipeline_statistics_query: bool,
    /// Texture compression BC
    pub texture_compression_bc: bool,
    /// Texture compression ETC2
    pub texture_compression_etc2: bool,
    /// Texture compression ASTC
    pub texture_compression_astc: bool,
    /// Indirect first instance
    pub indirect_first_instance: bool,
    /// Shader F16
    pub shader_f16: bool,
    /// RG11B10 UFloat renderable
    pub rg11b10ufloat_renderable: bool,
    /// BGRA8 UNorm storage
    pub bgra8unorm_storage: bool,
    /// Float32 filterable
    pub float32_filterable: bool,
    /// Ray tracing acceleration structure
    pub ray_tracing_acceleration_structure: bool,
    /// Ray query
    pub ray_query: bool,
    /// Shader unused vertex outputs
    pub shader_unused_vertex_outputs: bool,
    /// Texture adapter specific format features
    pub texture_adapter_specific_format_features: bool,
    /// Multi-draw indirect
    pub multi_draw_indirect: bool,
    /// Multi-draw indirect count
    pub multi_draw_indirect_count: bool,
    /// Push constants
    pub push_constants: bool,
    /// Address mode clamp to border
    pub address_mode_clamp_to_border: bool,
    /// Non-fill polygon mode
    pub polygon_mode_line: bool,
    pub polygon_mode_point: bool,
    /// Conservative rasterization
    pub conservative_rasterization: bool,
    /// Vertex writable storage
    pub vertex_writable_storage: bool,
    /// Clear commands
    pub clear_texture: bool,
    /// Spirv shader passthrough
    pub spirv_shader_passthrough: bool,
    /// Multiview
    pub multiview: bool,
    /// Vertex attribute 64bit
    pub vertex_attribute_64bit: bool,
    /// Texture format 16bit norm
    pub texture_format_16bit_norm: bool,
    /// Texture compression ASTC HDR
    pub texture_compression_astc_hdr: bool,
    /// Mappable primary buffers
    pub mappable_primary_buffers: bool,
    /// Buffer binding array
    pub buffer_binding_array: bool,
    /// Storage resource binding array
    pub storage_resource_binding_array: bool,
    /// Sampled texture and storage buffer array non-uniform indexing
    pub sampled_texture_and_storage_buffer_array_non_uniform_indexing: bool,
    /// Uniform buffer and storage texture array non-uniform indexing
    pub uniform_buffer_and_storage_texture_array_non_uniform_indexing: bool,
}

/// Display information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayInfo {
    /// Display ID
    pub id: String,
    /// Display name
    pub name: String,
    /// Native resolution
    pub native_resolution: Resolution,
    /// Current resolution
    pub current_resolution: Resolution,
    /// Physical size in millimeters
    pub physical_size_mm: (u32, u32),
    /// DPI (dots per inch)
    pub dpi: f32,
    /// Scale factor
    pub scale_factor: f32,
    /// Is primary display
    pub is_primary: bool,
    /// Display position
    pub position: (i32, i32),
    /// Display orientation
    pub orientation: DisplayOrientation,
    /// Color space
    pub color_space: ColorSpace,
    /// HDR support
    pub hdr_support: HdrSupport,
    /// Supported display modes
    pub supported_modes: Vec<DisplayMode>,
    /// Refresh rate range
    pub refresh_rate_range: (f32, f32),
    /// Color depth range
    pub color_depth_range: (u8, u8),
}

/// Display orientation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisplayOrientation {
    /// Landscape (normal)
    Landscape,
    /// Portrait
    Portrait,
    /// Landscape flipped
    LandscapeFlipped,
    /// Portrait flipped
    PortraitFlipped,
}

/// Color space support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorSpace {
    /// sRGB support
    pub srgb: bool,
    /// Adobe RGB support
    pub adobe_rgb: bool,
    /// DCI-P3 support
    pub dci_p3: bool,
    /// Rec. 2020 support
    pub rec_2020: bool,
    /// Custom color gamut
    pub custom_gamut: Option<String>,
    /// Wide color gamut support
    pub wide_color_gamut: bool,
}

/// HDR (High Dynamic Range) support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HdrSupport {
    /// HDR10 support
    pub hdr10: bool,
    /// HDR10+ support
    pub hdr10_plus: bool,
    /// Dolby Vision support
    pub dolby_vision: bool,
    /// Maximum luminance in nits
    pub max_luminance: f32,
    /// Minimum luminance in nits
    pub min_luminance: f32,
    /// Maximum content light level
    pub max_cll: f32,
    /// Maximum frame-average light level
    pub max_fall: f32,
    /// Electro-Optical Transfer Function
    pub eotf: HdrEotf,
}

/// HDR Electro-Optical Transfer Function
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HdrEotf {
    /// Standard Dynamic Range
    Sdr,
    /// Perceptual Quantizer (PQ) - SMPTE ST 2084
    Pq,
    /// Hybrid Log-Gamma (HLG) - ITU-R BT.2100
    Hlg,
}

impl DisplayAdapter {
    /// Enumerate all available display adapters using wgpu
    pub async fn enumerate() -> Result<Vec<DisplayAdapter>> {
        #[cfg(feature = "video")]
        {
            let mut backend_options = wgpu::BackendOptions::default();
            backend_options.dx12.shader_compiler = wgpu::Dx12Compiler::Fxc;

            let instance = Instance::new(&wgpu::InstanceDescriptor {
                backends: Backends::all(),
                flags: wgpu::InstanceFlags::default(),
                memory_budget_thresholds: Default::default(),
                backend_options,
            });

            let adapters = instance.enumerate_adapters(Backends::all());
            let mut display_adapters = Vec::new();

            for (index, adapter) in adapters.into_iter().enumerate() {
                let info = adapter.get_info();
                let limits = adapter.limits();
                let features = adapter.features();

                // Map wgpu device type to our enum
                let device_type = match info.device_type {
                    DeviceType::DiscreteGpu => AdapterDeviceType::DiscreteGpu,
                    DeviceType::IntegratedGpu => AdapterDeviceType::IntegratedGpu,
                    DeviceType::VirtualGpu => AdapterDeviceType::VirtualGpu,
                    DeviceType::Cpu => AdapterDeviceType::Cpu,
                    DeviceType::Other => AdapterDeviceType::Unknown,
                };

                // Map wgpu backend to our enum
                let backend = match info.backend {
                    wgpu::Backend::Vulkan => BackendType::Vulkan,
                    wgpu::Backend::Dx12 => BackendType::Dx12,
                    wgpu::Backend::Metal => BackendType::Metal,
                    wgpu::Backend::Gl => BackendType::OpenGL,
                    wgpu::Backend::BrowserWebGpu => BackendType::WebGpu,
                    _ => BackendType::Software,
                };

                // Calculate performance score based on device type and vendor
                let performance_score = Self::calculate_performance_score(&info, &features);

                // Build capabilities from wgpu limits and features
                let capabilities = AdapterCapabilities {
                    dedicated_video_memory: 0, // wgpu doesn't expose this directly
                    dedicated_system_memory: 0,
                    shared_system_memory: 0,
                    max_texture_size_1d: limits.max_texture_dimension_1d,
                    max_texture_size_2d: limits.max_texture_dimension_2d,
                    max_texture_size_3d: limits.max_texture_dimension_3d,
                    max_texture_array_layers: limits.max_texture_array_layers,
                    max_bind_groups: limits.max_bind_groups,
                    max_bindings_per_bind_group: limits.max_bindings_per_bind_group,
                    max_dynamic_uniform_buffers_per_pipeline_layout: limits
                        .max_dynamic_uniform_buffers_per_pipeline_layout,
                    max_dynamic_storage_buffers_per_pipeline_layout: limits
                        .max_dynamic_storage_buffers_per_pipeline_layout,
                    max_sampled_textures_per_shader_stage: limits
                        .max_sampled_textures_per_shader_stage,
                    max_samplers_per_shader_stage: limits.max_samplers_per_shader_stage,
                    max_storage_buffers_per_shader_stage: limits
                        .max_storage_buffers_per_shader_stage,
                    max_storage_textures_per_shader_stage: limits
                        .max_storage_textures_per_shader_stage,
                    max_uniform_buffers_per_shader_stage: limits
                        .max_uniform_buffers_per_shader_stage,
                    max_uniform_buffer_binding_size: limits.max_uniform_buffer_binding_size as u64,
                    max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size as u64,
                    max_vertex_buffers: limits.max_vertex_buffers,
                    max_vertex_attributes: limits.max_vertex_attributes,
                    max_vertex_buffer_array_stride: limits.max_vertex_buffer_array_stride,
                    max_inter_stage_shader_components: limits.max_inter_stage_shader_components,
                    max_compute_workgroup_storage_size: limits.max_compute_workgroup_storage_size,
                    max_compute_invocations_per_workgroup: limits
                        .max_compute_invocations_per_workgroup,
                    max_compute_workgroup_size_x: limits.max_compute_workgroup_size_x,
                    max_compute_workgroup_size_y: limits.max_compute_workgroup_size_y,
                    max_compute_workgroup_size_z: limits.max_compute_workgroup_size_z,
                    max_compute_workgroups_per_dimension: limits
                        .max_compute_workgroups_per_dimension,
                    min_uniform_buffer_offset_alignment: limits.min_uniform_buffer_offset_alignment,
                    min_storage_buffer_offset_alignment: limits.min_storage_buffer_offset_alignment,
                    features: Self::map_wgpu_features(&features),
                    supported_texture_formats: Self::get_supported_texture_formats(&adapter).await,
                    hardware_features: Self::detect_hardware_features(&info, &features),
                };

                let display_adapter = DisplayAdapter {
                    id: format!("adapter_{}", index),
                    name: info.name.clone(),
                    vendor: Self::vendor_name_from_id(info.vendor),
                    device_id: info.device,
                    vendor_id: info.vendor,
                    revision_id: 0, // wgpu doesn't expose revision
                    device_type,
                    backend,
                    is_primary: index == 0, // First adapter is typically primary
                    capabilities,
                    supported_modes: Self::generate_display_modes_for_adapter(device_type),
                    displays: Self::detect_displays().await,
                    performance_score,
                };

                display_adapters.push(display_adapter);
            }

            // Sort adapters by performance score (highest first)
            display_adapters.sort_by(|a, b| b.performance_score.cmp(&a.performance_score));

            Ok(display_adapters)
        }

        #[cfg(not(feature = "video"))]
        {
            // Fallback for when video feature is not enabled
            Err(VideoDeviceError::InitializationFailed(
                "Video feature not enabled".to_string(),
            ))
        }
    }

    /// Get the primary display adapter (highest performance)
    pub async fn get_primary() -> Result<DisplayAdapter> {
        let adapters = Self::enumerate().await?;
        adapters
            .into_iter()
            .next()
            .ok_or_else(|| VideoDeviceError::AdapterNotFound("No adapters found".to_string()))
    }

    /// Get adapter by power preference
    pub async fn get_by_power_preference(preference: PowerPreference) -> Result<DisplayAdapter> {
        let mut adapters = Self::enumerate().await?;

        if adapters.is_empty() {
            return Err(VideoDeviceError::AdapterNotFound(
                "No adapters found".to_string(),
            ));
        }

        match preference {
            PowerPreference::HighPerformance => adapters
                .iter()
                .find(|a| matches!(a.device_type, AdapterDeviceType::DiscreteGpu))
                .cloned()
                .or_else(|| adapters.first().cloned())
                .ok_or_else(|| {
                    VideoDeviceError::AdapterNotFound(
                        "No high-performance adapter found".to_string(),
                    )
                }),
            PowerPreference::LowPower => adapters
                .iter()
                .find(|a| matches!(a.device_type, AdapterDeviceType::IntegratedGpu))
                .cloned()
                .or_else(|| adapters.first().cloned())
                .ok_or_else(|| {
                    VideoDeviceError::AdapterNotFound("No low-power adapter found".to_string())
                }),
            PowerPreference::None => Ok(adapters.remove(0)),
        }
    }

    /// Find adapter by ID
    pub async fn find_by_id(id: &str) -> Result<DisplayAdapter> {
        let adapters = Self::enumerate().await?;
        adapters
            .into_iter()
            .find(|adapter| adapter.id == id)
            .ok_or_else(|| VideoDeviceError::AdapterNotFound(format!("Adapter not found: {}", id)))
    }

    /// Find adapter by name
    pub async fn find_by_name(name: &str) -> Result<DisplayAdapter> {
        let adapters = Self::enumerate().await?;
        adapters
            .into_iter()
            .find(|adapter| adapter.name.to_lowercase().contains(&name.to_lowercase()))
            .ok_or_else(|| {
                VideoDeviceError::AdapterNotFound(format!("Adapter not found: {}", name))
            })
    }

    /// Check if adapter supports a specific feature
    pub fn supports_feature(&self, feature: &str) -> bool {
        self.capabilities
            .hardware_features
            .get(feature)
            .copied()
            .unwrap_or(false)
    }

    /// Get best display mode for given resolution
    pub fn get_best_mode_for_resolution(
        &self,
        target_resolution: Resolution,
    ) -> Option<DisplayMode> {
        self.supported_modes
            .iter()
            .filter(|mode| mode.resolution == target_resolution)
            .max_by(|a, b| {
                a.refresh_rate
                    .hz
                    .partial_cmp(&b.refresh_rate.hz)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied()
    }

    /// Get memory budget for textures (estimated)
    pub fn get_texture_memory_budget(&self) -> u64 {
        // Return estimated memory based on device type
        match self.device_type {
            AdapterDeviceType::DiscreteGpu => {
                // Estimate 8GB for discrete GPU
                8 * 1024 * 1024 * 1024
            }
            AdapterDeviceType::IntegratedGpu => {
                // Estimate 2GB for integrated GPU
                2 * 1024 * 1024 * 1024
            }
            _ => {
                // Estimate 1GB for other types
                1024 * 1024 * 1024
            }
        }
    }

    /// Check if adapter supports ray tracing
    pub fn supports_ray_tracing(&self) -> bool {
        self.capabilities
            .features
            .ray_tracing_acceleration_structure
            && self.capabilities.features.ray_query
    }

    /// Check if adapter supports mesh shaders
    pub fn supports_mesh_shaders(&self) -> bool {
        self.supports_feature("mesh_shaders")
    }

    /// Check if adapter supports variable rate shading
    pub fn supports_variable_rate_shading(&self) -> bool {
        self.supports_feature("variable_rate_shading")
    }

    /// Get texture compression support summary
    pub fn get_texture_compression_support(&self) -> Vec<String> {
        let mut formats = Vec::new();

        if self.capabilities.features.texture_compression_bc {
            formats.push("BC (DirectX)".to_string());
        }
        if self.capabilities.features.texture_compression_etc2 {
            formats.push("ETC2 (Mobile)".to_string());
        }
        if self.capabilities.features.texture_compression_astc {
            formats.push("ASTC (Mobile/Vulkan)".to_string());
        }
        if self.capabilities.features.texture_compression_astc_hdr {
            formats.push("ASTC HDR".to_string());
        }

        formats
    }

    // Helper methods

    fn calculate_performance_score(info: &wgpu::AdapterInfo, features: &wgpu::Features) -> u32 {
        let mut score = 0u32;

        // Base score by device type
        match info.device_type {
            DeviceType::DiscreteGpu => score += 1000,
            DeviceType::IntegratedGpu => score += 500,
            DeviceType::VirtualGpu => score += 100,
            DeviceType::Cpu => score += 50,
            DeviceType::Other => score += 10,
        }

        // Bonus for advanced features
        if features.contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY) {
            score += 200;
        }
        if features.contains(wgpu::Features::TIMESTAMP_QUERY) {
            score += 50;
        }
        if features.contains(wgpu::Features::TEXTURE_COMPRESSION_BC) {
            score += 25;
        }

        // Vendor-specific bonuses (based on common high-performance vendors)
        match info.vendor {
            0x10DE => score += 100, // NVIDIA
            0x1002 => score += 90,  // AMD
            0x8086 => score += 50,  // Intel
            _ => {}
        }

        score
    }

    fn vendor_name_from_id(vendor_id: u32) -> String {
        match vendor_id {
            0x10DE => "NVIDIA".to_string(),
            0x1002 => "AMD".to_string(),
            0x8086 => "Intel".to_string(),
            0x1414 => "Microsoft".to_string(),
            0x5143 => "Qualcomm".to_string(),
            0x106B => "Apple".to_string(),
            0x1AE0 => "Google".to_string(),
            _ => format!("Unknown (0x{:04X})", vendor_id),
        }
    }

    fn map_wgpu_features(features: &wgpu::Features) -> AdapterFeatures {
        AdapterFeatures {
            depth_clip_control: features.contains(wgpu::Features::DEPTH_CLIP_CONTROL),
            depth32float_stencil8: features.contains(wgpu::Features::DEPTH32FLOAT_STENCIL8),
            timestamp_query: features.contains(wgpu::Features::TIMESTAMP_QUERY),
            pipeline_statistics_query: features.contains(wgpu::Features::PIPELINE_STATISTICS_QUERY),
            texture_compression_bc: features.contains(wgpu::Features::TEXTURE_COMPRESSION_BC),
            texture_compression_etc2: features.contains(wgpu::Features::TEXTURE_COMPRESSION_ETC2),
            texture_compression_astc: features.contains(wgpu::Features::TEXTURE_COMPRESSION_ASTC),
            indirect_first_instance: features.contains(wgpu::Features::INDIRECT_FIRST_INSTANCE),
            shader_f16: features.contains(wgpu::Features::SHADER_F16),
            rg11b10ufloat_renderable: features.contains(wgpu::Features::RG11B10UFLOAT_RENDERABLE),
            bgra8unorm_storage: features.contains(wgpu::Features::BGRA8UNORM_STORAGE),
            float32_filterable: features.contains(wgpu::Features::FLOAT32_FILTERABLE),
            ray_tracing_acceleration_structure: features
                .contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY),
            ray_query: features.contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY),
            shader_unused_vertex_outputs: false,
            texture_adapter_specific_format_features: features
                .contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES),
            multi_draw_indirect: features.contains(wgpu::Features::MULTI_DRAW_INDIRECT_COUNT),
            multi_draw_indirect_count: features.contains(wgpu::Features::MULTI_DRAW_INDIRECT_COUNT),
            push_constants: features.contains(wgpu::Features::PUSH_CONSTANTS),
            address_mode_clamp_to_border: features
                .contains(wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER),
            polygon_mode_line: features.contains(wgpu::Features::POLYGON_MODE_LINE),
            polygon_mode_point: features.contains(wgpu::Features::POLYGON_MODE_POINT),
            conservative_rasterization: features
                .contains(wgpu::Features::CONSERVATIVE_RASTERIZATION),
            vertex_writable_storage: features.contains(wgpu::Features::VERTEX_WRITABLE_STORAGE),
            clear_texture: features.contains(wgpu::Features::CLEAR_TEXTURE),
            spirv_shader_passthrough: false,
            multiview: features.contains(wgpu::Features::MULTIVIEW),
            vertex_attribute_64bit: features.contains(wgpu::Features::VERTEX_ATTRIBUTE_64BIT),
            texture_format_16bit_norm: features.contains(wgpu::Features::TEXTURE_FORMAT_16BIT_NORM),
            texture_compression_astc_hdr: features
                .contains(wgpu::Features::TEXTURE_COMPRESSION_ASTC_HDR),
            mappable_primary_buffers: features.contains(wgpu::Features::MAPPABLE_PRIMARY_BUFFERS),
            buffer_binding_array: features.contains(wgpu::Features::BUFFER_BINDING_ARRAY),
            storage_resource_binding_array: features
                .contains(wgpu::Features::STORAGE_RESOURCE_BINDING_ARRAY),
            sampled_texture_and_storage_buffer_array_non_uniform_indexing: features.contains(
                wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
            ),
            uniform_buffer_and_storage_texture_array_non_uniform_indexing: features
                .contains(wgpu::Features::UNIFORM_BUFFER_BINDING_ARRAYS)
                && features.contains(wgpu::Features::STORAGE_TEXTURE_ARRAY_NON_UNIFORM_INDEXING),
        }
    }

    async fn get_supported_texture_formats(_adapter: &Adapter) -> Vec<ColorFormat> {
        // Return common texture formats - in a real implementation,
        // you'd query the adapter for specific format support
        vec![
            ColorFormat::Rgba8,
            ColorFormat::Bgra8,
            ColorFormat::Rgba16,
            ColorFormat::Rgba32Float,
            ColorFormat::Rgb10A2,
            ColorFormat::Depth24Stencil8,
            ColorFormat::Depth32Float,
        ]
    }

    fn detect_hardware_features(
        info: &wgpu::AdapterInfo,
        features: &wgpu::Features,
    ) -> HashMap<String, bool> {
        let mut hw_features = HashMap::new();

        // Ray tracing support
        hw_features.insert(
            "ray_tracing".to_string(),
            features.contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY),
        );

        // Mesh shader support (vendor-specific detection)
        let mesh_shader_support = match info.vendor {
            0x10DE => true, // NVIDIA (Turing+)
            0x1002 => true, // AMD (RDNA2+)
            _ => false,
        };
        hw_features.insert("mesh_shaders".to_string(), mesh_shader_support);

        // Variable rate shading
        let vrs_support = features.contains(wgpu::Features::CONSERVATIVE_RASTERIZATION)
            || matches!(info.vendor, 0x10DE | 0x1002); // NVIDIA/AMD
        hw_features.insert("variable_rate_shading".to_string(), vrs_support);

        // Vendor-specific features
        match info.vendor {
            0x10DE => {
                hw_features.insert("dlss".to_string(), true);
                hw_features.insert("nvenc".to_string(), true);
            }
            0x1002 => {
                hw_features.insert("fsr".to_string(), true);
                hw_features.insert("vce".to_string(), true);
            }
            0x8086 => {
                hw_features.insert("quick_sync".to_string(), true);
                hw_features.insert("xe_hpg".to_string(), info.name.contains("Xe"));
            }
            _ => {}
        }

        // Texture compression
        hw_features.insert(
            "bc_compression".to_string(),
            features.contains(wgpu::Features::TEXTURE_COMPRESSION_BC),
        );
        hw_features.insert(
            "etc2_compression".to_string(),
            features.contains(wgpu::Features::TEXTURE_COMPRESSION_ETC2),
        );
        hw_features.insert(
            "astc_compression".to_string(),
            features.contains(wgpu::Features::TEXTURE_COMPRESSION_ASTC),
        );

        hw_features
    }

    async fn detect_displays() -> Vec<DisplayInfo> {
        #[cfg(feature = "video")]
        {
            use std::collections::HashSet;
            use winit::event_loop::EventLoopBuilder;

            let event_loop = std::panic::catch_unwind(|| EventLoopBuilder::new().build())
                .ok()
                .and_then(|result| result.ok());
            if let Some(event_loop) = event_loop {
                let primary_name = event_loop
                    .primary_monitor()
                    .and_then(|monitor| monitor.name());
                let mut displays = Vec::new();

                for (index, monitor) in event_loop.available_monitors().enumerate() {
                    let monitor_name = monitor
                        .name()
                        .unwrap_or_else(|| format!("Display {}", index + 1));
                    let is_primary = primary_name
                        .as_ref()
                        .map(|name| name == &monitor_name)
                        .unwrap_or(index == 0);

                    let mut seen = HashSet::new();
                    let mut modes = monitor
                        .video_modes()
                        .filter_map(|mode| {
                            let resolution = mode.size();
                            let hz = (mode.refresh_rate_millihertz() as f32 / 1000.0).max(1.0);
                            let hz_key = (hz * 1000.0).round() as u32;
                            let depth = mode.bit_depth() as u8;
                            let mode_key = (resolution.width, resolution.height, hz_key, depth);

                            if seen.insert(mode_key) {
                                Some(DisplayMode::new(
                                    Resolution::new(resolution.width, resolution.height),
                                    super::RefreshRate::new(hz),
                                    depth,
                                ))
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();

                    modes.sort_by(|a, b| {
                        b.resolution
                            .pixel_count()
                            .cmp(&a.resolution.pixel_count())
                            .then_with(|| {
                                b.refresh_rate
                                    .hz
                                    .partial_cmp(&a.refresh_rate.hz)
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            })
                            .then_with(|| b.color_depth.cmp(&a.color_depth))
                    });

                    let native_size = monitor.size();
                    let native_resolution = Resolution::new(native_size.width, native_size.height);
                    let current_resolution = modes
                        .first()
                        .map(|mode| mode.resolution)
                        .unwrap_or(native_resolution);
                    let scale_factor = monitor.scale_factor() as f32;
                    let dpi = (96.0 * scale_factor).max(1.0);
                    let pos = monitor.position();

                    let (refresh_min, refresh_max) = if modes.is_empty() {
                        (60.0, 60.0)
                    } else {
                        modes
                            .iter()
                            .fold((f32::MAX, 0.0f32), |(min_hz, max_hz), mode| {
                                (
                                    min_hz.min(mode.refresh_rate.hz),
                                    max_hz.max(mode.refresh_rate.hz),
                                )
                            })
                    };
                    let (depth_min, depth_max) = if modes.is_empty() {
                        (24u8, 32u8)
                    } else {
                        modes
                            .iter()
                            .fold((u8::MAX, 0u8), |(min_depth, max_depth), mode| {
                                (
                                    min_depth.min(mode.color_depth),
                                    max_depth.max(mode.color_depth),
                                )
                            })
                    };

                    if modes.is_empty() {
                        modes.push(DisplayMode::new(
                            current_resolution,
                            super::RefreshRate::rate_60hz(),
                            32,
                        ));
                    }

                    displays.push(DisplayInfo {
                        id: format!("display_{}", index),
                        name: monitor_name,
                        native_resolution,
                        current_resolution,
                        physical_size_mm: (0, 0),
                        dpi,
                        scale_factor,
                        is_primary,
                        position: (pos.x, pos.y),
                        orientation: DisplayOrientation::Landscape,
                        color_space: ColorSpace {
                            srgb: true,
                            adobe_rgb: false,
                            dci_p3: false,
                            rec_2020: false,
                            custom_gamut: None,
                            wide_color_gamut: false,
                        },
                        hdr_support: HdrSupport {
                            hdr10: false,
                            hdr10_plus: false,
                            dolby_vision: false,
                            max_luminance: 0.0,
                            min_luminance: 0.0,
                            max_cll: 0.0,
                            max_fall: 0.0,
                            eotf: HdrEotf::Sdr,
                        },
                        supported_modes: modes,
                        refresh_rate_range: (refresh_min, refresh_max),
                        color_depth_range: (depth_min, depth_max),
                    });
                }

                if !displays.is_empty() {
                    return displays;
                }
            }
        }

        vec![DisplayInfo {
            id: "display_0".to_string(),
            name: "Primary Display".to_string(),
            native_resolution: Resolution::new(1920, 1080),
            current_resolution: Resolution::new(1920, 1080),
            physical_size_mm: (0, 0),
            dpi: 96.0,
            scale_factor: 1.0,
            is_primary: true,
            position: (0, 0),
            orientation: DisplayOrientation::Landscape,
            color_space: ColorSpace {
                srgb: true,
                adobe_rgb: false,
                dci_p3: false,
                rec_2020: false,
                custom_gamut: None,
                wide_color_gamut: false,
            },
            hdr_support: HdrSupport {
                hdr10: false,
                hdr10_plus: false,
                dolby_vision: false,
                max_luminance: 0.0,
                min_luminance: 0.0,
                max_cll: 0.0,
                max_fall: 0.0,
                eotf: HdrEotf::Sdr,
            },
            supported_modes: vec![DisplayMode::new(
                Resolution::new(1920, 1080),
                super::RefreshRate::rate_60hz(),
                32,
            )],
            refresh_rate_range: (60.0, 60.0),
            color_depth_range: (32, 32),
        }]
    }

    fn generate_display_modes_for_adapter(device_type: AdapterDeviceType) -> Vec<DisplayMode> {
        match device_type {
            AdapterDeviceType::DiscreteGpu => vec![
                DisplayMode::new(
                    Resolution::new(1920, 1080),
                    super::RefreshRate::rate_60hz(),
                    32,
                ),
                DisplayMode::new(
                    Resolution::new(1920, 1080),
                    super::RefreshRate::rate_120hz(),
                    32,
                ),
                DisplayMode::new(
                    Resolution::new(1920, 1080),
                    super::RefreshRate::rate_144hz(),
                    32,
                ),
                DisplayMode::new(
                    Resolution::new(1920, 1080),
                    super::RefreshRate::rate_240hz(),
                    32,
                ),
                DisplayMode::new(
                    Resolution::new(2560, 1440),
                    super::RefreshRate::rate_60hz(),
                    32,
                ),
                DisplayMode::new(
                    Resolution::new(2560, 1440),
                    super::RefreshRate::rate_120hz(),
                    32,
                ),
                DisplayMode::new(
                    Resolution::new(2560, 1440),
                    super::RefreshRate::rate_144hz(),
                    32,
                ),
                DisplayMode::new(
                    Resolution::new(3840, 2160),
                    super::RefreshRate::rate_60hz(),
                    32,
                ),
                DisplayMode::new(
                    Resolution::new(3840, 2160),
                    super::RefreshRate::rate_120hz(),
                    32,
                ),
                DisplayMode::new(
                    Resolution::new(7680, 4320),
                    super::RefreshRate::rate_60hz(),
                    32,
                ),
            ],
            AdapterDeviceType::IntegratedGpu => vec![
                DisplayMode::new(
                    Resolution::new(1920, 1080),
                    super::RefreshRate::rate_60hz(),
                    32,
                ),
                DisplayMode::new(
                    Resolution::new(1920, 1080),
                    super::RefreshRate::rate_120hz(),
                    32,
                ),
                DisplayMode::new(
                    Resolution::new(2560, 1440),
                    super::RefreshRate::rate_60hz(),
                    32,
                ),
                DisplayMode::new(
                    Resolution::new(1600, 900),
                    super::RefreshRate::rate_60hz(),
                    32,
                ),
                DisplayMode::new(
                    Resolution::new(1366, 768),
                    super::RefreshRate::rate_60hz(),
                    32,
                ),
            ],
            _ => vec![
                DisplayMode::new(
                    Resolution::new(1920, 1080),
                    super::RefreshRate::rate_60hz(),
                    24,
                ),
                DisplayMode::new(
                    Resolution::new(1366, 768),
                    super::RefreshRate::rate_60hz(),
                    24,
                ),
                DisplayMode::new(
                    Resolution::new(1280, 720),
                    super::RefreshRate::rate_60hz(),
                    24,
                ),
                DisplayMode::new(
                    Resolution::new(1024, 768),
                    super::RefreshRate::rate_60hz(),
                    24,
                ),
            ],
        }
    }
}

// Default implementations

impl Default for AdapterCapabilities {
    fn default() -> Self {
        Self {
            dedicated_video_memory: 0,
            dedicated_system_memory: 0,
            shared_system_memory: 4 * 1024 * 1024 * 1024, // 4GB
            max_texture_size_1d: 8192,
            max_texture_size_2d: 8192,
            max_texture_size_3d: 2048,
            max_texture_array_layers: 256,
            max_bind_groups: 4,
            max_bindings_per_bind_group: 1000,
            max_dynamic_uniform_buffers_per_pipeline_layout: 8,
            max_dynamic_storage_buffers_per_pipeline_layout: 4,
            max_sampled_textures_per_shader_stage: 16,
            max_samplers_per_shader_stage: 16,
            max_storage_buffers_per_shader_stage: 8,
            max_storage_textures_per_shader_stage: 4,
            max_uniform_buffers_per_shader_stage: 12,
            max_uniform_buffer_binding_size: 65536,
            max_storage_buffer_binding_size: 134217728,
            max_vertex_buffers: 8,
            max_vertex_attributes: 16,
            max_vertex_buffer_array_stride: 2048,
            max_inter_stage_shader_components: 60,
            max_compute_workgroup_storage_size: 16384,
            max_compute_invocations_per_workgroup: 256,
            max_compute_workgroup_size_x: 256,
            max_compute_workgroup_size_y: 256,
            max_compute_workgroup_size_z: 64,
            max_compute_workgroups_per_dimension: 65535,
            min_uniform_buffer_offset_alignment: 256,
            min_storage_buffer_offset_alignment: 256,
            features: AdapterFeatures::default(),
            supported_texture_formats: vec![ColorFormat::Rgba8],
            hardware_features: HashMap::new(),
        }
    }
}

impl Default for AdapterFeatures {
    fn default() -> Self {
        Self {
            depth_clip_control: false,
            depth32float_stencil8: false,
            timestamp_query: false,
            pipeline_statistics_query: false,
            texture_compression_bc: false,
            texture_compression_etc2: false,
            texture_compression_astc: false,
            indirect_first_instance: false,
            shader_f16: false,
            rg11b10ufloat_renderable: false,
            bgra8unorm_storage: false,
            float32_filterable: false,
            ray_tracing_acceleration_structure: false,
            ray_query: false,
            shader_unused_vertex_outputs: false,
            texture_adapter_specific_format_features: false,
            multi_draw_indirect: false,
            multi_draw_indirect_count: false,
            push_constants: false,
            address_mode_clamp_to_border: false,
            polygon_mode_line: false,
            polygon_mode_point: false,
            conservative_rasterization: false,
            vertex_writable_storage: false,
            clear_texture: false,
            spirv_shader_passthrough: false,
            multiview: false,
            vertex_attribute_64bit: false,
            texture_format_16bit_norm: false,
            texture_compression_astc_hdr: false,
            mappable_primary_buffers: false,
            buffer_binding_array: false,
            storage_resource_binding_array: false,
            sampled_texture_and_storage_buffer_array_non_uniform_indexing: false,
            uniform_buffer_and_storage_texture_array_non_uniform_indexing: false,
        }
    }
}

impl Default for ColorSpace {
    fn default() -> Self {
        Self {
            srgb: true,
            adobe_rgb: false,
            dci_p3: false,
            rec_2020: false,
            custom_gamut: None,
            wide_color_gamut: false,
        }
    }
}

impl Default for HdrSupport {
    fn default() -> Self {
        Self {
            hdr10: false,
            hdr10_plus: false,
            dolby_vision: false,
            max_luminance: 100.0,
            min_luminance: 0.1,
            max_cll: 100.0,
            max_fall: 50.0,
            eotf: HdrEotf::Sdr,
        }
    }
}

impl std::fmt::Display for AdapterDeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdapterDeviceType::DiscreteGpu => write!(f, "Discrete GPU"),
            AdapterDeviceType::IntegratedGpu => write!(f, "Integrated GPU"),
            AdapterDeviceType::VirtualGpu => write!(f, "Virtual GPU"),
            AdapterDeviceType::Cpu => write!(f, "CPU"),
            AdapterDeviceType::Unknown => write!(f, "Unknown"),
        }
    }
}

impl std::fmt::Display for BackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendType::Vulkan => write!(f, "Vulkan"),
            BackendType::Dx12 => write!(f, "DirectX 12"),
            BackendType::Dx11 => write!(f, "DirectX 11"),
            BackendType::Metal => write!(f, "Metal"),
            BackendType::OpenGL => write!(f, "OpenGL"),
            BackendType::WebGpu => write!(f, "WebGPU"),
            BackendType::Software => write!(f, "Software"),
        }
    }
}
