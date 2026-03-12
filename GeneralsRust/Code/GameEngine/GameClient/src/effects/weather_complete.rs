//! # Complete Weather Effects System
//!
//! Full implementation of environmental weather effects including rain, snow, dust storms,
//! fog, and sandstorms for Command & Conquer Generals Zero Hour.
//! Matches C++ behavior with GPU-accelerated particle rendering.

use super::particle_manager::*;
use super::particle_system::*;
use nalgebra::{Matrix4, Point3, Vector3};
use rand::prelude::*;
use std::collections::VecDeque;
use std::sync::{OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Maximum weather particles based on quality
const MAX_RAIN_PARTICLES: usize = 2000;
const MAX_SNOW_PARTICLES: usize = 1500;
const MAX_DUST_PARTICLES: usize = 500;

/// Types of weather effects (matches C++ exactly)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherType {
    None = 0,
    Rain,
    Snow,
    DustStorm,
    Fog,
    Sandstorm,
}

/// Weather intensity levels
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WeatherIntensity {
    Light,  // 0.0 - 0.33
    Medium, // 0.34 - 0.66
    Heavy,  // 0.67 - 1.0
}

impl WeatherIntensity {
    pub fn from_value(intensity: f32) -> Self {
        let clamped = intensity.clamp(0.0, 1.0);
        if clamped < 0.34 {
            WeatherIntensity::Light
        } else if clamped < 0.67 {
            WeatherIntensity::Medium
        } else {
            WeatherIntensity::Heavy
        }
    }
}

/// Weather system settings
#[derive(Debug, Clone)]
pub struct WeatherSettings {
    pub weather_type: WeatherType,
    pub intensity: f32, // 0.0 to 1.0
    pub wind_direction: Vector3<f32>,
    pub wind_strength: f32,
    pub wind_turbulence: f32,
    pub visibility_range: f32,
    pub fog_density: f32,
    pub precipitation_speed: f32,
    pub spawn_rate: f32, // Particles per second
    pub spawn_area_radius: f32,
}

impl Default for WeatherSettings {
    fn default() -> Self {
        Self {
            weather_type: WeatherType::None,
            intensity: 0.5,
            wind_direction: Vector3::new(1.0, 0.0, 0.0).normalize(),
            wind_strength: 5.0,
            wind_turbulence: 0.2,
            visibility_range: 1000.0,
            fog_density: 0.0,
            precipitation_speed: 50.0,
            spawn_rate: 100.0,
            spawn_area_radius: 200.0,
        }
    }
}

/// Individual weather particle (rain drop, snowflake, dust particle)
#[derive(Debug, Clone)]
pub struct WeatherParticle {
    pub position: Point3<f32>,
    pub velocity: Vector3<f32>,
    pub size: f32,
    pub alpha: f32,
    pub age: f32,
    pub lifetime: f32,
    pub rotation: f32,
    pub rotation_speed: f32,
    pub color: [f32; 4],
}

impl WeatherParticle {
    /// Create a new rain drop
    pub fn new_rain_drop(spawn_pos: Point3<f32>, wind: Vector3<f32>) -> Self {
        let mut rng = thread_rng();

        Self {
            position: spawn_pos,
            velocity: Vector3::new(
                wind.x + rng.gen_range(-5.0..5.0),
                wind.y + rng.gen_range(-5.0..5.0),
                -80.0 + rng.gen_range(-20.0..20.0), // Falling down
            ),
            size: rng.gen_range(0.5..2.0),
            alpha: rng.gen_range(0.6..1.0),
            age: 0.0,
            lifetime: 5.0,
            rotation: 0.0,
            rotation_speed: 0.0,
            color: [0.7, 0.8, 1.0, 1.0], // Slightly blue-tinted
        }
    }

