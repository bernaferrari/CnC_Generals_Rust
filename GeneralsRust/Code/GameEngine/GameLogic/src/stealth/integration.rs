//! Integration helpers for the stealth detection system
//!
//! Provides hooks and utilities for integrating stealth with other game systems

use super::{StealthStateManager, VisibilityManager};
use crate::common::*;
use crate::modules::StealthUpdate;
use std::sync::{Arc, Mutex};

/// Stealth event types for game system integration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StealthEvent {
    /// Object entered stealth
    EnteredStealth,
    /// Object exited stealth
    ExitedStealth,
    /// Object was detected
    Detected,
    /// Object is no longer detected
    DetectionLost,
}

/// Callback trait for stealth events
pub trait StealthEventListener: Send + Sync {
    fn on_stealth_event(&mut self, object_id: ObjectID, event: StealthEvent);
}

/// Integration layer between stealth system and game systems
pub struct StealthIntegration {
    visibility_manager: Arc<VisibilityManager>,
    event_listeners: Vec<Box<dyn StealthEventListener>>,
}

impl StealthIntegration {
    pub fn new(visibility_manager: Arc<VisibilityManager>) -> Self {
        Self {
            visibility_manager,
            event_listeners: Vec::new(),
        }
    }

    /// Register an event listener
    pub fn add_listener(&mut self, listener: Box<dyn StealthEventListener>) {
        self.event_listeners.push(listener);
    }

    /// Notify listeners of a stealth event
    fn notify_event(&mut self, object_id: ObjectID, event: StealthEvent) {
        for listener in &mut self.event_listeners {
            listener.on_stealth_event(object_id, event);
        }
    }

    /// Hook for movement system: notify when object moves
    pub fn on_object_movement(
        &mut self,
        object_id: ObjectID,
        stealth_module: &Arc<Mutex<dyn StealthUpdate>>,
        _current_frame: u32,
    ) {
        // Check if stealth should break due to movement
        if let Ok(stealth_guard) = stealth_module.try_lock() {
            if stealth_guard.is_stealthed() {
                // Movement detected - stealth module will handle breaking
                self.notify_event(object_id, StealthEvent::ExitedStealth);
            }
        }
    }

    /// Hook for combat system: notify when object attacks
    pub fn on_object_attack(
        &mut self,
        object_id: ObjectID,
        stealth_module: &Arc<Mutex<dyn StealthUpdate>>,
        _current_frame: u32,
    ) {
        // Attack breaks stealth
        if let Ok(mut stealth_guard) = stealth_module.try_lock() {
            if stealth_guard.is_stealthed() {
                let _ = stealth_guard.end_stealth();
                self.notify_event(object_id, StealthEvent::ExitedStealth);
            }
        }
    }

    /// Hook for damage system: notify when object takes damage
    pub fn on_object_damaged(
        &mut self,
        object_id: ObjectID,
        stealth_module: &Arc<Mutex<dyn StealthUpdate>>,
        damage: f32,
        _current_frame: u32,
    ) {
        // Damage may break stealth depending on configuration
        if damage > 0.0 {
            if let Ok(mut stealth_guard) = stealth_module.try_lock() {
                if stealth_guard.is_stealthed() {
                    // Mark as detected
                    stealth_guard.mark_as_detected();
                    self.notify_event(object_id, StealthEvent::Detected);
                }
            }
        }
    }

    /// Hook for detection: object detected another stealthed object
    pub fn on_detection(
        &mut self,
        detector_id: ObjectID,
        detected_id: ObjectID,
        player_id: ObjectID,
        current_frame: u32,
    ) {
        self.visibility_manager
            .add_detector(detected_id, player_id, detector_id, current_frame);
        self.notify_event(detected_id, StealthEvent::Detected);
    }

