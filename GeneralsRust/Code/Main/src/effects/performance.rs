use glam::Vec3;
use log::{debug, info};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use ww3d_engine::FrameTiming;

use crate::effects::particle_system::{ParticlePriority, ParticleSystem, ParticleSystemTemplate};
use crate::effects::visual_effects::{ActiveEffect, EffectType};

/// Performance quality levels matching C&C settings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityLevel {
    Low,    // Minimum effects for low-end hardware
    Medium, // Balanced effects for average hardware
    High,   // Full effects for good hardware
    Ultra,  // Maximum effects for high-end hardware
}

impl QualityLevel {
    /// Get particle count multiplier for this quality level
    pub fn particle_multiplier(&self) -> f32 {
        match self {
            QualityLevel::Low => 0.3,
            QualityLevel::Medium => 0.6,
            QualityLevel::High => 1.0,
            QualityLevel::Ultra => 1.5,
        }
    }

    /// Get maximum light count for this quality level
    pub fn max_lights(&self) -> u32 {
        match self {
            QualityLevel::Low => 16,
            QualityLevel::Medium => 32,
            QualityLevel::High => 64,
            QualityLevel::Ultra => 128,
        }
    }

    /// Get LOD distance scale for this quality level
    pub fn lod_scale(&self) -> f32 {
        match self {
            QualityLevel::Low => 0.5, // More aggressive LOD
            QualityLevel::Medium => 0.7,
            QualityLevel::High => 1.0,
            QualityLevel::Ultra => 1.3, // Less aggressive LOD
        }
    }

    /// Get shadow quality for this level
    pub fn shadow_quality(&self) -> u32 {
        match self {
            QualityLevel::Low => 512,
            QualityLevel::Medium => 1024,
            QualityLevel::High => 2048,
            QualityLevel::Ultra => 4096,
        }
    }

    /// Whether to enable expensive effects
    pub fn enable_expensive_effects(&self) -> bool {
        matches!(self, QualityLevel::High | QualityLevel::Ultra)
    }
}

/// Level of Detail settings for effects
#[derive(Debug, Clone)]
pub struct LODSettings {
    pub max_distance: f32,            // Maximum distance to show effects
    pub fade_start_distance: f32,     // Distance to start fading
    pub particle_reduction: f32,      // Particle count multiplier (0.0-1.0)
    pub skip_secondary_effects: bool, // Skip less important effects
    pub reduce_update_rate: bool,     // Update effects less frequently
}

impl LODSettings {
    pub fn for_distance(distance: f32, quality: QualityLevel) -> Self {
        let scale = quality.lod_scale();

        if distance < 50.0 * scale {
            // Close range - full quality
            Self {
                max_distance: 200.0 * scale,
                fade_start_distance: 150.0 * scale,
                particle_reduction: 1.0,
                skip_secondary_effects: false,
                reduce_update_rate: false,
            }
        } else if distance < 100.0 * scale {
            // Medium range - reduced quality
            Self {
                max_distance: 150.0 * scale,
                fade_start_distance: 100.0 * scale,
                particle_reduction: 0.7,
                skip_secondary_effects: false,
                reduce_update_rate: false,
            }
        } else if distance < 200.0 * scale {
            // Long range - basic effects only
            Self {
                max_distance: 100.0 * scale,
                fade_start_distance: 80.0 * scale,
                particle_reduction: 0.4,
                skip_secondary_effects: true,
                reduce_update_rate: true,
            }
        } else {
            // Very long range - minimal effects
            Self {
                max_distance: 50.0 * scale,
                fade_start_distance: 40.0 * scale,
                particle_reduction: 0.2,
                skip_secondary_effects: true,
                reduce_update_rate: true,
            }
        }
    }
}

/// Pool for reusing particle systems to avoid allocation overhead
pub struct ParticleSystemPool {
    available_systems: VecDeque<ParticleSystem>,
    active_systems: HashMap<usize, ParticleSystem>,
    templates: HashMap<String, Arc<ParticleSystemTemplate>>,
    next_id: usize,
    max_pool_size: usize,
}

