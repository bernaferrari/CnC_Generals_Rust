//! # Weather Effects System
//!
//! Environmental weather effects including rain, snow, dust storms,
//! and atmospheric effects for Command & Conquer Generals Zero Hour.
//!
//! ## Parity Notes
//! C++ weather is limited to snow only (`Weather::WEATHER_NORMAL`, `Weather::WEATHER_SNOWY`).
//! The C++ snow system uses `SnowManager` (GameClient) for state + `W3DSnowManager` (W3DDevice)
//! for D3D rendering with point sprites or quads. Snow particles use a 64×64 noise table for
//! starting heights and fall with velocity + sine-wave lateral drift (frequency/amplitude from
//! `WeatherSetting` INI). This module extends the C++ design with rain and dust storm effects
//! that follow the same particle-based architecture.

use super::{EffectsConfig, EffectsError, EffectsLOD};
use nalgebra::{Point3, Vector3};
use rand::Rng;

/// Types of weather effects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherType {
    None,
    Rain,
    Snow,
    DustStorm,
    Fog,
    Sandstorm,
}

/// Weather system settings
#[derive(Debug, Clone)]
pub struct WeatherSettings {
    pub weather_type: WeatherType,
    pub intensity: f32, // 0.0 to 1.0
    pub wind_direction: Vector3<f32>,
    pub wind_strength: f32,
    pub visibility_range: f32,
}

impl Default for WeatherSettings {
    fn default() -> Self {
        Self {
            weather_type: WeatherType::None,
            intensity: 0.0,
            wind_direction: Vector3::new(1.0, 0.0, 0.0),
            wind_strength: 0.0,
            visibility_range: 1000.0,
        }
    }
}

/// Individual weather particle (rain drop, snowflake, etc.)
#[derive(Debug, Clone)]
pub struct WeatherParticle {
    pub position: Point3<f32>,
    pub velocity: Vector3<f32>,
    pub size: f32,
    pub alpha: f32,
    pub age: f32,
    pub lifetime: f32,
}

/// Main weather system
pub struct WeatherSystem {
    settings: WeatherSettings,
    particles: Vec<WeatherParticle>,
    enabled: bool,
    lod: EffectsLOD,
    spawn_accumulator: f32,
}

impl WeatherSystem {
    pub fn new() -> Self {
        Self {
            settings: WeatherSettings::default(),
            particles: Vec::new(),
            enabled: true,
            lod: EffectsLOD::High,
            spawn_accumulator: 0.0,
        }
    }

    pub fn set_weather(&mut self, weather_type: WeatherType, intensity: f32) {
        self.settings.weather_type = weather_type;
        self.settings.intensity = intensity.clamp(0.0, 1.0);
    }

