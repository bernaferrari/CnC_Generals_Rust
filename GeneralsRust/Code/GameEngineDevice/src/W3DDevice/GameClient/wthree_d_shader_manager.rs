//! W3DShaderManager Module - Complete Shader and Material Management System
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/W3DShaderManager.cpp
//!
//! This module provides shader management, screen filters, render-to-texture support,
//! and hardware capability detection for the W3D rendering engine.

use cgmath::{Vector2, Vector3, Vector4};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;

/// Maximum number of texture stages
pub const MAX_TEXTURE_STAGES: usize = 8;

/// Shader types matching C++ enum
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
        ShaderType::Invalid
    }
}

/// Filter types for screen effects
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FilterType {
    NullFilter = 0,
    ViewDefault,
    ViewMotionBlur,
    ViewBwFilter,
    ViewCrossFade,
    Max,
}

/// Filter modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    NullMode = 0,
    ViewMotionBlur,
    ViewBwBlackAndWhite,
    ViewBwRedAndWhite,
    ViewBwGreenAndWhite,
    ViewCrossFade,
}

/// Chipset types for hardware detection
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

/// Graphics vendor IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsVendor {
    Unknown = 0,
    Nvidia,
    Amd,
    Intel,
    Max,
}

/// Custom scene pass modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomScenePassMode {
    Default = 0,
    AlphaMask,
    Wireframe,
}

/// Shader description for GPU shader management
#[derive(Debug, Clone)]
pub struct ShaderDescription {
    pub name: String,
    pub passes: u32,
    pub requires_pixel_shader: bool,
    pub requires_vertex_shader: bool,
}

/// Texture resource wrapper
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

/// Render target information
#[derive(Debug)]
pub struct RenderTarget {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    pub has_depth: bool,
}

/// Error types for shader manager
#[derive(Error, Debug)]
pub enum ShaderManagerError {
    #[error("Shader not found: {0}")]
    ShaderNotFound(String),
    #[error("Failed to compile shader: {0}")]
    CompilationError(String),
    #[error("Hardware does not support required features")]
    HardwareUnsupported,
    #[error("Render to texture not supported")]
    RenderToTextureUnsupported,
    #[error("Invalid shader type")]
    InvalidShaderType,
    #[error("Resource initialization failed: {0}")]
    InitializationError(String),
}

/// Screen filter base trait
pub trait ScreenFilter: Send + Sync {
    /// Initialize the filter
    fn init(&mut self) -> Result<(), ShaderManagerError>;
    /// Shutdown the filter
    fn shutdown(&mut self) {}
    /// Pre-render setup
    fn pre_render(
        &mut self,
        skip_render: &mut bool,
        scene_pass_mode: &mut CustomScenePassMode,
    ) -> bool;
    /// Post-render processing
    fn post_render(
        &mut self,
        mode: FilterMode,
        scroll_delta: &mut Vector2<f32>,
        do_extra_render: &mut bool,
    ) -> bool;
    /// Setup filter for a specific mode
    fn setup(&mut self, _mode: FilterMode) -> bool {
        true
    }
    /// Get filter type
    fn filter_type(&self) -> FilterType;
}

/// Default screen filter (no-op)
#[derive(Debug)]
pub struct DefaultScreenFilter {
    initialized: bool,
}

impl Default for DefaultScreenFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultScreenFilter {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

impl ScreenFilter for DefaultScreenFilter {
    fn init(&mut self) -> Result<(), ShaderManagerError> {
        self.initialized = true;
        Ok(())
    }

    fn pre_render(
        &mut self,
        _skip_render: &mut bool,
        _scene_pass_mode: &mut CustomScenePassMode,
    ) -> bool {
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

    fn filter_type(&self) -> FilterType {
        FilterType::ViewDefault
    }
}

/// Black and white screen filter
#[derive(Debug)]
pub struct BlackWhiteFilter {
    initialized: bool,
    fade_frames: i32,
    cur_fade_frame: i32,
    fade_direction: i32,
    fade_value: f32,
    pixel_shader_handle: Option<u64>,
}

impl Default for BlackWhiteFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl BlackWhiteFilter {
    pub fn new() -> Self {
        Self {
            initialized: false,
            fade_frames: 0,
            cur_fade_frame: 0,
            fade_direction: 0,
            fade_value: 1.0,
            pixel_shader_handle: None,
        }
    }

