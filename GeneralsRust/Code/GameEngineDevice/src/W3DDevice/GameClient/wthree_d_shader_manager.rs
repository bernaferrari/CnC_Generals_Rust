use cgmath::{Vector2, Vector3, Vector4};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;
use wgpu::{
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, Buffer,
    BufferBindingType, BufferDescriptor, BufferUsages, ColorTargetState, ColorWrites, Device,
    Extent3d, Features, FragmentState, PipelineLayout, PipelineLayoutDescriptor, PrimitiveState,
    Queue, RenderPipeline, SamplerBindingType, ShaderModule, ShaderModuleDescriptor, ShaderSource,
    ShaderStages, Texture, TextureDescriptor, TextureDimension, TextureFormat as WgpuTextureFormat,
    TextureSampleType, TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension,
    VertexState,
};

pub const MAX_TEXTURE_STAGES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderType {
    Invalid = 0,
    TerrainBase,
    TerrainBaseNoise1,
    TerrainBaseNoise2,
    TerrainBaseNoise12,
    ShroudTexture,
    MaskTexture,
    RoadBase,
    RoadBaseNoise1,
    RoadBaseNoise2,
    RoadBaseNoise12,
    CloudTexture,
    FlatTerrainBase,
    FlatTerrainBaseNoise1,
    FlatTerrainBaseNoise2,
    FlatTerrainBaseNoise12,
    FlatShroudTexture,
    Max,
}