impl ParticleSystemPool {
    pub fn new(max_pool_size: usize) -> Self {
        Self {
            available_systems: VecDeque::new(),
            active_systems: HashMap::new(),
            templates: HashMap::new(),
            next_id: 1,
            max_pool_size,
        }
    }

    /// Get a particle system from the pool or create new one
    pub fn acquire(&mut self, template_name: &str, frame_count: u32) -> Option<usize> {
        if let Some(template) = self.templates.get(template_name) {
            let system_id = self.next_id;
            self.next_id += 1;

            // Try to reuse from pool
            if let Some(mut system) = self.available_systems.pop_front() {
                // Reset the system with new template
                system.id = system_id;
                system.template = template.clone();
                system.particles.clear();
                system.start_timestamp = frame_count;
                system.is_stopped = false;
                system.is_destroyed = false;
                self.active_systems.insert(system_id, system);
                debug!("Reused particle system {} from pool", system_id);
            } else {
                // Create new system
                let system = ParticleSystem::new(system_id, template.clone(), frame_count);
                self.active_systems.insert(system_id, system);
                debug!("Created new particle system {}", system_id);
            }

            Some(system_id)
        } else {
            None
        }
    }

    /// Return a particle system to the pool
    pub fn release(&mut self, system_id: usize) {
        if let Some(mut system) = self.active_systems.remove(&system_id) {
            // Clean up the system
            system.particles.clear();
            system.is_stopped = true;

            // Return to pool if not full
            if self.available_systems.len() < self.max_pool_size {
                self.available_systems.push_back(system);
                debug!("Returned particle system {} to pool", system_id);
            } else {
                debug!("Pool full, discarded particle system {}", system_id);
            }
        }
    }

    /// Register a template for pooling
    pub fn register_template(&mut self, template: ParticleSystemTemplate) {
        let name = template.name.clone();
        self.templates.insert(name, Arc::new(template));
    }

    /// Get active system count
    pub fn get_active_count(&self) -> usize {
        self.active_systems.len()
    }

    /// Get pool size
    pub fn get_pool_size(&self) -> usize {
        self.available_systems.len()
    }

    /// Update all active systems and handle recycling
    pub fn update(&mut self, frame_count: u32) -> Vec<usize> {
        let mut finished_systems = Vec::new();

        for (id, system) in &mut self.active_systems {
            if !system.update(frame_count) {
                finished_systems.push(*id);
            }
        }

        // Return finished systems to pool
        for id in &finished_systems {
            self.release(*id);
        }

        finished_systems
    }
}

/// Pool for reusing visual effect instances
pub struct EffectPool {
    available_effects: HashMap<EffectType, VecDeque<ActiveEffect>>,
    max_per_type: usize,
}

impl EffectPool {
    pub fn new(max_per_type: usize) -> Self {
        Self {
            available_effects: HashMap::new(),
            max_per_type,
        }
    }

    /// Get an effect from the pool or create new one
    pub fn acquire(&mut self, effect_type: EffectType) -> ActiveEffect {
        if let Some(pool) = self.available_effects.get_mut(&effect_type) {
            if let Some(mut effect) = pool.pop_front() {
                // Reset the effect
                effect.is_active = true;
                effect.fade_progress = 0.0;
                effect.start_time = 0.0; // Will be set by caller
                return effect;
            }
        }

        // Create new effect
        ActiveEffect {
            effect_type,
            particle_system_id: None,
            position: Vec3::ZERO,
            rotation: 0.0,
            scale: 1.0,
            start_time: 0.0,
            duration: 1.0,
            object_id: None,
            light_emission: None,
            screen_shake: None,
            camera_effects: None,
            is_active: true,
            fade_progress: 0.0,
        }
    }

    /// Return an effect to the pool
    pub fn release(&mut self, mut effect: ActiveEffect) {
        effect.is_active = false;

        let pool = self
            .available_effects
            .entry(effect.effect_type)
            .or_default();

        if pool.len() < self.max_per_type {
            pool.push_back(effect);
        }
    }

    /// Clear all pools
    pub fn clear(&mut self) {
        self.available_effects.clear();
    }
}

