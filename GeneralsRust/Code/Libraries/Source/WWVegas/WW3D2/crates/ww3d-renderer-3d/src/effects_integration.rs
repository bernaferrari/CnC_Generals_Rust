//! Effects Integration Module
//!
//! This module provides integration between ww3d-effects and ww3d-renderer-3d,
//! enabling particle systems, decals, and other effects to render correctly.

use crate::rendering::mesh_system::MeshClass;
use crate::Renderer;
use std::sync::Arc;
use ww3d_core::errors::W3DResult;

/// Effect type classification for rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectType {
    /// Particle system effect (needs alpha blending)
    Particle,
    /// Decal effect (needs depth offset and special blending)
    Decal,
    /// Screen-space effect (dazzle, flash)
    ScreenSpace,
    /// Line/trail effect
    Line,
    /// Ring/sphere effect
    Ring,
}

/// Effect rendering configuration
#[derive(Debug, Clone)]
pub struct EffectRenderConfig {
    pub effect_type: EffectType,
    pub blend_mode: BlendMode,
    pub depth_write: bool,
    pub depth_test: bool,
    pub sort_order: i32,
    pub alpha_threshold: f32,
}

impl Default for EffectRenderConfig {
    fn default() -> Self {
        Self {
            effect_type: EffectType::Particle,
            blend_mode: BlendMode::AlphaBlend,
            depth_write: false,
            depth_test: true,
            sort_order: 0,
            alpha_threshold: 0.01,
        }
    }
}

/// Blend mode for effects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    /// Standard alpha blending
    AlphaBlend,
    /// Additive blending (for glows, fire)
    Additive,
    /// Multiplicative blending (for darkening effects)
    Multiplicative,
    /// No blending (opaque)
    Opaque,
    /// Premultiplied alpha
    PremultipliedAlpha,
}

/// Effect mesh wrapper that includes rendering configuration
pub struct EffectMesh {
    pub mesh: Arc<MeshClass>,
    pub config: EffectRenderConfig,
    pub lifetime: f32,
    pub age: f32,
}

impl EffectMesh {
    /// Create a new effect mesh
    pub fn new(mesh: Arc<MeshClass>, config: EffectRenderConfig) -> Self {
        Self {
            mesh,
            config,
            lifetime: f32::INFINITY,
            age: 0.0,
        }
    }

    /// Create a temporary effect mesh with lifetime
    pub fn with_lifetime(mesh: Arc<MeshClass>, config: EffectRenderConfig, lifetime: f32) -> Self {
        Self {
            mesh,
            config,
            lifetime,
            age: 0.0,
        }
    }

    /// Update effect (returns true if still alive)
    pub fn update(&mut self, dt: f32) -> bool {
        self.age += dt;
        self.age < self.lifetime
    }

    /// Check if effect has expired
    pub fn is_expired(&self) -> bool {
        self.age >= self.lifetime
    }

    /// Get alpha based on age/lifetime
    pub fn get_fade_alpha(&self) -> f32 {
        if self.lifetime.is_infinite() {
            1.0
        } else {
            let t = self.age / self.lifetime;
            // Fade out in last 20% of lifetime
            if t > 0.8 {
                1.0 - (t - 0.8) * 5.0
            } else {
                1.0
            }
        }
    }
}

/// Effects manager that queues effects for rendering
pub struct EffectsRenderManager {
    /// Active particle effects
    particle_effects: Vec<EffectMesh>,
    /// Active decal effects
    decal_effects: Vec<EffectMesh>,
    /// Active screen-space effects
    screen_effects: Vec<EffectMesh>,
    /// Active line/trail effects
    line_effects: Vec<EffectMesh>,
    /// Statistics
    stats: EffectStats,
}

impl EffectsRenderManager {
    /// Create a new effects render manager
    pub fn new() -> Self {
        Self {
            particle_effects: Vec::new(),
            decal_effects: Vec::new(),
            screen_effects: Vec::new(),
            line_effects: Vec::new(),
            stats: EffectStats::default(),
        }
    }

    /// Add an effect to the render queue
    pub fn add_effect(&mut self, effect: EffectMesh) {
        match effect.config.effect_type {
            EffectType::Particle => self.particle_effects.push(effect),
            EffectType::Decal => self.decal_effects.push(effect),
            EffectType::ScreenSpace => self.screen_effects.push(effect),
            EffectType::Line | EffectType::Ring => self.line_effects.push(effect),
        }

        self.stats.active_effects += 1;
    }

    /// Update all effects
    pub fn update(&mut self, dt: f32) {
        // Update and remove expired effects
        self.particle_effects.retain_mut(|e| e.update(dt));
        self.decal_effects.retain_mut(|e| e.update(dt));
        self.screen_effects.retain_mut(|e| e.update(dt));
        self.line_effects.retain_mut(|e| e.update(dt));

        // Update stats
        self.stats.active_effects = self.particle_effects.len()
            + self.decal_effects.len()
            + self.screen_effects.len()
            + self.line_effects.len();
    }