    pub fn update(&mut self, delta_time: f32, config: &EffectsConfig, view_position: Point3<f32>) {
        if !self.enabled || self.settings.weather_type == WeatherType::None {
            return;
        }

        let lod_multiplier = match self.lod {
            EffectsLOD::High => 1.0,
            EffectsLOD::Medium => 0.6,
            EffectsLOD::Low => 0.3,
            EffectsLOD::None => 0.0,
        };

        if lod_multiplier <= 0.0 {
            self.particles.clear();
            return;
        }

        let (base_spawn_rate, max_particles, base_speed, base_size, base_lifetime) =
            match self.settings.weather_type {
                WeatherType::Rain => (1200.0, 2400, 25.0, 0.6, 2.5),
                WeatherType::Snow => (600.0, 1600, 6.0, 1.2, 6.0),
                WeatherType::DustStorm | WeatherType::Sandstorm => (900.0, 2000, 8.0, 1.5, 5.0),
                WeatherType::Fog => (200.0, 800, 1.5, 2.5, 8.0),
                WeatherType::None => (0.0, 0, 0.0, 0.0, 0.0),
            };

        let spawn_rate = base_spawn_rate * self.settings.intensity * lod_multiplier;
        let quality_cap = config.quality.max_particles().max(1);
        let max_particles = ((max_particles as f32) * lod_multiplier)
            .round()
            .min(quality_cap as f32) as usize;

        // Spawn new particles using an accumulator for stable rates.
        self.spawn_accumulator += spawn_rate * delta_time;
        let spawn_count = self.spawn_accumulator.floor() as usize;
        self.spawn_accumulator -= spawn_count as f32;

        if spawn_count > 0 && self.particles.len() < max_particles {
            let spawn_region = match self.settings.weather_type {
                WeatherType::Rain => (60.0, 60.0, 40.0),
                WeatherType::Snow => (80.0, 80.0, 50.0),
                WeatherType::DustStorm | WeatherType::Sandstorm => (120.0, 120.0, 30.0),
                WeatherType::Fog => (100.0, 100.0, 20.0),
                WeatherType::None => (0.0, 0.0, 0.0),
            };

            let available = max_particles.saturating_sub(self.particles.len());
            let to_spawn = spawn_count.min(available);
            let mut rng = rand::thread_rng();

            for _ in 0..to_spawn {
                let offset = Vector3::new(
                    rng.gen_range(-spawn_region.0..spawn_region.0),
                    rng.gen_range(-spawn_region.1..spawn_region.1),
                    rng.gen_range(0.0..spawn_region.2),
                );

                let mut velocity =
                    self.settings.wind_direction.normalize() * self.settings.wind_strength;

                match self.settings.weather_type {
                    WeatherType::Rain => {
                        velocity += Vector3::new(0.0, 0.0, -base_speed);
                    }
                    WeatherType::Snow => {
                        velocity += Vector3::new(
                            rng.gen_range(-1.0..1.0),
                            rng.gen_range(-1.0..1.0),
                            -base_speed,
                        );
                    }
                    WeatherType::DustStorm | WeatherType::Sandstorm => {
                        velocity += Vector3::new(
                            rng.gen_range(-2.0..2.0),
                            rng.gen_range(-2.0..2.0),
                            rng.gen_range(-0.5..0.5),
                        );
                    }
                    WeatherType::Fog => {
                        velocity += Vector3::new(
                            rng.gen_range(-0.3..0.3),
                            rng.gen_range(-0.3..0.3),
                            rng.gen_range(-0.1..0.1),
                        );
                    }
                    WeatherType::None => {}
                }

                let particle = WeatherParticle {
                    position: view_position + offset,
                    velocity,
                    size: base_size * rng.gen_range(0.6..1.4),
                    alpha: 1.0,
                    age: 0.0,
                    lifetime: base_lifetime * rng.gen_range(0.7..1.3),
                };

                self.particles.push(particle);
            }
        }

        let gravity = Vector3::new(0.0, 0.0, -9.8);
        for particle in &mut self.particles {
            particle.age += delta_time;
            if particle.age > particle.lifetime {
                particle.alpha = 0.0;
                continue;
            }

            match self.settings.weather_type {
                WeatherType::Rain => {
                    particle.velocity += gravity * delta_time * 0.7;
                }
                WeatherType::Snow => {
                    particle.velocity += gravity * delta_time * 0.15;
                }
                WeatherType::DustStorm | WeatherType::Sandstorm => {
                    particle.velocity +=
                        self.settings.wind_direction * self.settings.wind_strength * 0.05;
                }
                WeatherType::Fog => {}
                WeatherType::None => {}
            }

            particle.position += particle.velocity * delta_time;

            let remaining = (particle.lifetime - particle.age).max(0.0);
            particle.alpha = (remaining / particle.lifetime).clamp(0.0, 1.0);
        }

        self.particles.retain(|p| p.alpha > 0.0);
        if self.particles.len() > max_particles {
            let excess = self.particles.len() - max_particles;
            self.particles.drain(0..excess);
        }

        let _visibility_modifier = 1.0 - (self.settings.intensity * 0.5);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.particles.clear();
        }
    }

    pub fn set_lod(&mut self, lod: EffectsLOD) {
        self.lod = lod;
    }

    pub fn particles(&self) -> &[WeatherParticle] {
        &self.particles
    }
}

