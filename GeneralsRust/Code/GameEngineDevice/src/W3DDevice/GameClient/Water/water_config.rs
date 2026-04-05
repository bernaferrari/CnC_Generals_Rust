//! Water Configuration Module
//!
//! Corresponds to C++ files:
//! - GameEngine/Include/GameClient/Water.h
//! - GameEngine/Source/GameClient/Water.cpp
//! - GameEngine/Source/Common/INI/INIWater.cpp
//!
//! This module provides water settings and configuration that can be loaded from INI files.

use std::f32::consts::PI;

/// Number of time of day settings (Morning, Afternoon, Evening, Night)
pub const TIME_OF_DAY_COUNT: usize = 4;

/// Water rendering type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterType {
    /// Translucent water, no reflection (simple, fast)
    Translucent = 0,
    /// Legacy framebuffer reflection (non-translucent)
    FbReflection = 1,
    /// Pixel/vertex shader with texture reflection (modern)
    PvShader = 2,
    /// 3D mesh-based water with wave simulation
    GridMesh = 3,
}

/// Time of day enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeOfDay {
    Morning = 0,
    Afternoon = 1,
    Evening = 2,
    Night = 3,
}

impl TimeOfDay {
    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(TimeOfDay::Morning),
            1 => Some(TimeOfDay::Afternoon),
            2 => Some(TimeOfDay::Evening),
            3 => Some(TimeOfDay::Night),
            _ => None,
        }
    }
}

/// Water rendering settings for a specific time of day
/// Matches C++ WaterSetting struct from Water.h
#[derive(Debug, Clone)]
pub struct WaterSetting {
    /// Sky texture filename
    pub sky_texture_file: String,
    /// Water surface texture filename
    pub water_texture_file: String,
    /// Number of times to repeat water texture
    pub water_repeat_count: i32,
    /// Texel density of sky plane (higher value repeats texture more)
    pub sky_texels_per_unit: f32,
    /// Vertex colors for water quad corners
    pub vertex00_diffuse: [u8; 4], // RGBA
    pub vertex10_diffuse: [u8; 4],
    pub vertex11_diffuse: [u8; 4],
    pub vertex01_diffuse: [u8; 4],
    /// Water diffuse color
    pub water_diffuse_color: [u8; 4],
    /// Transparent water diffuse color
    pub transparent_water_diffuse: [u8; 4],
    /// UV scroll rate in U direction (texels per millisecond)
    pub u_scroll_per_ms: f32,
    /// UV scroll rate in V direction (texels per millisecond)
    pub v_scroll_per_ms: f32,
}

impl Default for WaterSetting {
    fn default() -> Self {
        Self {
            sky_texture_file: String::new(),
            water_texture_file: String::new(),
            water_repeat_count: 0,
            sky_texels_per_unit: 0.0,
            vertex00_diffuse: [0, 0, 0, 0],
            vertex10_diffuse: [0, 0, 0, 0],
            vertex11_diffuse: [0, 0, 0, 0],
            vertex01_diffuse: [0, 0, 0, 0],
            water_diffuse_color: [0, 0, 0, 0],
            transparent_water_diffuse: [0, 0, 0, 0],
            u_scroll_per_ms: 0.0,
            v_scroll_per_ms: 0.0,
        }
    }
}

/// Water transparency settings (map-specific overrides)
/// Matches C++ WaterTransparencySetting from Water.h
#[derive(Debug, Clone)]
pub struct WaterTransparencySetting {
    /// Depth at which water becomes fully opaque
    pub transparent_water_depth: f32,
    /// Minimum opacity for water surface
    pub min_water_opacity: f32,
    /// Color of standing water
    pub standing_water_color: [f32; 3], // RGB
    /// Water color on radar/minimap
    pub radar_color: [u8; 3], // RGB
    /// Use additive blending instead of alpha blending
    pub additive_blend: bool,
    /// Standing water texture filename
    pub standing_water_texture: String,
    /// Skybox texture filenames (5 sides: N, E, S, W, T)
    pub skybox_texture_n: String,
    pub skybox_texture_e: String,
    pub skybox_texture_s: String,
    pub skybox_texture_w: String,
    pub skybox_texture_t: String,
}

