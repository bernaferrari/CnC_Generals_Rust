//! Stealth State Management System
//!
//! Tracks stealth states, detection states, and visibility for objects

use crate::common::*;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Visibility state of a stealthed object
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityState {
    /// Fully visible (not stealthed)
    Visible,
    /// Stealthed and not detected
    Hidden,
    /// Stealthed but detected by some players
    Detected,
    /// Transitioning into stealth
    Stealthing,
    /// Transitioning out of stealth
    Unstealthing,
}

/// Detection state for an object
#[derive(Debug, Clone)]
pub struct DetectionState {
    /// Frame when detection started
    pub detected_frame: u32,
    /// ID of the detecting object
    pub detector_id: ObjectID,
    /// Detection strength (0.0 = barely detected, 1.0 = fully detected)
    pub detection_strength: f32,
    /// Whether this detection has been reported to player
    pub reported: bool,
}

impl DetectionState {
    pub fn new(detector_id: ObjectID, frame: u32) -> Self {
        Self {
            detected_frame: frame,
            detector_id,
            detection_strength: 1.0,
            reported: false,
        }
    }
}

/// Manages stealth state for a single object
#[derive(Debug)]
pub struct StealthStateManager {
    object_id: ObjectID,
    visibility_state: VisibilityState,
    stealth_enabled: bool,

    // Detection tracking
    detectors: HashMap<ObjectID, DetectionState>,

    // Transition timing
    stealth_transition_frame: u32,
    stealth_delay_frames: u32,
    unstealth_delay_frames: u32,

    // Stealth conditions
    forbidden_while_moving: bool,
    forbidden_while_attacking: bool,
    forbidden_while_damaged: bool,

    // State tracking
    last_attack_frame: u32,
    last_damage_frame: u32,
    last_movement_frame: u32,

    // Cooldown tracking
    stealth_cooldown_frames: u32,
}

impl StealthStateManager {
    pub fn new(object_id: ObjectID, stealth_delay_frames: u32) -> Self {
        Self {
            object_id,
            visibility_state: VisibilityState::Visible,
            stealth_enabled: false,
            detectors: HashMap::new(),
            stealth_transition_frame: 0,
            stealth_delay_frames,
            unstealth_delay_frames: stealth_delay_frames / 2,
            forbidden_while_moving: false,
            forbidden_while_attacking: true,
            forbidden_while_damaged: false,
            last_attack_frame: u32::MAX,
            last_damage_frame: u32::MAX,
            last_movement_frame: u32::MAX,
            stealth_cooldown_frames: 0,
        }
    }

    /// Get current visibility state
    pub fn get_visibility_state(&self) -> VisibilityState {
        self.visibility_state
    }

    pub fn is_stealth_enabled(&self) -> bool {
        self.stealth_enabled
    }

    pub fn set_stealth_enabled(&mut self, enabled: bool) {
        self.stealth_enabled = enabled;
    }

    /// Check if object is currently stealthed
    pub fn is_stealthed(&self) -> bool {
        matches!(
            self.visibility_state,
            VisibilityState::Hidden | VisibilityState::Detected
        )
    }

    #[cfg(test)]
    pub(crate) fn force_set_visibility_state_for_testing(&mut self, state: VisibilityState) {
        self.visibility_state = state;
    }

    /// Check if object is detected by any player
    pub fn is_detected(&self) -> bool {
        !self.detectors.is_empty()
    }

    /// Check if object is detected by a specific player
    pub fn is_detected_by(&self, detector_id: ObjectID) -> bool {
        self.detectors.contains_key(&detector_id)
    }

    /// Add a detector
    pub fn add_detector(&mut self, detector_id: ObjectID, current_frame: u32) {
        if !self.detectors.contains_key(&detector_id) {
            self.detectors
                .insert(detector_id, DetectionState::new(detector_id, current_frame));

            if self.visibility_state == VisibilityState::Hidden {
                self.visibility_state = VisibilityState::Detected;
            }
        }
    }