/// Level of Detail manager for effects
pub struct EffectLODManager {
    quality_level: QualityLevel,
    camera_position: Vec3,
    lod_update_interval: f32,
    last_lod_update: f32,
    effect_lods: HashMap<usize, LODSettings>, // Effect ID -> LOD settings

    // Performance budgets
    max_particles_budget: u32,
    max_lights_budget: u32,
    max_effects_budget: u32,
    current_particles: u32,
    current_lights: u32,
    current_effects: u32,

    // Culling settings
    culling_enabled: bool,
    frustum_culling: bool,
    distance_culling: bool,
    priority_culling: bool,
}

impl EffectLODManager {
    pub fn new(quality_level: QualityLevel) -> Self {
        Self {
            quality_level,
            camera_position: Vec3::ZERO,
            lod_update_interval: 0.1, // Update LOD 10 times per second
            last_lod_update: 0.0,
            effect_lods: HashMap::new(),
            max_particles_budget: (1000.0 * quality_level.particle_multiplier()) as u32,
            max_lights_budget: quality_level.max_lights(),
            max_effects_budget: match quality_level {
                QualityLevel::Low => 50,
                QualityLevel::Medium => 100,
                QualityLevel::High => 200,
                QualityLevel::Ultra => 400,
            },
            current_particles: 0,
            current_lights: 0,
            current_effects: 0,
            culling_enabled: true,
            frustum_culling: true,
            distance_culling: true,
            priority_culling: true,
        }
    }

    /// Update LOD settings for effects based on camera position
    pub fn update(&mut self, current_time: f32, camera_pos: Vec3) {
        self.camera_position = camera_pos;

        // Only update LOD periodically for performance
        if current_time - self.last_lod_update < self.lod_update_interval {
            return;
        }
        self.last_lod_update = current_time;

        // Clear old LOD data
        self.effect_lods.clear();

        debug!(
            "Updated effect LOD system (budget: {} particles, {} lights, {} effects)",
            self.max_particles_budget, self.max_lights_budget, self.max_effects_budget
        );
    }

    /// Get LOD settings for an effect at a specific position
    pub fn get_lod_settings(&mut self, effect_id: usize, position: Vec3) -> LODSettings {
        if let Some(lod) = self.effect_lods.get(&effect_id) {
            return lod.clone();
        }

        let distance = self.camera_position.distance(position);
        let lod_settings = LODSettings::for_distance(distance, self.quality_level);

        self.effect_lods.insert(effect_id, lod_settings.clone());
        lod_settings
    }

    /// Check if an effect should be rendered based on LOD
    pub fn should_render_effect(&self, position: Vec3, priority: ParticlePriority) -> bool {
        if !self.culling_enabled {
            return true;
        }

        let distance = self.camera_position.distance(position);
        let lod_settings = LODSettings::for_distance(distance, self.quality_level);

        // Distance culling
        if self.distance_culling && distance > lod_settings.max_distance {
            return false;
        }

        // Priority culling when over budget
        if self.priority_culling && self.is_over_budget() {
            // Only render high priority effects when over budget
            match priority {
                ParticlePriority::Critical | ParticlePriority::AlwaysRender => true,
                ParticlePriority::AreaEffect | ParticlePriority::WeaponTrail => distance < 100.0,
                _ => distance < 50.0,
            }
        } else {
            true
        }
    }

    /// Check if we're over performance budget
    fn is_over_budget(&self) -> bool {
        self.current_particles > self.max_particles_budget
            || self.current_lights > self.max_lights_budget
            || self.current_effects > self.max_effects_budget
    }

    /// Update current resource usage
    pub fn update_budgets(&mut self, particles: u32, lights: u32, effects: u32) {
        self.current_particles = particles;
        self.current_lights = lights;
        self.current_effects = effects;
    }

    /// Get particle count reduction factor for LOD
    pub fn get_particle_reduction(&self, position: Vec3) -> f32 {
        let distance = self.camera_position.distance(position);
        let lod_settings = LODSettings::for_distance(distance, self.quality_level);
        lod_settings.particle_reduction
    }

    /// Check if secondary effects should be skipped
    pub fn should_skip_secondary_effects(&self, position: Vec3) -> bool {
        let distance = self.camera_position.distance(position);
        let lod_settings = LODSettings::for_distance(distance, self.quality_level);
        lod_settings.skip_secondary_effects
    }

