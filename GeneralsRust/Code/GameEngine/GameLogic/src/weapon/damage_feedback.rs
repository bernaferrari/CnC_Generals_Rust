//! Damage Feedback System
//!
//! This module implements visual and audio feedback for damage events:
//! - Screen shake effects
//! - Camera trauma
//! - Hit markers and damage numbers
//! - Sound effects based on damage type
//! - Visual effects (sparks, blood, explosions)

use std::collections::VecDeque;
use std::sync::RwLock;

use crate::common::{Coord3D, ObjectID};
use crate::weapon::{DamageType, DeathType};
use crate::{GameLogicError, GameLogicResult};

/// Screen shake intensity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShakeIntensity {
    /// Light shake (small arms fire nearby)
    Light,
    /// Medium shake (explosions, vehicle damage)
    Medium,
    /// Heavy shake (building destruction, large explosions)
    Heavy,
    /// Extreme shake (superweapons, nuclear explosions)
    Extreme,
}

impl ShakeIntensity {
    /// Get shake magnitude multiplier
    pub fn magnitude(&self) -> f32 {
        match self {
            ShakeIntensity::Light => 1.0,
            ShakeIntensity::Medium => 2.5,
            ShakeIntensity::Heavy => 5.0,
            ShakeIntensity::Extreme => 10.0,
        }
    }

    /// Get shake duration in frames (30 FPS)
    pub fn duration_frames(&self) -> u32 {
        match self {
            ShakeIntensity::Light => 10,   // ~0.33 seconds
            ShakeIntensity::Medium => 20,  // ~0.67 seconds
            ShakeIntensity::Heavy => 30,   // 1 second
            ShakeIntensity::Extreme => 60, // 2 seconds
        }
    }

    /// Get shake frequency (higher = faster oscillation)
    pub fn frequency(&self) -> f32 {
        match self {
            ShakeIntensity::Light => 10.0,
            ShakeIntensity::Medium => 15.0,
            ShakeIntensity::Heavy => 20.0,
            ShakeIntensity::Extreme => 25.0,
        }
    }
}

/// Screen shake event
#[derive(Debug, Clone)]
pub struct ScreenShake {
    /// Position where shake originated
    pub origin: Coord3D,
    /// Intensity of shake
    pub intensity: ShakeIntensity,
    /// Custom magnitude override
    pub magnitude: f32,
    /// Frame when shake started
    pub start_frame: u32,
    /// Duration in frames
    pub duration_frames: u32,
    /// Whether shake affects all players or just nearby camera
    pub global: bool,
    /// Falloff radius (for non-global shakes)
    pub falloff_radius: f32,
}

impl ScreenShake {
    /// Create new screen shake
    pub fn new(origin: Coord3D, intensity: ShakeIntensity, current_frame: u32) -> Self {
        Self {
            origin,
            intensity,
            magnitude: intensity.magnitude(),
            start_frame: current_frame,
            duration_frames: intensity.duration_frames(),
            global: false,
            falloff_radius: 500.0, // Default falloff radius
        }
    }

    /// Create global shake (affects all players)
    pub fn global(intensity: ShakeIntensity, current_frame: u32) -> Self {
        Self {
            origin: Coord3D::new(0.0, 0.0, 0.0),
            intensity,
            magnitude: intensity.magnitude(),
            start_frame: current_frame,
            duration_frames: intensity.duration_frames(),
            global: true,
            falloff_radius: 0.0,
        }
    }

    /// Check if shake is still active
    pub fn is_active(&self, current_frame: u32) -> bool {
        current_frame < self.start_frame + self.duration_frames
    }

    /// Get shake strength at current frame (0.0 to 1.0)
    pub fn get_strength(&self, current_frame: u32) -> f32 {
        if !self.is_active(current_frame) {
            return 0.0;
        }

        let elapsed = current_frame - self.start_frame;
        let progress = elapsed as f32 / self.duration_frames as f32;

        // Exponential decay
        (1.0 - progress).powi(2)
    }

