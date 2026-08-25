//! Snow manager and weather settings (ported from GameClient/Snow.cpp).

use game_engine::common::ini::ini::{INI, INIError, INIResult, register_block_parser};
use game_engine::common::ini::ini_weather;
use once_cell::sync::OnceCell;
use rand::Rng;
use std::sync::{Arc, Mutex, RwLock};

const SNOW_NOISE_X: usize = 64;
const SNOW_NOISE_Y: usize = 64;

#[derive(Debug, Clone)]
pub struct WeatherSetting {
    pub snow_texture: String,
    pub snow_frequency_scale_x: f32,
    pub snow_frequency_scale_y: f32,
    pub snow_amplitude: f32,
    pub snow_point_size: f32,
    pub snow_max_point_size: f32,
    pub snow_min_point_size: f32,
    pub snow_quad_size: f32,
    pub snow_box_dimensions: f32,
    pub snow_box_density: f32,
    pub snow_velocity: f32,
    pub use_point_sprites: bool,
    pub snow_enabled: bool,
}

impl Default for WeatherSetting {
    fn default() -> Self {
        Self {
            snow_texture: "EXSnowFlake.tga".to_string(),
            snow_frequency_scale_x: 0.0533,
            snow_frequency_scale_y: 0.0275,
            snow_amplitude: 5.0,
            snow_point_size: 1.0,
            snow_max_point_size: 64.0,
            snow_min_point_size: 0.0,
            snow_quad_size: 0.5,
            snow_box_dimensions: 200.0,
            snow_box_density: 1.0,
            snow_velocity: 4.0,
            use_point_sprites: true,
            snow_enabled: false,
        }
    }
}

impl WeatherSetting {
    fn from_common(common: &ini_weather::WeatherSetting) -> Self {
        Self {
            snow_texture: common.snow_texture.clone(),
            snow_frequency_scale_x: common.snow_frequency_scale_x,
            snow_frequency_scale_y: common.snow_frequency_scale_y,
            snow_amplitude: common.snow_amplitude,
            snow_point_size: common.snow_point_size,
            snow_max_point_size: common.snow_max_point_size,
            snow_min_point_size: common.snow_min_point_size,
            snow_quad_size: common.snow_quad_size,
            snow_box_dimensions: common.snow_box_dimensions,
            snow_box_density: common.snow_box_density,
            snow_velocity: common.snow_velocity,
            use_point_sprites: common.use_point_sprites,
            snow_enabled: common.snow_enabled,
        }
    }

    fn apply_field(&mut self, key: &str, args: &[&str]) -> INIResult<()> {
        let mut tokens: Vec<&str> = args.iter().copied().filter(|t| *t != "=").collect();
        if tokens.is_empty() {
            return Err(INIError::InvalidData);
        }
        match key {
            "SnowTexture" => self.snow_texture = INI::parse_ascii_string(tokens[0])?,
            "SnowFrequencyScaleX" => self.snow_frequency_scale_x = INI::parse_real(tokens[0])?,
            "SnowFrequencyScaleY" => self.snow_frequency_scale_y = INI::parse_real(tokens[0])?,
            "SnowAmplitude" => self.snow_amplitude = INI::parse_real(tokens[0])?,
            "SnowPointSize" => self.snow_point_size = INI::parse_real(tokens[0])?,
            "SnowMaxPointSize" => self.snow_max_point_size = INI::parse_real(tokens[0])?,
            "SnowMinPointSize" => self.snow_min_point_size = INI::parse_real(tokens[0])?,
            "SnowQuadSize" => self.snow_quad_size = INI::parse_real(tokens[0])?,
            "SnowBoxDimensions" => self.snow_box_dimensions = INI::parse_real(tokens[0])?,
            "SnowBoxDensity" => self.snow_box_density = INI::parse_real(tokens[0])?,
            "SnowVelocity" => self.snow_velocity = INI::parse_real(tokens[0])?,
            "SnowPointSprites" => self.use_point_sprites = INI::parse_bool(tokens[0])?,
            "SnowEnabled" => self.snow_enabled = INI::parse_bool(tokens[0])?,
            _ => return Err(INIError::InvalidData),
        }
        Ok(())
    }
}