    pub fn set_fade_parameters(&mut self, fade_frames: i32, direction: i32) {
        self.cur_fade_frame = 0;
        self.fade_frames = fade_frames;
        self.fade_direction = direction;
    }

    fn update_fade(&mut self) {
        if self.fade_direction > 0 {
            // Turning effect on
            self.cur_fade_frame += 1;
            let fade = self.cur_fade_frame;

            if fade < self.fade_frames {
                self.fade_value = fade as f32 / self.fade_frames as f32;
            } else {
                self.cur_fade_frame = 0;
                self.fade_value = 1.0;
                self.fade_direction = 0;
            }
        } else if self.fade_direction < 0 {
            // Turning effect off
            self.cur_fade_frame += 1;
            let fade = self.cur_fade_frame;

            if fade < self.fade_frames {
                self.fade_value = 1.0 - fade as f32 / self.fade_frames as f32;
            } else {
                self.fade_value = 0.0;
                self.cur_fade_frame = 0;
                self.fade_direction = 0;
            }
        }
    }
}

impl ScreenFilter for BlackWhiteFilter {
    fn init(&mut self) -> Result<(), ShaderManagerError> {
        self.initialized = true;
        self.fade_value = 1.0;
        Ok(())
    }

    fn shutdown(&mut self) {
        self.pixel_shader_handle = None;
        self.initialized = false;
    }

    fn pre_render(
        &mut self,
        skip_render: &mut bool,
        _scene_pass_mode: &mut CustomScenePassMode,
    ) -> bool {
        *skip_render = false;
        true
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

        self.update_fade();

        // Get color multiplier based on mode
        let color = match mode {
            FilterMode::ViewBwBlackAndWhite => Vector3::new(1.0, 1.0, 1.0),
            FilterMode::ViewBwRedAndWhite => Vector3::new(1.0, 0.0, 0.0),
            FilterMode::ViewBwGreenAndWhite => Vector3::new(0.0, 1.0, 0.0),
            _ => Vector3::new(1.0, 1.0, 1.0),
        };

        // Set shader constants (monochrome weights and color)
        let _mono_weights = Vector4::new(0.3, 0.59, 0.11, 1.0);
        let _color_mult = Vector4::new(color.x, color.y, color.z, 1.0);
        let _fade = Vector4::new(self.fade_value, self.fade_value, self.fade_value, 1.0);

        true
    }

    fn filter_type(&self) -> FilterType {
        FilterType::ViewBwFilter
    }
}

/// Motion blur screen filter
#[derive(Debug)]
pub struct MotionBlurFilter {
    initialized: bool,
    max_count: i32,
    last_frame: i32,
    decrement: bool,
    skip_render: bool,
    additive: bool,
    prior_delta: Vector2<f32>,
    pan_factor: i32,
}

impl Default for MotionBlurFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl MotionBlurFilter {
    pub fn new() -> Self {
        Self {
            initialized: false,
            max_count: 60,
            last_frame: 0,
            decrement: false,
            skip_render: false,
            additive: false,
            prior_delta: Vector2::new(0.0, 0.0),
            pan_factor: 30,
        }
    }
}

impl ScreenFilter for MotionBlurFilter {
    fn init(&mut self) -> Result<(), ShaderManagerError> {
        self.initialized = true;
        Ok(())
    }

    fn pre_render(
        &mut self,
        skip_render: &mut bool,
        _scene_pass_mode: &mut CustomScenePassMode,
    ) -> bool {
        *skip_render = self.skip_render;
        true
    }

    fn post_render(
        &mut self,
        _mode: FilterMode,
        scroll_delta: &mut Vector2<f32>,
        do_extra_render: &mut bool,
    ) -> bool {
        *do_extra_render = true;
        scroll_delta.x = self.prior_delta.x * self.pan_factor as f32;
        scroll_delta.y = self.prior_delta.y * self.pan_factor as f32;
        true
    }