    /// Set quality level and update budgets
    pub fn set_quality_level(&mut self, quality: QualityLevel) {
        self.quality_level = quality;
        self.max_particles_budget = (1000.0 * quality.particle_multiplier()) as u32;
        self.max_lights_budget = quality.max_lights();
        self.max_effects_budget = match quality {
            QualityLevel::Low => 50,
            QualityLevel::Medium => 100,
            QualityLevel::High => 200,
            QualityLevel::Ultra => 400,
        };

        info!("Updated quality level to {:?}", quality);
    }

    /// Get current performance statistics
    pub fn get_performance_stats(&self) -> PerformanceStats {
        PerformanceStats {
            particles_used: self.current_particles,
            particles_budget: self.max_particles_budget,
            lights_used: self.current_lights,
            lights_budget: self.max_lights_budget,
            effects_used: self.current_effects,
            effects_budget: self.max_effects_budget,
            quality_level: self.quality_level,
        }
    }
}

/// Performance statistics for monitoring
#[derive(Debug, Clone)]
pub struct PerformanceStats {
    pub particles_used: u32,
    pub particles_budget: u32,
    pub lights_used: u32,
    pub lights_budget: u32,
    pub effects_used: u32,
    pub effects_budget: u32,
    pub quality_level: QualityLevel,
}

impl PerformanceStats {
    /// Get particle budget utilization (0.0 to 1.0+)
    pub fn particle_utilization(&self) -> f32 {
        self.particles_used as f32 / self.particles_budget as f32
    }

    /// Get light budget utilization (0.0 to 1.0+)
    pub fn light_utilization(&self) -> f32 {
        self.lights_used as f32 / self.lights_budget as f32
    }

    /// Get effects budget utilization (0.0 to 1.0+)
    pub fn effects_utilization(&self) -> f32 {
        self.effects_used as f32 / self.effects_budget as f32
    }

    /// Check if any budget is over limit
    pub fn is_over_budget(&self) -> bool {
        self.particle_utilization() > 1.0
            || self.light_utilization() > 1.0
            || self.effects_utilization() > 1.0
    }
}

/// Adaptive performance manager that adjusts quality based on performance
pub struct AdaptivePerformanceManager {
    lod_manager: EffectLODManager,
    target_fps: f32,
    current_fps: f32,
    fps_samples: VecDeque<f32>,
    max_samples: usize,

    // Performance adjustment
    auto_adjust_quality: bool,
    adjustment_cooldown: f32,
    last_adjustment: f32,
    adjustment_threshold: f32, // FPS difference to trigger adjustment

    // Frame timing
    frame_times: VecDeque<f32>,
    max_frame_samples: usize,
}

impl AdaptivePerformanceManager {
    pub fn new(quality_level: QualityLevel, target_fps: f32) -> Self {
        Self {
            lod_manager: EffectLODManager::new(quality_level),
            target_fps,
            current_fps: target_fps,
            fps_samples: VecDeque::new(),
            max_samples: 30, // 30 frame average
            auto_adjust_quality: true,
            adjustment_cooldown: 5.0, // 5 seconds between adjustments
            last_adjustment: 0.0,
            adjustment_threshold: 10.0, // 10 FPS difference
            frame_times: VecDeque::new(),
            max_frame_samples: 60,
        }
    }

    /// Update using WW3D frame timing
    pub fn update_with_timing(&mut self, timing: &FrameTiming, camera_pos: Vec3) {
        self.update_internal(timing.delta_seconds(), timing.total_seconds(), camera_pos);
    }

    /// Update with explicit timing values (legacy path)
    pub fn update(&mut self, delta_time: f32, current_time: f32, camera_pos: Vec3) {
        self.update_internal(delta_time, current_time, camera_pos);
    }