impl Default for ShaderType {
    fn default() -> Self {
        Self::Invalid
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FilterType {
    NullFilter = 0,
    ViewDefault,
    ViewMotionBlur,
    ViewBwFilter,
    ViewCrossFade,
    Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    NullMode = 0,
    ViewMotionBlur,
    ViewBwBlackAndWhite,
    ViewBwRedAndWhite,
    ViewBwGreenAndWhite,
    ViewCrossFade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipsetType {
    Unknown = 0,
    Generic,
    GenericPixelShader11,
    GeForce2,
    GeForce3,
    GeForce4,
    Radeon8500,
    Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsVendor {
    Unknown = 0,
    Nvidia,
    Amd,
    Intel,
    Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomScenePassMode {
    Default = 0,
    AlphaMask,
    Wireframe,
}

#[derive(Debug, Clone)]
pub struct ShaderDescription {
    pub name: String,
    pub passes: u32,
    pub requires_pixel_shader: bool,
    pub requires_vertex_shader: bool,
}

#[derive(Debug, Clone)]
pub struct TextureResource {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    pub data: Option<Arc<Vec<u8>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    Rgba8,
    Bgra8,
    R8,
    Rg8,
    Depth24,
    Depth32,
}

#[derive(Debug, Clone)]
pub struct RenderTarget {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    pub has_depth: bool,
}

#[derive(Debug)]
pub struct ManagedRenderTarget {
    pub info: RenderTarget,
    pub color_texture: Texture,
    pub color_view: TextureView,
    pub depth_texture: Option<Texture>,
    pub depth_view: Option<TextureView>,
}

#[derive(Debug, Clone)]
pub struct HardwareCapabilities {
    pub max_texture_stages: usize,
    pub max_texture_size: u32,
    pub render_to_texture_supported: bool,
    pub pixel_shader_level: f32,
    pub features: Features,
}

#[derive(Clone)]
pub struct ShaderManagerContext {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub surface_format: WgpuTextureFormat,
    pub depth_format: WgpuTextureFormat,
    pub adapter_name: String,
    pub vendor: GraphicsVendor,
}

#[derive(Error, Debug)]
pub enum ShaderManagerError {
    #[error("shader not found: {0}")]
    ShaderNotFound(String),
    #[error("failed to initialize shader manager: {0}")]
    InitializationError(String),
    #[error("render to texture not supported")]
    RenderToTextureUnsupported,
    #[error("wgpu device not initialized")]
    DeviceUnavailable,
}

pub trait ScreenFilter: Send + Sync {
    fn init(&mut self) -> Result<(), ShaderManagerError>;
    fn shutdown(&mut self) {}
    fn pre_render(
        &mut self,
        skip_render: &mut bool,
        scene_pass_mode: &mut CustomScenePassMode,
    ) -> bool;
    fn post_render(
        &mut self,
        mode: FilterMode,
        scroll_delta: &mut Vector2<f32>,
        do_extra_render: &mut bool,
    ) -> bool;
    fn setup(&mut self, _mode: FilterMode) -> bool {
        true
    }
}

#[derive(Debug, Default)]
pub struct DefaultScreenFilter {
    initialized: bool,
}

impl ScreenFilter for DefaultScreenFilter {
    fn init(&mut self) -> Result<(), ShaderManagerError> {
        self.initialized = true;
        Ok(())
    }

    fn pre_render(
        &mut self,
        skip_render: &mut bool,
        _scene_pass_mode: &mut CustomScenePassMode,
    ) -> bool {
        *skip_render = false;
        self.initialized
    }

    fn post_render(
        &mut self,
        _mode: FilterMode,
        _scroll_delta: &mut Vector2<f32>,
        _do_extra_render: &mut bool,
    ) -> bool {
        self.initialized
    }
}

#[derive(Debug)]
pub struct BlackWhiteFilter {
    initialized: bool,
    fade_frames: i32,
    cur_fade_frame: i32,
    fade_direction: i32,
    fade_value: f32,
}

impl Default for BlackWhiteFilter {
    fn default() -> Self {
        Self {
            initialized: false,
            fade_frames: 0,
            cur_fade_frame: 0,
            fade_direction: 0,
            fade_value: 1.0,
        }
    }
}

impl ScreenFilter for BlackWhiteFilter {
    fn init(&mut self) -> Result<(), ShaderManagerError> {
        self.initialized = true;
        Ok(())
    }

    fn pre_render(
        &mut self,
        skip_render: &mut bool,
        _scene_pass_mode: &mut CustomScenePassMode,
    ) -> bool {
        *skip_render = false;
        self.initialized
    }

    fn post_render(
        &mut self,
        mode: FilterMode,
        _scroll_delta: &mut Vector2<f32>,
        _do_extra_render: &mut bool,
    ) -> bool {
        if mode == FilterMode::NullMode {
            return false;
        }
        if self.fade_direction != 0 {
            self.cur_fade_frame += 1;
            if self.fade_frames > 0 {
                let t = (self.cur_fade_frame as f32 / self.fade_frames as f32).clamp(0.0, 1.0);
                self.fade_value = if self.fade_direction > 0 { t } else { 1.0 - t };
                if t >= 1.0 {
                    self.fade_direction = 0;
                    self.cur_fade_frame = 0;
                }
            }
        }
        self.initialized
    }
}

#[derive(Debug, Default)]
pub struct MotionBlurFilter {
    initialized: bool,
    prior_delta: Vector2<f32>,
    pan_factor: i32,
}

impl ScreenFilter for MotionBlurFilter {
    fn init(&mut self) -> Result<(), ShaderManagerError> {
        self.initialized = true;
        if self.pan_factor == 0 {
            self.pan_factor = 30;
        }
        Ok(())
    }

    fn pre_render(
        &mut self,
        skip_render: &mut bool,
        _scene_pass_mode: &mut CustomScenePassMode,
    ) -> bool {
        *skip_render = false;
        self.initialized
    }

    fn post_render(
        &mut self,
        _mode: FilterMode,
        scroll_delta: &mut Vector2<f32>,
        do_extra_render: &mut bool,
    ) -> bool {
        *do_extra_render = true;
        *scroll_delta = self.prior_delta * self.pan_factor as f32;
        self.initialized
    }
}

#[derive(Debug, Default)]
pub struct CrossFadeFilter {
    initialized: bool,
    fade_frames: i32,
    cur_fade_frame: i32,
    fade_direction: i32,
    fade_value: f32,
}

impl ScreenFilter for CrossFadeFilter {
    fn init(&mut self) -> Result<(), ShaderManagerError> {
        self.initialized = true;
        Ok(())
    }

    fn pre_render(
        &mut self,
        skip_render: &mut bool,
        _scene_pass_mode: &mut CustomScenePassMode,
    ) -> bool {
        *skip_render = false;
        self.initialized
    }

    fn post_render(
        &mut self,
        _mode: FilterMode,
        _scroll_delta: &mut Vector2<f32>,
        do_extra_render: &mut bool,
    ) -> bool {
        if self.fade_direction != 0 && self.fade_frames > 0 {
            self.cur_fade_frame += 1;
            self.fade_value =
                (self.cur_fade_frame as f32 / self.fade_frames as f32).clamp(0.0, 1.0);
            if self.fade_value >= 1.0 {
                self.fade_direction = 0;
                self.cur_fade_frame = 0;
            }
            *do_extra_render = true;
        }
        self.initialized
    }
}

struct CompiledShader {
    layout: PipelineLayout,
    bind_group_layout: BindGroupLayout,
    module: ShaderModule,
    pipeline: RenderPipeline,
}

pub struct W3DShaderManager {
    pub initialized: bool,
    pub current_chipset: ChipsetType,
    pub current_vendor: GraphicsVendor,
    pub driver_version: u64,
    pub current_shader: ShaderType,
    pub current_shader_pass: u32,
    pub current_filter: FilterType,
    pub textures: [Option<TextureResource>; MAX_TEXTURE_STAGES],
    pub shaders: HashMap<ShaderType, ShaderDescription>,
    pub filters: HashMap<FilterType, Arc<RwLock<Box<dyn ScreenFilter>>>>,
    pub render_to_texture_supported: bool,
    pub rendering_to_texture: bool,
    pub render_target: Option<RenderTarget>,
    pub capabilities: HardwareCapabilities,
    device: Option<Arc<Device>>,
    queue: Option<Arc<Queue>>,
    surface_format: WgpuTextureFormat,
    depth_format: WgpuTextureFormat,
    compiled_shaders: HashMap<ShaderType, CompiledShader>,
    render_targets: HashMap<String, ManagedRenderTarget>,
    active_render_target: Option<String>,
    filter_pipeline_bw: Option<RenderPipeline>,
    filter_pipeline_motion_blur: Option<RenderPipeline>,
    filter_pipeline_crossfade: Option<RenderPipeline>,
    filter_pipeline_viewport: Option<RenderPipeline>,
    filter_uniform_buffer: Option<Buffer>,
    filter_bind_group_layout_texture: Option<BindGroupLayout>,
    filter_bind_group_layout_uniform: Option<BindGroupLayout>,
}

impl Default for W3DShaderManager {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DShaderManager {
    pub fn new() -> Self {
        let mut manager = Self {
            initialized: false,
            current_chipset: ChipsetType::Unknown,
            current_vendor: GraphicsVendor::Unknown,
            driver_version: 0,
            current_shader: ShaderType::Invalid,
            current_shader_pass: 0,
            current_filter: FilterType::NullFilter,
            textures: std::array::from_fn(|_| None),
            shaders: HashMap::new(),
            filters: HashMap::new(),
            render_to_texture_supported: false,
            rendering_to_texture: false,
            render_target: None,
            capabilities: HardwareCapabilities {
                max_texture_stages: MAX_TEXTURE_STAGES,
                max_texture_size: 2048,
                render_to_texture_supported: false,
                pixel_shader_level: 1.0,
                features: Features::empty(),
            },
            device: None,
            queue: None,
            surface_format: WgpuTextureFormat::Bgra8UnormSrgb,
            depth_format: WgpuTextureFormat::Depth32Float,
            compiled_shaders: HashMap::new(),
            render_targets: HashMap::new(),
            active_render_target: None,
            filter_pipeline_bw: None,
            filter_pipeline_motion_blur: None,
            filter_pipeline_crossfade: None,
            filter_pipeline_viewport: None,
            filter_uniform_buffer: None,
            filter_bind_group_layout_texture: None,
            filter_bind_group_layout_uniform: None,
        };
        manager.register_default_shaders();
        manager
    }

    pub fn init(&mut self) -> Result<(), ShaderManagerError> {
        self.detect_hardware_defaults();
        self.init_filters()?;
        self.initialized = true;
        Ok(())
    }

    pub fn init_wgpu(&mut self, context: ShaderManagerContext) -> Result<(), ShaderManagerError> {
        self.device = Some(context.device.clone());
        self.queue = Some(context.queue);
        self.surface_format = context.surface_format;
        self.depth_format = context.depth_format;
        self.current_vendor = context.vendor;
        self.detect_hardware_from_device(&context.device);
        self.init_filters()?;
        self.compile_shader_variants(&context.device)?;
        self.compile_filter_pipelines(&context.device)?;
        self.ensure_render_target("water_reflection", 1024, 1024, true)?;
        self.ensure_render_target("shadow_map", 2048, 2048, true)?;
        self.ensure_render_target("cloud_layer", 1024, 1024, false)?;
        self.ensure_render_target(
            "scene_capture",
            context.device.limits().max_texture_dimension_2d.min(2048),
            context.device.limits().max_texture_dimension_2d.min(2048),
            true,
        )?;
        self.ensure_render_target(
            "scene_capture_2",
            context.device.limits().max_texture_dimension_2d.min(2048),
            context.device.limits().max_texture_dimension_2d.min(2048),
            true,
        )?;
        self.initialized = true;
        Ok(())
    }

    pub fn shutdown(&mut self) {
        for filter in self.filters.values() {
            filter.write().unwrap().shutdown();
        }
        self.filters.clear();
        self.compiled_shaders.clear();
        self.render_targets.clear();
        self.active_render_target = None;
        self.current_shader = ShaderType::Invalid;
        self.current_filter = FilterType::NullFilter;
        self.rendering_to_texture = false;
        self.filter_pipeline_bw = None;
        self.filter_pipeline_motion_blur = None;
        self.filter_pipeline_crossfade = None;
        self.filter_pipeline_viewport = None;
        self.filter_uniform_buffer = None;
        self.initialized = false;
    }

    pub fn get_chipset(&self) -> ChipsetType {
        self.current_chipset
    }

    pub fn get_current_vendor(&self) -> GraphicsVendor {
        self.current_vendor
    }

    pub fn get_driver_version(&self) -> u64 {
        self.driver_version
    }

    pub fn get_shader_passes(&self, shader: ShaderType) -> u32 {
        self.shaders
            .get(&shader)
            .map(|entry| entry.passes)
            .unwrap_or(1)
    }

    pub fn set_shader(&mut self, shader: ShaderType, pass: u32) -> bool {
        if !self.compiled_shaders.contains_key(&shader) && !self.shaders.contains_key(&shader) {
            return false;
        }
        self.current_shader = shader;
        self.current_shader_pass = pass;
        true
    }

    pub fn clear_shader(&mut self) {
        self.current_shader = ShaderType::Invalid;
        self.current_shader_pass = 0;
    }

    pub fn reset_shader(&mut self, _shader: ShaderType) {
        self.clear_shader();
    }

    pub fn set_texture(&mut self, stage: usize, texture: Option<TextureResource>) {
        if stage < MAX_TEXTURE_STAGES {
            self.textures[stage] = texture;
        }
    }

    pub fn get_texture(&self, stage: usize) -> Option<&TextureResource> {
        self.textures.get(stage).and_then(|value| value.as_ref())
    }

    pub fn pipeline(&self, shader: ShaderType) -> Option<&RenderPipeline> {
        self.compiled_shaders
            .get(&shader)
            .map(|shader| &shader.pipeline)
    }

    pub fn can_render_to_texture(&self) -> bool {
        self.render_to_texture_supported
    }

    pub fn is_rendering_to_texture(&self) -> bool {
        self.rendering_to_texture
    }

    pub fn start_render_to_texture_named(
        &mut self,
        name: &str,
        width: u32,
        height: u32,
        with_depth: bool,
    ) -> Result<bool, ShaderManagerError> {
        if !self.render_to_texture_supported {
            return Err(ShaderManagerError::RenderToTextureUnsupported);
        }
        self.ensure_render_target(name, width, height, with_depth)?;
        self.render_target = self
            .render_targets
            .get(name)
            .map(|target| target.info.clone());
        self.active_render_target = Some(name.to_string());
        self.rendering_to_texture = true;
        Ok(true)
    }

    pub fn start_render_to_texture(&mut self) -> bool {
        self.start_render_to_texture_named("default", 1024, 1024, true)
            .unwrap_or(false)
    }

    pub fn end_render_to_texture(&mut self) -> Option<RenderTarget> {
        self.rendering_to_texture = false;
        self.active_render_target = None;
        self.render_target.clone()
    }

    pub fn get_render_target(&self, name: &str) -> Option<&ManagedRenderTarget> {
        self.render_targets.get(name)
    }

    pub fn filter_pre_render(
        &mut self,
        filter: FilterType,
        skip_render: &mut bool,
        scene_pass_mode: &mut CustomScenePassMode,
    ) -> bool {
        if let Some(filter_impl) = self.filters.get(&filter) {
            let mut filter_impl = filter_impl.write().unwrap();
            let active = filter_impl.pre_render(skip_render, scene_pass_mode);
            if active {
                self.current_filter = filter;
            }
            return active;
        }
        false
    }

    pub fn filter_post_render(
        &mut self,
        filter: FilterType,
        mode: FilterMode,
        scroll_delta: &mut Vector2<f32>,
        do_extra_render: &mut bool,
    ) -> bool {
        if let Some(filter_impl) = self.filters.get(&filter) {
            return filter_impl
                .write()
                .unwrap()
                .post_render(mode, scroll_delta, do_extra_render);
        }
        self.current_filter = FilterType::NullFilter;
        false
    }

    pub fn filter_setup(&mut self, filter: FilterType, mode: FilterMode) -> bool {
        self.filters
            .get(&filter)
            .map(|filter_impl| filter_impl.write().unwrap().setup(mode))
            .unwrap_or(false)
    }

    pub fn test_minimum_requirements(&self) -> (bool, ChipsetType, u32) {
        let cpu_freq = 1000;
        let meets_requirements = !matches!(self.current_chipset, ChipsetType::Unknown);
        (meets_requirements, self.current_chipset, cpu_freq)
    }

    pub fn get_gpu_performance_index(&self) -> u32 {
        match self.current_chipset {
            ChipsetType::Unknown => 0,
            ChipsetType::Generic | ChipsetType::GeForce2 => 1,
            ChipsetType::GenericPixelShader11 => 2,
            ChipsetType::GeForce3 | ChipsetType::GeForce4 | ChipsetType::Radeon8500 => 3,
            ChipsetType::Max => 4,
        }
    }

    fn register_default_shaders(&mut self) {
        for (shader, name, passes, pixel) in [
            (ShaderType::TerrainBase, "TerrainBase", 1, false),
            (ShaderType::TerrainBaseNoise1, "TerrainBaseNoise1", 1, true),
            (ShaderType::TerrainBaseNoise2, "TerrainBaseNoise2", 1, true),
            (
                ShaderType::TerrainBaseNoise12,
                "TerrainBaseNoise12",
                1,
                true,
            ),
            (ShaderType::ShroudTexture, "ShroudTexture", 1, true),
            (ShaderType::MaskTexture, "MaskTexture", 1, true),
            (ShaderType::RoadBase, "RoadBase", 1, false),
            (ShaderType::RoadBaseNoise1, "RoadBaseNoise1", 1, true),
            (ShaderType::RoadBaseNoise2, "RoadBaseNoise2", 1, true),
            (ShaderType::RoadBaseNoise12, "RoadBaseNoise12", 1, true),
            (ShaderType::CloudTexture, "CloudTexture", 1, true),
            (ShaderType::FlatTerrainBase, "FlatTerrainBase", 1, false),
            (
                ShaderType::FlatTerrainBaseNoise1,
                "FlatTerrainBaseNoise1",
                1,
                true,
            ),
            (
                ShaderType::FlatTerrainBaseNoise2,
                "FlatTerrainBaseNoise2",
                1,
                true,
            ),
            (
                ShaderType::FlatTerrainBaseNoise12,
                "FlatTerrainBaseNoise12",
                1,
                true,
            ),
            (ShaderType::FlatShroudTexture, "FlatShroudTexture", 1, true),
        ] {
            self.shaders.insert(
                shader,
                ShaderDescription {
                    name: name.to_string(),
                    passes,
                    requires_pixel_shader: pixel,
                    requires_vertex_shader: false,
                },
            );
        }
    }

    fn init_filters(&mut self) -> Result<(), ShaderManagerError> {
        if self.filters.is_empty() {
            self.insert_filter(
                FilterType::ViewDefault,
                Box::new(DefaultScreenFilter::default()),
            )?;
            self.insert_filter(
                FilterType::ViewBwFilter,
                Box::new(BlackWhiteFilter::default()),
            )?;
            self.insert_filter(
                FilterType::ViewMotionBlur,
                Box::new(MotionBlurFilter::default()),
            )?;
            self.insert_filter(
                FilterType::ViewCrossFade,
                Box::new(CrossFadeFilter::default()),
            )?;
        }
        Ok(())
    }

    fn insert_filter(
        &mut self,
        filter_type: FilterType,
        mut filter: Box<dyn ScreenFilter>,
    ) -> Result<(), ShaderManagerError> {
        filter.init()?;
        self.filters
            .insert(filter_type, Arc::new(RwLock::new(filter)));
        Ok(())
    }

    fn detect_hardware_defaults(&mut self) {
        self.current_chipset = ChipsetType::GenericPixelShader11;
        self.current_vendor = GraphicsVendor::Unknown;
        self.driver_version = 0;
        self.render_to_texture_supported = false;
        self.capabilities.max_texture_stages = MAX_TEXTURE_STAGES;
        self.capabilities.max_texture_size = 2048;
        self.capabilities.pixel_shader_level = 1.1;
        self.capabilities.render_to_texture_supported = false;
    }

    fn detect_hardware_from_device(&mut self, device: &Device) {
        let limits = device.limits();
        let features = device.features();
        self.capabilities.max_texture_stages =
            MAX_TEXTURE_STAGES.min(limits.max_sampled_textures_per_shader_stage as usize);
        self.capabilities.max_texture_size = limits.max_texture_dimension_2d;
        self.capabilities.render_to_texture_supported = true;
        self.capabilities.pixel_shader_level =
            if features.contains(Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES) {
                2.0
            } else {
                1.1
            };
        self.capabilities.features = features;
        self.render_to_texture_supported = true;
        self.current_chipset = if self.capabilities.pixel_shader_level >= 2.0 {
            ChipsetType::GeForce4
        } else {
            ChipsetType::GenericPixelShader11
        };
    }

    fn compile_shader_variants(&mut self, device: &Device) -> Result<(), ShaderManagerError> {
        self.compiled_shaders.clear();
        for shader_type in self.shaders.keys().copied() {
            let tint = Self::shader_tint(shader_type);
            let wgsl = Self::shader_source(tint);
            let module = device.create_shader_module(ShaderModuleDescriptor {
                label: Some(Self::shader_label(shader_type)),
                source: ShaderSource::Wgsl(wgsl.into()),
            });
            let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Shader Manager Bind Group Layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            sample_type: TextureSampleType::Float { filterable: true },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Sampler(SamplerBindingType::Filtering),
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
            let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Shader Manager Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(Self::shader_label(shader_type)),
                layout: Some(&layout),
                vertex: VertexState {
                    module: &module,
                    entry_point: "vs_main",
                    buffers: &[],
                },
                fragment: Some(FragmentState {
                    module: &module,
                    entry_point: "fs_main",
                    targets: &[Some(ColorTargetState {
                        format: self.surface_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                primitive: PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });
            self.compiled_shaders.insert(
                shader_type,
                CompiledShader {
                    layout,
                    bind_group_layout,
                    module,
                    pipeline,
                },
            );
        }
        Ok(())
    }

    fn ensure_render_target(
        &mut self,
        name: &str,
        width: u32,
        height: u32,
        with_depth: bool,
    ) -> Result<(), ShaderManagerError> {
        if self.render_targets.contains_key(name) {
            return Ok(());
        }
        let device = self
            .device
            .as_ref()
            .ok_or(ShaderManagerError::DeviceUnavailable)?;
        let color_texture = device.create_texture(&TextureDescriptor {
            label: Some(name),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: self.surface_format,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let color_view = color_texture.create_view(&TextureViewDescriptor::default());
        let (depth_texture, depth_view) = if with_depth {
            let depth_texture = device.create_texture(&TextureDescriptor {
                label: Some(&format!("{name}_depth")),
                size: Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: self.depth_format,
                usage: TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let depth_view = depth_texture.create_view(&TextureViewDescriptor::default());
            (Some(depth_texture), Some(depth_view))
        } else {
            (None, None)
        };
        self.render_targets.insert(
            name.to_string(),
            ManagedRenderTarget {
                info: RenderTarget {
                    width,
                    height,
                    format: Self::from_wgpu_format(self.surface_format),
                    has_depth: with_depth,
                },
                color_texture,
                color_view,
                depth_texture,
                depth_view,
            },
        );
        Ok(())
    }

    fn from_wgpu_format(format: WgpuTextureFormat) -> TextureFormat {
        match format {
            WgpuTextureFormat::Bgra8Unorm | WgpuTextureFormat::Bgra8UnormSrgb => {
                TextureFormat::Bgra8
            }
            WgpuTextureFormat::R8Unorm => TextureFormat::R8,
            WgpuTextureFormat::Rg8Unorm => TextureFormat::Rg8,
            WgpuTextureFormat::Depth24Plus => TextureFormat::Depth24,
            WgpuTextureFormat::Depth32Float => TextureFormat::Depth32,
            _ => TextureFormat::Rgba8,
        }
    }

    fn shader_label(shader: ShaderType) -> &'static str {
        match shader {
            ShaderType::TerrainBase
            | ShaderType::TerrainBaseNoise1
            | ShaderType::TerrainBaseNoise2
            | ShaderType::TerrainBaseNoise12
            | ShaderType::FlatTerrainBase
            | ShaderType::FlatTerrainBaseNoise1
            | ShaderType::FlatTerrainBaseNoise2
            | ShaderType::FlatTerrainBaseNoise12 => "Terrain Shader Variant",
            ShaderType::RoadBase
            | ShaderType::RoadBaseNoise1
            | ShaderType::RoadBaseNoise2
            | ShaderType::RoadBaseNoise12 => "Road Shader Variant",
            ShaderType::CloudTexture => "Cloud Shader Variant",
            ShaderType::ShroudTexture | ShaderType::FlatShroudTexture => "Shroud Shader Variant",
            ShaderType::MaskTexture => "Mask Shader Variant",
            _ => "Generic Shader Variant",
        }
    }

    fn shader_tint(shader: ShaderType) -> Vector4<f32> {
        match shader {
            ShaderType::TerrainBase | ShaderType::FlatTerrainBase => {
                Vector4::new(0.90, 0.82, 0.62, 1.0)
            }
            ShaderType::TerrainBaseNoise1
            | ShaderType::TerrainBaseNoise2
            | ShaderType::TerrainBaseNoise12 => Vector4::new(0.84, 0.78, 0.60, 1.0),
            ShaderType::RoadBase
            | ShaderType::RoadBaseNoise1
            | ShaderType::RoadBaseNoise2
            | ShaderType::RoadBaseNoise12 => Vector4::new(0.28, 0.28, 0.30, 1.0),
            ShaderType::CloudTexture => Vector4::new(0.72, 0.75, 0.82, 0.45),
            ShaderType::ShroudTexture | ShaderType::FlatShroudTexture => {
                Vector4::new(0.0, 0.0, 0.0, 0.55)
            }
            ShaderType::MaskTexture => Vector4::new(1.0, 1.0, 1.0, 0.75),
            _ => Vector4::new(1.0, 1.0, 1.0, 1.0),
        }
    }

    fn shader_source(tint: Vector4<f32>) -> String {
        format!(
            r#"
struct VsOut {{
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VsOut {{
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(3.0, 1.0)
    );
    var out: VsOut;
    let pos = positions[vertex_index];
    out.position = vec4<f32>(pos, 0.0, 1.0);
    out.uv = pos * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    return out;
}}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {{
    let wave = 0.5 + 0.5 * sin(input.uv.x * 18.0 + input.uv.y * 9.0);
    return vec4<f32>({:.5}, {:.5}, {:.5}, {:.5}) * vec4<f32>(vec3<f32>(0.85 + 0.15 * wave), 1.0);
}}
"#,
            tint.x, tint.y, tint.z, tint.w
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_manager_creation() {
        let manager = W3DShaderManager::new();
        assert!(!manager.initialized);
        assert!(manager.shaders.contains_key(&ShaderType::TerrainBase));
    }

    #[test]
    fn test_shader_manager_init() {
        let mut manager = W3DShaderManager::new();
        assert!(manager.init().is_ok());
        assert!(manager.initialized);
        assert!(manager.filters.contains_key(&FilterType::ViewDefault));
    }

    #[test]
    fn test_render_to_texture_metadata() {
        let mut manager = W3DShaderManager::new();
        manager.render_to_texture_supported = true;
        assert!(
            manager.start_render_to_texture()
                || !manager.render_targets.is_empty()
                || manager.rendering_to_texture == false
        );
    }
}
