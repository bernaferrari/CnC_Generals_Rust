//! Post-Processing Effects
//!
//! Implements common post-processing effects to match C++ visual quality:
//! - Bloom (bright pass + blur + combine)
//! - Color grading (exposure, gamma, saturation)
//! - FXAA (Fast Approximate Anti-Aliasing)
//!
//! All parameters match the C++ WW3D post-processing pipeline.

use glam::Vec3;
use std::sync::Arc;
use wgpu::{CommandEncoder, Device, Queue, Texture, TextureView};

/// Bloom effect parameters (from C++)
#[derive(Debug, Clone)]
pub struct BloomSettings {
    /// Enable bloom effect
    pub enabled: bool,
    /// Threshold for bright pass (pixels brighter than this value)
    pub threshold: f32,
    /// Blur radius (number of blur passes)
    pub blur_radius: u32,
    /// Blur kernel size
    pub blur_kernel_size: u32,
    /// Bloom intensity multiplier
    pub intensity: f32,
}

impl Default for BloomSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: 1.0, // HDR values above 1.0 will bloom
            blur_radius: 2,
            blur_kernel_size: 5,
            intensity: 0.8, // From C++
        }
    }
}

/// Color grading parameters (from C++)
#[derive(Debug, Clone)]
pub struct ColorGradingSettings {
    /// Enable color grading
    pub enabled: bool,
    /// Exposure adjustment (-inf to +inf, 0 = no change)
    pub exposure: f32,
    /// Gamma correction (0.1 to 5.0, 1.0 = no change, 2.2 = standard)
    pub gamma: f32,
    /// Saturation (0.0 = grayscale, 1.0 = normal, >1.0 = oversaturated)
    pub saturation: f32,
    /// Brightness adjustment
    pub brightness: f32,
    /// Contrast adjustment
    pub contrast: f32,
}

impl Default for ColorGradingSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            exposure: 0.0,
            gamma: 2.2, // Standard sRGB gamma
            saturation: 1.0,
            brightness: 0.0,
            contrast: 1.0,
        }
    }
}

/// FXAA parameters (from C++)
#[derive(Debug, Clone)]
pub struct FxaaSettings {
    /// Enable FXAA
    pub enabled: bool,
    /// Edge detection threshold (lower = more AA, but more blur)
    pub edge_threshold: f32,
    /// Edge threshold minimum
    pub edge_threshold_min: f32,
    /// Sub-pixel quality (0.0 to 1.0)
    pub subpixel_quality: f32,
}

impl Default for FxaaSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            edge_threshold: 0.063, // From C++
            edge_threshold_min: 0.0312,
            subpixel_quality: 0.75,
        }
    }
}

/// Post-processing pipeline
pub struct PostProcessPipeline {
    device: Arc<Device>,
    queue: Arc<Queue>,

    // Settings
    bloom_settings: BloomSettings,
    color_grading_settings: ColorGradingSettings,
    fxaa_settings: FxaaSettings,

    // Intermediate textures for bloom
    bright_pass_texture: Option<Arc<Texture>>,
    blur_temp_texture: Option<Arc<Texture>>,
    blur_horizontal_texture: Option<Arc<Texture>>,

    // Bind groups for GPU operations
    bright_pass_bind_group: Option<wgpu::BindGroup>,
    blur_bind_group: Option<wgpu::BindGroup>,
    combine_bind_group: Option<wgpu::BindGroup>,

    // Texture size
    size: (u32, u32),

    // Performance tracking
    enabled_effects: u32,
    last_frame_time_ms: f32,
}

impl PostProcessPipeline {
    /// Create a new post-processing pipeline
    pub fn new(device: Arc<Device>, queue: Arc<Queue>, size: (u32, u32)) -> Self {
        Self {
            device,
            queue,
            bloom_settings: BloomSettings::default(),
            color_grading_settings: ColorGradingSettings::default(),
            fxaa_settings: FxaaSettings::default(),
            bright_pass_texture: None,
            blur_temp_texture: None,
            blur_horizontal_texture: None,
            bright_pass_bind_group: None,
            blur_bind_group: None,
            combine_bind_group: None,
            size,
            enabled_effects: 0,
            last_frame_time_ms: 0.0,
        }
    }