    fn update_internal(&mut self, delta_time: f32, current_time: f32, camera_pos: Vec3) {
        // Update frame timing
        if delta_time > 0.0 {
            let fps = 1.0 / delta_time;
            self.fps_samples.push_back(fps);
            if self.fps_samples.len() > self.max_samples {
                self.fps_samples.pop_front();
            }

            self.frame_times.push_back(delta_time);
            if self.frame_times.len() > self.max_frame_samples {
                self.frame_times.pop_front();
            }
        }

        // Calculate average FPS
        if !self.fps_samples.is_empty() {
            self.current_fps = self.fps_samples.iter().sum::<f32>() / self.fps_samples.len() as f32;
        }

        // Auto-adjust quality if enabled
        if self.auto_adjust_quality
            && current_time - self.last_adjustment > self.adjustment_cooldown
        {
            self.auto_adjust_performance(current_time);
        }

        // Update LOD manager
        self.lod_manager.update(current_time, camera_pos);
    }

    /// Automatically adjust quality based on performance
    fn auto_adjust_performance(&mut self, current_time: f32) {
        let fps_difference = self.current_fps - self.target_fps;

        if fps_difference < -self.adjustment_threshold {
            // Performance too low, reduce quality
            let new_quality = match self.lod_manager.quality_level {
                QualityLevel::Ultra => QualityLevel::High,
                QualityLevel::High => QualityLevel::Medium,
                QualityLevel::Medium => QualityLevel::Low,
                QualityLevel::Low => return, // Already at lowest
            };

            self.lod_manager.set_quality_level(new_quality);
            self.last_adjustment = current_time;

            info!(
                "Performance too low ({:.1} FPS), reduced quality to {:?}",
                self.current_fps, new_quality
            );
        } else if fps_difference > self.adjustment_threshold * 2.0 {
            // Performance headroom, increase quality
            let new_quality = match self.lod_manager.quality_level {
                QualityLevel::Low => QualityLevel::Medium,
                QualityLevel::Medium => QualityLevel::High,
                QualityLevel::High => QualityLevel::Ultra,
                QualityLevel::Ultra => return, // Already at highest
            };

            self.lod_manager.set_quality_level(new_quality);
            self.last_adjustment = current_time;

            info!(
                "Performance headroom ({:.1} FPS), increased quality to {:?}",
                self.current_fps, new_quality
            );
        }
    }

    /// Get current FPS
    pub fn get_current_fps(&self) -> f32 {
        self.current_fps
    }

    /// Get average frame time in milliseconds
    pub fn get_average_frame_time_ms(&self) -> f32 {
        if self.frame_times.is_empty() {
            return 16.67; // Assume 60 FPS
        }

        let avg_time = self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32;
        avg_time * 1000.0
    }

    /// Get frame time percentiles for performance analysis
    pub fn get_frame_time_percentiles(&self) -> (f32, f32, f32) {
        if self.frame_times.is_empty() {
            return (16.67, 16.67, 16.67);
        }

        let mut sorted_times: Vec<f32> = self.frame_times.iter().copied().collect();
        sorted_times.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let len = sorted_times.len();
        let p50 = sorted_times[len / 2] * 1000.0;
        let p95 = sorted_times[(len * 95) / 100] * 1000.0;
        let p99 = sorted_times[(len * 99) / 100] * 1000.0;

        (p50, p95, p99)
    }

    /// Enable/disable auto quality adjustment
    pub fn set_auto_adjust(&mut self, enabled: bool) {
        self.auto_adjust_quality = enabled;
        info!(
            "Auto quality adjustment {}",
            if enabled { "enabled" } else { "disabled" }
        );
    }

    /// Set target FPS
    pub fn set_target_fps(&mut self, target_fps: f32) {
        self.target_fps = target_fps;
        info!("Target FPS set to {:.1}", target_fps);
    }

    /// Get LOD manager reference
    pub fn get_lod_manager(&self) -> &EffectLODManager {
        &self.lod_manager
    }

    /// Get mutable LOD manager reference
    pub fn get_lod_manager_mut(&mut self) -> &mut EffectLODManager {
        &mut self.lod_manager
    }

    /// Force quality level (disables auto-adjustment temporarily)
    pub fn force_quality_level(&mut self, quality: QualityLevel, current_time: f32) {
        self.lod_manager.set_quality_level(quality);
        self.last_adjustment = current_time;
        info!("Forced quality level to {:?}", quality);
    }
}