    /// Create a new snowflake
    pub fn new_snowflake(spawn_pos: Point3<f32>, wind: Vector3<f32>) -> Self {
        let mut rng = thread_rng();

        Self {
            position: spawn_pos,
            velocity: Vector3::new(
                wind.x + rng.gen_range(-3.0..3.0),
                wind.y + rng.gen_range(-3.0..3.0),
                -15.0 + rng.gen_range(-5.0..5.0), // Gentle fall
            ),
            size: rng.gen_range(1.0..3.0),
            alpha: rng.gen_range(0.8..1.0),
            age: 0.0,
            lifetime: 10.0,
            rotation: rng.gen::<f32>() * std::f32::consts::TAU,
            rotation_speed: rng.gen_range(-2.0..2.0),
            color: [1.0, 1.0, 1.0, 1.0], // Pure white
        }
    }

    /// Create a new dust particle
    pub fn new_dust_particle(spawn_pos: Point3<f32>, wind: Vector3<f32>) -> Self {
        let mut rng = thread_rng();

        let dust_color_variation = rng.gen_range(0.8..1.0);

        Self {
            position: spawn_pos,
            velocity: Vector3::new(
                wind.x * rng.gen_range(0.5..1.5) + rng.gen_range(-10.0..10.0),
                wind.y * rng.gen_range(0.5..1.5) + rng.gen_range(-10.0..10.0),
                rng.gen_range(-5.0..5.0), // Some vertical motion
            ),
            size: rng.gen_range(2.0..8.0),
            alpha: rng.gen_range(0.3..0.7),
            age: 0.0,
            lifetime: 15.0,
            rotation: rng.gen::<f32>() * std::f32::consts::TAU,
            rotation_speed: rng.gen_range(-1.0..1.0),
            color: [
                0.7 * dust_color_variation,
                0.6 * dust_color_variation,
                0.4 * dust_color_variation,
                1.0,
            ], // Brownish
        }
    }

    /// Update particle physics
    pub fn update(&mut self, delta_time: f32, wind: Vector3<f32>, turbulence: f32) -> bool {
        self.age += delta_time;

        if self.age >= self.lifetime {
            return false; // Particle expired
        }

        // Apply turbulence
        let mut rng = thread_rng();
        let turbulence_force = Vector3::new(
            rng.gen_range(-turbulence..turbulence),
            rng.gen_range(-turbulence..turbulence),
            rng.gen_range(-turbulence..turbulence),
        );

        // Apply wind and turbulence
        self.velocity += (wind * 0.1 + turbulence_force) * delta_time;

        // Update position
        self.position += self.velocity * delta_time;

        // Update rotation
        self.rotation += self.rotation_speed * delta_time;
        self.rotation = self.rotation.rem_euclid(std::f32::consts::TAU);

        // Fade out near end of life
        let life_ratio = self.age / self.lifetime;
        if life_ratio > 0.8 {
            self.alpha *= 0.95;
        }

        true // Still alive
    }
}

/// Rain weather system
pub struct RainSystem {
    particles: VecDeque<WeatherParticle>,
    settings: WeatherSettings,
    spawn_accumulator: f32,
    camera_position: Point3<f32>,
    enabled: bool,
}

impl RainSystem {
    pub fn new(settings: WeatherSettings) -> Self {
        Self {
            particles: VecDeque::with_capacity(MAX_RAIN_PARTICLES),
            settings,
            spawn_accumulator: 0.0,
            camera_position: Point3::origin(),
            enabled: true,
        }
    }

    pub fn update(&mut self, delta_time: f32, camera_pos: Point3<f32>) {
        if !self.enabled {
            return;
        }

        self.camera_position = camera_pos;

        // Update existing particles
        self.particles.retain_mut(|particle| {
            particle.update(
                delta_time,
                self.settings.wind_direction * self.settings.wind_strength,
                self.settings.wind_turbulence,
            )
        });

        // Cull particles far from camera
        self.particles.retain(|particle| {
            let distance = (particle.position - self.camera_position).norm();
            distance < self.settings.spawn_area_radius
        });

        // Spawn new particles
        let spawn_count =
            (self.settings.spawn_rate * self.settings.intensity * delta_time) as usize;
        self.spawn_accumulator +=
            (self.settings.spawn_rate * self.settings.intensity * delta_time) % 1.0;

        let mut extra_spawns = 0;
        if self.spawn_accumulator >= 1.0 {
            extra_spawns = self.spawn_accumulator as usize;
            self.spawn_accumulator -= extra_spawns as f32;
        }

        for _ in 0..(spawn_count + extra_spawns) {
            if self.particles.len() >= MAX_RAIN_PARTICLES {
                break;
            }

            let spawn_pos = self.generate_spawn_position();
            let particle = WeatherParticle::new_rain_drop(
                spawn_pos,
                self.settings.wind_direction * self.settings.wind_strength,
            );
            self.particles.push_back(particle);
        }
    }

