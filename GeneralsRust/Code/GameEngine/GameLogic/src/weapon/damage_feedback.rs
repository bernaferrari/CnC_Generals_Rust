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
//! Camera shake is triggered through ViewShakeFXNugget (FXList.cpp:359-414)
//! which calls TheTacticalView->shake(). This is already wired in the Rust
//! fx_list.rs via register_camera_shake_system() and the ViewShakeNugget
//! implementation that calls CameraShakeSystem::add_camera_shake().
//!
//! Audio on damage is triggered through SoundFXNugget in FXList, which calls
//! TheAudio->addAudioEvent(). The Rust fx_list.rs has register_fx_audio() for
//! this hook, and AudioEventDispatcher in audio_events.rs handles unit/building
//! audio events (Damaged, Died, etc.) via GameAudioManager.
//!
//! The DamageSoundEffect struct is retained because C++ does trigger sounds
//! via FXList on damage events, though the actual sound IDs would come from
//! INI data, not hardcoded here.

use crate::common::Coord3D;
use crate::weapon::{DamageType, DeathType};
use crate::GameLogicResult;

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
    pub fn for_damage_type(_damage_type: DamageType, position: Coord3D) -> Self {
        Self {
            // C++ parity: this is data-driven through FXList/INI. Keep empty in stub mode.
            sound_id: String::new(),
            position,
            volume: 1.0,
            looping: false,
        }
    }

    /// Get sound for death type
    pub fn for_death_type(_death_type: DeathType, position: Coord3D) -> Self {
        Self {
            // C++ parity: this is data-driven through FXList/INI. Keep empty in stub mode.
            sound_id: String::new(),
            position,
            volume: 1.0,
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
pub struct DamageFeedbackManager {}

impl DamageFeedbackManager {
    /// Create new feedback manager
    pub fn new() -> Self {
        Self {}
    }

    /// Queue sound effect.
    ///
    /// PARITY_NOTE: No-op — C++ does not have a DamageFeedbackManager queue.
    /// Sound is dispatched directly by FXList::SoundFXNugget via TheAudio->addAudioEvent().
    /// Use fx_list::register_fx_audio() to hook into the real audio pipeline.
    pub fn queue_sound(&self, _sound: DamageSoundEffect) -> GameLogicResult<()> {
        Ok(())
    }

    /// Get and clear pending sound effects.
    ///
    /// PARITY_NOTE: Always returns empty — no internal queue exists in C++ either.
    pub fn consume_sound_effects(&self) -> GameLogicResult<Vec<DamageSoundEffect>> {
        Ok(Vec::new())
    }

    /// Add feedback for damage event.
    ///
    /// PARITY_NOTE: No-op — C++ has no DamageFeedbackManager. Damage audio/visual
    /// feedback is triggered by FXList execution on weapon impact. The relevant
    /// FXList nuggets are:
    ///   - ViewShakeFXNugget (FXList.cpp:359) → camera shake via TacticalView::shake()
    ///   - SoundFXNugget (FXList.cpp) → audio via TheAudio->addAudioEvent()
    /// Both are already wired in Rust fx_list.rs via register_camera_shake_system()
    /// and register_fx_audio().
    pub fn add_damage_feedback(
        &self,
        _damage_type: DamageType,
        _position: Coord3D,
        _damage_amount: f32,
        _is_critical: bool,
        _is_kill: bool,
    ) -> GameLogicResult<()> {
        Ok(())
    }

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

        // Stub mode should not emit hardcoded sound effects.
        let sounds = manager.consume_sound_effects().unwrap();
        assert!(sounds.is_empty());
    }

    #[test]
    fn test_stub_methods_no_panic() {
        let manager = DamageFeedbackManager::new();

        // These should all be no-ops and not panic
        manager.set_current_frame(0).unwrap();
        assert_eq!(manager.get_current_frame().unwrap(), 0);
        manager
            .add_screen_shake(ScreenShake::new(Coord3D::ZERO, ShakeIntensity::Light, 0))
            .unwrap();
        manager
            .add_hit_marker(HitMarker::new(Coord3D::ZERO, HitMarkerType::Normal, 0.0, 0))
            .unwrap();
        assert!(manager.get_active_shakes().unwrap().is_empty());
        assert!(manager.get_active_markers().unwrap().is_empty());
        assert_eq!(
            manager.calculate_camera_shake(&Coord3D::ZERO).unwrap(),
            Coord3D::ZERO
        );
    }
}