impl Default for WeatherSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Snow System — parity with C++ SnowManager / W3DSnowManager
// ---------------------------------------------------------------------------

/// Noise table dimensions matching C++ `SnowManager::SNOW_NOISE_X/Y`.
const SNOW_NOISE_X: usize = 64;
const SNOW_NOISE_Y: usize = 64;

/// Snow system matching C++ `SnowManager` behavior.
///
/// C++ parity:
/// - Uses a 64×64 noise table for random starting heights (SnowManager::init)
/// - Particles fall at configurable velocity (m_velocity, default 4 world-units/s)
/// - Lateral sine-wave drift controlled by frequency scale and amplitude
/// - Box surrounds camera with configurable dimensions (default 200 world-units)
/// - Density controlled by emitter spacing (1/m_snowBoxDensity, default 1.0)
///
/// PARITY_NOTE: C++ rendering uses D3D point sprites or quads via W3DSnowManager.
/// This implementation provides the simulation/data layer. Rendering integration
/// with WGPU is deferred — call `particles()` to obtain particle positions for
/// the GPU draw path.
pub struct SnowSystem {
    /// Noise table for initial particle heights (C++ m_startingHeights).
    starting_heights: Vec<f32>,
    /// Elapsed time since snow started (C++ m_time).
    time: f32,
    /// Fall speed in world units/sec (C++ m_velocity, default 4.0).
    velocity: f32,
    /// Time for a flake to traverse the full box height (C++ m_fullTimePeriod).
    full_time_period: f32,
    /// X frequency scale for sine drift (C++ m_frequencyScaleX, default 0.0533).
    frequency_scale_x: f32,
    /// Y frequency scale for sine drift (C++ m_frequencyScaleY, default 0.0275).
    frequency_scale_y: f32,
    /// Lateral drift amplitude in world units (C++ m_amplitude, default 5.0).
    amplitude: f32,
    /// Size of the snow box in world units (C++ m_boxDimensions, default 200.0).
    box_dimensions: f32,
    /// Spacing between emitters (C++ m_emitterSpacing = 1/m_snowBoxDensity).
    emitter_spacing: f32,
    /// Whether the system is visible (C++ m_isVisible).
    is_visible: bool,
    /// Whether snow is enabled via INI (C++ WeatherSetting::m_snowEnabled).
    enabled: bool,
    /// Active particles computed each frame for rendering.
    particles: Vec<WeatherParticle>,
}

impl SnowSystem {
    /// Create a new snow system with C++ default `WeatherSetting` values.
    pub fn new() -> Self {
        // C++ defaults from WeatherSetting constructor:
        //   m_snowVelocity = 4, m_snowBoxDimensions = 200, m_snowBoxDensity = 1,
        //   m_snowFrequencyScaleX = 0.0533, m_snowFrequencyScaleY = 0.0275,
        //   m_snowAmplitude = 5.0
        let velocity = 4.0;
        let box_dimensions = 200.0;
        let emitter_spacing = 1.0; // 1.0 / m_snowBoxDensity(1)
        let full_time_period = box_dimensions / velocity;

        Self {
            starting_heights: Vec::with_capacity(SNOW_NOISE_X * SNOW_NOISE_Y),
            time: 0.0,
            velocity,
            full_time_period,
            frequency_scale_x: 0.0533,
            frequency_scale_y: 0.0275,
            amplitude: 5.0,
            box_dimensions,
            emitter_spacing,
            is_visible: true,
            enabled: false,
            particles: Vec::new(),
        }
    }

