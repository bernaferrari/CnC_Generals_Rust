//! # Particle System Performance Optimization
//!
//! Advanced performance optimization and LOD system for particles,
//! matching C++ behavior while adding modern GPU optimizations.

use std::collections::{BTreeMap, VecDeque};
use std::time::Instant;
use nalgebra::{Point3, Vector3};

use super::particle_manager::*;
use super::particle_system::*;

/// Particle LOD settings based on distance and performance
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleLODSettings {
    /// Distance thresholds for LOD levels
    pub near_distance: f32,
    pub medium_distance: f32,
    pub far_distance: f32,
    
    /// Particle count multipliers for each LOD
    pub high_lod_multiplier: f32,
    pub medium_lod_multiplier: f32,
    pub low_lod_multiplier: f32,
    
    /// Update frequency multipliers
    pub high_update_freq: f32,
    pub medium_update_freq: f32,
    pub low_update_freq: f32,
}

impl Default for ParticleLODSettings {
    fn default() -> Self {
        Self {
            near_distance: 100.0,
            medium_distance: 300.0,
            far_distance: 600.0,
            
            high_lod_multiplier: 1.0,
            medium_lod_multiplier: 0.6,
            low_lod_multiplier: 0.3,
            
            high_update_freq: 1.0,
            medium_update_freq: 0.8,
            low_update_freq: 0.5,
        }
    }
}

/// Performance budgeting for particles
#[derive(Debug, Clone)]
pub struct ParticlePerformanceBudget {
    /// Maximum total particles allowed
    pub max_total_particles: usize,
    
    /// Maximum particles per priority level
    pub max_particles_by_priority: BTreeMap<ParticlePriorityType, usize>,
    
    /// Maximum particle systems active
    pub max_particle_systems: usize,
    
    /// GPU memory budget (bytes)
    pub gpu_memory_budget: usize,
    
    /// CPU time budget per frame (milliseconds)
    pub cpu_time_budget_ms: f64,
    
    /// Particle culling distance
    pub cull_distance: f32,
}

impl Default for ParticlePerformanceBudget {
    fn default() -> Self {
        let mut max_by_priority = BTreeMap::new();
        
        // Set budgets based on priority (higher priority gets more particles)
        max_by_priority.insert(ParticlePriorityType::AlwaysRender, usize::MAX); // No limit
        max_by_priority.insert(ParticlePriorityType::Critical, 1000);
        max_by_priority.insert(ParticlePriorityType::AreaEffect, 800);
        max_by_priority.insert(ParticlePriorityType::WeaponTrail, 600);
        max_by_priority.insert(ParticlePriorityType::Constant, 400);
        max_by_priority.insert(ParticlePriorityType::SemiConstant, 400);
        max_by_priority.insert(ParticlePriorityType::DeathExplosion, 300);
        max_by_priority.insert(ParticlePriorityType::UnitDamageFx, 200);
        max_by_priority.insert(ParticlePriorityType::DebrisTrail, 150);
        max_by_priority.insert(ParticlePriorityType::Buildup, 100);
        max_by_priority.insert(ParticlePriorityType::DustTrail, 100);
        max_by_priority.insert(ParticlePriorityType::ScorchMark, 50);
        max_by_priority.insert(ParticlePriorityType::WeaponExplosion, 200);
        
        Self {
            max_total_particles: 3000,
            max_particles_by_priority: max_by_priority,
            max_particle_systems: 100,
            gpu_memory_budget: 64 * 1024 * 1024, // 64 MB
            cpu_time_budget_ms: 2.0, // 2ms per frame at 60fps
            cull_distance: 1000.0,
        }
    }
}

/// Particle performance optimizer
pub struct ParticleOptimizer {
    /// Performance settings
    pub lod_settings: ParticleLODSettings,
    pub performance_budget: ParticlePerformanceBudget,
    
    /// Runtime performance tracking
    frame_times: VecDeque<f64>,
    particle_counts: VecDeque<usize>,
    
    /// Adaptive quality settings
    current_quality_scale: f32,
    performance_trend: f32,
    
    /// Camera position for distance calculations
    camera_position: Point3<f32>,
    
    /// Frame timing
    last_update_time: Instant,
    performance_check_timer: f64,
}

impl ParticleOptimizer {
    /// Create new particle optimizer
    pub fn new() -> Self {
        Self {
            lod_settings: ParticleLODSettings::default(),
            performance_budget: ParticlePerformanceBudget::default(),
            
            frame_times: VecDeque::with_capacity(60), // Track last 60 frames
            particle_counts: VecDeque::with_capacity(60),
            
            current_quality_scale: 1.0,
            performance_trend: 0.0,
            
            camera_position: Point3::origin(),
            
            last_update_time: Instant::now(),
            performance_check_timer: 0.0,
        }
    }
    
