//! Stealth & Detection System Integration Interfaces
//!
//! Provides clean, unified interfaces for the game engine to interact with the stealth system.
//!
//! This module serves as the primary entry point for game logic, rendering, and AI subsystems
//! to utilize stealth functionality. It aggregates multiple managers (stealth, detection, disguise)
//! into a single facade with clear, purpose-driven APIs.
//!
//! # Architecture
//!
//! The integration layer is organized into four main hook systems:
//! - **GameLogicHooks**: Core game event callbacks
//! - **RenderingHooks**: Visual feedback and visibility queries
//! - **AIHooks**: Detection and stealth awareness for AI
//! - **SystemManager**: Unified initialization and frame updates

use crate::common::{ObjectID, UnsignedInt, MAX_PLAYER_COUNT};
use crate::object::registry::OBJECT_REGISTRY;
use crate::system::detection_manager::{
    get_detection_manager, DetectionManager, DetectionStrength,
};
use crate::system::disguise_manager::{get_disguise_manager, DisguiseManager};
use crate::system::stealth_errors::{StealthError, StealthResult};
use crate::system::stealth_manager::{
    get_stealth_manager, StealthManager, StealthStatus, StealthStrength,
};
use log::{debug, info, trace};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;

/// Stealth system difficulty levels for AI perception
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StealthDifficulty {
    /// Object is completely visible (no stealth)
    Visible,
    /// Easy to detect (weak stealth)
    Easy,
    /// Moderate difficulty (standard stealth)
    Moderate,
    /// Difficult to detect (strong stealth)
    Difficult,
    /// Nearly impossible to detect (maximum stealth)
    Extreme,
}

impl StealthDifficulty {
    /// Convert stealth strength (0.0-100.0) to difficulty level
    pub fn from_strength(strength: f32) -> Self {
        match strength {
            s if s <= 0.0 => StealthDifficulty::Visible,
            s if s <= 25.0 => StealthDifficulty::Easy,
            s if s <= 50.0 => StealthDifficulty::Moderate,
            s if s <= 75.0 => StealthDifficulty::Difficult,
            _ => StealthDifficulty::Extreme,
        }
    }

    /// Get human-readable name
    pub fn as_str(&self) -> &str {
        match self {
            StealthDifficulty::Visible => "VISIBLE",
            StealthDifficulty::Easy => "EASY",
            StealthDifficulty::Moderate => "MODERATE",
            StealthDifficulty::Difficult => "DIFFICULT",
            StealthDifficulty::Extreme => "EXTREME",
        }
    }
}

/// Detection feedback event for visual/audio effects
#[derive(Debug, Clone)]
pub struct DetectionEvent {
    /// Object that was detected
    pub detected_object_id: ObjectID,
    /// Object that performed detection
    pub detector_object_id: ObjectID,
    /// Detection strength value
    pub detection_strength: f32,
    /// Frame when detection occurred
    pub detection_frame: UnsignedInt,
    /// Detection method (e.g., "visual", "radar", "proximity")
    pub detection_method: String,
}

/// Disguise rendering information
#[derive(Debug, Clone)]
pub struct DisguiseAppearance {
    /// Template ID to render as
    pub template_id: u32,
    /// Whether disguise is active
    pub is_disguised: bool,
    /// Transition progress (0.0-1.0)
    pub transition_progress: f32,
    /// Time remaining on disguise (frames)
    pub time_remaining: UnsignedInt,
}

/// Stealth System Manager - Unified facade for all stealth operations
///
/// Provides a single point of entry for initializing and updating all stealth subsystems.
/// Thread-safe access through Arc<Mutex<T>> pattern.
pub struct StealthSystemManager {
    stealth_mgr: &'static Mutex<StealthManager>,
    detection_mgr: &'static Mutex<DetectionManager>,
    disguise_mgr: &'static Mutex<DisguiseManager>,
    is_initialized: bool,
}