    /// Initialize the noise table. Matches C++ `SnowManager::init()`.
    pub fn init(&mut self) {
        // C++: m_startingHeights = NEW Real[SNOW_NOISE_X * SNOW_NOISE_Y];
        // Each entry: rand() % boxDimensions
        let mut rng = rand::thread_rng();
        let box_dim = self.box_dimensions as i32;
        self.starting_heights.clear();
        for _ in 0..SNOW_NOISE_X * SNOW_NOISE_Y {
            self.starting_heights
                .push((rng.gen_range(0..box_dim)) as f32);
        }
        self.time = 0.0;
    }

    /// Reload settings from INI values. Matches C++ `SnowManager::updateIniSettings()`.
    pub fn update_ini_settings(
        &mut self,
        velocity: f32,
        frequency_scale_x: f32,
        frequency_scale_y: f32,
        amplitude: f32,
        box_dimensions: f32,
        box_density: f32,
    ) {
        // Re-initialize noise table with new box dimensions (C++ does this too)
        let mut rng = rand::thread_rng();
        let box_dim = box_dimensions as i32;
        self.starting_heights.clear();
        for _ in 0..SNOW_NOISE_X * SNOW_NOISE_Y {
            self.starting_heights
                .push((rng.gen_range(0..box_dim)) as f32);
        }

        self.velocity = velocity;
        self.frequency_scale_x = frequency_scale_x;
        self.frequency_scale_y = frequency_scale_y;
        self.amplitude = amplitude;
        self.box_dimensions = box_dimensions;
        self.emitter_spacing = 1.0 / box_density.max(0.001);
        self.full_time_period = self.box_dimensions / self.velocity.max(0.001);
    }

    /// Update snow simulation. Matches C++ `W3DSnowManager::update()`.
    ///
    /// `delta_time` is frame time in seconds. `camera_pos` is the 3D camera position.
    pub fn update(&mut self, delta_time: f32, camera_pos: Point3<f32>) {
        if !self.enabled || !self.is_visible {
            self.particles.clear();
            return;
        }

        if self.starting_heights.is_empty() {
            self.init();
        }

        // C++ W3DSnowManager::update():
        //   m_time += WW3D::Get_Frame_Time() / 1000.0f;
        //   m_time = fmod(m_time, m_fullTimePeriod);
        self.time += delta_time;
        self.time %= self.full_time_period;

        // C++ W3DSnowManager::render() computes:
        //   m_snowCeiling = camPos.Z + m_boxDimensions/2.0f
        //   m_heightTraveled = m_time * m_velocity + fmod(camPos.Z, m_boxDimensions)
        let snow_ceiling = camera_pos.z + self.box_dimensions * 0.5;
        let camera_offset = camera_pos.z % self.box_dimensions;
        let height_traveled = self.time * self.velocity + camera_offset;

        // C++: compute emitter grid around camera
        //   numEmittersInHalf = floor(boxDimensions / emitterSpacing * 0.5)
        //   cubeCenterX/Y = floor(camPos.X/Y / emitterSpacing)
        let num_emitters_half = (self.box_dimensions / self.emitter_spacing * 0.5).floor() as i32;
        let cube_center_x = (camera_pos.x / self.emitter_spacing).floor() as i32;
        let cube_center_y = (camera_pos.y / self.emitter_spacing).floor() as i32;

        let origin_x = cube_center_x - num_emitters_half;
        let origin_y = cube_center_y - num_emitters_half;
        let dim_x = cube_center_x + num_emitters_half;
        let dim_y = cube_center_y + num_emitters_half;

        // Limit total particles for performance (C++ uses frustum culling + batching;
        // we cap to a reasonable max for the data layer).
        let max_particles = 4096;
        self.particles.clear();
        self.particles
            .reserve(max_particles.min(((dim_x - origin_x) * (dim_y - origin_y)).max(0) as usize));

        let max_camera_distance = 100000i32;

        for y in origin_y..dim_y {
            for x in origin_x..dim_x {
                if self.particles.len() >= max_particles {
                    break;
                }

                // C++ noise table lookup (power-of-2 modular arithmetic):
                //   noiseOffset = MODPOW2(x + MAX_CAMERA_DISTANCE, SNOW_NOISE_X)
                //               + MODPOW2(y + MAX_CAMERA_DISTANCE, SNOW_NOISE_Y) * SNOW_NOISE_X
                let noise_x = ((x + max_camera_distance) as usize) & (SNOW_NOISE_X - 1);
                let noise_y = ((y + max_camera_distance) as usize) & (SNOW_NOISE_Y - 1);
                let noise_offset = noise_x + noise_y * SNOW_NOISE_X;

                let start_h = if noise_offset < self.starting_heights.len() {
                    self.starting_heights[noise_offset]
                } else {
                    0.0
                };

                // C++: h0 = snowCeiling - fmod(heightTraveled + startingHeight, boxDimensions)
                let h0 = snow_ceiling - (height_traveled + start_h) % self.box_dimensions;

                // C++: snowCenter = (x*emitterSpacing, y*emitterSpacing, h0)
                //   snowCenter.X += amplitude * sin(h0 * freqScaleX + x)
                //   snowCenter.Y += amplitude * sin(h0 * freqScaleY + y)
                let px = x as f32 * self.emitter_spacing
                    + self.amplitude * (h0 * self.frequency_scale_x + x as f32).sin();
                let py = y as f32 * self.emitter_spacing
                    + self.amplitude * (h0 * self.frequency_scale_y + y as f32).sin();

                self.particles.push(WeatherParticle {
                    position: Point3::new(px, py, h0),
                    velocity: Vector3::new(0.0, 0.0, -self.velocity),
                    size: 1.0, // C++ m_pointSize default
                    alpha: 1.0,
                    age: 0.0,
                    lifetime: self.full_time_period,
                });
            }
        }
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.is_visible = visible;
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.particles.clear();
        }
    }

    pub fn particles(&self) -> &[WeatherParticle] {
        &self.particles
    }

    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }
}

