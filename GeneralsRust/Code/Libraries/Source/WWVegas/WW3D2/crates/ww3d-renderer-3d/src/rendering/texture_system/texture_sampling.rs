//! Texture Sampling and Filtering System
//!
//! This module provides texture sampling configuration, filtering modes,
//! and anisotropic filtering support for the texture system.

use std::collections::HashMap;
use std::sync::Arc;
use wgpu::{
    AddressMode, CompareFunction, Device, FilterMode, Sampler, SamplerBorderColor,
    SamplerDescriptor,
};

/// Texture filtering quality levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureFilterQuality {
    /// Point sampling (nearest neighbor)
    Point,
    /// Bilinear filtering
    Bilinear,
    /// Trilinear filtering (bilinear + mipmap interpolation)
    Trilinear,
    /// Anisotropic filtering
    Anisotropic,
}

/// Texture address/wrap modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureAddressMode {
    /// Repeat the texture
    Repeat,
    /// Mirror the texture
    MirrorRepeat,
    /// Clamp to edge
    ClampToEdge,
    /// Clamp to border
    ClampToBorder,
}

/// Texture sampling configuration
#[derive(Debug, Clone)]
pub struct TextureSamplingConfig {
    pub filter_quality: TextureFilterQuality,
    pub address_mode_u: TextureAddressMode,
    pub address_mode_v: TextureAddressMode,
    pub address_mode_w: TextureAddressMode,
    pub anisotropy_level: u16,
    pub mipmap_bias: f32,
    pub compare_function: Option<CompareFunction>,
    pub border_color: SamplerBorderColor,
}

impl PartialEq for TextureSamplingConfig {
    fn eq(&self, other: &Self) -> bool {
        self.filter_quality == other.filter_quality
            && self.address_mode_u == other.address_mode_u
            && self.address_mode_v == other.address_mode_v
            && self.address_mode_w == other.address_mode_w
            && self.anisotropy_level == other.anisotropy_level
            && (self.mipmap_bias - other.mipmap_bias).abs() < f32::EPSILON
            && self.compare_function == other.compare_function
            && self.border_color == other.border_color
    }
}

impl Eq for TextureSamplingConfig {}

impl std::hash::Hash for TextureSamplingConfig {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.filter_quality.hash(state);
        self.address_mode_u.hash(state);
        self.address_mode_v.hash(state);
        self.address_mode_w.hash(state);
        self.anisotropy_level.hash(state);
        // Note: mipmap_bias is not included in hash due to f32 precision issues
        self.compare_function.hash(state);
        self.border_color.hash(state);
    }
}

impl Default for TextureSamplingConfig {
    fn default() -> Self {
        Self {
            filter_quality: TextureFilterQuality::Trilinear,
            address_mode_u: TextureAddressMode::Repeat,
            address_mode_v: TextureAddressMode::Repeat,
            address_mode_w: TextureAddressMode::Repeat,
            anisotropy_level: 16,
            mipmap_bias: 0.0,
            compare_function: None,
            border_color: SamplerBorderColor::TransparentBlack,
        }
    }
}

impl TextureSamplingConfig {
    /// Create a configuration optimized for UI textures
    pub fn ui_optimized() -> Self {
        Self {
            filter_quality: TextureFilterQuality::Bilinear,
            address_mode_u: TextureAddressMode::ClampToEdge,
            address_mode_v: TextureAddressMode::ClampToEdge,
            address_mode_w: TextureAddressMode::ClampToEdge,
            anisotropy_level: 1,
            mipmap_bias: 0.0,
            compare_function: None,
            border_color: SamplerBorderColor::TransparentBlack,
        }
    }

    /// Create a configuration optimized for terrain textures
    pub fn terrain_optimized() -> Self {
        Self {
            filter_quality: TextureFilterQuality::Anisotropic,
            address_mode_u: TextureAddressMode::Repeat,
            address_mode_v: TextureAddressMode::Repeat,
            address_mode_w: TextureAddressMode::Repeat,
            anisotropy_level: 16,
            mipmap_bias: -0.5, // Sharper terrain textures
            compare_function: None,
            border_color: SamplerBorderColor::TransparentBlack,
        }
    }

