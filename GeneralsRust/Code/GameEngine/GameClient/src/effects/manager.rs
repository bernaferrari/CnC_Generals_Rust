//! # Effects Manager
//!
//! Central coordinator for all visual effects in Command & Conquer Generals
//! Zero Hour, managing particle systems, weather, decals, and performance.

use std::collections::HashMap;
use std::time::Instant;

use super::{
    calculate_effects_lod, particles::ParticleType, DecalId, DecalManager, DecalSettings,
    EffectsConfig, EffectsError, EffectsLOD, EffectsStats, ParticleRenderer, ParticleSystem,
    ParticleSystemDesc, ParticleSystemId, WeatherSystem, WeatherType,
};
use crate::system::SubsystemInterface;

/// Central effects manager coordinating all visual effects
pub struct EffectsManager {
    /// Configuration settings
    config: EffectsConfig,

    /// Particle systems management
    particle_systems: HashMap<ParticleSystemId, ParticleSystem>,
    particle_renderer: ParticleRenderer,
    next_particle_id: ParticleSystemId,

    /// Weather system
    weather_system: WeatherSystem,

    /// Decal management
    decal_manager: DecalManager,

    /// Performance statistics
    stats: EffectsStats,

    /// Whether effects are enabled
    enabled: bool,

    /// Frame timing
    last_update: Option<Instant>,

    /// View position for LOD calculations
    view_position: nalgebra::Point3<f32>,
}

impl EffectsManager {
    /// Create a new effects manager
    pub fn new() -> Self {
        Self {
            config: EffectsConfig::default(),
            particle_systems: HashMap::new(),
            particle_renderer: ParticleRenderer::new(),
            next_particle_id: 1,
            weather_system: WeatherSystem::new(),
            decal_manager: DecalManager::new(),
            stats: EffectsStats::default(),
            enabled: true,
            last_update: None,
            view_position: nalgebra::Point3::new(0.0, 0.0, 0.0),
        }
    }

    /// Create effects manager with custom configuration
    pub fn with_config(config: EffectsConfig) -> Self {
        let mut manager = Self::new();
        manager.set_config(config);
        manager
    }

    /// Set configuration
    pub fn set_config(&mut self, config: EffectsConfig) {
        self.config = config.clone();

        // Apply configuration to subsystems
        self.weather_system.set_enabled(config.weather_enabled);
        self.decal_manager.set_enabled(config.decals_enabled);
        self.particle_renderer.set_enabled(config.particles_enabled);
        self.decal_manager
            .set_max_decals((config.max_particle_systems as f32 * 10.0) as usize);
    }

    /// Get current configuration
    pub fn config(&self) -> &EffectsConfig {
        &self.config
    }

    /// Enable or disable all effects
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;

        if !enabled {
            self.clear_all_effects();
        }