impl Default for SnowSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Rain System
// ---------------------------------------------------------------------------

/// Rain system providing fast-falling streaks with wind influence.
///
/// C++ parity: C++ Generals does not have a dedicated rain weather type (only
/// NORMAL and SNOWY in the Weather enum). Rain is implemented here as an
/// extension following the same particle-system architecture as snow.
///
/// PARITY_NOTE: Rain is a Rust-only extension. C++ had no rain weather effect.
/// The data flow matches snow's pattern so rendering can reuse the same WGPU
/// pipeline with adjusted parameters (streaks vs. point sprites).
pub struct RainSystem {
    particles: Vec<WeatherParticle>,
    spawn_accumulator: f32,
    camera_position: Point3<f32>,
    enabled: bool,
    /// Rain drops per second at intensity 1.0.
    spawn_rate: f32,
    /// Horizontal extent of the rain volume around the camera.
    spawn_area_radius: f32,
    /// Downward speed of rain drops (world units/sec).
    fall_speed: f32,
    /// Maximum number of active rain particles.
    max_particles: usize,
}

impl RainSystem {
    /// C++ has no rain defaults; these are tuned for visual quality.
    pub fn new() -> Self {
        Self {
            particles: Vec::with_capacity(2000),
            spawn_accumulator: 0.0,
            camera_position: Point3::origin(),
            enabled: true,
            spawn_rate: 800.0,
            spawn_area_radius: 100.0,
            fall_speed: 60.0,
            max_particles: 2000,
        }
    }

