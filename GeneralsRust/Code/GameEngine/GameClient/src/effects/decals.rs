//! # Decal System
//!
//! Ground decals, radius decals, and surface markings for Command & Conquer
//! Generals Zero Hour including explosion marks, tire tracks, and scorch marks.

use nalgebra::{Point3, Vector3};
use std::collections::HashMap;
use std::time::Instant;

use super::{EffectsConfig, EffectsError};

/// Unique identifier for decals
pub type DecalId = u32;

/// Types of decals
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecalType {
    /// Explosion scorch marks
    Scorch,
    /// Bullet impact marks
    BulletHole,
    /// Tire/track marks
    TireTrack,
    /// Blood splatter
    Blood,
    /// Oil stains
    Oil,
    /// Crater from large explosions
    Crater,
    /// Generic texture decal
    Generic,
}

/// Decal configuration
#[derive(Debug, Clone)]
pub struct DecalSettings {
    pub decal_type: DecalType,
    pub position: Point3<f32>,
    pub normal: Vector3<f32>,
    pub size: f32,
    pub rotation: f32,
    pub color: [f32; 4],
    pub lifetime: Option<f32>, // None = permanent
    pub fade_time: f32,        // Time to fade out before removal
}

impl DecalSettings {
    pub fn new(decal_type: DecalType, position: Point3<f32>) -> Self {
        Self {
            decal_type,
            position,
            normal: Vector3::new(0.0, 0.0, 1.0), // Up
            size: 1.0,
            rotation: 0.0,
            color: [1.0, 1.0, 1.0, 1.0],
            lifetime: Some(60.0), // 1 minute default
            fade_time: 5.0,
        }
    }

    pub fn scorch_mark(position: Point3<f32>, size: f32) -> Self {
        Self {
            decal_type: DecalType::Scorch,
            position,
            size,
            color: [0.2, 0.1, 0.0, 0.8], // Dark brown
            lifetime: Some(120.0),       // 2 minutes
            fade_time: 10.0,
            ..Self::new(DecalType::Scorch, position)
        }
    }

    pub fn bullet_hole(position: Point3<f32>) -> Self {
        Self {
            decal_type: DecalType::BulletHole,
            position,
            size: 0.1,
            color: [0.1, 0.1, 0.1, 1.0], // Dark gray
            lifetime: Some(300.0),       // 5 minutes
            fade_time: 15.0,
            ..Self::new(DecalType::BulletHole, position)
        }
    }
}

/// Individual decal instance
#[derive(Debug, Clone)]
pub struct Decal {
    pub id: DecalId,
    pub settings: DecalSettings,
    pub age: f32,
    pub alpha: f32,
    pub active: bool,
    pub creation_time: Instant,
}

impl Decal {
    pub fn new(id: DecalId, settings: DecalSettings) -> Self {
        let alpha = settings.color[3];
        Self {
            id,
            settings,
            age: 0.0,
            alpha,
            active: true,
            creation_time: Instant::now(),
        }
    }

    pub fn update(&mut self, delta_time: f32) {
        if !self.active {
            return;
        }

        self.age += delta_time;

        if let Some(lifetime) = self.settings.lifetime {
            if self.age >= lifetime {
                self.active = false;
                return;
            }

            // Start fading near end of life
            let fade_start = lifetime - self.settings.fade_time;
            if self.age >= fade_start {
                let fade_progress = (self.age - fade_start) / self.settings.fade_time;
                self.alpha = self.settings.color[3] * (1.0 - fade_progress);
            }
        }
    }

    pub fn is_alive(&self) -> bool {
        self.active && self.alpha > 0.001
    }
}

#[derive(Debug, Clone)]
pub struct DecalRenderItem {
    pub position: Point3<f32>,
    pub size: f32,
    pub rotation: f32,
    pub color: [f32; 4],
}