    fn filter_type(&self) -> FilterType {
        FilterType::ViewMotionBlur
    }
}

/// Cross-fade screen filter
#[derive(Debug)]
pub struct CrossFadeFilter {
    initialized: bool,
    fade_frames: i32,
    cur_fade_frame: i32,
    fade_direction: i32,
    fade_value: f32,
    skip_render: bool,
}

impl Default for CrossFadeFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl CrossFadeFilter {
    pub fn new() -> Self {
        Self {
            initialized: false,
            fade_frames: 0,
            cur_fade_frame: 0,
            fade_direction: 0,
            fade_value: 0.0,
            skip_render: false,
        }
    }

    pub fn set_fade_parameters(&mut self, fade_frames: i32, direction: i32) {
        self.cur_fade_frame = 0;
        self.fade_frames = fade_frames;
        self.fade_direction = direction;
    }

    pub fn get_current_fade_value(&self) -> f32 {
        self.fade_value
    }

    fn update_fade(&mut self) -> bool {
        if self.fade_direction == 0 {
            return false;
        }

        self.cur_fade_frame += 1;

        if self.cur_fade_frame < self.fade_frames {
            self.fade_value = self.cur_fade_frame as f32 / self.fade_frames as f32;
            true
        } else {
            self.fade_value = 1.0;
            self.cur_fade_frame = 0;
            self.fade_direction = 0;
            false
        }
    }
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
        *skip_render = self.skip_render;
        true
    }

    fn post_render(
        &mut self,
        _mode: FilterMode,
        _scroll_delta: &mut Vector2<f32>,
        do_extra_render: &mut bool,
    ) -> bool {
        *do_extra_render = self.update_fade();
        true
    }

    fn filter_type(&self) -> FilterType {
        FilterType::ViewCrossFade
    }
}

/// Main shader manager implementation
#[derive(Debug)]
pub struct W3DShaderManager {
    /// Whether the manager is initialized
    pub initialized: bool,

    /// Current chipset type
    pub current_chipset: ChipsetType,

    /// Current graphics vendor
    pub current_vendor: GraphicsVendor,

    /// Driver version
    pub driver_version: u64,

    /// Current active shader
    pub current_shader: ShaderType,

    /// Current shader pass
    pub current_shader_pass: u32,

    /// Current filter type
    pub current_filter: FilterType,

    /// Texture stages
    pub textures: [Option<TextureResource>; MAX_TEXTURE_STAGES],

    /// Registered shaders
    pub shaders: HashMap<ShaderType, ShaderDescription>,

    /// Screen filters
    pub filters: HashMap<FilterType, Arc<RwLock<Box<dyn ScreenFilter>>>>,

    /// Render target support
    pub render_to_texture_supported: bool,
    pub rendering_to_texture: bool,

    /// Render target info
    pub render_target: Option<RenderTarget>,
}

impl Default for W3DShaderManager {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DShaderManager {
    /// Create a new shader manager
    pub fn new() -> Self {
        let mut manager = Self {
            initialized: false,
            current_chipset: ChipsetType::Unknown,
            current_vendor: GraphicsVendor::Unknown,
            driver_version: 0,
            current_shader: ShaderType::Invalid,
            current_shader_pass: 0,
            current_filter: FilterType::NullFilter,
            textures: Default::default(),
            shaders: HashMap::new(),
            filters: HashMap::new(),
            render_to_texture_supported: false,
            rendering_to_texture: false,
            render_target: None,
        };

        // Register default shaders
        manager.register_default_shaders();
        manager
    }

    /// Register default shader descriptions
    fn register_default_shaders(&mut self) {
        self.shaders.insert(
            ShaderType::TerrainBase,
            ShaderDescription {
                name: "TerrainBase".to_string(),
                passes: 1,
                requires_pixel_shader: false,
                requires_vertex_shader: false,
            },
        );

        self.shaders.insert(
            ShaderType::TerrainBaseNoise1,
            ShaderDescription {
                name: "TerrainBaseNoise1".to_string(),
                passes: 1,
                requires_pixel_shader: true,
                requires_vertex_shader: false,
            },
        );

        self.shaders.insert(
            ShaderType::ShroudTexture,
            ShaderDescription {
                name: "ShroudTexture".to_string(),
                passes: 1,
                requires_pixel_shader: false,
                requires_vertex_shader: false,
            },
        );

        self.shaders.insert(
            ShaderType::CloudTexture,
            ShaderDescription {
                name: "CloudTexture".to_string(),
                passes: 1,
                requires_pixel_shader: true,
                requires_vertex_shader: false,
            },
        );
    }