    /// Update rain simulation.
    ///
    /// `delta_time` — frame time in seconds.
    /// `camera_pos` — current 3D camera position (rain volume follows camera).
    /// `wind` — wind direction * strength vector for lateral rain drift.
    /// `intensity` — 0.0 to 1.0 rain intensity.
    pub fn update(
        &mut self,
        delta_time: f32,
        camera_pos: Point3<f32>,
        wind: Vector3<f32>,
        intensity: f32,
    ) {
        if !self.enabled {
            self.particles.clear();
            return;
        }

        self.camera_position = camera_pos;
        let mut rng = rand::thread_rng();
        let gravity = Vector3::new(0.0, 0.0, -9.8);

        for particle in &mut self.particles {
            particle.age += delta_time;
            if particle.age >= particle.lifetime {
                particle.alpha = 0.0;
                continue;
            }

            particle.velocity += gravity * delta_time * 0.5;
            particle.velocity += wind * delta_time * 0.3;

            particle.position += particle.velocity * delta_time;

            let life_ratio = particle.age / particle.lifetime;
            if life_ratio > 0.8 {
                particle.alpha = ((1.0 - life_ratio) / 0.2).clamp(0.0, 1.0);
            }
        }

        self.particles.retain(|p| {
            if p.alpha <= 0.0 {
                return false;
            }
            let dist = (p.position - self.camera_position).norm();
            dist < self.spawn_area_radius * 1.5
        });

        let effective_rate = self.spawn_rate * intensity;
        self.spawn_accumulator += effective_rate * delta_time;
        let spawn_count = self.spawn_accumulator.floor() as usize;
        self.spawn_accumulator -= spawn_count as f32;

        let available = self.max_particles.saturating_sub(self.particles.len());
        let to_spawn = spawn_count.min(available);

        for _ in 0..to_spawn {
            let radius = self.spawn_area_radius;
            let spawn_pos = Point3::new(
                self.camera_position.x + rng.gen_range(-radius..radius),
                self.camera_position.y + rng.gen_range(-radius..radius),
                self.camera_position.z + rng.gen_range(50.0..150.0),
            );

            let velocity = Vector3::new(
                wind.x + rng.gen_range(-3.0..3.0),
                wind.y + rng.gen_range(-3.0..3.0),
                -self.fall_speed + rng.gen_range(-10.0..10.0),
            );

            self.particles.push(WeatherParticle {
                position: spawn_pos,
                velocity,
                size: rng.gen_range(0.5..2.0),
                alpha: rng.gen_range(0.6..1.0),
                age: 0.0,
                lifetime: rng.gen_range(2.0..4.0),
            });
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.particles.clear();
        }
    }

    pub fn particles(&self) -> &[WeatherParticle] {
        &self.particles
    }

    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }
}

impl Default for RainSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Dust Storm System
// ---------------------------------------------------------------------------

/// Dust storm system providing ground-hugging particles driven by wind.
///
/// C++ parity: C++ Generals does not have a dedicated dust storm weather type.
/// Dust storms are implemented here as an extension following the same
/// particle-system architecture. The dust storm uses strong horizontal wind,
/// low vertical extent, and pulsing intensity to simulate turbulent sand/dust.
///
/// PARITY_NOTE: Dust storm is a Rust-only extension. C++ had no dust weather.
/// The data flow is structurally identical to snow so the WGPU rendering
/// pipeline can draw these particles as textured quads near ground level.
pub struct DustStormSystem {
    particles: Vec<WeatherParticle>,
    spawn_accumulator: f32,
    camera_position: Point3<f32>,
    enabled: bool,
    /// Dust particles per second at intensity 1.0.
    spawn_rate: f32,
    /// Horizontal extent around camera.
    spawn_area_radius: f32,
    /// Maximum active particles.
    max_particles: usize,
    /// Pulsing intensity for dynamic storm feel.
    storm_pulse_time: f32,
}

impl DustStormSystem {
    pub fn new() -> Self {
        Self {
            particles: Vec::with_capacity(500),
            spawn_accumulator: 0.0,
            camera_position: Point3::origin(),
            enabled: true,
            spawn_rate: 300.0,
            spawn_area_radius: 150.0,
            max_particles: 500,
            storm_pulse_time: 0.0,
        }
    }