/// Special radius-based decals
pub struct RadiusDecal {
    pub center: Point3<f32>,
    pub radius: f32,
    pub decal_type: DecalType,
    pub alpha: f32,
}

impl RadiusDecal {
    pub fn new(center: Point3<f32>, radius: f32, decal_type: DecalType) -> Self {
        Self {
            center,
            radius,
            decal_type,
            alpha: 1.0,
        }
    }
}

/// Decal manager handling all ground decals
pub struct DecalManager {
    decals: HashMap<DecalId, Decal>,
    radius_decals: Vec<RadiusDecal>,
    next_id: DecalId,
    max_decals: usize,
    enabled: bool,
}

impl DecalManager {
    pub fn new() -> Self {
        Self {
            decals: HashMap::new(),
            radius_decals: Vec::new(),
            next_id: 1,
            max_decals: 500,
            enabled: true,
        }
    }

    pub fn create_decal(&mut self, settings: DecalSettings) -> DecalId {
        if !self.enabled {
            return 0;
        }

        let id = self.next_id;
        self.next_id += 1;

        let decal = Decal::new(id, settings);
        self.decals.insert(id, decal);

        // Remove oldest decals if we exceed maximum
        while self.decals.len() > self.max_decals {
            if let Some(oldest_id) = self.find_oldest_decal() {
                self.decals.remove(&oldest_id);
            }
        }

        id
    }

    pub fn create_radius_decal(&mut self, center: Point3<f32>, radius: f32, decal_type: DecalType) {
        if !self.enabled {
            return;
        }

        let radius_decal = RadiusDecal::new(center, radius, decal_type);
        self.radius_decals.push(radius_decal);
    }

    pub fn remove_decal(&mut self, id: DecalId) -> Option<Decal> {
        self.decals.remove(&id)
    }

    pub fn update(&mut self, delta_time: f32, _config: &EffectsConfig) {
        if !self.enabled {
            return;
        }

        // Update all decals
        for decal in self.decals.values_mut() {
            decal.update(delta_time);
        }

        // Remove dead decals
        self.decals.retain(|_, decal| decal.is_alive());

        // Update radius decals (simple fade for now)
        for radius_decal in &mut self.radius_decals {
            radius_decal.alpha = (radius_decal.alpha - delta_time * 0.1).max(0.0);
        }

        // Remove faded radius decals
        self.radius_decals.retain(|rd| rd.alpha > 0.001);
    }

    pub fn clear_all(&mut self) {
        self.decals.clear();
        self.radius_decals.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.clear_all();
        }
    }

    pub fn set_max_decals(&mut self, max: usize) {
        self.max_decals = max;

        // Remove excess decals if new limit is lower
        while self.decals.len() > self.max_decals {
            if let Some(oldest_id) = self.find_oldest_decal() {
                self.decals.remove(&oldest_id);
            }
        }
    }

    pub fn active_decal_count(&self) -> usize {
        self.decals.len()
    }

    pub fn active_radius_decal_count(&self) -> usize {
        self.radius_decals.len()
    }

    fn find_oldest_decal(&self) -> Option<DecalId> {
        self.decals
            .iter()
            .min_by_key(|(_, decal)| decal.creation_time)
            .map(|(id, _)| *id)
    }

    // Access methods for rendering
    pub fn decals(&self) -> &HashMap<DecalId, Decal> {
        &self.decals
    }

    pub fn radius_decals(&self) -> &[RadiusDecal] {
        &self.radius_decals
    }

    pub fn collect_render_items(&self) -> Vec<DecalRenderItem> {
        let mut items = Vec::new();

        for decal in self.decals.values() {
            if !decal.is_alive() {
                continue;
            }
            let color = [
                decal.settings.color[0],
                decal.settings.color[1],
                decal.settings.color[2],
                decal.alpha,
            ];
            items.push(DecalRenderItem {
                position: decal.settings.position,
                size: decal.settings.size,
                rotation: decal.settings.rotation,
                color,
            });
        }

        for radius in &self.radius_decals {
            let color = match radius.decal_type {
                DecalType::Scorch => [0.2, 0.1, 0.0, radius.alpha],
                DecalType::BulletHole => [0.1, 0.1, 0.1, radius.alpha],
                DecalType::TireTrack => [0.15, 0.1, 0.05, radius.alpha],
                DecalType::Blood => [0.5, 0.05, 0.05, radius.alpha],
                DecalType::Oil => [0.05, 0.05, 0.05, radius.alpha],
                DecalType::Crater => [0.2, 0.15, 0.1, radius.alpha],
                DecalType::Generic => [1.0, 1.0, 1.0, radius.alpha],
            };
            items.push(DecalRenderItem {
                position: radius.center,
                size: radius.radius * 2.0,
                rotation: 0.0,
                color,
            });
        }

        items
    }
}