    /// Update camera position for distance-based optimization
    pub fn update_camera_position(&mut self, position: Point3<f32>) {
        self.camera_position = position;
    }
    
    /// Update performance metrics and adjust quality
    pub fn update_performance_metrics(&mut self, particle_count: usize, frame_time_ms: f64) {
        let now = Instant::now();
        let delta_time = now.duration_since(self.last_update_time).as_secs_f64() * 1000.0;
        self.last_update_time = now;
        
        // Track performance history
        self.frame_times.push_back(frame_time_ms);
        self.particle_counts.push_back(particle_count);
        
        // Keep only recent history
        while self.frame_times.len() > 60 {
            self.frame_times.pop_front();
        }
        while self.particle_counts.len() > 60 {
            self.particle_counts.pop_front();
        }
        
        // Update performance check timer
        self.performance_check_timer += delta_time;
        
        // Check and adjust performance every 500ms
        if self.performance_check_timer >= 500.0 {
            self.adjust_quality_for_performance();
            self.performance_check_timer = 0.0;
        }
    }
    
    /// Calculate LOD level for a particle system based on distance
    pub fn calculate_lod_level(&self, system_position: Point3<f32>) -> ParticleLODLevel {
        let distance = (system_position - self.camera_position).norm();
        
        if distance <= self.lod_settings.near_distance {
            ParticleLODLevel::High
        } else if distance <= self.lod_settings.medium_distance {
            ParticleLODLevel::Medium
        } else if distance <= self.lod_settings.far_distance {
            ParticleLODLevel::Low
        } else if distance <= self.performance_budget.cull_distance {
            ParticleLODLevel::Minimal
        } else {
            ParticleLODLevel::Culled
        }
    }
    
    /// Get particle count multiplier for LOD level
    pub fn get_lod_particle_multiplier(&self, lod_level: ParticleLODLevel) -> f32 {
        let base_multiplier = match lod_level {
            ParticleLODLevel::High => self.lod_settings.high_lod_multiplier,
            ParticleLODLevel::Medium => self.lod_settings.medium_lod_multiplier,
            ParticleLODLevel::Low => self.lod_settings.low_lod_multiplier,
            ParticleLODLevel::Minimal => 0.1,
            ParticleLODLevel::Culled => 0.0,
        };
        
        base_multiplier * self.current_quality_scale
    }
    
    /// Get update frequency multiplier for LOD level
    pub fn get_lod_update_multiplier(&self, lod_level: ParticleLODLevel) -> f32 {
        match lod_level {
            ParticleLODLevel::High => self.lod_settings.high_update_freq,
            ParticleLODLevel::Medium => self.lod_settings.medium_update_freq,
            ParticleLODLevel::Low => self.lod_settings.low_update_freq,
            ParticleLODLevel::Minimal => 0.25,
            ParticleLODLevel::Culled => 0.0,
        }
    }
    
    /// Check if particle system should be culled based on various factors
    pub fn should_cull_system(
        &self, 
        system: &ParticleSystem, 
        current_particle_count: usize
    ) -> bool {
        let template = system.template();
        let info = template.info();
        
        // Never cull ALWAYS_RENDER priority
        if info.priority == ParticlePriorityType::AlwaysRender {
            return false;
        }
        
        // Check distance culling
        let distance = (system.position() - self.camera_position).norm();
        if distance > self.performance_budget.cull_distance {
            return true;
        }
        
        // Check if we're over particle budget
        if current_particle_count > self.performance_budget.max_total_particles {
            // Cull lower priority systems first
            let priority_budget = self.performance_budget.max_particles_by_priority
                .get(&info.priority)
                .copied()
                .unwrap_or(0);
                
            if priority_budget == 0 {
                return true;
            }
        }
        
        // Check if system is invisible (all particles invisible)
        if system.particles().iter().all(|p| p.is_invisible(info.shader_type)) {
            return true;
        }
        
        false
    }
    