    /// Render all effects through the renderer
    pub fn render(&mut self, renderer: &mut Renderer) -> W3DResult<()> {
        self.stats.effects_rendered = 0;

        // Sort effects by depth/priority
        self.sort_effects();

        // Render decals first (depth offset)
        for effect in &self.decal_effects {
            renderer.queue_decal_mesh(Arc::clone(&effect.mesh))?;
            self.stats.effects_rendered += 1;
        }

        // Render particle effects (alpha blended)
        for effect in &self.particle_effects {
            renderer.queue_mesh(Arc::clone(&effect.mesh))?;
            self.stats.effects_rendered += 1;
        }

        // Render line/trail effects
        for effect in &self.line_effects {
            renderer.queue_mesh(Arc::clone(&effect.mesh))?;
            self.stats.effects_rendered += 1;
        }

        // Screen-space effects rendered last
        for effect in &self.screen_effects {
            renderer.queue_mesh(Arc::clone(&effect.mesh))?;
            self.stats.effects_rendered += 1;
        }

        Ok(())
    }

    /// Sort effects for proper rendering order
    fn sort_effects(&mut self) {
        // Sort by sort order (higher values rendered last)
        self.particle_effects.sort_by_key(|e| e.config.sort_order);
        self.decal_effects.sort_by_key(|e| e.config.sort_order);
        self.screen_effects.sort_by_key(|e| e.config.sort_order);
        self.line_effects.sort_by_key(|e| e.config.sort_order);
    }

    /// Clear all effects
    pub fn clear(&mut self) {
        self.particle_effects.clear();
        self.decal_effects.clear();
        self.screen_effects.clear();
        self.line_effects.clear();
        self.stats.active_effects = 0;
    }

    /// Get rendering statistics
    pub fn stats(&self) -> &EffectStats {
        &self.stats
    }
}

impl Default for EffectsRenderManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Effect rendering statistics
#[derive(Debug, Clone, Default)]
pub struct EffectStats {
    pub active_effects: usize,
    pub effects_rendered: usize,
}

/// Helper to create common effect configurations
pub struct EffectPresets;

impl EffectPresets {
    /// Standard particle effect (additive blending, no depth write)
    pub fn particle_additive() -> EffectRenderConfig {
        EffectRenderConfig {
            effect_type: EffectType::Particle,
            blend_mode: BlendMode::Additive,
            depth_write: false,
            depth_test: true,
            sort_order: 100,
            alpha_threshold: 0.01,
        }
    }

    /// Smoke particle effect (alpha blending)
    pub fn particle_smoke() -> EffectRenderConfig {
        EffectRenderConfig {
            effect_type: EffectType::Particle,
            blend_mode: BlendMode::AlphaBlend,
            depth_write: false,
            depth_test: true,
            sort_order: 50,
            alpha_threshold: 0.1,
        }
    }

    /// Decal effect (special depth handling)
    pub fn decal() -> EffectRenderConfig {
        EffectRenderConfig {
            effect_type: EffectType::Decal,
            blend_mode: BlendMode::AlphaBlend,
            depth_write: false,
            depth_test: true,
            sort_order: 0,
            alpha_threshold: 0.01,
        }
    }

    /// Screen flash effect
    pub fn screen_flash() -> EffectRenderConfig {
        EffectRenderConfig {
            effect_type: EffectType::ScreenSpace,
            blend_mode: BlendMode::Additive,
            depth_write: false,
            depth_test: false,
            sort_order: 1000,
            alpha_threshold: 0.0,
        }
    }

    /// Laser/beam effect
    pub fn laser_beam() -> EffectRenderConfig {
        EffectRenderConfig {
            effect_type: EffectType::Line,
            blend_mode: BlendMode::Additive,
            depth_write: false,
            depth_test: true,
            sort_order: 200,
            alpha_threshold: 0.01,
        }
    }

    /// Explosion ring effect
    pub fn explosion_ring() -> EffectRenderConfig {
        EffectRenderConfig {
            effect_type: EffectType::Ring,
            blend_mode: BlendMode::Additive,
            depth_write: false,
            depth_test: true,
            sort_order: 150,
            alpha_threshold: 0.01,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effect_manager_creation() {
        let manager = EffectsRenderManager::new();
        assert_eq!(manager.stats().active_effects, 0);
    }

    #[test]
    fn test_effect_lifetime() {
        let mesh = Arc::new(MeshClass::new());
        let config = EffectPresets::particle_additive();
        let mut effect = EffectMesh::with_lifetime(mesh, config, 1.0);

        assert!(!effect.is_expired());
        assert!(effect.update(0.5));
        assert!(!effect.is_expired());
        assert!(!effect.update(0.6));
        assert!(effect.is_expired());
    }

    #[test]
    fn test_effect_fade_alpha() {
        let mesh = Arc::new(MeshClass::new());
        let config = EffectPresets::particle_smoke();
        let mut effect = EffectMesh::with_lifetime(mesh, config, 1.0);

        // At start, full alpha
        assert_eq!(effect.get_fade_alpha(), 1.0);

        // At 50%, still full alpha
        effect.age = 0.5;
        assert_eq!(effect.get_fade_alpha(), 1.0);

        // At 90%, fading
        effect.age = 0.9;
        assert!(effect.get_fade_alpha() < 1.0);
        assert!(effect.get_fade_alpha() > 0.0);
    }

    #[test]
    fn test_blend_modes() {
        let additive = EffectPresets::particle_additive();
        assert_eq!(additive.blend_mode, BlendMode::Additive);

        let smoke = EffectPresets::particle_smoke();
        assert_eq!(smoke.blend_mode, BlendMode::AlphaBlend);

        let flash = EffectPresets::screen_flash();
        assert!(!flash.depth_test);
    }
}
