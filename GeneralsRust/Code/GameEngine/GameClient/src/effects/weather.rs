//! # Weather Effects System
//!
//! Environmental weather effects including rain, snow, dust storms,
//! and atmospheric effects for Command & Conquer Generals Zero Hour.

use super::{EffectsConfig, EffectsError, EffectsLOD};
use nalgebra::{Point3, Vector3};

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

    pub fn update(
        &mut self,
        delta_time: f32,
        config: &EffectsConfig,
        view_position: Point3<f32>,
    ) {
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

        let spawn_rate =
            base_spawn_rate * self.settings.intensity * lod_multiplier;
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

            for _ in 0..to_spawn {
                let offset = Vector3::new(
                    crate::helpers::get_game_logic_random_value_real(-spawn_region.0, spawn_region.0),
                    crate::helpers::get_game_logic_random_value_real(-spawn_region.1, spawn_region.1),
                    crate::helpers::get_game_logic_random_value_real(0.0, spawn_region.2),
                );

                let mut velocity = self.settings.wind_direction.normalize()
                    * self.settings.wind_strength;

                match self.settings.weather_type {
                    WeatherType::Rain => {
                        velocity += Vector3::new(0.0, 0.0, -base_speed);
                    }
                    WeatherType::Snow => {
                        velocity += Vector3::new(
                            crate::helpers::get_game_logic_random_value_real(-1.0, 1.0),
                            crate::helpers::get_game_logic_random_value_real(-1.0, 1.0),
                            -base_speed,
                        );
                    }
                    WeatherType::DustStorm | WeatherType::Sandstorm => {
                        velocity += Vector3::new(
                            crate::helpers::get_game_logic_random_value_real(-2.0, 2.0),
                            crate::helpers::get_game_logic_random_value_real(-2.0, 2.0),
                            crate::helpers::get_game_logic_random_value_real(-0.5, 0.5),
                        );
                    }
                    WeatherType::Fog => {
                        velocity += Vector3::new(
                            crate::helpers::get_game_logic_random_value_real(-0.3, 0.3),
                            crate::helpers::get_game_logic_random_value_real(-0.3, 0.3),
                            crate::helpers::get_game_logic_random_value_real(-0.1, 0.1),
                        );
                    }
                    WeatherType::None => {}
                }

                let particle = WeatherParticle {
                    position: view_position + offset,
                    velocity,
                    size: base_size
                        * crate::helpers::get_game_logic_random_value_real(0.6, 1.4),
                    alpha: 1.0,
                    age: 0.0,
                    lifetime: base_lifetime
                        * crate::helpers::get_game_logic_random_value_real(0.7, 1.3),
                };

                self.particles.push(particle);
            }
        }

        // Update particles
        let gravity = Vector3::new(0.0, 0.0, -9.8);
        for particle in &mut self.particles {
            particle.age += delta_time;
            if particle.age > particle.lifetime {
                particle.alpha = 0.0;
                continue;
            }

            // Apply wind + gravity (weather-specific)
            match self.settings.weather_type {
                WeatherType::Rain => {
                    particle.velocity += gravity * delta_time * 0.7;
                }
                WeatherType::Snow => {
                    particle.velocity += gravity * delta_time * 0.15;
                }
                WeatherType::DustStorm | WeatherType::Sandstorm => {
                    particle.velocity += self.settings.wind_direction * self.settings.wind_strength * 0.05;
                }
                WeatherType::Fog => {}
                WeatherType::None => {}
            }

            particle.position += particle.velocity * delta_time;

            // Fade out near end of life
            let remaining = (particle.lifetime - particle.age).max(0.0);
            particle.alpha = (remaining / particle.lifetime).clamp(0.0, 1.0);
        }

        // Cull dead particles and clamp to max
        self.particles.retain(|p| p.alpha > 0.0);
        if self.particles.len() > max_particles {
            let excess = self.particles.len() - max_particles;
            self.particles.drain(0..excess);
        }

        // Keep visibility range in sync with quality/intensity
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

// Placeholder implementations for specific weather types
pub struct SnowSystem;
pub struct RainSystem;
pub struct DustStormSystem;

impl SnowSystem {
    pub fn new() -> Self {
        Self
    }
    pub fn update(&mut self, _delta_time: f32) {}
}

impl RainSystem {
    pub fn new() -> Self {
        Self
    }
    pub fn update(&mut self, _delta_time: f32) {}
}

impl DustStormSystem {
    pub fn new() -> Self {
        Self
    }
    pub fn update(&mut self, _delta_time: f32) {}
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
}