    /// Optimize particle system parameters based on LOD and performance
    pub fn optimize_system_parameters(&self, system: &mut ParticleSystem) {
        let lod_level = self.calculate_lod_level(system.position());
        let particle_multiplier = self.get_lod_particle_multiplier(lod_level);
        let update_multiplier = self.get_lod_update_multiplier(lod_level);
        
        // Apply LOD multipliers to burst count and delay
        system.set_burst_count_multiplier(particle_multiplier);
        system.set_burst_delay_multiplier(1.0 / update_multiplier.max(0.1)); // Slower updates = longer delays
        
        // Reduce size for distant particles to save fill rate
        let size_multiplier = match lod_level {
            ParticleLODLevel::High => 1.0,
            ParticleLODLevel::Medium => 0.8,
            ParticleLODLevel::Low => 0.6,
            ParticleLODLevel::Minimal => 0.4,
            ParticleLODLevel::Culled => 0.0,
        };
        system.set_size_multiplier(size_multiplier * self.current_quality_scale);
    }
    
    /// Remove oldest particles when over budget (matches C++ removeOldestParticles)
    pub fn enforce_particle_budget(
        &self, 
        manager: &mut ParticleSystemManager,
        priority_cap: ParticlePriorityType
    ) -> usize {
        let mut removed_count = 0;
        let current_count = manager.particle_count();
        
        if current_count <= self.performance_budget.max_total_particles {
            return 0; // Under budget
        }
        
        let target_removal = current_count - self.performance_budget.max_total_particles;
        let mut systems_by_age: Vec<(ParticleSystemId, u32)> = Vec::new();
        
        // Collect systems sorted by age (oldest first) that are below priority cap
        for system in manager.all_particle_systems() {
            let info = system.template().info();
            if info.priority <= priority_cap {
                systems_by_age.push((system.system_id(), system.start_timestamp()));
            }
        }
        
        // Sort by age (oldest first)
        systems_by_age.sort_by_key(|&(_, timestamp)| timestamp);
        
        // Remove particles from oldest systems first
        for (system_id, _) in systems_by_age {
            if removed_count >= target_removal {
                break;
            }
            
            if let Some(system) = manager.find_particle_system_mut(system_id) {
                let particles_to_remove = (system.particle_count() / 2).max(1);
                let actually_removed = system.remove_oldest_particles(particles_to_remove);
                removed_count += actually_removed;
            }
        }
        
        removed_count
    }
    
    /// Adjust quality based on performance trends
    fn adjust_quality_for_performance(&mut self) {
        if self.frame_times.len() < 10 {
            return; // Not enough data yet
        }
        
        // Calculate average frame time and particle count
        let avg_frame_time: f64 = self.frame_times.iter().sum::<f64>() / self.frame_times.len() as f64;
        let avg_particle_count: f64 = self.particle_counts.iter().sum::<usize>() as f64 / self.particle_counts.len() as f64;
        
        // Calculate performance trend (positive = getting worse, negative = getting better)
        let recent_frame_time: f64 = self.frame_times.iter().rev().take(10).sum::<f64>() / 10.0;
        let old_frame_time: f64 = self.frame_times.iter().take(10).sum::<f64>() / 10.0;
        self.performance_trend = (recent_frame_time - old_frame_time) as f32;
        
        // Adjust quality based on performance
        let target_frame_time = self.performance_budget.cpu_time_budget_ms;
        
        if avg_frame_time > target_frame_time * 1.2 || self.performance_trend > 0.5 {
            // Performance is poor, reduce quality
            self.current_quality_scale = (self.current_quality_scale * 0.9).max(0.3);
            
            // Also reduce LOD distances to cull more aggressively
            self.lod_settings.near_distance *= 0.95;
            self.lod_settings.medium_distance *= 0.95;
            self.lod_settings.far_distance *= 0.95;
        } else if avg_frame_time < target_frame_time * 0.8 && self.performance_trend < -0.2 {
            // Performance is good, can increase quality
            self.current_quality_scale = (self.current_quality_scale * 1.05).min(1.0);
            
            // Restore LOD distances gradually
            let default_settings = ParticleLODSettings::default();
            self.lod_settings.near_distance = self.lod_settings.near_distance.max(default_settings.near_distance * 1.01);
            self.lod_settings.medium_distance = self.lod_settings.medium_distance.max(default_settings.medium_distance * 1.01);
            self.lod_settings.far_distance = self.lod_settings.far_distance.max(default_settings.far_distance * 1.01);
        }
        
        // Clamp values to reasonable ranges
        self.lod_settings.near_distance = self.lod_settings.near_distance.clamp(50.0, 200.0);
        self.lod_settings.medium_distance = self.lod_settings.medium_distance.clamp(150.0, 500.0);
        self.lod_settings.far_distance = self.lod_settings.far_distance.clamp(300.0, 1000.0);
    }
    