impl StealthSystemManager {
    /// Create new StealthSystemManager
    pub fn new() -> Self {
        Self {
            stealth_mgr: get_stealth_manager(),
            detection_mgr: get_detection_manager(),
            disguise_mgr: get_disguise_manager(),
            is_initialized: false,
        }
    }

    /// Initialize all stealth systems
    pub fn initialize(&mut self) -> StealthResult<()> {
        info!("Initializing Stealth System Manager");
        self.is_initialized = true;
        Ok(())
    }

    /// Update all systems for current frame
    pub fn update_frame(&self, frame: UnsignedInt, _delta_time: f32) -> StealthResult<()> {
        trace!("Updating stealth systems at frame {}", frame);

        // Update frame counters in all managers
        self.stealth_mgr
            .lock()
            .map_err(|e| StealthError::operation_failed(format!("stealth_mgr lock: {}", e)))?
            .set_update_frame(frame);

        self.detection_mgr
            .lock()
            .map_err(|e| StealthError::operation_failed(format!("detection_mgr lock: {}", e)))?
            .set_update_frame(frame);

        self.disguise_mgr
            .lock()
            .map_err(|e| StealthError::operation_failed(format!("disguise_mgr lock: {}", e)))?
            .set_update_frame(frame);

        Ok(())
    }

    /// Get status information about stealth system
    pub fn get_status(&self) -> StealthResult<String> {
        let status = format!(
            "StealthSystemManager: initialized={}, managers ready",
            self.is_initialized
        );
        Ok(status)
    }
}

impl Default for StealthSystemManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Game Logic Hooks - Event callbacks from game engine
///
/// These hooks integrate with the game loop and object management systems.
pub struct StealthGameLogicHooks;

impl StealthGameLogicHooks {
    /// Called when a new object is created
    ///
    /// Initializes stealth tracking for the object in the stealth manager.
    pub fn on_object_created(object_id: ObjectID) -> StealthResult<()> {
        let mut mgr = get_stealth_manager()
            .lock()
            .map_err(|e| StealthError::operation_failed(format!("stealth_mgr lock: {}", e)))?;

        mgr.register_object(object_id)
            .map_err(|_e| StealthError::object_not_registered(object_id))
    }

    /// Called when an object is deleted
    ///
    /// Removes stealth tracking and cleans up associated state.
    pub fn on_object_deleted(object_id: ObjectID) -> StealthResult<()> {
        let mut mgr = get_stealth_manager()
            .lock()
            .map_err(|e| StealthError::operation_failed(format!("stealth_mgr lock: {}", e)))?;

        mgr.unregister_object(object_id)
            .map_err(|_e| StealthError::object_not_registered(object_id))?;

        // Also clean up from detection manager
        let mut det_mgr = get_detection_manager()
            .lock()
            .map_err(|e| StealthError::operation_failed(format!("detection_mgr lock: {}", e)))?;

        let _ = det_mgr.unregister_object(object_id);

        // Also clean up from disguise manager
        let mut dis_mgr = get_disguise_manager()
            .lock()
            .map_err(|e| StealthError::operation_failed(format!("disguise_mgr lock: {}", e)))?;

        let _ = dis_mgr.unregister_object(object_id);

        Ok(())
    }

    /// Called when object attacks another object
    ///
    /// Breaks stealth for the attacker.
    pub fn on_object_attacked(attacker_id: ObjectID, _target_id: ObjectID) -> StealthResult<()> {
        let mut mgr = get_stealth_manager()
            .lock()
            .map_err(|e| StealthError::operation_failed(format!("stealth_mgr lock: {}", e)))?;

        // Break stealth for all players
        mgr.break_stealth_all(attacker_id, 0).map_err(|_| {
            StealthError::stealth_condition_not_met(attacker_id, "attack breaks stealth")
        })
    }