        self.weather_system
            .set_enabled(enabled && self.config.weather_enabled);
        self.decal_manager
            .set_enabled(enabled && self.config.decals_enabled);
        self.particle_renderer
            .set_enabled(enabled && self.config.particles_enabled);
    }

    /// Check if effects are enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set view position for LOD calculations
    pub fn set_view_position(&mut self, position: nalgebra::Point3<f32>) {
        self.view_position = position;
    }

    /// Spawn a particle system
    pub fn spawn_particle_system(&mut self, desc: ParticleSystemDesc) -> Option<ParticleSystemId> {
        if !self.enabled || !self.config.particles_enabled {
            return None;
        }

        // Check if we've reached the maximum number of systems
        if self.particle_systems.len() >= self.config.max_particle_systems {
            // Remove oldest finished system
            if let Some(oldest_id) = self.find_oldest_finished_system() {
                self.particle_systems.remove(&oldest_id);
            } else {
                // If no finished systems, reject new one
                return None;
            }
        }

        let id = self.next_particle_id;
        self.next_particle_id += 1;

        let mut system = desc.build(id);

        // Calculate LOD based on distance from view
        let distance = (system.position - self.view_position).norm();
        let lod = calculate_effects_lod(distance, &self.config);
        system.set_lod(lod);

        self.particle_systems.insert(id, system);
        Some(id)
    }

    /// Remove a particle system
    pub fn remove_particle_system(&mut self, id: ParticleSystemId) -> Option<ParticleSystem> {
        self.particle_systems.remove(&id)
    }

    /// Get particle system by ID
    pub fn get_particle_system(&self, id: ParticleSystemId) -> Option<&ParticleSystem> {
        self.particle_systems.get(&id)
    }

    /// Get mutable particle system by ID
    pub fn get_particle_system_mut(&mut self, id: ParticleSystemId) -> Option<&mut ParticleSystem> {
        self.particle_systems.get_mut(&id)
    }

    /// Set weather
    pub fn set_weather(&mut self, weather_type: WeatherType, intensity: f32) {
        self.weather_system.set_weather(weather_type, intensity);
    }

    /// Create a decal
    pub fn create_decal(&mut self, settings: DecalSettings) -> Option<DecalId> {
        if !self.enabled || !self.config.decals_enabled {
            return None;
        }

        Some(self.decal_manager.create_decal(settings))
    }

    /// Create a radius decal
    pub fn create_radius_decal(
        &mut self,
        center: nalgebra::Point3<f32>,
        radius: f32,
        decal_type: super::decals::DecalType,
    ) {
        if !self.enabled || !self.config.decals_enabled {
            return;
        }

        self.decal_manager
            .create_radius_decal(center, radius, decal_type);
    }

    /// Update all effects systems
    pub fn update(&mut self, delta_time: f32) {
        if !self.enabled {
            return;
        }

        let update_start = Instant::now();
        self.last_update = Some(update_start);

        // Update particle systems
        let mut systems_to_remove = Vec::new();

        for (&id, system) in &mut self.particle_systems {
            // Update LOD based on distance from view
            let distance = (system.position - self.view_position).norm();
            let lod = calculate_effects_lod(distance, &self.config);
            system.set_lod(lod);

            // Update system
            system.update(delta_time, &self.config);

            // Mark finished systems for removal
            if system.is_finished() {
                systems_to_remove.push(id);
            }
        }

        // Remove finished systems
        for id in systems_to_remove {
            self.particle_systems.remove(&id);
        }

        // Update weather system
        if self.config.weather_enabled {
            self.weather_system.update(delta_time, self.view_position);
        }

        // Update decal manager
        if self.config.decals_enabled {
            self.decal_manager.update(delta_time, &self.config);
        }

        // Update statistics
        self.update_stats(update_start);
    }

    /// Render all effects
    pub fn render(&self) -> Result<(), EffectsError> {
        if !self.enabled {
            return Ok(());
        }

        let render_start = Instant::now();

        // Render particle systems
        if self.config.particles_enabled {
            for system in self.particle_systems.values() {
                if system.lod != EffectsLOD::None {
                    self.particle_renderer.render(system)?;
                }
            }
        }

        // Weather and decals are rendered through the display-driven GPU pipeline.

        // Update render timing stats
        let render_time = render_start.elapsed();
        // Note: This is a hack since stats is not mutable here
        // In a real implementation, you'd use Arc<Mutex<>> or similar

        Ok(())
    }

    /// Clear all effects
    pub fn clear_all_effects(&mut self) {
        self.particle_systems.clear();
        self.decal_manager.clear_all();
        // Weather system doesn't need clearing, just disable it
    }

    /// Get performance statistics
    pub fn stats(&self) -> &EffectsStats {
        &self.stats
    }

    /// Reset performance statistics
    pub fn reset_stats(&mut self) {
        self.stats.reset();
    }

    /// Get active particle system count
    pub fn active_particle_system_count(&self) -> usize {
        self.particle_systems.len()
    }

    /// Get total active particle count
    pub fn active_particle_count(&self) -> usize {
        self.particle_systems
            .values()
            .map(|system| system.active_particle_count())
            .sum()
    }

    /// Get active decal count
    pub fn active_decal_count(&self) -> usize {
        self.decal_manager.active_decal_count() + self.decal_manager.active_radius_decal_count()
    }

    /// Find oldest finished particle system for removal
    fn find_oldest_finished_system(&self) -> Option<ParticleSystemId> {
        self.particle_systems
            .iter()
            .filter(|(_, system)| system.is_finished())
            .min_by_key(|(_, system)| system.age as u64)
            .map(|(id, _)| *id)
    }

    /// Update performance statistics
    fn update_stats(&mut self, update_start: Instant) {
        let update_time = update_start.elapsed();

        self.stats.active_particle_systems = self.particle_systems.len();
        self.stats.active_particles = self.active_particle_count();
        self.stats.active_decals = self.active_decal_count();
        self.stats.update_time_ms = update_time.as_secs_f64() * 1000.0;
        self.stats.last_update = Some(update_start);

        // Estimate GPU memory usage
        self.stats.gpu_memory_used = self.estimate_gpu_memory_usage();
    }

    /// Estimate GPU memory usage (rough calculation)
    fn estimate_gpu_memory_usage(&self) -> usize {
        let particle_memory = self.stats.active_particles * 64; // ~64 bytes per particle
        let decal_memory = self.stats.active_decals * 256; // ~256 bytes per decal
        let system_overhead = self.stats.active_particle_systems * 1024; // ~1KB per system

        particle_memory + decal_memory + system_overhead
    }

    /// Create common effect types with convenience methods
    pub fn create_explosion(
        &mut self,
        position: nalgebra::Point3<f32>,
        scale: f32,
    ) -> Option<ParticleSystemId> {
        let desc = ParticleSystemDesc::explosion()
            .at_position(position.x, position.y, position.z)
            .with_scale(scale);
        self.spawn_particle_system(desc)
    }

    pub fn create_fire(&mut self, position: nalgebra::Point3<f32>) -> Option<ParticleSystemId> {
        let desc = ParticleSystemDesc::fire().at_position(position.x, position.y, position.z);
        self.spawn_particle_system(desc)
    }

    pub fn create_smoke(
        &mut self,
        position: nalgebra::Point3<f32>,
        duration: f32,
    ) -> Option<ParticleSystemId> {
        let desc = ParticleSystemDesc::smoke()
            .at_position(position.x, position.y, position.z)
            .with_duration(duration);
        self.spawn_particle_system(desc)
    }

    pub fn create_scorch_mark(
        &mut self,
        position: nalgebra::Point3<f32>,
        size: f32,
    ) -> Option<DecalId> {
        let settings = DecalSettings::scorch_mark(position, size);
        self.create_decal(settings)
    }
}