impl Default for WaterTransparencySetting {
    fn default() -> Self {
        Self {
            transparent_water_depth: 3.0,
            min_water_opacity: 1.0,
            standing_water_color: [1.0, 1.0, 1.0],
            radar_color: [140, 140, 255],
            additive_blend: false,
            standing_water_texture: "TWWater01.tga".to_string(),
            skybox_texture_n: "TSMorningN.tga".to_string(),
            skybox_texture_e: "TSMorningE.tga".to_string(),
            skybox_texture_s: "TSMorningS.tga".to_string(),
            skybox_texture_w: "TSMorningW.tga".to_string(),
            skybox_texture_t: "TSMorningT.tga".to_string(),
        }
    }
}

/// Water rendering constants
/// Matches defines from C++ W3DWater.cpp
pub mod constants {
    use super::*;

    // Sky rendering
    pub const SKYPLANE_SIZE: f32 = 384.0; // Size of sky plane
    pub const SKYPLANE_HEIGHT: f32 = 30.0; // Height of sky plane
    pub const SKYBODY_SIZE: f32 = 45.0; // Size of sky body (sun/moon)
    pub const SKYBODY_HEIGHT: f32 = SKYPLANE_HEIGHT;

    // GeForce3/modern water system
    pub const PATCH_SIZE: usize = 15; // Vertices on patch edge
    pub const PATCH_UV_TILES: f32 = 42.0; // Times bump map tiles across patch
    pub const PATCH_SCALE: f32 = 4.0; // Horizontal scale factor
    pub const SEA_REFLECTION_SIZE: u32 = 256; // Reflection texture dimensions
    pub const SEA_BUMP_SCALE: f32 = 0.06; // Bump map perturbation scale
    pub const BUMP_SIZE: f32 = 50.0;
    pub const REFLECTION_FACTOR: f32 = 0.1;
    pub const PATCH_WIDTH: usize = PATCH_SIZE - 1;
    pub const PATCH_UV_SCALE: f32 = PATCH_UV_TILES / PATCH_WIDTH as f32;

    // 3D Grid mesh water
    pub const WATER_MESH_OPACITY: f32 = 0.5;
    pub const WATER_MESH_X_VERTICES: usize = 128;
    pub const WATER_MESH_Y_VERTICES: usize = 128;
    pub const WATER_MESH_SPACING: f32 = 1.0; // Same as terrain

    // Animation
    pub const NUM_BUMP_FRAMES: usize = 32; // Bump map animation frames

    // Wave simulation
    pub const DONUT_SIDES: usize = 90;
    pub const INNER_RADIUS: f32 = 200.0;
    pub const OUTER_RADIUS: f32 = 250.0;
    pub const TEXTURE_REPEAT_COUNT: usize = 16;
    pub const DONUT_HEIGHT: f32 = 15.0;
    pub const AMP_SCALE: f32 = 30.0 / 120.0;
    pub const WAVE_FREQ: f32 = 0.3;
    pub const AMP_SCALE2: f32 = 10.0 / 120.0;
    pub const NOISE_FREQ: f32 = 2.0 * PI / WAVE_FREQ;
    pub const NOISE_REPEAT_FACTOR: f32 = 1.0 / 16.0;
}

/// Wave animation parameters for water tracks/wakes
#[derive(Debug, Clone)]
pub struct WaveParameters {
    /// Initial width of wave
    pub initial_width: f32,
    /// Initial height of wave
    pub initial_height: f32,
    /// Final width at full expansion
    pub final_width: f32,
    /// Fraction along path when wave reaches full width
    pub final_width_peak_frac: f32,
    /// Final height of wave
    pub final_height: f32,
    /// Initial velocity (world units per ms)
    pub initial_velocity: f32,
    /// Total distance traveled
    pub wave_distance: f32,
    /// Time to reach beach/shore
    pub time_to_reach_beach: f32,
    /// Front slow-down acceleration
    pub front_slowdown_acc: f32,
    /// Time to stop moving
    pub time_to_stop: f32,
    /// Time to retreat
    pub time_to_retreat: f32,
    /// Back slow-down acceleration
    pub back_slowdown_acc: f32,
    /// Time to compress
    pub time_to_compress: f32,
    /// Fade time after stopping
    pub fade_ms: f32,
    /// Total animation time
    pub total_ms: f32,
}