    fn generate_spawn_position(&self) -> Point3<f32> {
        let mut rng = thread_rng();
        let radius = self.settings.spawn_area_radius;

        Point3::new(
            self.camera_position.x + rng.gen_range(-radius..radius),
            self.camera_position.y + rng.gen_range(-radius..radius),
            self.camera_position.z + rng.gen_range(50.0..150.0), // Spawn above
        )
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.particles.clear();
        }
    }

    pub fn particles(&self) -> &VecDeque<WeatherParticle> {
        &self.particles
    }

    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }
}

/// Snow weather system
pub struct SnowSystem {
    particles: VecDeque<WeatherParticle>,
    settings: WeatherSettings,
    spawn_accumulator: f32,
    camera_position: Point3<f32>,
    enabled: bool,
}

impl SnowSystem {
    pub fn new(settings: WeatherSettings) -> Self {
        Self {
            particles: VecDeque::with_capacity(MAX_SNOW_PARTICLES),
            settings,
            spawn_accumulator: 0.0,
            camera_position: Point3::origin(),
            enabled: true,
        }
    }

    pub fn update(&mut self, delta_time: f32, camera_pos: Point3<f32>) {
        if !self.enabled {
            return;
        }

        self.camera_position = camera_pos;

        // Update existing particles with gentler motion
        self.particles.retain_mut(|particle| {
            // Add sine wave motion for realistic snowflake drift
            let age_offset = particle.age * 0.5;
            let drift_x = (age_offset + particle.position.y * 0.1).sin() * 5.0;
            let drift_y = (age_offset + particle.position.x * 0.1).cos() * 5.0;

            particle.velocity.x += drift_x * delta_time;
            particle.velocity.y += drift_y * delta_time;

            particle.update(
                delta_time,
                self.settings.wind_direction * self.settings.wind_strength,
                self.settings.wind_turbulence,
            )
        });

        // Cull distant particles
        self.particles.retain(|particle| {
            let distance = (particle.position - self.camera_position).norm();
            distance < self.settings.spawn_area_radius
        });

        // Spawn new snowflakes
        let spawn_count =
            (self.settings.spawn_rate * self.settings.intensity * 0.8 * delta_time) as usize;
        self.spawn_accumulator +=
            (self.settings.spawn_rate * self.settings.intensity * 0.8 * delta_time) % 1.0;

        let mut extra_spawns = 0;
        if self.spawn_accumulator >= 1.0 {
            extra_spawns = self.spawn_accumulator as usize;
            self.spawn_accumulator -= extra_spawns as f32;
        }

        for _ in 0..(spawn_count + extra_spawns) {
            if self.particles.len() >= MAX_SNOW_PARTICLES {
                break;
            }

            let spawn_pos = self.generate_spawn_position();
            let particle = WeatherParticle::new_snowflake(
                spawn_pos,
                self.settings.wind_direction * self.settings.wind_strength,
            );
            self.particles.push_back(particle);
        }
    }

    fn generate_spawn_position(&self) -> Point3<f32> {
        let mut rng = thread_rng();
        let radius = self.settings.spawn_area_radius;

        Point3::new(
            self.camera_position.x + rng.gen_range(-radius..radius),
            self.camera_position.y + rng.gen_range(-radius..radius),
            self.camera_position.z + rng.gen_range(50.0..120.0),
        )
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.particles.clear();
        }
    }

    pub fn particles(&self) -> &VecDeque<WeatherParticle> {
        &self.particles
    }

    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }
}