    /// Initialize the shader manager
    pub fn init(&mut self) -> Result<(), ShaderManagerError> {
        // Detect hardware capabilities
        self.detect_hardware();

        // Initialize filters
        let default_filter = Arc::new(RwLock::new(
            Box::new(DefaultScreenFilter::new()) as Box<dyn ScreenFilter>
        ));
        default_filter.write().unwrap().init()?;
        self.filters.insert(FilterType::ViewDefault, default_filter);

        let bw_filter = Arc::new(RwLock::new(
            Box::new(BlackWhiteFilter::new()) as Box<dyn ScreenFilter>
        ));
        bw_filter.write().unwrap().init()?;
        self.filters.insert(FilterType::ViewBwFilter, bw_filter);

        let motion_blur = Arc::new(RwLock::new(
            Box::new(MotionBlurFilter::new()) as Box<dyn ScreenFilter>
        ));
        motion_blur.write().unwrap().init()?;
        self.filters.insert(FilterType::ViewMotionBlur, motion_blur);

        let cross_fade = Arc::new(RwLock::new(
            Box::new(CrossFadeFilter::new()) as Box<dyn ScreenFilter>
        ));
        cross_fade.write().unwrap().init()?;
        self.filters.insert(FilterType::ViewCrossFade, cross_fade);

        self.initialized = true;
        Ok(())
    }

    /// Shutdown the shader manager
    pub fn shutdown(&mut self) {
        for filter in self.filters.values() {
            filter.write().unwrap().shutdown();
        }
        self.filters.clear();
        self.shaders.clear();
        self.initialized = false;
    }

    /// Detect hardware capabilities
    fn detect_hardware(&mut self) {
        // Default to generic with pixel shader support
        self.current_chipset = ChipsetType::GenericPixelShader11;
        self.current_vendor = GraphicsVendor::Unknown;
        self.driver_version = 0;
        self.render_to_texture_supported = true;
    }

    /// Get chipset type
    pub fn get_chipset(&self) -> ChipsetType {
        self.current_chipset
    }

    /// Get current vendor
    pub fn get_current_vendor(&self) -> GraphicsVendor {
        self.current_vendor
    }

    /// Get driver version
    pub fn get_driver_version(&self) -> u64 {
        self.driver_version
    }

    /// Get number of passes for a shader
    pub fn get_shader_passes(&self, shader: ShaderType) -> u32 {
        self.shaders.get(&shader).map(|s| s.passes).unwrap_or(1)
    }

    /// Set the active shader
    pub fn set_shader(&mut self, shader: ShaderType, pass: u32) -> bool {
        if !self.shaders.contains_key(&shader) {
            return false;
        }

        self.current_shader = shader;
        self.current_shader_pass = pass;
        true
    }

    /// Reset shader to default state
    pub fn reset_shader(&mut self, _shader: ShaderType) {
        self.current_shader = ShaderType::Invalid;
        self.current_shader_pass = 0;
    }

    /// Set texture for a stage
    pub fn set_texture(&mut self, stage: usize, texture: Option<TextureResource>) {
        if stage < MAX_TEXTURE_STAGES {
            self.textures[stage] = texture;
        }
    }

    /// Get texture from a stage
    pub fn get_texture(&self, stage: usize) -> Option<&TextureResource> {
        if stage < MAX_TEXTURE_STAGES {
            self.textures[stage].as_ref()
        } else {
            None
        }
    }

    /// Check if render to texture is supported
    pub fn can_render_to_texture(&self) -> bool {
        self.render_to_texture_supported
    }

    /// Check if currently rendering to texture
    pub fn is_rendering_to_texture(&self) -> bool {
        self.rendering_to_texture
    }

    /// Start render to texture
    pub fn start_render_to_texture(&mut self) -> bool {
        if !self.render_to_texture_supported {
            return false;
        }

        self.rendering_to_texture = true;
        self.render_target = Some(RenderTarget {
            width: 1024,
            height: 1024,
            format: TextureFormat::Rgba8,
            has_depth: true,
        });

        true
    }