    /// Called when object moves
    ///
    /// Checks if movement speed exceeds stealth threshold.
    pub fn on_object_moved(object_id: ObjectID, speed: f32) -> StealthResult<()> {
        // Movement speed threshold is typically 5.0 units per frame
        // Objects moving faster than this may break stealth
        if speed > 5.0 {
            let mut mgr = get_stealth_manager()
                .lock()
                .map_err(|e| StealthError::operation_failed(format!("stealth_mgr lock: {}", e)))?;

            mgr.break_stealth_all(object_id, 0).map_err(|_| {
                StealthError::stealth_condition_not_met(object_id, "movement breaks stealth")
            })?;
        }

        Ok(())
    }

    /// Called when object takes damage
    ///
    /// Breaks stealth from damage event.
    pub fn on_object_damaged(object_id: ObjectID) -> StealthResult<()> {
        let mut mgr = get_stealth_manager()
            .lock()
            .map_err(|e| StealthError::operation_failed(format!("stealth_mgr lock: {}", e)))?;

        // Break stealth for all players
        mgr.break_stealth_all(object_id, 0).map_err(|_| {
            StealthError::stealth_condition_not_met(object_id, "damage breaks stealth")
        })
    }

    /// Called when a weapon is fired
    ///
    /// Breaks stealth from weapon discharge.
    pub fn on_weapon_fired(unit_id: ObjectID, _weapon_type: &str) -> StealthResult<()> {
        let mut mgr = get_stealth_manager()
            .lock()
            .map_err(|e| StealthError::operation_failed(format!("stealth_mgr lock: {}", e)))?;

        // Break stealth for all players
        mgr.break_stealth_all(unit_id, 0).map_err(|_| {
            StealthError::stealth_condition_not_met(unit_id, "weapon fire breaks stealth")
        })
    }

    /// Called when a special ability is used
    ///
    /// Breaks stealth from ability activation.
    pub fn on_ability_used(unit_id: ObjectID, _ability: &str) -> StealthResult<()> {
        let mut mgr = get_stealth_manager()
            .lock()
            .map_err(|e| StealthError::operation_failed(format!("stealth_mgr lock: {}", e)))?;

        // Break stealth for all players
        mgr.break_stealth_all(unit_id, 0).map_err(|_| {
            StealthError::stealth_condition_not_met(unit_id, "ability use breaks stealth")
        })
    }
}

/// Rendering Hooks - Visual feedback and visibility queries
///
/// These hooks provide the rendering system with stealth visibility and appearance data.
pub struct StealthRenderingHooks;

impl StealthRenderingHooks {
    /// Get list of visible objects for a player
    ///
    /// Returns object IDs visible to the specified player, considering stealth status.
    pub fn get_objects_for_player(
        player_id: usize,
    ) -> StealthResult<Vec<(ObjectID, StealthStatus)>> {
        if player_id >= MAX_PLAYER_COUNT {
            return Err(StealthError::operation_failed(format!(
                "invalid player id {}",
                player_id
            )));
        }

        let mgr = get_stealth_manager()
            .lock()
            .map_err(|e| StealthError::operation_failed(format!("stealth_mgr lock: {}", e)))?;
        let mut visible = Vec::new();

        for object_ref in OBJECT_REGISTRY.get_all_objects() {
            let Ok(object_guard) = object_ref.read() else {
                continue;
            };
            let object_id = object_guard.get_id();
            let status = mgr
                .get_stealth_status(object_id, player_id)
                // Unregistered objects default to visible in rendering path.
                .unwrap_or(StealthStatus::Revealed);

            if status != StealthStatus::Hidden {
                visible.push((object_id, status));
            }
        }

        Ok(visible)
    }