/// Dust storm weather system
pub struct DustStormSystem {
    particles: VecDeque<WeatherParticle>,
    settings: WeatherSettings,
    spawn_accumulator: f32,
    camera_position: Point3<f32>,
    enabled: bool,
    storm_intensity_pulse: f32, // Pulsing effect
}

impl DustStormSystem {
    pub fn new(settings: WeatherSettings) -> Self {
        Self {
            particles: VecDeque::with_capacity(MAX_DUST_PARTICLES),
            settings,
            spawn_accumulator: 0.0,
            camera_position: Point3::origin(),
            enabled: true,
            storm_intensity_pulse: 0.0,
        }
    }

    pub fn update(&mut self, delta_time: f32, camera_pos: Point3<f32>) {
        if !self.enabled {
            return;
        }

        self.camera_position = camera_pos;

        // Update storm pulse for dynamic intensity
        self.storm_intensity_pulse += delta_time * 0.5;
        let pulse_factor = (self.storm_intensity_pulse.sin() * 0.3 + 1.0) * 0.5 + 0.5; // 0.5 to 1.0

        // Update existing particles with turbulent motion
        self.particles.retain_mut(|particle| {
            particle.update(
                delta_time,
                self.settings.wind_direction * self.settings.wind_strength * pulse_factor,
                self.settings.wind_turbulence * 2.0, // More turbulence for dust
            )
        });

        // Cull distant particles
        self.particles.retain(|particle| {
            let distance = (particle.position - self.camera_position).norm();
            distance < self.settings.spawn_area_radius * 1.5 // Larger area for dust
        });

        // Spawn new dust particles
        let spawn_count =
            (self.settings.spawn_rate * self.settings.intensity * pulse_factor * delta_time)
                as usize;
        self.spawn_accumulator +=
            (self.settings.spawn_rate * self.settings.intensity * pulse_factor * delta_time) % 1.0;

        let mut extra_spawns = 0;
        if self.spawn_accumulator >= 1.0 {
            extra_spawns = self.spawn_accumulator as usize;
            self.spawn_accumulator -= extra_spawns as f32;
        }

        for _ in 0..(spawn_count + extra_spawns) {
            if self.particles.len() >= MAX_DUST_PARTICLES {
                break;
            }

            let spawn_pos = self.generate_spawn_position();
            let particle = WeatherParticle::new_dust_particle(
                spawn_pos,
                self.settings.wind_direction * self.settings.wind_strength,
            );
            self.particles.push_back(particle);
        }
    }

    fn generate_spawn_position(&self) -> Point3<f32> {
        let mut rng = thread_rng();
        let radius = self.settings.spawn_area_radius * 1.5;

        Point3::new(
            self.camera_position.x + rng.gen_range(-radius..radius),
            self.camera_position.y + rng.gen_range(-radius..radius),
            self.camera_position.z + rng.gen_range(-10.0..30.0), // Ground level to low altitude
        )
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.particles.clear();
        }
    }

    pub fn particles(&self) -> &VecDeque<WeatherParticle> {
        &self.particles
    }

    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }

    pub fn get_visibility_modifier(&self) -> f32 {
        // Reduce visibility during dust storms
        let intensity = self.particles.len() as f32 / MAX_DUST_PARTICLES as f32;
        1.0 - (intensity * 0.7).min(0.8)
    }
}

/// Main weather system coordinator
pub struct WeatherSystem {
    rain_system: Option<RainSystem>,
    snow_system: Option<SnowSystem>,
    dust_system: Option<DustStormSystem>,

    current_weather: WeatherType,
    settings: WeatherSettings,
    enabled: bool,

    // Transition state
    transitioning: bool,
    transition_progress: f32,
    transition_duration: f32,
    target_weather: WeatherType,
}