static WEATHER_SETTING: OnceCell<Arc<RwLock<WeatherSetting>>> = OnceCell::new();
static SNOW_MANAGER: OnceCell<Arc<Mutex<SnowManager>>> = OnceCell::new();

fn sync_weather_from_common(dest: &Arc<RwLock<WeatherSetting>>) {
    if let Some(common) = ini_weather::get_weather_setting() {
        if let Ok(mut guard) = dest.write() {
            *guard = WeatherSetting::from_common(&common);
        }
    }
}

pub fn get_weather_setting() -> Option<Arc<RwLock<WeatherSetting>>> {
    let settings = ensure_weather_setting();
    sync_weather_from_common(&settings);
    Some(settings)
}

pub fn get_snow_manager() -> Option<Arc<Mutex<SnowManager>>> {
    SNOW_MANAGER.get().cloned()
}

pub fn ensure_weather_setting() -> Arc<RwLock<WeatherSetting>> {
    let settings = WEATHER_SETTING
        .get_or_init(|| {
            let initial = ini_weather::get_weather_setting()
                .map(|common| WeatherSetting::from_common(&common))
                .unwrap_or_default();
            Arc::new(RwLock::new(initial))
        })
        .clone();
    sync_weather_from_common(&settings);
    settings
}

pub fn initialize_snow_manager() -> Arc<Mutex<SnowManager>> {
    let _ = ensure_weather_setting();
    let manager = SNOW_MANAGER.get_or_init(|| Arc::new(Mutex::new(SnowManager::new())));
    if let Ok(mut guard) = manager.lock() {
        guard.init();
    }
    manager.clone()
}

pub fn register_weather_definition_parser() {
    register_block_parser("Weather", parse_weather_definition);
}

fn parse_weather_definition(ini: &mut INI) -> INIResult<()> {
    // Single TheWeatherSetting store (C++ Snow.cpp). Common owns the override chain.
    ini_weather::parse_weather_definition(ini)?;
    let settings = ensure_weather_setting();
    sync_weather_from_common(&settings);

    if let Some(manager) = get_snow_manager() {
        if let Ok(mut guard) = manager.lock() {
            guard.update_ini_settings();
        }
    }

    Ok(())
}

/// C++ `AABoxClass` X/Y used to clip the snow emitter cube
/// (`W3DSnow.cpp:345-363`). Horizontal plane is C++ Z-up X/Y
/// (leftover Y-up X/Z).
#[derive(Debug, Clone, Copy)]
pub struct SnowVisibleBoxXy {
    pub center_x: f32,
    pub center_y: f32,
    pub extent_x: f32,
    pub extent_y: f32,
}

/// C++ `W3DSnowManager::renderAsQuads` view-space offsets
/// `(-0.5,0.5,0)*m_quadSize` and matching UVs. `right`/`up` are camera
/// axes in the same world space as `center`.
#[must_use]
pub fn camera_facing_quad_corners(
    center: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
    quad_size: f32,
) -> [([f32; 3], [f32; 2]); 4] {
    let offsets = [
        ([-0.5, 0.5], [0.0, 0.0]),
        ([-0.5, -0.5], [0.0, 1.0]),
        ([0.5, -0.5], [1.0, 1.0]),
        ([0.5, 0.5], [1.0, 0.0]),
    ];
    offsets.map(|([ox, oy], uv)| {
        (
            [
                center[0] + (right[0] * ox + up[0] * oy) * quad_size,
                center[1] + (right[1] * ox + up[1] * oy) * quad_size,
                center[2] + (right[2] * ox + up[2] * oy) * quad_size,
            ],
            uv,
        )
    })
}