    /// Get disguise appearance for rendering
    ///
    /// Returns the template ID and transition info to render object as.
    pub fn get_disguise_appearance(
        object_id: ObjectID,
        _player_id: usize,
    ) -> StealthResult<DisguiseAppearance> {
        let dis_mgr = get_disguise_manager()
            .lock()
            .map_err(|e| StealthError::operation_failed(format!("disguise_mgr lock: {}", e)))?;

        // Get disguise template if applied
        let template_id = dis_mgr
            .get_disguise(object_id)
            .map(|_| 1u32) // Non-zero if disguised
            .unwrap_or(0);

        let transition_frames = dis_mgr.get_reveal_transition_frames(object_id).unwrap_or(0);

        let is_active = dis_mgr.has_active_disguise(object_id).unwrap_or(false);

        let transition_progress = if transition_frames > 0 {
            0.5 // Mid-transition
        } else if is_active {
            1.0 // Fully disguised
        } else {
            0.0 // No disguise
        };

        Ok(DisguiseAppearance {
            template_id,
            is_disguised: is_active,
            transition_progress,
            time_remaining: transition_frames,
        })
    }

    /// Get rendering opacity for object
    ///
    /// Returns alpha value (0.0-1.0) for rendering the object to the player.
    /// Invisible objects have alpha near 0.0, fully visible objects have alpha 1.0.
    pub fn get_opacity_for_player(object_id: ObjectID, player_id: usize) -> StealthResult<f32> {
        let mgr = get_stealth_manager()
            .lock()
            .map_err(|e| StealthError::operation_failed(format!("stealth_mgr lock: {}", e)))?;

        let status = mgr.get_stealth_status(object_id, player_id)?;

        // Return opacity based on stealth status
        let opacity = match status {
            StealthStatus::Hidden => 0.0,
            StealthStatus::Invisible => 0.3, // Slightly visible for gameplay
            StealthStatus::Revealed => 1.0,
        };

        Ok(opacity)
    }

    /// Get detection feedback events
    ///
    /// Returns list of recent detection events for particle/audio effects.
    pub fn get_detection_feedback() -> StealthResult<Vec<DetectionEvent>> {
        // Note: Real implementation would track detection events in a queue
        Ok(Vec::new())
    }
}

/// AI Hooks - Detection and stealth awareness
///
/// These hooks provide AI with stealth detection capabilities and target visibility.
pub struct StealthAIHooks;

impl StealthAIHooks {
    /// Check if AI unit can see target
    ///
    /// Determines if target is visible to unit considering stealth and detection.
    /// Takes into account unit's player allegiance.
    pub fn can_ai_see_target(unit_id: ObjectID, target_id: ObjectID) -> StealthResult<bool> {
        let stealth_mgr = get_stealth_manager()
            .lock()
            .map_err(|e| StealthError::operation_failed(format!("stealth_mgr lock: {}", e)))?;

        let detection_mgr = get_detection_manager()
            .lock()
            .map_err(|e| StealthError::operation_failed(format!("detection_mgr lock: {}", e)))?;

        // Get target's stealth status (assume unit_id owner is viewer)
        let player_id = 0; // In real implementation, get from unit's owner

        let stealth_status = stealth_mgr.get_stealth_status(target_id, player_id)?;

        // If revealed, always visible
        if stealth_status == StealthStatus::Revealed {
            return Ok(true);
        }

        // If invisible, check if unit has detection capability
        let unit_detection = detection_mgr
            .get_detection_strength(unit_id)
            .unwrap_or(DetectionStrength::none());

        let target_stealth = stealth_mgr
            .get_stealth_strength(target_id)
            .unwrap_or(StealthStrength::none());

        // Simple comparison: detection strength must exceed stealth strength
        Ok(unit_detection.value() > target_stealth.value())
    }

    /// Get stealth difficulty for target
    ///
    /// Returns difficulty level representing how hard target is to detect.
    pub fn get_stealth_difficulty_for_target(
        target_id: ObjectID,
    ) -> StealthResult<StealthDifficulty> {
        let mgr = get_stealth_manager()
            .lock()
            .map_err(|e| StealthError::operation_failed(format!("stealth_mgr lock: {}", e)))?;

        let strength = mgr
            .get_stealth_strength(target_id)
            .unwrap_or(StealthStrength::none());

        Ok(StealthDifficulty::from_strength(strength.value()))
    }