    /// Create a configuration for shadow mapping
    pub fn shadow_mapping() -> Self {
        Self {
            filter_quality: TextureFilterQuality::Point,
            address_mode_u: TextureAddressMode::ClampToBorder,
            address_mode_v: TextureAddressMode::ClampToBorder,
            address_mode_w: TextureAddressMode::ClampToBorder,
            anisotropy_level: 1,
            mipmap_bias: 0.0,
            compare_function: Some(CompareFunction::LessEqual),
            border_color: SamplerBorderColor::OpaqueWhite,
        }
    }

    /// Create a point sampling configuration
    pub fn point_sampling() -> Self {
        Self {
            filter_quality: TextureFilterQuality::Point,
            address_mode_u: TextureAddressMode::ClampToEdge,
            address_mode_v: TextureAddressMode::ClampToEdge,
            address_mode_w: TextureAddressMode::ClampToEdge,
            anisotropy_level: 1,
            mipmap_bias: 0.0,
            compare_function: None,
            border_color: SamplerBorderColor::TransparentBlack,
        }
    }
}

/// Texture sampler manager
pub struct TextureSamplerManager {
    device: Arc<Device>,
    samplers: HashMap<TextureSamplingConfig, Arc<Sampler>>,
    default_sampler: Arc<Sampler>,
}

impl TextureSamplerManager {
    /// Create new texture sampler manager
    pub fn new(device: Arc<Device>) -> Self {
        let default_config = TextureSamplingConfig::default();
        let default_sampler = Arc::new(Self::create_sampler(&device, &default_config));

        let mut manager = Self {
            device,
            samplers: HashMap::new(),
            default_sampler: default_sampler.clone(),
        };

        // Pre-create common samplers
        manager.get_or_create_sampler(&default_config);
        manager.get_or_create_sampler(&TextureSamplingConfig::ui_optimized());
        manager.get_or_create_sampler(&TextureSamplingConfig::terrain_optimized());
        manager.get_or_create_sampler(&TextureSamplingConfig::shadow_mapping());
        manager.get_or_create_sampler(&TextureSamplingConfig::point_sampling());

        manager
    }

    /// Get or create a sampler for the given configuration
    pub fn get_or_create_sampler(&mut self, config: &TextureSamplingConfig) -> Arc<Sampler> {
        if let Some(sampler) = self.samplers.get(config) {
            return sampler.clone();
        }

        let sampler = Arc::new(Self::create_sampler(&self.device, config));
        self.samplers.insert(config.clone(), sampler.clone());
        sampler
    }

    /// Get the default sampler
    pub fn get_default_sampler(&self) -> Arc<Sampler> {
        self.default_sampler.clone()
    }

    /// Get a pre-configured sampler for UI textures
    pub fn get_ui_sampler(&mut self) -> Arc<Sampler> {
        self.get_or_create_sampler(&TextureSamplingConfig::ui_optimized())
    }

    /// Get a pre-configured sampler for terrain textures
    pub fn get_terrain_sampler(&mut self) -> Arc<Sampler> {
        self.get_or_create_sampler(&TextureSamplingConfig::terrain_optimized())
    }

    /// Get a pre-configured sampler for shadow mapping
    pub fn get_shadow_sampler(&mut self) -> Arc<Sampler> {
        self.get_or_create_sampler(&TextureSamplingConfig::shadow_mapping())
    }

    /// Get a pre-configured point sampler
    pub fn get_point_sampler(&mut self) -> Arc<Sampler> {
        self.get_or_create_sampler(&TextureSamplingConfig::point_sampling())
    }