    /// Calculate shake offset at camera position
    pub fn calculate_offset(&self, current_frame: u32, camera_pos: &Coord3D) -> Coord3D {
        if !self.is_active(current_frame) {
            return Coord3D::new(0.0, 0.0, 0.0);
        }

        let mut strength = self.get_strength(current_frame);

        // Apply distance falloff if not global
        if !self.global {
            let distance = self.origin.distance(*camera_pos);
            if distance > self.falloff_radius {
                return Coord3D::new(0.0, 0.0, 0.0);
            }
            let falloff = 1.0 - (distance / self.falloff_radius);
            strength *= falloff;
        }

        // Calculate oscillating offset using sine wave
        let elapsed = current_frame - self.start_frame;
        let frequency = self.intensity.frequency();
        let angle = elapsed as f32 * frequency * 0.1;

        let x = angle.sin() * self.magnitude * strength;
        let y = (angle * 1.3).cos() * self.magnitude * strength;
        let z = (angle * 0.7).sin() * self.magnitude * strength * 0.5;

        Coord3D::new(x, y, z)
    }
}

/// Hit marker type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitMarkerType {
    /// Standard hit
    Normal,
    /// Critical hit
    Critical,
    /// Headshot
    Headshot,
    /// Killing blow
    Kill,
    /// Armor deflection
    Deflected,
}

/// Hit marker event
#[derive(Debug, Clone)]
pub struct HitMarker {
    /// Position of hit
    pub position: Coord3D,
    /// Type of hit
    pub marker_type: HitMarkerType,
    /// Damage amount (for displaying damage numbers)
    pub damage_amount: f32,
    /// Frame when marker was created
    pub frame: u32,
    /// Duration to display (frames)
    pub display_duration: u32,
}

impl HitMarker {
    /// Create new hit marker
    pub fn new(
        position: Coord3D,
        marker_type: HitMarkerType,
        damage_amount: f32,
        current_frame: u32,
    ) -> Self {
        let display_duration = match marker_type {
            HitMarkerType::Normal => 30,    // 1 second
            HitMarkerType::Critical => 45,  // 1.5 seconds
            HitMarkerType::Headshot => 60,  // 2 seconds
            HitMarkerType::Kill => 90,      // 3 seconds
            HitMarkerType::Deflected => 20, // 0.67 seconds
        };

        Self {
            position,
            marker_type,
            damage_amount,
            frame: current_frame,
            display_duration,
        }
    }

    /// Check if marker should still be displayed
    pub fn is_active(&self, current_frame: u32) -> bool {
        current_frame < self.frame + self.display_duration
    }

    /// Get fade alpha (1.0 at start, 0.0 at end)
    pub fn get_alpha(&self, current_frame: u32) -> f32 {
        if !self.is_active(current_frame) {
            return 0.0;
        }

        let elapsed = current_frame - self.frame;
        let progress = elapsed as f32 / self.display_duration as f32;

        1.0 - progress
    }
}

/// Sound effect for damage event
#[derive(Debug, Clone)]
pub struct DamageSoundEffect {
    /// Sound name/ID to play
    pub sound_id: String,
    /// Position to play sound at
    pub position: Coord3D,
    /// Volume multiplier (0.0 to 1.0)
    pub volume: f32,
    /// Whether sound loops
    pub looping: bool,
}