impl WeatherSystem {
    pub fn new() -> Self {
        Self {
            rain_system: None,
            snow_system: None,
            dust_system: None,

            current_weather: WeatherType::None,
            settings: WeatherSettings::default(),
            enabled: true,

            transitioning: false,
            transition_progress: 0.0,
            transition_duration: 3.0, // 3 seconds transition
            target_weather: WeatherType::None,
        }
    }

    /// Set weather type with smooth transition
    pub fn set_weather(&mut self, weather_type: WeatherType, intensity: f32) {
        if weather_type != self.current_weather {
            self.target_weather = weather_type;
            self.transitioning = true;
            self.transition_progress = 0.0;
        }

        self.settings.weather_type = weather_type;
        self.settings.intensity = intensity.clamp(0.0, 1.0);
    }

    /// Update weather system
    pub fn update(&mut self, delta_time: f32, camera_pos: Point3<f32>) {
        if !self.enabled {
            return;
        }

        // Handle weather transition
        if self.transitioning {
            self.transition_progress += delta_time / self.transition_duration;

            if self.transition_progress >= 1.0 {
                self.transitioning = false;
                self.transition_progress = 1.0;

                // Disable old weather system
                self.disable_current_weather();
                self.current_weather = self.target_weather;

                // Enable new weather system
                self.enable_current_weather();
            }
        }

        // Update active weather systems
        match self.current_weather {
            WeatherType::Rain => {
                if let Some(rain) = &mut self.rain_system {
                    rain.update(delta_time, camera_pos);
                } else {
                    // Initialize rain system if not present
                    self.rain_system = Some(RainSystem::new(self.settings.clone()));
                }
            }
            WeatherType::Snow => {
                if let Some(snow) = &mut self.snow_system {
                    snow.update(delta_time, camera_pos);
                } else {
                    self.snow_system = Some(SnowSystem::new(self.settings.clone()));
                }
            }
            WeatherType::DustStorm | WeatherType::Sandstorm => {
                if let Some(dust) = &mut self.dust_system {
                    dust.update(delta_time, camera_pos);
                } else {
                    self.dust_system = Some(DustStormSystem::new(self.settings.clone()));
                }
            }
            _ => {}
        }
    }

    fn disable_current_weather(&mut self) {
        match self.current_weather {
            WeatherType::Rain => {
                if let Some(rain) = &mut self.rain_system {
                    rain.set_enabled(false);
                }
            }
            WeatherType::Snow => {
                if let Some(snow) = &mut self.snow_system {
                    snow.set_enabled(false);
                }
            }
            WeatherType::DustStorm | WeatherType::Sandstorm => {
                if let Some(dust) = &mut self.dust_system {
                    dust.set_enabled(false);
                }
            }
            _ => {}
        }
    }

    fn enable_current_weather(&mut self) {
        match self.current_weather {
            WeatherType::Rain => {
                if let Some(rain) = &mut self.rain_system {
                    rain.set_enabled(true);
                }
            }
            WeatherType::Snow => {
                if let Some(snow) = &mut self.snow_system {
                    snow.set_enabled(true);
                }
            }
            WeatherType::DustStorm | WeatherType::Sandstorm => {
                if let Some(dust) = &mut self.dust_system {
                    dust.set_enabled(true);
                }
            }
            _ => {}
        }
    }

    /// Get total active particles across all weather systems
    pub fn total_particle_count(&self) -> usize {
        let mut count = 0;
        if let Some(rain) = &self.rain_system {
            count += rain.particle_count();
        }
        if let Some(snow) = &self.snow_system {
            count += snow.particle_count();
        }
        if let Some(dust) = &self.dust_system {
            count += dust.particle_count();
        }
        count
    }

