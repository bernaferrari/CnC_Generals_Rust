//! Snow manager and weather settings (ported from GameClient/Snow.cpp).

use game_engine::common::ini::ini::{register_block_parser, INIError, INILoadType, INIResult, INI};
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
            snow_texture: String::new(),
            snow_frequency_scale_x: 1.0,
            snow_frequency_scale_y: 1.0,
            snow_amplitude: 1.0,
            snow_point_size: 1.0,
            snow_max_point_size: 1.0,
            snow_min_point_size: 1.0,
            snow_quad_size: 1.0,
            snow_box_dimensions: 128.0,
            snow_box_density: 1.0,
            snow_velocity: 1.0,
            use_point_sprites: false,
            snow_enabled: true,
        }
    }
}

impl WeatherSetting {
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

pub fn get_weather_setting() -> Option<Arc<RwLock<WeatherSetting>>> {
    WEATHER_SETTING.get().cloned()
}

pub fn get_snow_manager() -> Option<Arc<Mutex<SnowManager>>> {
    SNOW_MANAGER.get().cloned()
}

pub fn ensure_weather_setting() -> Arc<RwLock<WeatherSetting>> {
    WEATHER_SETTING
        .get_or_init(|| Arc::new(RwLock::new(WeatherSetting::default())))
        .clone()
}

pub fn initialize_snow_manager() -> Arc<Mutex<SnowManager>> {
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
    let settings = ensure_weather_setting();
    {
        let mut guard = settings.write().map_err(|_| INIError::InvalidData)?;
        if ini.get_load_type() == INILoadType::Overwrite {
            *guard = WeatherSetting::default();
        }

        loop {
            ini.read_line()?;
            if ini.is_eof() {
                return Err(INIError::EndOfFile);
            }
            let tokens = ini.get_line_tokens();
            if tokens.is_empty() {
                continue;
            }
            let key = tokens[0];
            if key.eq_ignore_ascii_case("End") {
                break;
            }
            let args: Vec<&str> = tokens[1..].iter().copied().collect();
            guard.apply_field(key, &args)?;
        }
    }

    if let Some(manager) = get_snow_manager() {
        if let Ok(mut guard) = manager.lock() {
            guard.update_ini_settings();
        }
    }

    Ok(())
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
        let guard = settings.read().unwrap();

        let mut rng = rand::thread_rng();
        let box_dimensions = guard.snow_box_dimensions.max(0.0);
        for height in &mut self.starting_heights {
            *height = rng.gen_range(0.0..box_dimensions.max(1.0));
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
        self.is_visible = guard.snow_enabled;
    }

    pub fn set_visible(&mut self, show_weather: bool) {
        self.is_visible = show_weather;
    }

    pub fn reset(&mut self) {
        self.is_visible = true;
    }

    pub fn is_visible(&self) -> bool {
        self.is_visible
    }
}