    /// Remove a detector
    pub fn remove_detector(&mut self, detector_id: ObjectID) {
        self.detectors.remove(&detector_id);

        if self.detectors.is_empty() && self.visibility_state == VisibilityState::Detected {
            self.visibility_state = VisibilityState::Hidden;
        }
    }

    /// Clear all detectors
    pub fn clear_detectors(&mut self) {
        self.detectors.clear();
        if self.visibility_state == VisibilityState::Detected {
            self.visibility_state = VisibilityState::Hidden;
        }
    }

    /// Get list of all detectors
    pub fn get_detectors(&self) -> Vec<ObjectID> {
        self.detectors.keys().copied().collect()
    }

    /// Set stealth break conditions
    pub fn set_forbidden_conditions(
        &mut self,
        while_moving: bool,
        while_attacking: bool,
        while_damaged: bool,
    ) {
        self.forbidden_while_moving = while_moving;
        self.forbidden_while_attacking = while_attacking;
        self.forbidden_while_damaged = while_damaged;
    }

    /// Try to enable stealth
    pub fn try_enable_stealth(&mut self, current_frame: u32) -> bool {
        if self.stealth_cooldown_frames > 0 {
            return false;
        }

        if !self.can_stealth(current_frame) {
            return false;
        }

        self.visibility_state = VisibilityState::Stealthing;
        self.stealth_transition_frame = current_frame;
        true
    }

    /// Force disable stealth
    pub fn force_disable_stealth(&mut self, current_frame: u32) {
        if self.is_stealthed() {
            self.visibility_state = VisibilityState::Unstealthing;
            self.stealth_transition_frame = current_frame;
            self.stealth_cooldown_frames = self.stealth_delay_frames;
        }
        self.clear_detectors();
    }

    /// Check if stealth is allowed given current conditions
    pub fn can_stealth(&self, current_frame: u32) -> bool {
        if !self.stealth_enabled {
            return false;
        }

        // Check cooldown
        if self.stealth_cooldown_frames > 0 {
            return false;
        }

        // Check movement restriction
        if self.forbidden_while_moving {
            let frames_since_move = if self.last_movement_frame == u32::MAX {
                u32::MAX
            } else {
                current_frame.saturating_sub(self.last_movement_frame)
            };
            if frames_since_move < self.stealth_delay_frames {
                return false;
            }
        }

        // Check attack restriction
        if self.forbidden_while_attacking {
            let frames_since_attack = if self.last_attack_frame == u32::MAX {
                u32::MAX
            } else {
                current_frame.saturating_sub(self.last_attack_frame)
            };
            if frames_since_attack < self.stealth_delay_frames {
                return false;
            }
        }

        // Check damage restriction
        if self.forbidden_while_damaged {
            let frames_since_damage = if self.last_damage_frame == u32::MAX {
                u32::MAX
            } else {
                current_frame.saturating_sub(self.last_damage_frame)
            };
            if frames_since_damage < self.stealth_delay_frames {
                return false;
            }
        }

        true
    }

    /// Record that object moved
    pub fn on_movement(&mut self, current_frame: u32) {
        self.last_movement_frame = current_frame;

        if self.forbidden_while_moving && self.is_stealthed() {
            self.force_disable_stealth(current_frame);
        }
    }

    /// Record that object attacked
    pub fn on_attack(&mut self, current_frame: u32) {
        self.last_attack_frame = current_frame;

        if self.forbidden_while_attacking && self.is_stealthed() {
            self.force_disable_stealth(current_frame);
        }
    }

    /// Record that object took damage
    pub fn on_damage(&mut self, current_frame: u32) {
        self.last_damage_frame = current_frame;

        if self.forbidden_while_damaged && self.is_stealthed() {
            self.force_disable_stealth(current_frame);
        }
    }