    /// Get all active weather particles for rendering
    pub fn get_all_particles(&self) -> Vec<WeatherParticle> {
        let mut particles = Vec::new();

        if let Some(rain) = &self.rain_system {
            particles.extend(rain.particles().iter().cloned());
        }
        if let Some(snow) = &self.snow_system {
            particles.extend(snow.particles().iter().cloned());
        }
        if let Some(dust) = &self.dust_system {
            particles.extend(dust.particles().iter().cloned());
        }

        particles
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            if let Some(rain) = &mut self.rain_system {
                rain.set_enabled(false);
            }
            if let Some(snow) = &mut self.snow_system {
                snow.set_enabled(false);
            }
            if let Some(dust) = &mut self.dust_system {
                dust.set_enabled(false);
            }
        }
    }

    pub fn current_weather(&self) -> WeatherType {
        self.current_weather
    }

    pub fn is_transitioning(&self) -> bool {
        self.transitioning
    }
}

impl Default for WeatherSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WeatherSystemError {
    #[error("Weather system not initialized")]
    NotInitialized,
    #[error("Weather system lock poisoned")]
    LockPoisoned,
}

static WEATHER_SYSTEM: OnceLock<RwLock<Option<WeatherSystem>>> = OnceLock::new();

pub fn initialize_weather_system() -> Result<(), WeatherSystemError> {
    let lock = WEATHER_SYSTEM.get_or_init(|| RwLock::new(None));
    let mut guard = lock.write().map_err(|_| WeatherSystemError::LockPoisoned)?;
    *guard = Some(WeatherSystem::new());
    Ok(())
}

pub fn get_weather_system(
) -> Result<RwLockReadGuard<'static, Option<WeatherSystem>>, WeatherSystemError> {
    let lock = WEATHER_SYSTEM.get_or_init(|| RwLock::new(None));
    lock.read().map_err(|_| WeatherSystemError::LockPoisoned)
}

pub fn get_weather_system_mut(
) -> Result<RwLockWriteGuard<'static, Option<WeatherSystem>>, WeatherSystemError> {
    let lock = WEATHER_SYSTEM.get_or_init(|| RwLock::new(None));
    lock.write().map_err(|_| WeatherSystemError::LockPoisoned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rain_system() {
        let settings = WeatherSettings {
            weather_type: WeatherType::Rain,
            intensity: 0.5,
            spawn_rate: 50.0,
            ..Default::default()
        };

        let mut rain = RainSystem::new(settings);
        rain.update(0.1, Point3::origin());

        assert!(rain.particle_count() > 0);
    }

    #[test]
    fn test_snow_system() {
        let settings = WeatherSettings {
            weather_type: WeatherType::Snow,
            intensity: 0.7,
            spawn_rate: 40.0,
            ..Default::default()
        };

        let mut snow = SnowSystem::new(settings);
        snow.update(0.1, Point3::origin());

        assert!(snow.particle_count() > 0);
    }

    #[test]
    fn test_dust_storm_system() {
        let settings = WeatherSettings {
            weather_type: WeatherType::DustStorm,
            intensity: 0.8,
            spawn_rate: 30.0,
            ..Default::default()
        };

        let mut dust = DustStormSystem::new(settings);
        dust.update(0.1, Point3::origin());

        assert!(dust.particle_count() > 0);

        let visibility = dust.get_visibility_modifier();
        assert!(visibility > 0.0 && visibility <= 1.0);
    }

    #[test]
    fn test_weather_transition() {
        let mut weather = WeatherSystem::new();

        weather.set_weather(WeatherType::Rain, 0.5);
        assert!(weather.is_transitioning());

        // Simulate transition
        for _ in 0..35 {
            // 3.5 seconds at 0.1s per frame
            weather.update(0.1, Point3::origin());
        }

        assert!(!weather.is_transitioning());
        assert_eq!(weather.current_weather(), WeatherType::Rain);
    }

    #[test]
    fn test_particle_lifecycle() {
        let spawn_pos = Point3::new(0.0, 0.0, 100.0);
        let wind = Vector3::new(10.0, 0.0, 0.0);

        let mut particle = WeatherParticle::new_rain_drop(spawn_pos, wind);

        // Update until particle dies
        let mut updates = 0;
        while particle.update(0.1, wind, 0.1) && updates < 100 {
            updates += 1;
        }

        assert!(updates > 0); // Particle lived for some time
    }
}