impl DamageSoundEffect {
    /// Get sound for damage type
    pub fn for_damage_type(damage_type: DamageType, position: Coord3D) -> Self {
        let (sound_id, volume) = match damage_type {
            DamageType::Explosion => ("explosion_large".to_string(), 1.0),
            DamageType::SmallArms => ("gunfire_small".to_string(), 0.5),
            DamageType::Flame => ("fire_whoosh".to_string(), 0.7),
            DamageType::Laser => ("laser_beam".to_string(), 0.6),
            DamageType::Sniper => ("sniper_shot".to_string(), 0.8),
            DamageType::Poison => ("poison_hiss".to_string(), 0.4),
            DamageType::Radiation => ("geiger_counter".to_string(), 0.5),
            DamageType::ParticleBeam => ("particle_beam_fire".to_string(), 0.9),
            DamageType::Crush => ("vehicle_crush".to_string(), 0.7),
            _ => ("impact_generic".to_string(), 0.5),
        };

        Self {
            sound_id,
            position,
            volume,
            looping: false,
        }
    }

    /// Get sound for death type
    pub fn for_death_type(death_type: DeathType, position: Coord3D) -> Self {
        let (sound_id, volume) = match death_type {
            DeathType::Exploded => ("death_explosion".to_string(), 1.0),
            DeathType::Burned => ("death_burning".to_string(), 0.8),
            DeathType::Crushed => ("death_crushed".to_string(), 0.7),
            DeathType::Poisoned => ("death_poison".to_string(), 0.6),
            DeathType::Lasered => ("death_laser".to_string(), 0.7),
            _ => ("death_generic".to_string(), 0.6),
        };

        Self {
            sound_id,
            position,
            volume,
            looping: false,
        }
    }
}

/// Damage feedback manager
#[derive(Debug)]
pub struct DamageFeedbackManager {
    /// Active screen shakes
    screen_shakes: RwLock<VecDeque<ScreenShake>>,
    /// Active hit markers
    hit_markers: RwLock<VecDeque<HitMarker>>,
    /// Pending sound effects to play
    sound_effects: RwLock<VecDeque<DamageSoundEffect>>,
    /// Current game frame
    current_frame: RwLock<u32>,
    /// Maximum number of active shakes
    max_shakes: usize,
    /// Maximum number of active markers
    max_markers: usize,
}

impl DamageFeedbackManager {
    /// Create new feedback manager
    pub fn new() -> Self {
        Self {
            screen_shakes: RwLock::new(VecDeque::new()),
            hit_markers: RwLock::new(VecDeque::new()),
            sound_effects: RwLock::new(VecDeque::new()),
            current_frame: RwLock::new(0),
            max_shakes: 10,
            max_markers: 50,
        }
    }