    /// Create a WGPU sampler from configuration
    fn create_sampler(device: &Device, config: &TextureSamplingConfig) -> Sampler {
        let (mag_filter, min_filter, mipmap_filter) = match config.filter_quality {
            TextureFilterQuality::Point => (
                FilterMode::Nearest,
                FilterMode::Nearest,
                FilterMode::Nearest,
            ),
            TextureFilterQuality::Bilinear => {
                (FilterMode::Linear, FilterMode::Linear, FilterMode::Nearest)
            }
            TextureFilterQuality::Trilinear | TextureFilterQuality::Anisotropic => {
                (FilterMode::Linear, FilterMode::Linear, FilterMode::Linear)
            }
        };

        let address_mode_u = Self::convert_address_mode(config.address_mode_u);
        let address_mode_v = Self::convert_address_mode(config.address_mode_v);
        let address_mode_w = Self::convert_address_mode(config.address_mode_w);

        let anisotropy_clamp = match config.filter_quality {
            TextureFilterQuality::Anisotropic => config.anisotropy_level,
            _ => 1, // Default to 1 when not using anisotropic filtering
        };

        device.create_sampler(&SamplerDescriptor {
            label: Some("Texture Sampler"),
            address_mode_u,
            address_mode_v,
            address_mode_w,
            mag_filter,
            min_filter,
            mipmap_filter,
            lod_min_clamp: config.mipmap_bias,
            lod_max_clamp: 32.0,
            compare: config.compare_function,
            anisotropy_clamp,
            border_color: Some(config.border_color),
        })
    }

    /// Convert custom address mode to WGPU address mode
    fn convert_address_mode(mode: TextureAddressMode) -> AddressMode {
        match mode {
            TextureAddressMode::Repeat => AddressMode::Repeat,
            TextureAddressMode::MirrorRepeat => AddressMode::MirrorRepeat,
            TextureAddressMode::ClampToEdge => AddressMode::ClampToEdge,
            TextureAddressMode::ClampToBorder => AddressMode::ClampToBorder,
        }
    }

    /// Get statistics about created samplers
    pub fn get_stats(&self) -> TextureSamplerStats {
        TextureSamplerStats {
            total_samplers: self.samplers.len(),
            unique_configs: self.samplers.len(),
        }
    }

    /// Clear unused samplers (call periodically to manage memory)
    pub fn cleanup_unused_samplers(&mut self) {
        // Remove samplers that are only referenced by the manager
        self.samplers
            .retain(|_, sampler| Arc::strong_count(sampler) > 1);
    }
}

/// Statistics for texture sampler manager
#[derive(Debug, Clone)]
pub struct TextureSamplerStats {
    pub total_samplers: usize,
    pub unique_configs: usize,
}

/// Texture filtering utilities
pub struct TextureFilteringUtils;

impl TextureFilteringUtils {
    /// Calculate optimal anisotropy level based on distance and angle
    pub fn calculate_anisotropy_level(
        distance: f32,
        surface_angle: f32,
        max_anisotropy: u16,
    ) -> u16 {
        // Calculate anisotropy based on viewing angle
        // Use sin for perpendicular views (90 degrees = high anisotropy)
        // and cos for grazing angles (0 degrees = low anisotropy)
        let angle_factor = (surface_angle.abs().sin()).max(0.1);

        // Distance-based anisotropy reduction
        let distance_factor = (100.0 / (distance + 10.0)).min(1.0);

        let calculated_level = (max_anisotropy as f32 * angle_factor * distance_factor) as u16;
        calculated_level.max(1).min(max_anisotropy)
    }

    /// Get recommended filter quality based on texture usage
    pub fn get_recommended_filter_quality(texture_usage: TextureUsage) -> TextureFilterQuality {
        match texture_usage {
            TextureUsage::Diffuse => TextureFilterQuality::Anisotropic,
            TextureUsage::Normal => TextureFilterQuality::Trilinear,
            TextureUsage::Specular => TextureFilterQuality::Trilinear,
            TextureUsage::UI => TextureFilterQuality::Bilinear,
            TextureUsage::HUD => TextureFilterQuality::Point,
            TextureUsage::Font => TextureFilterQuality::Bilinear,
            TextureUsage::Terrain => TextureFilterQuality::Anisotropic,
            TextureUsage::Water => TextureFilterQuality::Trilinear,
            TextureUsage::Sky => TextureFilterQuality::Bilinear,
            TextureUsage::Shadow => TextureFilterQuality::Point,
        }
    }