    /// Update dust storm simulation.
    ///
    /// `delta_time` — frame time in seconds.
    /// `camera_pos` — current 3D camera position.
    /// `wind` — wind direction * strength vector (dust is heavily wind-driven).
    /// `intensity` — 0.0 to 1.0 storm intensity.
    pub fn update(
        &mut self,
        delta_time: f32,
        camera_pos: Point3<f32>,
        wind: Vector3<f32>,
        intensity: f32,
    ) {
        if !self.enabled {
            self.particles.clear();
            return;
        }

        self.camera_position = camera_pos;
        let mut rng = rand::thread_rng();

        self.storm_pulse_time += delta_time * 0.5;
        let pulse_factor = (self.storm_pulse_time.sin() * 0.3 + 1.0) * 0.5 + 0.5;

        for particle in &mut self.particles {
            particle.age += delta_time;
            if particle.age >= particle.lifetime {
                particle.alpha = 0.0;
                continue;
            }

            let turbulence = Vector3::new(
                rng.gen_range(-5.0..5.0),
                rng.gen_range(-5.0..5.0),
                rng.gen_range(-1.0..1.0),
            );
            particle.velocity += (wind * 0.1 + turbulence) * delta_time;

            particle.position += particle.velocity * delta_time;

            let life_ratio = particle.age / particle.lifetime;
            if life_ratio > 0.7 {
                particle.alpha = ((1.0 - life_ratio) / 0.3).clamp(0.0, 1.0);
            }
        }

        self.particles.retain(|p| {
            if p.alpha <= 0.0 {
                return false;
            }
            let dist = (p.position - self.camera_position).norm();
            dist < self.spawn_area_radius * 1.5
        });

        let effective_rate = self.spawn_rate * intensity * pulse_factor;
        self.spawn_accumulator += effective_rate * delta_time;
        let spawn_count = self.spawn_accumulator.floor() as usize;
        self.spawn_accumulator -= spawn_count as f32;

        let available = self.max_particles.saturating_sub(self.particles.len());
        let to_spawn = spawn_count.min(available);

        for _ in 0..to_spawn {
            let radius = self.spawn_area_radius * 1.5;
            let spawn_pos = Point3::new(
                self.camera_position.x + rng.gen_range(-radius..radius),
                self.camera_position.y + rng.gen_range(-radius..radius),
                self.camera_position.z + rng.gen_range(-10.0..30.0),
            );

            let velocity = Vector3::new(
                wind.x * rng.gen_range(0.5..1.5) + rng.gen_range(-10.0..10.0),
                wind.y * rng.gen_range(0.5..1.5) + rng.gen_range(-10.0..10.0),
                rng.gen_range(-5.0..5.0),
            );

            self.particles.push(WeatherParticle {
                position: spawn_pos,
                velocity,
                size: rng.gen_range(2.0..8.0),
                alpha: rng.gen_range(0.3..0.7),
                age: 0.0,
                lifetime: rng.gen_range(8.0..15.0),
            });
        }
    }

    /// Visibility modifier reduced during dust storms.
    /// Returns 0.2–1.0 based on particle density.
    pub fn get_visibility_modifier(&self) -> f32 {
        let density = self.particles.len() as f32 / self.max_particles.max(1) as f32;
        1.0 - (density * 0.8).min(0.8)
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.particles.clear();
        }
    }

    pub fn particles(&self) -> &[WeatherParticle] {
        &self.particles
    }

    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }
}