    /// Get current performance statistics
    pub fn get_performance_stats(&self) -> ParticleOptimizationStats {
        let avg_frame_time = if !self.frame_times.is_empty() {
            self.frame_times.iter().sum::<f64>() / self.frame_times.len() as f64
        } else {
            0.0
        };
        
        let avg_particle_count = if !self.particle_counts.is_empty() {
            self.particle_counts.iter().sum::<usize>() / self.particle_counts.len()
        } else {
            0
        };
        
        ParticleOptimizationStats {
            current_quality_scale: self.current_quality_scale,
            performance_trend: self.performance_trend,
            average_frame_time_ms: avg_frame_time,
            average_particle_count: avg_particle_count,
            lod_distances: [
                self.lod_settings.near_distance,
                self.lod_settings.medium_distance,
                self.lod_settings.far_distance,
            ],
        }
    }
}

/// Particle LOD levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleLODLevel {
    High,    // Full quality, close to camera
    Medium,  // Reduced quality
    Low,     // Minimal quality
    Minimal, // Barely visible
    Culled,  // Not rendered
}

/// Performance optimization statistics
#[derive(Debug, Clone)]
pub struct ParticleOptimizationStats {
    pub current_quality_scale: f32,
    pub performance_trend: f32,
    pub average_frame_time_ms: f64,
    pub average_particle_count: usize,
    pub lod_distances: [f32; 3], // [near, medium, far]
}

// Extension trait to add optimization methods to ParticleSystem
pub trait ParticleSystemOptimization {
    fn set_burst_count_multiplier(&mut self, multiplier: f32);
    fn set_burst_delay_multiplier(&mut self, multiplier: f32);
    fn set_size_multiplier(&mut self, multiplier: f32);
    fn remove_oldest_particles(&mut self, count: usize) -> usize;
    fn start_timestamp(&self) -> u32;
}

// This would be implemented as part of the ParticleSystem impl
// For now, just declare the interface
impl ParticleSystemOptimization for ParticleSystem {
    fn set_burst_count_multiplier(&mut self, multiplier: f32) {
        self.count_coeff = multiplier;
    }
    
    fn set_burst_delay_multiplier(&mut self, multiplier: f32) {
        self.delay_coeff = multiplier;
    }
    
    fn set_size_multiplier(&mut self, multiplier: f32) {
        self.size_coeff = multiplier;
    }
    
    fn remove_oldest_particles(&mut self, count: usize) -> usize {
        let mut removed = 0;
        while removed < count && !self.particles().is_empty() {
            // Remove from front (oldest particles)
            self.particles_mut().pop_front();
            self.particle_count = self.particle_count.saturating_sub(1);
            removed += 1;
        }
        removed
    }
    
    fn start_timestamp(&self) -> u32 {
        self.start_timestamp
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_optimizer_creation() {
        let optimizer = ParticleOptimizer::new();
        assert_eq!(optimizer.current_quality_scale, 1.0);
        assert_eq!(optimizer.performance_trend, 0.0);
    }
    
    #[test]
    fn test_lod_calculation() {
        let mut optimizer = ParticleOptimizer::new();
        optimizer.update_camera_position(Point3::origin());
        
        // Test distance-based LOD
        let near_pos = Point3::new(50.0, 0.0, 0.0);
        assert_eq!(optimizer.calculate_lod_level(near_pos), ParticleLODLevel::High);
        
        let far_pos = Point3::new(500.0, 0.0, 0.0);
        assert_eq!(optimizer.calculate_lod_level(far_pos), ParticleLODLevel::Low);
        
        let very_far_pos = Point3::new(2000.0, 0.0, 0.0);
        assert_eq!(optimizer.calculate_lod_level(very_far_pos), ParticleLODLevel::Culled);
    }
    
    #[test]
    fn test_performance_budget() {
        let budget = ParticlePerformanceBudget::default();
        
        // Always render should have unlimited budget
        assert_eq!(
            budget.max_particles_by_priority.get(&ParticlePriorityType::AlwaysRender),
            Some(&usize::MAX)
        );
        
        // Other priorities should have limits
        assert!(
            budget.max_particles_by_priority.get(&ParticlePriorityType::WeaponExplosion).unwrap()
                < &usize::MAX
        );
    }
    
    #[test]
    fn test_quality_scaling() {
        let mut optimizer = ParticleOptimizer::new();
        
        // Simulate poor performance
        for _ in 0..20 {
            optimizer.update_performance_metrics(3000, 20.0); // 20ms frame time
        }
        
        // Quality should have been reduced
        assert!(optimizer.current_quality_scale < 1.0);
    }
}