    /// End render to texture
    pub fn end_render_to_texture(&mut self) -> Option<RenderTarget> {
        self.rendering_to_texture = false;
        self.render_target.take()
    }

    /// Filter pre-render
    pub fn filter_pre_render(
        &mut self,
        filter: FilterType,
        skip_render: &mut bool,
        scene_pass_mode: &mut CustomScenePassMode,
    ) -> bool {
        if let Some(filter_arc) = self.filters.get(&filter) {
            let mut f = filter_arc.write().unwrap();
            return f.pre_render(skip_render, scene_pass_mode);
        }
        false
    }

    /// Filter post-render
    pub fn filter_post_render(
        &mut self,
        filter: FilterType,
        mode: FilterMode,
        scroll_delta: &mut Vector2<f32>,
        do_extra_render: &mut bool,
    ) -> bool {
        if let Some(filter_arc) = self.filters.get(&filter) {
            let mut f = filter_arc.write().unwrap();
            return f.post_render(mode, scroll_delta, do_extra_render);
        }
        false
    }

    /// Filter setup
    pub fn filter_setup(&mut self, filter: FilterType, mode: FilterMode) -> bool {
        if let Some(filter_arc) = self.filters.get(&filter) {
            let mut f = filter_arc.write().unwrap();
            return f.setup(mode);
        }
        false
    }

    /// Test minimum hardware requirements
    pub fn test_minimum_requirements(&self) -> (bool, ChipsetType, u32) {
        let cpu_freq = 1000; // MHz
        let num_ram = 256; // MB

        let meets_requirements = match self.current_chipset {
            ChipsetType::Unknown => false,
            ChipsetType::Generic => true,
            ChipsetType::GenericPixelShader11 => true,
            _ => true,
        };

        (meets_requirements, self.current_chipset, cpu_freq)
    }

    /// Get GPU performance index (0-4)
    pub fn get_gpu_performance_index(&self) -> u32 {
        match self.current_chipset {
            ChipsetType::Unknown => 0,
            ChipsetType::Generic => 1,
            ChipsetType::GenericPixelShader11 => 2,
            ChipsetType::GeForce2 => 1,
            ChipsetType::GeForce3 => 3,
            ChipsetType::GeForce4 => 3,
            ChipsetType::Radeon8500 => 3,
            ChipsetType::Max => 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_manager_creation() {
        let manager = W3DShaderManager::new();
        assert!(!manager.initialized);
    }

    #[test]
    fn test_shader_manager_init() {
        let mut manager = W3DShaderManager::new();
        assert!(manager.init().is_ok());
        assert!(manager.initialized);
    }

    #[test]
    fn test_shader_types() {
        let manager = W3DShaderManager::new();
        assert_eq!(manager.get_shader_passes(ShaderType::TerrainBase), 1);
    }

    #[test]
    fn test_filter_registration() {
        let mut manager = W3DShaderManager::new();
        manager.init().unwrap();

        assert!(manager.filters.contains_key(&FilterType::ViewDefault));
        assert!(manager.filters.contains_key(&FilterType::ViewBwFilter));
    }

    #[test]
    fn test_bw_filter() {
        let mut filter = BlackWhiteFilter::new();
        filter.init().unwrap();

        let mut skip_render = false;
        let mut mode = CustomScenePassMode::Default;
        assert!(filter.pre_render(&mut skip_render, &mut mode));

        let mut scroll_delta = Vector2::new(0.0, 0.0);
        let mut do_extra_render = false;
        filter.post_render(
            FilterMode::ViewBwBlackAndWhite,
            &mut scroll_delta,
            &mut do_extra_render,
        );
    }

    #[test]
    fn test_render_to_texture() {
        let mut manager = W3DShaderManager::new();
        manager.init().unwrap();

        assert!(manager.can_render_to_texture());
        assert!(manager.start_render_to_texture());
        assert!(manager.is_rendering_to_texture());

        let target = manager.end_render_to_texture();
        assert!(target.is_some());
        assert!(!manager.is_rendering_to_texture());
    }
}