    /// Hook for detection loss: detector no longer detects object
    pub fn on_detection_lost(
        &mut self,
        detector_id: ObjectID,
        detected_id: ObjectID,
        player_id: ObjectID,
    ) {
        self.visibility_manager
            .remove_detector(detected_id, player_id, detector_id);

        // Check if still detected by anyone else
        let visible_to = self.visibility_manager.get_visible_to_players(detected_id);
        if visible_to.is_empty() {
            self.notify_event(detected_id, StealthEvent::DetectionLost);
        }
    }

    /// Get visibility manager
    pub fn get_visibility_manager(&self) -> Arc<VisibilityManager> {
        self.visibility_manager.clone()
    }
}

/// Visual effects manager for stealth
pub struct StealthVisualEffects {
    /// Shimmer effect intensity (0.0 = no shimmer, 1.0 = full shimmer)
    shimmer_intensity: f32,
    /// Flicker rate when moving (Hz)
    flicker_rate: f32,
}

impl StealthVisualEffects {
    pub fn new() -> Self {
        Self {
            shimmer_intensity: 0.3,
            flicker_rate: 5.0,
        }
    }

    /// Calculate opacity for rendering based on stealth state
    pub fn calculate_opacity(
        &self,
        is_stealthed: bool,
        is_detected: bool,
        is_friendly: bool,
        is_moving: bool,
        time: f32,
    ) -> f32 {
        if !is_stealthed {
            return 1.0; // Fully visible
        }

        if is_friendly {
            // Friendly stealthed units are partially visible to allies
            return 0.5;
        }

        if is_detected {
            // Detected units show shimmer effect
            let shimmer = (time * self.flicker_rate).sin() * self.shimmer_intensity;
            return 0.3 + shimmer;
        }

        // Not detected - very low visibility
        if is_moving {
            // Moving stealthed units flicker slightly
            let flicker = (time * self.flicker_rate * 2.0).sin() * 0.1;
            return 0.1 + flicker;
        }

        0.05 // Nearly invisible
    }

    /// Get shader parameters for stealth rendering
    pub fn get_shader_params(&self, is_detected: bool, time: f32) -> StealthShaderParams {
        StealthShaderParams {
            opacity: if is_detected { 0.6 } else { 0.2 },
            shimmer_intensity: if is_detected {
                self.shimmer_intensity
            } else {
                0.0
            },
            shimmer_speed: self.flicker_rate,
            time,
        }
    }
}

impl Default for StealthVisualEffects {
    fn default() -> Self {
        Self::new()
    }
}

/// Shader parameters for stealth rendering
#[derive(Debug, Clone, Copy)]
pub struct StealthShaderParams {
    pub opacity: f32,
    pub shimmer_intensity: f32,
    pub shimmer_speed: f32,
    pub time: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visual_effects_opacity() {
        let vfx = StealthVisualEffects::new();

        // Not stealthed
        assert_eq!(vfx.calculate_opacity(false, false, false, false, 0.0), 1.0);

        // Friendly stealthed
        assert_eq!(vfx.calculate_opacity(true, false, true, false, 0.0), 0.5);

        // Enemy stealthed not detected
        let opacity = vfx.calculate_opacity(true, false, false, false, 0.0);
        assert!(opacity < 0.2);

        // Enemy stealthed and detected
        let opacity = vfx.calculate_opacity(true, true, false, false, 0.0);
        assert!(opacity > 0.2 && opacity < 0.7);
    }

    #[test]
    fn test_shader_params() {
        let vfx = StealthVisualEffects::new();

        let detected_params = vfx.get_shader_params(true, 1.0);
        assert_eq!(detected_params.opacity, 0.6);
        assert!(detected_params.shimmer_intensity > 0.0);

        let hidden_params = vfx.get_shader_params(false, 1.0);
        assert_eq!(hidden_params.opacity, 0.2);
        assert_eq!(hidden_params.shimmer_intensity, 0.0);
    }

    #[test]
    fn test_integration_creation() {
        let manager = Arc::new(VisibilityManager::new());
        let integration = StealthIntegration::new(manager);
        assert_eq!(integration.event_listeners.len(), 0);
    }
}