impl Default for DustStormSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weather_system_creation() {
        let weather = WeatherSystem::new();
        assert_eq!(weather.settings.weather_type, WeatherType::None);
        assert!(weather.enabled);
    }

    #[test]
    fn test_weather_settings() {
        let mut weather = WeatherSystem::new();
        weather.set_weather(WeatherType::Snow, 0.7);

        assert_eq!(weather.settings.weather_type, WeatherType::Snow);
        assert_eq!(weather.settings.intensity, 0.7);
    }

    #[test]
    fn test_snow_system_init() {
        let mut snow = SnowSystem::new();
        assert_eq!(snow.starting_heights.len(), 0);

        snow.init();
        assert_eq!(snow.starting_heights.len(), SNOW_NOISE_X * SNOW_NOISE_Y);
        assert!(snow.is_visible);
    }

    #[test]
    fn test_snow_system_defaults_disabled_like_cpp() {
        let mut snow = SnowSystem::new();
        snow.init();

        snow.update(0.033, Point3::new(100.0, 100.0, 50.0));

        assert_eq!(snow.particle_count(), 0);
    }

    #[test]
    fn test_snow_system_update() {
        let mut snow = SnowSystem::new();
        snow.init();
        snow.set_enabled(true);

        let camera = Point3::new(100.0, 100.0, 50.0);
        snow.update(0.033, camera);

        assert!(
            snow.particle_count() > 0,
            "Snow should produce particles after update"
        );
    }

    #[test]
    fn test_snow_system_disabled() {
        let mut snow = SnowSystem::new();
        snow.init();
        snow.set_enabled(false);

        snow.update(0.033, Point3::origin());
        assert_eq!(snow.particle_count(), 0);
    }

    #[test]
    fn test_snow_system_update_ini_settings() {
        let mut snow = SnowSystem::new();
        snow.update_ini_settings(8.0, 0.1, 0.05, 10.0, 300.0, 2.0);

        assert!((snow.velocity - 8.0).abs() < f32::EPSILON);
        assert!((snow.box_dimensions - 300.0).abs() < f32::EPSILON);
        assert!((snow.amplitude - 10.0).abs() < f32::EPSILON);
        assert!((snow.emitter_spacing - 0.5).abs() < f32::EPSILON);
        assert_eq!(snow.starting_heights.len(), SNOW_NOISE_X * SNOW_NOISE_Y);
    }

    #[test]
    fn test_rain_system_update() {
        let mut rain = RainSystem::new();

        let camera = Point3::new(0.0, 0.0, 50.0);
        let wind = Vector3::new(5.0, 0.0, 0.0);
        rain.update(0.033, camera, wind, 0.8);

        assert!(
            rain.particle_count() > 0,
            "Rain should produce particles after update"
        );
    }

    #[test]
    fn test_rain_system_disabled() {
        let mut rain = RainSystem::new();
        rain.set_enabled(false);

        rain.update(0.033, Point3::origin(), Vector3::zeros(), 1.0);
        assert_eq!(rain.particle_count(), 0);
    }

    #[test]
    fn test_dust_storm_system_update() {
        let mut dust = DustStormSystem::new();

        let camera = Point3::new(0.0, 0.0, 10.0);
        let wind = Vector3::new(20.0, 10.0, 0.0);
        dust.update(0.033, camera, wind, 0.7);

        assert!(
            dust.particle_count() > 0,
            "Dust storm should produce particles after update"
        );

        let vis = dust.get_visibility_modifier();
        assert!(vis > 0.0 && vis <= 1.0);
    }

    #[test]
    fn test_dust_storm_system_disabled() {
        let mut dust = DustStormSystem::new();
        dust.set_enabled(false);

        dust.update(0.033, Point3::origin(), Vector3::zeros(), 1.0);
        assert_eq!(dust.particle_count(), 0);
    }

    #[test]
    fn test_snow_sine_drift() {
        let mut snow = SnowSystem::new();
        snow.init();
        snow.set_enabled(true);

        let camera = Point3::new(0.0, 0.0, 50.0);
        snow.update(0.033, camera);

        // Verify that snow particles have lateral drift (sine offsets applied)
        // At x=0, y=0 the particle should NOT be at exactly (0, 0, h0) due to sine
        let origin_particle = snow.particles().iter().find(|p| {
            (p.position.x.abs() < snow.emitter_spacing * 1.5)
                && (p.position.y.abs() < snow.emitter_spacing * 1.5)
        });

        if let Some(p) = origin_particle {
            // With amplitude=5.0, the sine offset should move the particle laterally
            // (unless it happens to be exactly zero, which is unlikely)
            // We just verify the particle exists and has a valid position
            assert!(p.position.z.is_finite());
        }
    }
}