    /// Update current frame
    pub fn set_current_frame(&self, frame: u32) -> GameLogicResult<()> {
        let mut current = self.current_frame.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire frame lock: {}", e))
        })?;
        *current = frame;
        Ok(())
    }

    /// Get current frame
    pub fn get_current_frame(&self) -> GameLogicResult<u32> {
        let current = self.current_frame.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire frame lock: {}", e))
        })?;
        Ok(*current)
    }

    /// Add screen shake
    pub fn add_screen_shake(&self, shake: ScreenShake) -> GameLogicResult<()> {
        let mut shakes = self.screen_shakes.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire shakes lock: {}", e))
        })?;

        // Remove oldest if at capacity
        if shakes.len() >= self.max_shakes {
            shakes.pop_front();
        }

        shakes.push_back(shake);
        Ok(())
    }

    /// Add hit marker
    pub fn add_hit_marker(&self, marker: HitMarker) -> GameLogicResult<()> {
        let mut markers = self.hit_markers.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire markers lock: {}", e))
        })?;

        // Remove oldest if at capacity
        if markers.len() >= self.max_markers {
            markers.pop_front();
        }

        markers.push_back(marker);
        Ok(())
    }

    /// Queue sound effect
    pub fn queue_sound(&self, sound: DamageSoundEffect) -> GameLogicResult<()> {
        let mut sounds = self.sound_effects.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire sounds lock: {}", e))
        })?;

        sounds.push_back(sound);
        Ok(())
    }

    /// Update and get active screen shakes
    pub fn get_active_shakes(&self) -> GameLogicResult<Vec<ScreenShake>> {
        let current_frame = self.get_current_frame()?;

        let mut shakes = self.screen_shakes.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire shakes lock: {}", e))
        })?;

        // Remove expired shakes
        shakes.retain(|shake| shake.is_active(current_frame));

        Ok(shakes.iter().cloned().collect())
    }

    /// Calculate total camera offset from all shakes
    pub fn calculate_camera_shake(&self, camera_pos: &Coord3D) -> GameLogicResult<Coord3D> {
        let current_frame = self.get_current_frame()?;
        let shakes = self.get_active_shakes()?;

        let mut total_offset = Coord3D::new(0.0, 0.0, 0.0);

        for shake in shakes {
            let offset = shake.calculate_offset(current_frame, camera_pos);
            total_offset.x += offset.x;
            total_offset.y += offset.y;
            total_offset.z += offset.z;
        }

        Ok(total_offset)
    }

    /// Get active hit markers
    pub fn get_active_markers(&self) -> GameLogicResult<Vec<HitMarker>> {
        let current_frame = self.get_current_frame()?;

        let mut markers = self.hit_markers.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire markers lock: {}", e))
        })?;

        // Remove expired markers
        markers.retain(|marker| marker.is_active(current_frame));

        Ok(markers.iter().cloned().collect())
    }

    /// Get and clear pending sound effects
    pub fn consume_sound_effects(&self) -> GameLogicResult<Vec<DamageSoundEffect>> {
        let mut sounds = self.sound_effects.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire sounds lock: {}", e))
        })?;

        let effects: Vec<_> = sounds.drain(..).collect();
        Ok(effects)
    }

    /// Add feedback for damage event
    pub fn add_damage_feedback(
        &self,
        damage_type: DamageType,
        position: Coord3D,
        damage_amount: f32,
        is_critical: bool,
        is_kill: bool,
    ) -> GameLogicResult<()> {
        let current_frame = self.get_current_frame()?;

        // Determine shake intensity based on damage type and amount
        let shake_intensity = if damage_amount > 500.0 {
            ShakeIntensity::Extreme
        } else if damage_amount > 200.0 {
            ShakeIntensity::Heavy
        } else if damage_amount > 50.0 {
            ShakeIntensity::Medium
        } else {
            ShakeIntensity::Light
        };

        // Add screen shake for significant damage
        if damage_amount > 10.0 {
            let shake = ScreenShake::new(position, shake_intensity, current_frame);
            self.add_screen_shake(shake)?;
        }

        // Add hit marker
        let marker_type = if is_kill {
            HitMarkerType::Kill
        } else if is_critical {
            HitMarkerType::Critical
        } else {
            HitMarkerType::Normal
        };

        let marker = HitMarker::new(position, marker_type, damage_amount, current_frame);
        self.add_hit_marker(marker)?;

        // Queue sound effect
        let sound = DamageSoundEffect::for_damage_type(damage_type, position);
        self.queue_sound(sound)?;

        Ok(())
    }

    /// Clear all feedback
    pub fn clear_all(&self) -> GameLogicResult<()> {
        let mut shakes = self.screen_shakes.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire shakes lock: {}", e))
        })?;
        shakes.clear();

        let mut markers = self.hit_markers.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire markers lock: {}", e))
        })?;
        markers.clear();

        let mut sounds = self.sound_effects.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire sounds lock: {}", e))
        })?;
        sounds.clear();

        Ok(())
    }
}