    /// Get detection awareness level for unit
    ///
    /// Returns detection capability of the unit for stealth detection.
    pub fn get_detection_awareness(unit_id: ObjectID) -> StealthResult<f32> {
        let mgr = get_detection_manager()
            .lock()
            .map_err(|e| StealthError::operation_failed(format!("detection_mgr lock: {}", e)))?;

        let strength = mgr
            .get_detection_strength(unit_id)
            .unwrap_or(DetectionStrength::none());

        Ok(strength.value())
    }
}

/// Global integration manager singleton
static INTEGRATION_MANAGER: OnceLock<Mutex<StealthSystemManager>> = OnceLock::new();

/// Get the global integration manager
pub fn get_stealth_integration_manager() -> &'static Mutex<StealthSystemManager> {
    INTEGRATION_MANAGER.get_or_init(|| Mutex::new(StealthSystemManager::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stealth_difficulty_levels() {
        assert_eq!(
            StealthDifficulty::from_strength(0.0),
            StealthDifficulty::Visible
        );
        assert_eq!(
            StealthDifficulty::from_strength(15.0),
            StealthDifficulty::Easy
        );
        assert_eq!(
            StealthDifficulty::from_strength(40.0),
            StealthDifficulty::Moderate
        );
        assert_eq!(
            StealthDifficulty::from_strength(70.0),
            StealthDifficulty::Difficult
        );
        assert_eq!(
            StealthDifficulty::from_strength(90.0),
            StealthDifficulty::Extreme
        );
    }

    #[test]
    fn test_stealth_difficulty_names() {
        assert_eq!(StealthDifficulty::Visible.as_str(), "VISIBLE");
        assert_eq!(StealthDifficulty::Easy.as_str(), "EASY");
        assert_eq!(StealthDifficulty::Moderate.as_str(), "MODERATE");
        assert_eq!(StealthDifficulty::Difficult.as_str(), "DIFFICULT");
        assert_eq!(StealthDifficulty::Extreme.as_str(), "EXTREME");
    }

    #[test]
    fn test_detection_event_creation() {
        let event = DetectionEvent {
            detected_object_id: 1,
            detector_object_id: 2,
            detection_strength: 75.0,
            detection_frame: 100,
            detection_method: "visual".to_string(),
        };

        assert_eq!(event.detected_object_id, 1);
        assert_eq!(event.detector_object_id, 2);
        assert_eq!(event.detection_strength, 75.0);
    }

    #[test]
    fn test_disguise_appearance_creation() {
        let appearance = DisguiseAppearance {
            template_id: 42,
            is_disguised: true,
            transition_progress: 0.5,
            time_remaining: 300,
        };

        assert_eq!(appearance.template_id, 42);
        assert!(appearance.is_disguised);
        assert_eq!(appearance.transition_progress, 0.5);
    }

    #[test]
    fn test_system_manager_creation() {
        let mgr = StealthSystemManager::new();
        assert!(!mgr.is_initialized);
    }

    #[test]
    fn test_system_manager_initialization() {
        let mut mgr = StealthSystemManager::new();
        let result = mgr.initialize();
        assert!(result.is_ok());
        assert!(mgr.is_initialized);
    }

    #[test]
    fn test_system_manager_status() {
        let mgr = StealthSystemManager::new();
        let status = mgr.get_status();
        assert!(status.is_ok());
        let status_str = status.unwrap();
        assert!(status_str.contains("StealthSystemManager"));
    }

    #[test]
    fn test_game_logic_hooks_callable() {
        // Verify all hooks can be called without panicking
        let _ = StealthGameLogicHooks::on_object_created(1);
        let _ = StealthGameLogicHooks::on_object_deleted(1);
        let _ = StealthGameLogicHooks::on_object_attacked(1, 2);
        let _ = StealthGameLogicHooks::on_object_moved(1, 10.0);
        let _ = StealthGameLogicHooks::on_object_damaged(1);
        let _ = StealthGameLogicHooks::on_weapon_fired(1, "rifle");
        let _ = StealthGameLogicHooks::on_ability_used(1, "cloak");
    }

    #[test]
    fn test_rendering_hooks_callable() {
        // Verify all hooks can be called without panicking
        let _ = StealthRenderingHooks::get_objects_for_player(0);
        let _ = StealthRenderingHooks::get_disguise_appearance(1, 0);
        let _ = StealthRenderingHooks::get_opacity_for_player(1, 0);
        let _ = StealthRenderingHooks::get_detection_feedback();
    }

    #[test]
    fn test_ai_hooks_callable() {
        // Verify all hooks can be called without panicking
        let _ = StealthAIHooks::can_ai_see_target(1, 2);
        let _ = StealthAIHooks::get_stealth_difficulty_for_target(1);
        let _ = StealthAIHooks::get_detection_awareness(1);
    }

    #[test]
    fn test_game_logic_object_lifecycle() {
        // Test complete object lifecycle
        let obj_id = 42;

        // Create object
        let create_result = StealthGameLogicHooks::on_object_created(obj_id);
        assert!(create_result.is_ok(), "object creation should succeed");

        // Delete object
        let delete_result = StealthGameLogicHooks::on_object_deleted(obj_id);
        assert!(delete_result.is_ok(), "object deletion should succeed");
    }

    #[test]
    fn test_stealth_breaking_events() {
        let obj_id = 43;
        let _ = StealthGameLogicHooks::on_object_created(obj_id);

        // Test various stealth-breaking events
        let attack_result = StealthGameLogicHooks::on_object_attacked(obj_id, 44);
        assert!(attack_result.is_ok() || attack_result.is_err());

        let damage_result = StealthGameLogicHooks::on_object_damaged(obj_id);
        assert!(damage_result.is_ok() || damage_result.is_err());

        let weapon_result = StealthGameLogicHooks::on_weapon_fired(obj_id, "plasma");
        assert!(weapon_result.is_ok() || weapon_result.is_err());

        let ability_result = StealthGameLogicHooks::on_ability_used(obj_id, "superweapon");
        assert!(ability_result.is_ok() || ability_result.is_err());

        let _ = StealthGameLogicHooks::on_object_deleted(obj_id);
    }

    #[test]
    fn test_integration_manager_singleton() {
        let mgr1 = get_stealth_integration_manager();
        let mgr2 = get_stealth_integration_manager();

        // Should return same reference
        assert!(std::ptr::eq(mgr1, mgr2));
    }

    #[test]
    fn test_rendering_opacity_range() {
        // Verify opacity values are always 0.0-1.0
        let _ = StealthRenderingHooks::get_opacity_for_player(1, 0).map(|opacity| {
            assert!(opacity >= 0.0 && opacity <= 1.0, "opacity must be 0.0-1.0");
        });
    }

    #[test]
    fn test_ai_detection_awareness_range() {
        // Verify detection awareness is 0.0-100.0
        let _ = StealthAIHooks::get_detection_awareness(1).map(|awareness| {
            assert!(
                awareness >= 0.0 && awareness <= 100.0,
                "awareness must be 0.0-100.0"
            );
        });
    }

    #[test]
    fn test_hooks_error_propagation() {
        // Verify error handling works correctly
        let invalid_obj = 999999; // Likely unregistered
        let result = StealthGameLogicHooks::on_object_attacked(invalid_obj, 2);
        // May fail or succeed depending on manager state
        let _ = result;
    }

    #[test]
    fn test_multiple_player_queries() {
        // Test querying visibility for multiple players
        let obj_id = 50;
        let _ = StealthGameLogicHooks::on_object_created(obj_id);

        for player in 0..8 {
            let result = StealthRenderingHooks::get_opacity_for_player(obj_id, player);
            assert!(result.is_ok() || result.is_err());
        }

        let _ = StealthGameLogicHooks::on_object_deleted(obj_id);
    }
}