impl Default for EffectsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for EffectsManager {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Initializing EffectsManager subsystem");

        self.enabled = true;
        self.reset_stats();

        // Initialize subsystems (they don't have init methods yet, but this is where they'd be called)

        log::info!("EffectsManager initialization complete");
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Resetting EffectsManager subsystem");

        self.clear_all_effects();
        self.reset_stats();
        self.next_particle_id = 1;

        log::info!("EffectsManager reset complete");
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Calculate delta time
        let now = Instant::now();
        let delta_time = if let Some(last_update) = self.last_update {
            now.duration_since(last_update).as_secs_f32()
        } else {
            1.0 / 60.0 // Assume 60 FPS on first frame
        };

        self.update(delta_time);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::particles::ParticleType;

    #[test]
    fn test_effects_manager_creation() {
        let manager = EffectsManager::new();
        assert!(manager.is_enabled());
        assert_eq!(manager.active_particle_system_count(), 0);
        assert_eq!(manager.active_particle_count(), 0);
        assert_eq!(manager.active_decal_count(), 0);
    }

    #[test]
    fn test_effects_manager_with_config() {
        let mut config = EffectsConfig::default();
        config.particles_enabled = false;
        config.weather_enabled = false;

        let manager = EffectsManager::with_config(config);
        assert!(!manager.config.particles_enabled);
        assert!(!manager.config.weather_enabled);
    }

    #[test]
    fn test_particle_system_spawning() {
        let mut manager = EffectsManager::new();

        let position = nalgebra::Point3::new(10.0, 20.0, 30.0);
        let id = manager.create_explosion(position, 2.0);

        assert!(id.is_some());
        assert_eq!(manager.active_particle_system_count(), 1);

        let system = manager.get_particle_system(id.unwrap());
        assert!(system.is_some());
        assert_eq!(system.unwrap().particle_type, ParticleType::Explosion);
    }

    #[test]
    fn test_decal_creation() {
        let mut manager = EffectsManager::new();

        let position = nalgebra::Point3::new(5.0, 10.0, 0.0);
        let id = manager.create_scorch_mark(position, 1.5);

        assert!(id.is_some());
        assert_eq!(manager.active_decal_count(), 1);
    }

    #[test]
    fn test_weather_system() {
        let mut manager = EffectsManager::new();

        manager.set_weather(WeatherType::Snow, 0.8);
        // Weather system doesn't expose its settings publicly for testing,
        // but this verifies the call doesn't panic
    }

    #[test]
    fn test_effects_enable_disable() {
        let mut manager = EffectsManager::new();

        // Create some effects
        let _ = manager.create_explosion(nalgebra::Point3::new(0.0, 0.0, 0.0), 1.0);
        let _ = manager.create_scorch_mark(nalgebra::Point3::new(1.0, 1.0, 0.0), 1.0);

        assert!(manager.active_particle_system_count() > 0);
        assert!(manager.active_decal_count() > 0);

        // Disable effects
        manager.set_enabled(false);
        assert!(!manager.is_enabled());
        assert_eq!(manager.active_particle_system_count(), 0);
        assert_eq!(manager.active_decal_count(), 0);

        // Re-enable effects
        manager.set_enabled(true);
        assert!(manager.is_enabled());
    }

    #[test]
    fn test_max_particle_systems_limit() {
        let mut config = EffectsConfig::default();
        config.max_particle_systems = 2;

        let mut manager = EffectsManager::with_config(config);

        // Create 3 systems
        for i in 0..3 {
            let pos = nalgebra::Point3::new(i as f32, 0.0, 0.0);
            manager.create_explosion(pos, 1.0);
        }

        // Should only have 2 systems due to limit
        assert!(manager.active_particle_system_count() <= 2);
    }

    #[test]
    fn test_stats_tracking() {
        let mut manager = EffectsManager::new();

        // Initial stats
        let stats = manager.stats();
        assert_eq!(stats.active_particle_systems, 0);
        assert_eq!(stats.active_particles, 0);

        // Create effects and update to populate stats
        manager.create_explosion(nalgebra::Point3::new(0.0, 0.0, 0.0), 1.0);
        manager.update(1.0 / 60.0); // One frame at 60 FPS

        let stats = manager.stats();
        assert!(stats.active_particle_systems > 0);
        assert!(stats.update_time_ms >= 0.0);
        assert!(stats.last_update.is_some());
    }
}