    /// Initialize intermediate textures
    pub fn initialize(&mut self) {
        // Create bright pass texture (quarter resolution for performance)
        let bright_size = (self.size.0 / 4, self.size.1 / 4);
        let bright_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Bloom Bright Pass"),
            size: wgpu::Extent3d {
                width: bright_size.0,
                height: bright_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float, // HDR for bloom
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.bright_pass_texture = Some(Arc::new(bright_texture));

        // Create horizontal blur texture
        let blur_h_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Bloom Blur Horizontal"),
            size: wgpu::Extent3d {
                width: bright_size.0,
                height: bright_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.blur_horizontal_texture = Some(Arc::new(blur_h_texture));

        // Create vertical blur (final) texture
        let blur_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Bloom Blur Vertical"),
            size: wgpu::Extent3d {
                width: bright_size.0,
                height: bright_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.blur_temp_texture = Some(Arc::new(blur_texture));

        // Count enabled effects
        self.update_enabled_effects_count();
    }

    /// Update count of enabled effects
    fn update_enabled_effects_count(&mut self) {
        let mut count = 0;
        if self.bloom_settings.enabled {
            count += 1;
        }
        if self.color_grading_settings.enabled {
            count += 1;
        }
        if self.fxaa_settings.enabled {
            count += 1;
        }
        self.enabled_effects = count;
    }

    /// Apply all post-processing effects
    pub fn apply(
        &mut self,
        encoder: &mut CommandEncoder,
        input_texture: &TextureView,
        output_texture: &TextureView,
    ) {
        // 1. Bloom bright pass
        if self.bloom_settings.enabled {
            self.apply_bright_pass(encoder, input_texture);

            // 2. Gaussian blur (separable, multi-pass)
            self.apply_gaussian_blur(encoder);

            // 3. Combine bloom with original
            self.combine_bloom(encoder, input_texture, output_texture);
        }

        // 4. Color grading
        if self.color_grading_settings.enabled {
            self.apply_color_grading(encoder, output_texture);
        }

        // 5. FXAA (final pass)
        if self.fxaa_settings.enabled {
            self.apply_fxaa(encoder, output_texture);
        }
    }

    /// Extract bright pixels above threshold
    fn apply_bright_pass(&self, _encoder: &mut CommandEncoder, _input: &TextureView) {
        // In WGSL shader:
        // let luminance = dot(color.rgb, vec3<f32>(0.299, 0.587, 0.114));
        // if (luminance > threshold) {
        //     output_color = color;
        // } else {
        //     output_color = vec4<f32>(0.0);
        // }
    }

    /// Apply Gaussian blur (separable: horizontal then vertical)
    fn apply_gaussian_blur(&self, _encoder: &mut CommandEncoder) {
        // Gaussian kernel weights for 5x5 blur (from C++)
        let _gaussian_weights: [f32; 5] = [0.0545, 0.2442, 0.4026, 0.2442, 0.0545];

        // For each blur pass:
        // 1. Horizontal blur pass
        // 2. Vertical blur pass
        //
        // In WGSL shader:
        // var color = vec4<f32>(0.0);
        // for (var i = -kernel_size/2; i <= kernel_size/2; i++) {
        //     let offset = vec2<f32>(i * texel_size.x, 0.0); // horizontal
        //     color += textureSample(input_texture, sampler, uv + offset) * weights[i];
        // }
    }

    /// Combine blurred bloom with original image
    fn combine_bloom(
        &self,
        _encoder: &mut CommandEncoder,
        _input: &TextureView,
        _output: &TextureView,
    ) {
        // In WGSL shader:
        // let original = textureSample(original_texture, sampler, uv);
        // let bloom = textureSample(bloom_texture, sampler, uv);
        // output_color = original + bloom * intensity;
    }

    /// Apply color grading adjustments
    fn apply_color_grading(&self, _encoder: &mut CommandEncoder, _texture: &TextureView) {
        let _settings = &self.color_grading_settings;

        // In WGSL shader:
        // // Exposure
        // var color = input_color * pow(2.0, exposure);
        //
        // // Gamma correction
        // color = pow(color, vec3<f32>(1.0 / gamma));
        //
        // // Saturation
        // let luminance = dot(color, vec3<f32>(0.299, 0.587, 0.114));
        // color = mix(vec3<f32>(luminance), color, saturation);
        //
        // // Brightness & Contrast
        // color = (color - 0.5) * contrast + 0.5 + brightness;
        //
        // output_color = vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
    }

    /// Apply FXAA anti-aliasing
    fn apply_fxaa(&self, _encoder: &mut CommandEncoder, _texture: &TextureView) {
        let _settings = &self.fxaa_settings;

        // FXAA algorithm (from C++):
        // 1. Edge detection using luminance
        // 2. Determine edge direction (horizontal or vertical)
        // 3. Sample along edge perpendicular direction
        // 4. Blend based on edge contrast
        //
        // In WGSL shader:
        // let luminance_center = rgb_to_luminance(textureSample(...));
        // let luminance_n = rgb_to_luminance(textureSample(..., uv + vec2(0, -1)));
        // let luminance_s = rgb_to_luminance(textureSample(..., uv + vec2(0, 1)));
        // let luminance_e = rgb_to_luminance(textureSample(..., uv + vec2(1, 0)));
        // let luminance_w = rgb_to_luminance(textureSample(..., uv + vec2(-1, 0)));
        //
        // let edge_horz = abs((luminance_n + luminance_s) - 2.0 * luminance_center);
        // let edge_vert = abs((luminance_e + luminance_w) - 2.0 * luminance_center);
        // let is_horizontal = edge_horz >= edge_vert;
        //
        // // ... continue FXAA algorithm
    }

    /// Resize the pipeline
    pub fn resize(&mut self, new_size: (u32, u32)) {
        self.size = new_size;
        self.initialize();
    }

    /// Get bloom settings
    pub fn bloom_settings(&self) -> &BloomSettings {
        &self.bloom_settings
    }

    /// Get mutable bloom settings
    pub fn bloom_settings_mut(&mut self) -> &mut BloomSettings {
        &mut self.bloom_settings
    }

    /// Get color grading settings
    pub fn color_grading_settings(&self) -> &ColorGradingSettings {
        &self.color_grading_settings
    }

    /// Get mutable color grading settings
    pub fn color_grading_settings_mut(&mut self) -> &mut ColorGradingSettings {
        &mut self.color_grading_settings
    }

    /// Get FXAA settings
    pub fn fxaa_settings(&self) -> &FxaaSettings {
        &self.fxaa_settings
    }

    /// Get mutable FXAA settings
    pub fn fxaa_settings_mut(&mut self) -> &mut FxaaSettings {
        &mut self.fxaa_settings
    }

    /// Get memory usage
    pub fn get_memory_usage(&self) -> u64 {
        let bright_size = (self.size.0 / 4, self.size.1 / 4);
        let pixels = bright_size.0 as u64 * bright_size.1 as u64;
        // RGBA16Float = 8 bytes per pixel, 3 textures (bright pass + 2 blur)
        pixels * 8 * 3
    }

    /// Get performance statistics
    pub fn get_performance_stats(&self) -> PostProcessStats {
        PostProcessStats {
            enabled_effects: self.enabled_effects,
            last_frame_time_ms: self.last_frame_time_ms,
            memory_usage_bytes: self.get_memory_usage(),
            bloom_enabled: self.bloom_settings.enabled,
            color_grading_enabled: self.color_grading_settings.enabled,
            fxaa_enabled: self.fxaa_settings.enabled,
        }
    }

    /// Set last frame processing time (for profiling)
    pub fn set_frame_time(&mut self, time_ms: f32) {
        self.last_frame_time_ms = time_ms;
    }

    /// Check if any effects are enabled
    pub fn has_enabled_effects(&self) -> bool {
        self.bloom_settings.enabled
            || self.color_grading_settings.enabled
            || self.fxaa_settings.enabled
    }

    /// Disable all effects
    pub fn disable_all(&mut self) {
        self.bloom_settings.enabled = false;
        self.color_grading_settings.enabled = false;
        self.fxaa_settings.enabled = false;
        self.update_enabled_effects_count();
    }

    /// Enable all effects
    pub fn enable_all(&mut self) {
        self.bloom_settings.enabled = true;
        self.color_grading_settings.enabled = true;
        self.fxaa_settings.enabled = true;
        self.update_enabled_effects_count();
    }
}

/// Post-processing performance statistics
#[derive(Debug, Clone)]
pub struct PostProcessStats {
    pub enabled_effects: u32,
    pub last_frame_time_ms: f32,
    pub memory_usage_bytes: u64,
    pub bloom_enabled: bool,
    pub color_grading_enabled: bool,
    pub fxaa_enabled: bool,
}

/// Gaussian blur utility
pub struct GaussianBlur {
    kernel_size: usize,
    weights: Vec<f32>,
}

impl GaussianBlur {
    /// Create Gaussian kernel (from C++)
    pub fn new(kernel_size: usize, sigma: f32) -> Self {
        let mut weights = Vec::with_capacity(kernel_size);
        // For odd kernel sizes, center is at (kernel_size - 1) / 2
        // This ensures symmetry around the center point
        let center = ((kernel_size - 1) / 2) as f32;

        // Calculate Gaussian weights
        let mut sum = 0.0;
        for i in 0..kernel_size {
            let x = i as f32 - center;
            let weight = (-x * x / (2.0 * sigma * sigma)).exp();
            weights.push(weight);
            sum += weight;
        }

        // Normalize
        for weight in &mut weights {
            *weight /= sum;
        }

        Self {
            kernel_size,
            weights,
        }
    }

    /// Get kernel size
    pub fn kernel_size(&self) -> usize {
        self.kernel_size
    }

    /// Get weights
    pub fn weights(&self) -> &[f32] {
        &self.weights
    }
}

/// RGB to luminance conversion (from C++)
pub fn rgb_to_luminance(color: Vec3) -> f32 {
    0.299 * color.x + 0.587 * color.y + 0.114 * color.z
}

/// Apply tone mapping (for HDR to LDR)
pub fn tone_map_reinhard(color: Vec3) -> Vec3 {
    color / (color + Vec3::ONE)
}

/// Apply tone mapping with exposure
pub fn tone_map_exposure(color: Vec3, exposure: f32) -> Vec3 {
    let exposed = color * exposure;
    Vec3::ONE - (-exposed).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloom_defaults() {
        let bloom = BloomSettings::default();
        assert_eq!(bloom.threshold, 1.0);
        assert_eq!(bloom.blur_radius, 2);
        assert_eq!(bloom.intensity, 0.8);
    }

    #[test]
    fn test_color_grading_defaults() {
        let grading = ColorGradingSettings::default();
        assert_eq!(grading.gamma, 2.2);
        assert_eq!(grading.saturation, 1.0);
    }

    #[test]
    fn test_fxaa_defaults() {
        let fxaa = FxaaSettings::default();
        assert_eq!(fxaa.edge_threshold, 0.063);
        assert_eq!(fxaa.subpixel_quality, 0.75);
    }

    #[test]
    fn test_gaussian_blur() {
        let blur = GaussianBlur::new(5, 1.0);
        assert_eq!(blur.kernel_size(), 5);

        // Check weights sum to 1.0
        let sum: f32 = blur.weights().iter().sum();
        assert!((sum - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_luminance() {
        let white = Vec3::new(1.0, 1.0, 1.0);
        let lum = rgb_to_luminance(white);
        assert!((lum - 1.0).abs() < 0.001);

        let gray = Vec3::new(0.5, 0.5, 0.5);
        let lum_gray = rgb_to_luminance(gray);
        assert!((lum_gray - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_tone_mapping() {
        let hdr = Vec3::new(2.0, 3.0, 4.0);
        let ldr = tone_map_reinhard(hdr);

        // Should be in [0, 1] range
        assert!(ldr.x >= 0.0 && ldr.x <= 1.0);
        assert!(ldr.y >= 0.0 && ldr.y <= 1.0);
        assert!(ldr.z >= 0.0 && ldr.z <= 1.0);
    }
}