    /// Update state machine transitions
    pub fn update(&mut self, current_frame: u32) {
        // Update cooldown
        if self.stealth_cooldown_frames > 0 {
            self.stealth_cooldown_frames = self.stealth_cooldown_frames.saturating_sub(1);
        }

        // Handle state transitions
        match self.visibility_state {
            VisibilityState::Stealthing => {
                let frames_elapsed = current_frame.saturating_sub(self.stealth_transition_frame);

                if frames_elapsed >= self.stealth_delay_frames {
                    if self.can_stealth(current_frame) {
                        self.visibility_state = VisibilityState::Hidden;
                    } else {
                        // Conditions changed during transition
                        self.visibility_state = VisibilityState::Visible;
                    }
                } else if !self.can_stealth(current_frame) {
                    // Abort transition
                    self.visibility_state = VisibilityState::Visible;
                }
            }

            VisibilityState::Unstealthing => {
                let frames_elapsed = current_frame.saturating_sub(self.stealth_transition_frame);

                if frames_elapsed >= self.unstealth_delay_frames {
                    self.visibility_state = VisibilityState::Visible;
                    self.clear_detectors();
                }
            }

            VisibilityState::Hidden | VisibilityState::Detected => {
                // Check if conditions still allow stealth
                if !self.can_stealth(current_frame) {
                    self.force_disable_stealth(current_frame);
                }
            }

            VisibilityState::Visible => {
                // Auto-stealth if enabled and conditions are met
                if self.stealth_enabled && self.can_stealth(current_frame) {
                    let _ = self.try_enable_stealth(current_frame);
                }
            }
        }
    }

    /// Get stealth opacity (for rendering)
    /// Returns 0.0 for fully invisible, 1.0 for fully visible
    pub fn get_opacity(&self) -> f32 {
        match self.visibility_state {
            VisibilityState::Visible => 1.0,
            VisibilityState::Hidden => 0.2,
            VisibilityState::Detected => 0.6,
            VisibilityState::Stealthing => 0.5,
            VisibilityState::Unstealthing => 0.7,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stealth_state_transitions() {
        let mut manager = StealthStateManager::new(1, 30);
        assert_eq!(manager.get_visibility_state(), VisibilityState::Visible);

        manager.stealth_enabled = true;
        assert!(manager.try_enable_stealth(0));
        assert_eq!(manager.get_visibility_state(), VisibilityState::Stealthing);

        manager.update(30);
        assert_eq!(manager.get_visibility_state(), VisibilityState::Hidden);
    }

    #[test]
    fn test_detection() {
        let mut manager = StealthStateManager::new(1, 30);
        manager.visibility_state = VisibilityState::Hidden;

        assert!(!manager.is_detected());

        manager.add_detector(2, 0);
        assert!(manager.is_detected());
        assert!(manager.is_detected_by(2));
        assert_eq!(manager.get_visibility_state(), VisibilityState::Detected);

        manager.remove_detector(2);
        assert!(!manager.is_detected());
        assert_eq!(manager.get_visibility_state(), VisibilityState::Hidden);
    }

    #[test]
    fn test_stealth_break_on_attack() {
        let mut manager = StealthStateManager::new(1, 30);
        manager.stealth_enabled = true;
        manager.forbidden_while_attacking = true;
        manager.visibility_state = VisibilityState::Hidden;

        manager.on_attack(100);
        assert_eq!(
            manager.get_visibility_state(),
            VisibilityState::Unstealthing
        );
    }

    #[test]
    fn test_stealth_cooldown() {
        let mut manager = StealthStateManager::new(1, 30);
        manager.stealth_enabled = true;
        manager.visibility_state = VisibilityState::Hidden;

        manager.force_disable_stealth(0);
        assert_eq!(manager.stealth_cooldown_frames, 30);

        assert!(!manager.can_stealth(0));

        for _ in 0..30 {
            manager.update(1);
        }

        assert_eq!(manager.stealth_cooldown_frames, 0);
    }
}