#[derive(Debug)]
pub struct SnowManager {
    starting_heights: Vec<f32>,
    time: f32,
    velocity: f32,
    frequency_scale_x: f32,
    frequency_scale_y: f32,
    amplitude: f32,
    point_size: f32,
    quad_size: f32,
    box_dimensions: f32,
    emitter_spacing: f32,
    max_point_size: f32,
    min_point_size: f32,
    full_time_period: f32,
    is_visible: bool,
}

impl Default for SnowManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SnowManager {
    pub fn new() -> Self {
        Self {
            starting_heights: Vec::new(),
            time: 0.0,
            velocity: 1.0,
            frequency_scale_x: 1.0,
            frequency_scale_y: 1.0,
            amplitude: 1.0,
            point_size: 1.0,
            quad_size: 1.0,
            box_dimensions: 128.0,
            emitter_spacing: 1.0,
            max_point_size: 1.0,
            min_point_size: 1.0,
            full_time_period: 0.0,
            is_visible: true,
        }
    }

    pub fn init(&mut self) {
        self.starting_heights = vec![0.0; SNOW_NOISE_X * SNOW_NOISE_Y];
        self.time = 0.0;
        self.update_ini_settings();
    }

    pub fn update_ini_settings(&mut self) {
        let Some(settings) = get_weather_setting() else {
            return;
        };
        let guard = settings.read().unwrap_or_else(|e| e.into_inner());

        // C++ SnowManager::updateIniSettings never touches m_isVisible.
        if self.starting_heights.len() != SNOW_NOISE_X * SNOW_NOISE_Y {
            self.starting_heights = vec![0.0; SNOW_NOISE_X * SNOW_NOISE_Y];
        }
        let mut rng = rand::thread_rng();
        let box_dimensions = guard.snow_box_dimensions.max(0.0);
        let box_i = box_dimensions.max(1.0) as i32;
        for height in &mut self.starting_heights {
            *height = rng.gen_range(0..box_i) as f32;
        }

        self.velocity = guard.snow_velocity;
        self.frequency_scale_x = guard.snow_frequency_scale_x;
        self.frequency_scale_y = guard.snow_frequency_scale_y;
        self.amplitude = guard.snow_amplitude;
        self.point_size = guard.snow_point_size;
        self.quad_size = guard.snow_quad_size;
        self.box_dimensions = guard.snow_box_dimensions;
        self.emitter_spacing = if guard.snow_box_density != 0.0 {
            1.0 / guard.snow_box_density
        } else {
            0.0
        };
        self.max_point_size = guard.snow_max_point_size;
        self.min_point_size = guard.snow_min_point_size;
        self.full_time_period = if self.velocity.abs() > f32::EPSILON {
            self.box_dimensions / self.velocity
        } else {
            0.0
        };
    }

    pub fn set_visible(&mut self, show_weather: bool) {
        self.is_visible = show_weather;
    }

    pub fn reset(&mut self) {
        self.is_visible = true;
    }

    /// C++ `W3DSnowManager::update` residual: advance `m_time` by frame dt
    /// and wrap on `m_fullTimePeriod`. Base `SnowManager::update` is empty.
    pub fn update(&mut self, delta_seconds: f32) {
        if self.full_time_period <= f32::EPSILON {
            self.time += delta_seconds.max(0.0);
            return;
        }
        self.time += delta_seconds.max(0.0);
        self.time %= self.full_time_period;
    }

    pub fn time(&self) -> f32 {
        self.time
    }

    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    pub fn quad_size(&self) -> f32 {
        self.quad_size
    }

    pub fn starting_heights(&self) -> &[f32] {
        &self.starting_heights
    }

    /// C++ `W3DSnowManager::render` flake centers, mapped to wgpu Y-up.
    ///
    /// `camera` is `[x, y_up, z]`. C++ uses Z-up (`x, y, z_up`).
    pub fn flake_positions_y_up(&self, camera: [f32; 3]) -> Vec<[f32; 3]> {
        self.flake_positions_y_up_clipped(camera, None)
    }

