//! Damage Feedback — C++ FXList Parity
//!
//! PARITY_NOTE: The following fabricated mechanics have been removed because they
//! have NO equivalent in the C++ GeneralsMD codebase:
//!   - ScreenShake system (ShakeIntensity, ScreenShake struct with oscillation,
//!     exponential decay, distance falloff, camera offset calculation)
//!   - HitMarker system (HitMarkerType, HitMarker with damage numbers,
//!     alpha fade, display durations)
//!
//! In C++, damage visual/audio feedback is handled by FXList::doFXPos() which
//! triggers particle effects and sounds at a position. There is no persistent
//! screen shake state machine or hit marker overlay system.
//!
//! The DamageSoundEffect struct is retained because C++ does trigger sounds
//! via FXList on damage events, though the actual sound IDs would come from
//! INI data, not hardcoded here.

use std::collections::VecDeque;
use std::sync::RwLock;

use crate::common::Coord3D;
use crate::weapon::{DamageType, DeathType};
use crate::{GameLogicError, GameLogicResult};

// ---------------------------------------------------------------------------
// Stub types for removed fabricated systems
// ---------------------------------------------------------------------------

/// PARITY_NOTE: Fabricated — C++ has no screen shake intensity levels.
/// Kept as stub for API compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShakeIntensity {
    Light,
}

/// PARITY_NOTE: Fabricated — C++ uses FXList::doFXPos, not a persistent
/// ScreenShake state machine. Kept as empty stub for API compatibility.
#[derive(Debug, Clone)]
pub struct ScreenShake {}

impl ScreenShake {
    pub fn new(_origin: Coord3D, _intensity: ShakeIntensity, _current_frame: u32) -> Self {
        Self {}
    }

    pub fn global(_intensity: ShakeIntensity, _current_frame: u32) -> Self {
        Self {}
    }
}

/// PARITY_NOTE: Fabricated — C++ has no hit marker overlay system.
/// Kept as stub for API compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitMarkerType {
    Normal,
}

/// PARITY_NOTE: Fabricated — kept as empty stub.
#[derive(Debug, Clone)]
pub struct HitMarker {}

impl HitMarker {
    pub fn new(
        _position: Coord3D,
        _marker_type: HitMarkerType,
        _damage_amount: f32,
        _current_frame: u32,
    ) -> Self {
        Self {}
    }
}

// ---------------------------------------------------------------------------
// Sound effects — retained (C++ does trigger sounds via FXList)
// ---------------------------------------------------------------------------

/// Sound effect for damage event.
///
/// PARITY_NOTE: In C++, sound names come from INI FXList entries, not from
/// hardcoded mappings. The for_damage_type / for_death_type constructors are
/// approximate stubs.
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
        let sound_id = match damage_type {
            DamageType::Explosion => "explosion_large",
            DamageType::SmallArms => "gunfire_small",
            DamageType::Flame => "fire_whoosh",
            DamageType::Laser => "laser_beam",
            _ => "impact_generic",
        };

        Self {
            sound_id: sound_id.to_string(),
            position,
            volume: 0.5,
            looping: false,
        }
    }

    /// Get sound for death type
    pub fn for_death_type(death_type: DeathType, position: Coord3D) -> Self {
        let sound_id = match death_type {
            DeathType::Exploded => "death_explosion",
            DeathType::Burned => "death_burning",
            DeathType::Crushed => "death_crushed",
            _ => "death_generic",
        };

        Self {
            sound_id: sound_id.to_string(),
            position,
            volume: 0.6,
            looping: false,
        }
    }
}

// ---------------------------------------------------------------------------
// DamageFeedbackManager — simplified to sound FX only
// ---------------------------------------------------------------------------

/// Damage feedback manager.
///
/// PARITY_NOTE: Simplified from the fabricated version. C++ triggers visual FX
/// via FXList::doFXPos() at the damage location. This manager now only queues
/// sound effects, matching the C++ approach of delegating visuals to the FX system.
#[derive(Debug)]
pub struct DamageFeedbackManager {
    /// Pending sound effects to play
    sound_effects: RwLock<VecDeque<DamageSoundEffect>>,
}