    /// Calculate mipmap bias for different scenarios
    pub fn calculate_mipmap_bias(
        texture_usage: TextureUsage,
        distance: f32,
        quality_preference: f32, // 0.0 = performance, 1.0 = quality
    ) -> f32 {
        // UI elements should always have zero bias regardless of distance
        if matches!(
            texture_usage,
            TextureUsage::UI | TextureUsage::HUD | TextureUsage::Font
        ) {
            return 0.0;
        }

        let base_bias = match texture_usage {
            TextureUsage::Terrain => -0.5, // Sharper terrain
            _ => -0.25,                    // Slightly sharper for most textures
        };

        // Adjust based on distance (closer objects get sharper textures)
        let distance_bias = if distance < 50.0 {
            -0.25 * (1.0 - distance / 50.0)
        } else {
            0.25 * ((distance - 50.0) / 200.0).min(1.0)
        };

        // Adjust based on quality preference
        let quality_bias = (1.0 - quality_preference) * 0.5;

        (base_bias + distance_bias + quality_bias).clamp(-2.0, 2.0)
    }
}

/// Texture usage categories for automatic filter selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureUsage {
    /// Diffuse/albedo textures
    Diffuse,
    /// Normal maps
    Normal,
    /// Specular/metallic textures
    Specular,
    /// UI elements
    UI,
    /// HUD elements
    HUD,
    /// Text/fonts
    Font,
    /// Terrain textures
    Terrain,
    /// Water surfaces
    Water,
    /// Sky textures
    Sky,
    /// Shadow maps
    Shadow,
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_anisotropy_calculation() {
        // Perpendicular view (90 degrees) should get full anisotropy
        let level = TextureFilteringUtils::calculate_anisotropy_level(10.0, 1.57, 16);
        assert!(level > 8); // Should be high anisotropy

        // Grazing angle (near 0 degrees) should get low anisotropy
        let level = TextureFilteringUtils::calculate_anisotropy_level(10.0, 0.1, 16);
        assert!(level < 6); // Should be low anisotropy (sin(0.1) ≈ 0.1)

        // Very far distance should reduce anisotropy
        let level = TextureFilteringUtils::calculate_anisotropy_level(1000.0, 1.57, 16);
        assert!(level < 8); // Should be reduced due to distance
    }

    #[test]
    fn test_filter_quality_recommendations() {
        assert_eq!(
            TextureFilteringUtils::get_recommended_filter_quality(TextureUsage::Terrain),
            TextureFilterQuality::Anisotropic
        );

        assert_eq!(
            TextureFilteringUtils::get_recommended_filter_quality(TextureUsage::HUD),
            TextureFilterQuality::Point
        );

        assert_eq!(
            TextureFilteringUtils::get_recommended_filter_quality(TextureUsage::UI),
            TextureFilterQuality::Bilinear
        );
    }

    #[test]
    fn test_mipmap_bias_calculation() {
        // Terrain should get negative bias (sharper)
        let bias = TextureFilteringUtils::calculate_mipmap_bias(TextureUsage::Terrain, 25.0, 1.0);
        assert!(bias < 0.0);

        // UI should get zero bias
        let bias = TextureFilteringUtils::calculate_mipmap_bias(TextureUsage::UI, 25.0, 1.0);
        assert_eq!(bias, 0.0);

        // Close objects should get sharper textures
        let bias_close =
            TextureFilteringUtils::calculate_mipmap_bias(TextureUsage::Diffuse, 10.0, 1.0);
        let bias_far =
            TextureFilteringUtils::calculate_mipmap_bias(TextureUsage::Diffuse, 100.0, 1.0);
        assert!(bias_close < bias_far);
    }
}