impl Default for DamageFeedbackManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shake_intensity_values() {
        assert_eq!(ShakeIntensity::Light.magnitude(), 1.0);
        assert_eq!(ShakeIntensity::Medium.magnitude(), 2.5);
        assert_eq!(ShakeIntensity::Heavy.magnitude(), 5.0);
        assert_eq!(ShakeIntensity::Extreme.magnitude(), 10.0);
    }

    #[test]
    fn test_screen_shake_creation() {
        let origin = Coord3D::new(100.0, 100.0, 0.0);
        let shake = ScreenShake::new(origin, ShakeIntensity::Medium, 0);

        assert_eq!(shake.intensity, ShakeIntensity::Medium);
        assert_eq!(shake.start_frame, 0);
        assert!(!shake.global);
    }

    #[test]
    fn test_screen_shake_active() {
        let shake = ScreenShake::new(Coord3D::new(0.0, 0.0, 0.0), ShakeIntensity::Light, 0);

        assert!(shake.is_active(0));
        assert!(shake.is_active(9));
        assert!(!shake.is_active(10));
    }

    #[test]
    fn test_screen_shake_strength_decay() {
        let shake = ScreenShake::new(Coord3D::new(0.0, 0.0, 0.0), ShakeIntensity::Light, 0);

        let strength_0 = shake.get_strength(0);
        let strength_5 = shake.get_strength(5);
        let strength_10 = shake.get_strength(10);

        assert_eq!(strength_0, 1.0);
        assert!(strength_5 > 0.0 && strength_5 < 1.0);
        assert_eq!(strength_10, 0.0);
    }

    #[test]
    fn test_hit_marker_creation() {
        let pos = Coord3D::new(50.0, 50.0, 0.0);
        let marker = HitMarker::new(pos, HitMarkerType::Critical, 150.0, 0);

        assert_eq!(marker.marker_type, HitMarkerType::Critical);
        assert_eq!(marker.damage_amount, 150.0);
        assert!(marker.is_active(0));
    }

    #[test]
    fn test_damage_feedback_manager() {
        let manager = DamageFeedbackManager::new();
        manager.set_current_frame(0).unwrap();

        let shake = ScreenShake::new(Coord3D::new(0.0, 0.0, 0.0), ShakeIntensity::Medium, 0);
        manager.add_screen_shake(shake).unwrap();

        let shakes = manager.get_active_shakes().unwrap();
        assert_eq!(shakes.len(), 1);
    }

    #[test]
    fn test_camera_shake_calculation() {
        let manager = DamageFeedbackManager::new();
        manager.set_current_frame(0).unwrap();

        let shake = ScreenShake::new(Coord3D::new(0.0, 0.0, 0.0), ShakeIntensity::Medium, 0);
        manager.add_screen_shake(shake).unwrap();

        let camera_pos = Coord3D::new(0.0, 0.0, 0.0);
        let offset = manager.calculate_camera_shake(&camera_pos).unwrap();

        // Should have some offset
        assert!(offset.x != 0.0 || offset.y != 0.0);
    }

    #[test]
    fn test_sound_effects_queue() {
        let manager = DamageFeedbackManager::new();

        let sound =
            DamageSoundEffect::for_damage_type(DamageType::Explosion, Coord3D::new(0.0, 0.0, 0.0));
        manager.queue_sound(sound).unwrap();

        let sounds = manager.consume_sound_effects().unwrap();
        assert_eq!(sounds.len(), 1);
        assert_eq!(sounds[0].sound_id, "explosion_large");

        // Should be empty after consume
        let sounds2 = manager.consume_sound_effects().unwrap();
        assert_eq!(sounds2.len(), 0);
    }

    #[test]
    fn test_add_damage_feedback() {
        let manager = DamageFeedbackManager::new();
        manager.set_current_frame(0).unwrap();

        manager
            .add_damage_feedback(
                DamageType::Explosion,
                Coord3D::new(100.0, 100.0, 0.0),
                150.0,
                true,
                false,
            )
            .unwrap();

        let shakes = manager.get_active_shakes().unwrap();
        assert_eq!(shakes.len(), 1);

        let markers = manager.get_active_markers().unwrap();
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].marker_type, HitMarkerType::Critical);

        let sounds = manager.consume_sound_effects().unwrap();
        assert_eq!(sounds.len(), 1);
    }
}