impl DamageFeedbackManager {
    /// Create new feedback manager
    pub fn new() -> Self {
        Self {
            sound_effects: RwLock::new(VecDeque::new()),
        }
    }

    /// Queue sound effect
    pub fn queue_sound(&self, sound: DamageSoundEffect) -> GameLogicResult<()> {
        let mut sounds = self.sound_effects.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire sounds lock: {}", e))
        })?;
        sounds.push_back(sound);
        Ok(())
    }

    /// Get and clear pending sound effects
    pub fn consume_sound_effects(&self) -> GameLogicResult<Vec<DamageSoundEffect>> {
        let mut sounds = self.sound_effects.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire sounds lock: {}", e))
        })?;
        let effects: Vec<_> = sounds.drain(..).collect();
        Ok(effects)
    }

    /// Add feedback for damage event.
    ///
    /// PARITY_NOTE: Simplified — no screen shake or hit markers.
    /// Only queues a sound effect, matching C++ FXList behavior.
    pub fn add_damage_feedback(
        &self,
        damage_type: DamageType,
        position: Coord3D,
        _damage_amount: f32,
        _is_critical: bool,
        _is_kill: bool,
    ) -> GameLogicResult<()> {
        let sound = DamageSoundEffect::for_damage_type(damage_type, position);
        self.queue_sound(sound)?;
        Ok(())
    }

    /// Stub methods for removed fabricated systems.
    pub fn set_current_frame(&self, _frame: u32) -> GameLogicResult<()> {
        Ok(())
    }

    pub fn get_current_frame(&self) -> GameLogicResult<u32> {
        Ok(0)
    }

    pub fn add_screen_shake(&self, _shake: ScreenShake) -> GameLogicResult<()> {
        // PARITY_NOTE: No-op — screen shake is fabricated
        Ok(())
    }

    pub fn add_hit_marker(&self, _marker: HitMarker) -> GameLogicResult<()> {
        // PARITY_NOTE: No-op — hit markers are fabricated
        Ok(())
    }

    pub fn get_active_shakes(&self) -> GameLogicResult<Vec<ScreenShake>> {
        Ok(Vec::new())
    }

    pub fn calculate_camera_shake(&self, _camera_pos: &Coord3D) -> GameLogicResult<Coord3D> {
        Ok(Coord3D::ZERO)
    }

    pub fn get_active_markers(&self) -> GameLogicResult<Vec<HitMarker>> {
        Ok(Vec::new())
    }

    /// Clear all feedback
    pub fn clear_all(&self) -> GameLogicResult<()> {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_damage_feedback_manager_sound_only() {
        let manager = DamageFeedbackManager::new();

        manager
            .add_damage_feedback(
                DamageType::Explosion,
                Coord3D::new(100.0, 100.0, 0.0),
                150.0,
                false,
                false,
            )
            .unwrap();

        let sounds = manager.consume_sound_effects().unwrap();
        assert_eq!(sounds.len(), 1);
        assert_eq!(sounds[0].sound_id, "explosion_large");

        // Should be empty after consume
        let sounds2 = manager.consume_sound_effects().unwrap();
        assert_eq!(sounds2.len(), 0);
    }

    #[test]
    fn test_stub_methods_no_panic() {
        let manager = DamageFeedbackManager::new();

        // These should all be no-ops and not panic
        manager.set_current_frame(0).unwrap();
        assert_eq!(manager.get_current_frame().unwrap(), 0);
        manager.add_screen_shake(ScreenShake::new(Coord3D::ZERO, ShakeIntensity::Light, 0)).unwrap();
        manager.add_hit_marker(HitMarker::new(Coord3D::ZERO, HitMarkerType::Normal, 0.0, 0)).unwrap();
        assert!(manager.get_active_shakes().unwrap().is_empty());
        assert!(manager.get_active_markers().unwrap().is_empty());
        assert_eq!(manager.calculate_camera_shake(&Coord3D::ZERO).unwrap(), Coord3D::ZERO);
    }
}