    /// Same centers as [`Self::flake_positions_y_up`], clipped to the
    /// terrain-and-sky visible AABB like `W3DSnow.cpp:345-366`.
    pub fn flake_positions_y_up_clipped(
        &self,
        camera: [f32; 3],
        visible_xy: Option<SnowVisibleBoxXy>,
    ) -> Vec<[f32; 3]> {
        const MAXIMUM_CAMERA_DISTANCE: i32 = 100_000;
        if !self.is_visible {
            return Vec::new();
        }
        let spacing = self.emitter_spacing;
        if !(spacing > f32::EPSILON) || self.box_dimensions <= 0.0 {
            return Vec::new();
        }
        if self.starting_heights.len() < SNOW_NOISE_X * SNOW_NOISE_Y {
            return Vec::new();
        }

        let cam_x = camera[0];
        let cam_y_cpp = camera[2];
        let cam_z_cpp = camera[1];

        let half = (self.box_dimensions / spacing * 0.5).floor() as i32;
        let cube_center_x = (cam_x / spacing).floor() as i32;
        let cube_center_y = (cam_y_cpp / spacing).floor() as i32;
        let mut origin_x = cube_center_x - half;
        let mut origin_y = cube_center_y - half;
        let mut dim_x = cube_center_x + half;
        let mut dim_y = cube_center_y + half;

        // C++ W3DSnow.cpp:347-366 — expand by sine amplitude + quad radius,
        // then clip the emitter cube to the terrain-and-sky AABB so flakes
        // do not keep falling under the ground.
        if let Some(bbox) = visible_xy {
            let expand = self.amplitude + self.quad_size;
            let min_x = bbox.center_x - (bbox.extent_x + expand);
            let min_y = bbox.center_y - (bbox.extent_y + expand);
            let max_x = bbox.center_x + (bbox.extent_x + expand);
            let max_y = bbox.center_y + (bbox.extent_y + expand);
            if (origin_x as f32) * spacing < min_x {
                origin_x = (min_x / spacing).floor() as i32;
            }
            if (origin_y as f32) * spacing < min_y {
                origin_y = (min_y / spacing).floor() as i32;
            }
            if (dim_x as f32) * spacing > max_x {
                dim_x = (max_x / spacing).floor() as i32;
            }
            if (dim_y as f32) * spacing > max_y {
                dim_y = (max_y / spacing).floor() as i32;
            }
        }
        if (dim_y - origin_y) < 0 || (dim_x - origin_x) < 0 {
            return Vec::new();
        }
        let total = (dim_y - origin_y) * (dim_x - origin_x);
        if total <= 0 {
            return Vec::new();
        }

        let snow_ceiling = cam_z_cpp + self.box_dimensions * 0.5;
        let camera_offset = cam_z_cpp.rem_euclid(self.box_dimensions);
        let height_traveled = self.time * self.velocity + camera_offset;

        let mut flakes = Vec::with_capacity(total as usize);
        for y in origin_y..dim_y {
            for x in origin_x..dim_x {
                let noise_x = (x + MAXIMUM_CAMERA_DISTANCE) & (SNOW_NOISE_X as i32 - 1);
                let noise_y = (y + MAXIMUM_CAMERA_DISTANCE) & (SNOW_NOISE_Y as i32 - 1);
                let mut noise_offset = (noise_x + noise_y * SNOW_NOISE_X as i32) as usize;
                if noise_offset >= self.starting_heights.len() {
                    noise_offset = 0;
                }
                let h0 = snow_ceiling
                    - (height_traveled + self.starting_heights[noise_offset])
                        .rem_euclid(self.box_dimensions);
                let wx = x as f32 * spacing
                    + self.amplitude * (h0 * self.frequency_scale_x + x as f32).sin();
                let wy_cpp = y as f32 * spacing
                    + self.amplitude * (h0 * self.frequency_scale_y + y as f32).sin();
                flakes.push([wx, h0, wy_cpp]);
            }
        }
        flakes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terrain_visual_update_does_not_double_tick_snow() {
        // C++ GameClient.cpp:560 is the only TheSnowManager->UPDATE.
        let traits = include_str!("terrain/terrain_visual/traits.rs");
        let update_fn = traits
            .split("fn update(&mut self)")
            .nth(1)
            .and_then(|rest| rest.split("fn reset(").next())
            .unwrap_or(traits);
        assert!(
            !update_fn.contains("get_snow_manager") && !update_fn.contains("guard.update("),
            "TerrainVisual::update must not tick SnowManager"
        );
        let client = include_str!("core/game_client/impl_update.rs");
        assert!(
            client.contains("update_cpp_snow_and_anim2d"),
            "GameClient keeps the single C++ snow UPDATE"
        );
    }

    #[test]
    fn weather_setting_defaults_match_cpp_constructor() {
        let setting = WeatherSetting::default();

        assert_eq!(setting.snow_texture, "EXSnowFlake.tga");
        assert!((setting.snow_frequency_scale_x - 0.0533).abs() < f32::EPSILON);
        assert!((setting.snow_frequency_scale_y - 0.0275).abs() < f32::EPSILON);
        assert!((setting.snow_amplitude - 5.0).abs() < f32::EPSILON);
        assert!((setting.snow_point_size - 1.0).abs() < f32::EPSILON);
        assert!((setting.snow_max_point_size - 64.0).abs() < f32::EPSILON);
        assert!((setting.snow_min_point_size - 0.0).abs() < f32::EPSILON);
        assert!((setting.snow_quad_size - 0.5).abs() < f32::EPSILON);
        assert!((setting.snow_box_dimensions - 200.0).abs() < f32::EPSILON);
        assert!((setting.snow_box_density - 1.0).abs() < f32::EPSILON);
        assert!((setting.snow_velocity - 4.0).abs() < f32::EPSILON);
        assert!(setting.use_point_sprites);
        assert!(!setting.snow_enabled);
    }

    #[test]
    fn weather_fields_accept_cpp_ini_token_style() {
        let mut setting = WeatherSetting::default();

        setting
            .apply_field("SnowTexture", &["=", "CustomSnow.tga"])
            .expect("texture");
        setting
            .apply_field("SnowAmplitude", &["=", "7.5f"])
            .expect("amplitude");
        setting
            .apply_field("SnowPointSprites", &["=", "false"])
            .expect("point sprites");
        setting
            .apply_field("SnowEnabled", &["=", "true"])
            .expect("enabled");

        assert_eq!(setting.snow_texture, "CustomSnow.tga");
        assert!((setting.snow_amplitude - 7.5).abs() < f32::EPSILON);
        assert!(!setting.use_point_sprites);
        assert!(setting.snow_enabled);
    }

    #[test]
    fn snow_update_advances_and_wraps_time() {
        let mut snow = SnowManager::new();
        snow.full_time_period = 2.0;
        snow.update(0.75);
        assert!((snow.time() - 0.75).abs() < 1e-6);
        snow.update(1.5);
        assert!((snow.time() - 0.25).abs() < 1e-6);
    }

    #[test]
    fn update_ini_settings_does_not_overwrite_script_visibility() {
        let _ = initialize_snow_manager();
        let settings = ensure_weather_setting();
        {
            let mut guard = settings.write().unwrap_or_else(|e| e.into_inner());
            guard.snow_enabled = true;
            guard.snow_box_dimensions = 50.0;
            guard.snow_box_density = 1.0;
        }
        let mut snow = SnowManager::new();
        snow.init();
        snow.set_visible(false);
        snow.update_ini_settings();
        assert!(
            !snow.is_visible(),
            "C++ updateIniSettings never derives m_isVisible from SnowEnabled"
        );
        assert_eq!(snow.starting_heights().len(), SNOW_NOISE_X * SNOW_NOISE_Y);
        let flakes = snow.flake_positions_y_up([0.0, 10.0, 0.0]);
        assert!(flakes.is_empty(), "hidden snow must not emit flake centers");
        snow.set_visible(true);
        let flakes = snow.flake_positions_y_up([0.0, 10.0, 0.0]);
        assert_eq!(flakes.len(), 50 * 50);
        assert!(
            flakes
                .iter()
                .any(|p| p[0].abs() > 0.01 || p[2].abs() > 0.01)
        );
    }

    #[test]
    fn reset_restores_script_visibility() {
        let mut snow = SnowManager::new();
        snow.set_visible(false);
        snow.reset();
        assert!(
            snow.is_visible(),
            "C++ SnowManager::reset restores m_isVisible=TRUE"
        );
    }

    #[test]
    fn flake_positions_clip_to_terrain_visible_box() {
        let _ = initialize_snow_manager();
        let settings = ensure_weather_setting();
        {
            let mut guard = settings.write().unwrap_or_else(|e| e.into_inner());
            guard.snow_enabled = true;
            guard.snow_box_dimensions = 50.0;
            guard.snow_box_density = 1.0;
            guard.snow_amplitude = 0.0;
            guard.snow_quad_size = 0.0;
        }
        let mut snow = SnowManager::new();
        snow.init();
        snow.set_visible(true);
        let unclipped = snow.flake_positions_y_up([0.0, 10.0, 0.0]);
        assert_eq!(unclipped.len(), 50 * 50);

        let clipped = snow.flake_positions_y_up_clipped(
            [0.0, 10.0, 0.0],
            Some(SnowVisibleBoxXy {
                center_x: 0.0,
                center_y: 0.0,
                extent_x: 5.0,
                extent_y: 5.0,
            }),
        );
        assert!(
            clipped.len() < unclipped.len(),
            "terrain AABB must shrink the emitter cube, got {} vs {}",
            clipped.len(),
            unclipped.len()
        );
        assert!(!clipped.is_empty(), "on-screen box still emits flakes");

        let culled = snow.flake_positions_y_up_clipped(
            [0.0, 10.0, 0.0],
            Some(SnowVisibleBoxXy {
                center_x: 10_000.0,
                center_y: 10_000.0,
                extent_x: 1.0,
                extent_y: 1.0,
            }),
        );
        assert!(
            culled.is_empty(),
            "emitter cube fully outside the visible box must emit nothing"
        );
    }

    #[test]
    fn camera_facing_quads_are_not_world_axis_cards() {
        let corners =
            camera_facing_quad_corners([0.0, 10.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.8, 0.6], 1.0);
        // C++ half-size is 0.5 * quadSize, not ±quadSize.
        let top_left = corners[0].0;
        assert!((top_left[0] + 0.5).abs() < 1e-5);
        assert!((top_left[1] - 10.4).abs() < 1e-5);
        assert!((top_left[2] - 0.3).abs() < 1e-5);
        // A world-axis XY card keeps constant Z. Camera-facing with tilted
        // up must change Z across the quad.
        let zs: Vec<f32> = corners.iter().map(|c| c.0[2]).collect();
        assert!(
            zs.iter().any(|z| (*z - zs[0]).abs() > 1e-4),
            "billboard must not be a constant-Z world card, zs={zs:?}"
        );
        assert_eq!(corners[0].1, [0.0, 0.0]);
        assert_eq!(corners[1].1, [0.0, 1.0]);
        assert_eq!(corners[2].1, [1.0, 1.0]);
        assert_eq!(corners[3].1, [1.0, 0.0]);
    }

    #[test]
    fn game_client_reset_source_resets_snow_manager() {
        let init = include_str!("core/game_client/impl_init.rs");
        let reset_fn = init
            .split("pub fn reset(&mut self)")
            .nth(1)
            .and_then(|rest| rest.split("pub fn init_savegame_counter_bridge").next())
            .unwrap_or(init);
        assert!(
            reset_fn.contains("get_snow_manager") && reset_fn.contains("guard.reset()"),
            "C++ GameClient::reset calls TheSnowManager->reset()"
        );
    }
}