impl Default for WaveParameters {
    fn default() -> Self {
        Self {
            initial_width: 2.0,
            initial_height: 0.5,
            final_width: 10.0,
            final_width_peak_frac: 0.7,
            final_height: 1.5,
            initial_velocity: 0.1,
            wave_distance: 100.0,
            time_to_reach_beach: 5000.0,
            front_slowdown_acc: 0.01,
            time_to_stop: 1000.0,
            time_to_retreat: 2000.0,
            back_slowdown_acc: 0.02,
            time_to_compress: 1500.0,
            fade_ms: 2000.0,
            total_ms: 10000.0,
        }
    }
}

/// Grid-based water mesh data for 3D water simulation
/// Matches C++ WaterMeshData from W3DWater.h
#[derive(Debug, Clone)]
pub struct WaterMeshData {
    /// Height of mesh at this point
    pub height: f32,
    /// Velocity in Z (vertical) direction
    pub velocity: f32,
    /// Status flags for this grid point
    pub status: u8,
    /// Preferred height to settle to
    pub preferred_height: u8,
}

impl Default for WaterMeshData {
    fn default() -> Self {
        Self {
            height: 0.0,
            velocity: 0.0,
            status: 0,
            preferred_height: 0,
        }
    }
}

/// Grid mesh status flags
pub mod mesh_status {
    pub const AT_REST: u8 = 0x00;
    pub const IN_MOTION: u8 = 0x01;
}

/// Grid transformation parameters
#[derive(Debug, Clone)]
pub struct GridTransform {
    /// Grid origin in world space
    pub origin: [f32; 2],
    /// Direction vector along X axis (scaled to world space)
    pub direction_x: [f32; 2],
    /// Direction vector along Y axis (scaled to world space)
    pub direction_y: [f32; 2],
    /// Width in object space
    pub width: f32,
    /// Height in object space
    pub height: f32,
    /// Minimum allowed height
    pub min_height: f32,
    /// Maximum allowed height
    pub max_height: f32,
    /// Cell size in world space
    pub cell_size: f32,
    /// Number of cells along X
    pub cells_x: usize,
    /// Number of cells along Y
    pub cells_y: usize,
    /// Maximum range for height changes
    pub change_max_range: f32,
    /// Attenuation factors for height changes
    pub change_att0: f32,
    pub change_att1: f32,
    pub change_att2: f32,
}

impl Default for GridTransform {
    fn default() -> Self {
        Self {
            origin: [0.0, 0.0],
            direction_x: [1.0, 0.0],
            direction_y: [0.0, 1.0],
            width: constants::WATER_MESH_X_VERTICES as f32 * constants::WATER_MESH_SPACING,
            height: constants::WATER_MESH_Y_VERTICES as f32 * constants::WATER_MESH_SPACING,
            min_height: -10.0,
            max_height: 10.0,
            cell_size: constants::WATER_MESH_SPACING,
            cells_x: constants::WATER_MESH_X_VERTICES,
            cells_y: constants::WATER_MESH_Y_VERTICES,
            change_max_range: 50.0,
            change_att0: 1.0,
            change_att1: 0.5,
            change_att2: 0.1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_of_day_conversion() {
        assert_eq!(TimeOfDay::from_index(0), Some(TimeOfDay::Morning));
        assert_eq!(TimeOfDay::from_index(1), Some(TimeOfDay::Afternoon));
        assert_eq!(TimeOfDay::from_index(2), Some(TimeOfDay::Evening));
        assert_eq!(TimeOfDay::from_index(3), Some(TimeOfDay::Night));
        assert_eq!(TimeOfDay::from_index(4), None);
    }

    #[test]
    fn test_water_setting_defaults() {
        let setting = WaterSetting::default();
        assert_eq!(setting.water_repeat_count, 0);
        assert_eq!(setting.sky_texels_per_unit, 0.0);
    }

    #[test]
    fn test_transparency_defaults() {
        let trans = WaterTransparencySetting::default();
        assert_eq!(trans.transparent_water_depth, 3.0);
        assert_eq!(trans.min_water_opacity, 1.0);
        assert_eq!(trans.standing_water_texture, "TWWater01.tga");
    }

    #[test]
    fn test_constants() {
        use constants::*;
        assert_eq!(PATCH_SIZE, 15);
        assert_eq!(PATCH_WIDTH, 14);
        assert_eq!(NUM_BUMP_FRAMES, 32);
        assert_eq!(SEA_REFLECTION_SIZE, 256);
    }
}