impl Default for DecalManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decal_creation() {
        let settings = DecalSettings::scorch_mark(Point3::new(10.0, 20.0, 0.0), 2.0);
        let decal = Decal::new(1, settings.clone());

        assert_eq!(decal.id, 1);
        assert_eq!(decal.settings.decal_type, DecalType::Scorch);
        assert_eq!(decal.settings.position, Point3::new(10.0, 20.0, 0.0));
        assert_eq!(decal.settings.size, 2.0);
        assert!(decal.is_alive());
    }

    #[test]
    fn test_decal_aging() {
        let settings = DecalSettings {
            lifetime: Some(2.0),
            fade_time: 1.0,
            ..DecalSettings::bullet_hole(Point3::new(0.0, 0.0, 0.0))
        };

        let mut decal = Decal::new(1, settings);
        let initial_alpha = decal.alpha;

        // Age to fade start
        decal.update(1.0); // lifetime - fade_time = 2.0 - 1.0 = 1.0
        assert!(decal.is_alive());
        assert_eq!(decal.alpha, initial_alpha); // Not fading yet

        // Age into fade period
        decal.update(0.5); // Now at 1.5 seconds, halfway through fade
        assert!(decal.alpha < initial_alpha);
        assert!(decal.is_alive());

        // Age to death
        decal.update(0.5); // Now at 2.0 seconds, fully aged
        assert!(!decal.is_alive());
    }

    #[test]
    fn test_decal_manager() {
        let mut manager = DecalManager::new();

        // Create decal
        let settings = DecalSettings::scorch_mark(Point3::new(0.0, 0.0, 0.0), 1.0);
        let id = manager.create_decal(settings);
        assert!(id > 0);
        assert_eq!(manager.active_decal_count(), 1);

        // Create radius decal
        manager.create_radius_decal(Point3::new(5.0, 5.0, 0.0), 3.0, DecalType::Crater);
        assert_eq!(manager.active_radius_decal_count(), 1);

        // Remove decal
        let removed = manager.remove_decal(id);
        assert!(removed.is_some());
        assert_eq!(manager.active_decal_count(), 0);
    }

    #[test]
    fn test_decal_manager_limits() {
        let mut manager = DecalManager::new();
        manager.set_max_decals(2);

        // Create 3 decals
        for i in 0..3 {
            let settings = DecalSettings::bullet_hole(Point3::new(i as f32, 0.0, 0.0));
            manager.create_decal(settings);
        }

        // Should only have 2 decals due to limit
        assert_eq!(manager.active_decal_count(), 2);
    }

    #[test]
    fn test_radius_decal() {
        let radius_decal = RadiusDecal::new(Point3::new(1.0, 2.0, 3.0), 5.0, DecalType::Crater);

        assert_eq!(radius_decal.center, Point3::new(1.0, 2.0, 3.0));
        assert_eq!(radius_decal.radius, 5.0);
        assert_eq!(radius_decal.decal_type, DecalType::Crater);
        assert_eq!(radius_decal.alpha, 1.0);
    }
}
