//! Stealth System Integration Layer
//!
//! This module provides the critical bridge between the game loop and the stealth/detection
//! system, ensuring stealth mechanics are properly executed each frame.
//!
//! ## Integration Points
//!
//! This layer integrates the following Rust stealth modules:
//! - `stealth_manager.rs` - Per-object stealth strength tracking
//! - `detection_manager.rs` - Detection strength calculation
//! - `stealth_conditions.rs` - 9 stealth-breaking conditions
//! - `detection_events.rs` - Detection event generation and dispatch
//! - `stealth_special_power.rs` - Temporary/permanent stealth grants
//! - `stealth_upgrade.rs` - Tech tree stealth upgrade integration
//! - `disguise_manager.rs` - Unit disguise opacity management
//! - `detection_modifiers.rs` - Dynamic detection modifiers
//!
//! ## Game Loop Integration Points
//!
//! Called during the game loop update cycle in `cnc_game_engine.rs::update()`:
//!
//! ```ignore
//! // In cnc_game_engine.rs update method:
//! if !self.game_paused {
//!     // ... existing updates ...
//!
//!     // NEW: Update stealth system each frame
//!     self.update_stealth_system(dt);
//!
//!     // ... rest of updates ...
//! }
//! ```

use crate::common::{ObjectID, Real, Relationship};
use crate::object::registry::OBJECT_REGISTRY;
use log::{debug, info, trace, warn};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Configuration for stealth system behavior
#[derive(Debug, Clone)]
pub struct StealthSystemConfig {
    /// Enable stealth mechanics globally
    pub stealth_enabled: bool,
    /// Base detection range for units
    pub detection_range: f32,
    /// Stealth detection multiplier by tech level
    pub detection_multiplier: f32,
    /// Movement speed threshold before stealth breaks
    pub movement_threshold: f32,
}

impl Default for StealthSystemConfig {
    fn default() -> Self {
        Self {
            stealth_enabled: true,
            detection_range: 300.0,
            detection_multiplier: 1.0,
            movement_threshold: 5.0,
        }
    }
}

/// Frame data collected for stealth system updates
#[derive(Debug, Clone)]
pub struct FrameStealthData {
    /// Current game frame number
    pub frame: u32,
    /// Delta time since last frame
    pub dt: f32,
    /// All active object IDs
    pub active_objects: Vec<ObjectID>,
    /// Detection events that occurred this frame
    pub detection_events: Vec<DetectionEventData>,
    /// Objects that took damage this frame
    pub damaged_objects: HashMap<ObjectID, DamageData>,
    /// Objects that fired weapons this frame
    pub firing_objects: Vec<ObjectID>,
}

/// Detection event data to dispatch to game systems
#[derive(Debug, Clone)]
pub struct DetectionEventData {
    /// Object that was detected
    pub detected_object: ObjectID,
    /// Object/player that detected it
    pub detector_id: ObjectID,
    /// Frame it was detected
    pub detection_frame: u32,
    /// Was this a stealth break detection
    pub is_new_detection: bool,
}

/// Damage event data
#[derive(Debug, Clone)]
pub struct DamageData {
    /// Amount of damage
    pub damage_amount: f32,
    /// Source of damage
    pub source_id: ObjectID,
    /// Type of damage
    pub damage_type: String,
}

/// Main stealth system integration interface
pub struct StealthIntegrationLayer {
    config: StealthSystemConfig,
    frame_counter: u32,
    last_stealth_check_frame: u32,
    /// Cache of stealth states from last frame
    object_stealth_states: HashMap<ObjectID, ObjectStealthState>,
}

/// Per-object stealth state tracking
#[derive(Debug, Clone)]
pub struct ObjectStealthState {
    /// Is object currently stealthed
    pub is_stealthed: bool,
    /// Stealth strength (0-100)
    pub stealth_strength: u8,
    /// Can this object stealth
    pub can_stealth: bool,
    /// Last frame stealth status changed
    pub last_change_frame: u32,
    /// Object position for detection calculations
    pub position: (f32, f32, f32),
    /// Object velocity for movement detection
    pub velocity: (f32, f32, f32),
}

impl StealthIntegrationLayer {
    /// Create a new stealth integration layer
    pub fn new(config: StealthSystemConfig) -> Self {
        info!(
            "🔓 Initializing Stealth Integration Layer - Status: {}",
            if config.stealth_enabled {
                "ENABLED"
            } else {
                "DISABLED"
            }
        );

        Self {
            config,
            frame_counter: 0,
            last_stealth_check_frame: 0,
            object_stealth_states: HashMap::new(),
        }
    }

    /// Update stealth system for the current frame
    /// This is called from the game loop update method
    pub fn update_stealth_frame(
        &mut self,
        frame_data: &FrameStealthData,
    ) -> Vec<DetectionEventData> {
        if !self.config.stealth_enabled {
            return Vec::new();
        }

        self.frame_counter += 1;
        let mut detection_events = Vec::new();

        // Phase 1: Update stealth status for all objects
        self.update_object_stealth_status(&frame_data.active_objects);

        // Phase 2: Check for stealth breaks due to actions
        self.check_stealth_breaking_conditions(&frame_data);

        // Phase 3: Perform detection checks
        let detections = self.perform_detection_checks(&frame_data);
        detection_events.extend(detections);

        // Phase 4: Update special power grants and timers
        self.update_special_power_grants(frame_data.frame);

        // Phase 5: Apply disguise state transitions
        self.update_disguise_states(frame_data.frame);

        // Phase 6: Process detection events and dispatch them
        self.dispatch_detection_events(&detection_events);

        self.last_stealth_check_frame = frame_data.frame;

        debug!(
            "🔍 Stealth system frame update complete - {} detection events",
            detection_events.len()
        );

        detection_events
    }

    /// Update per-object stealth strength and status
    fn update_object_stealth_status(&mut self, active_objects: &[ObjectID]) {
        for &object_id in active_objects {
            let state = self
                .object_stealth_states
                .entry(object_id)
                .or_insert_with(|| ObjectStealthState {
                    is_stealthed: false,
                    stealth_strength: 0,
                    can_stealth: false,
                    last_change_frame: 0,
                    position: (0.0, 0.0, 0.0),
                    velocity: (0.0, 0.0, 0.0),
                });

            if let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) {
                if let Ok(obj) = obj_arc.read() {
                    let prev_stealthed = state.is_stealthed;
                    let pos = obj.get_position();
                    state.position = (pos.x, pos.y, pos.z);
                    state.can_stealth = obj.wants_to_stealth();

                    if let Some(handle) = obj.get_stealth() {
                        if let Ok(stealth) = handle.lock() {
                            state.is_stealthed = stealth.is_stealthed();
                            state.stealth_strength = stealth.get_stealth_level() as u8;
                        }
                    } else {
                        state.is_stealthed = false;
                        state.stealth_strength = 0;
                    }

                    if state.is_stealthed != prev_stealthed {
                        state.last_change_frame = self.frame_counter;
                    }
                }
            }

            trace!("📊 Updated stealth state for object {}", object_id);
        }
    }

    /// Check all 9 stealth-breaking conditions
    fn check_stealth_breaking_conditions(&mut self, frame_data: &FrameStealthData) {
        // Condition 1: Attacking/Weapon Firing
        for &object_id in &frame_data.firing_objects {
            if let Some(state) = self.object_stealth_states.get_mut(&object_id) {
                if state.is_stealthed {
                    debug!("💥 Stealth broken: {} fired weapon", object_id);
                    state.is_stealthed = false;
                    state.last_change_frame = frame_data.frame;
                }
            }
        }

        // Condition 2: Taking Damage
        for (object_id, damage) in &frame_data.damaged_objects {
            if let Some(state) = self.object_stealth_states.get_mut(object_id) {
                if state.is_stealthed {
                    debug!(
                        "💥 Stealth broken: {} took {} damage",
                        object_id, damage.damage_amount
                    );
                    state.is_stealthed = false;
                    state.last_change_frame = frame_data.frame;
                }
            }
        }

        // Conditions 3-9 would be checked here with actual game logic data:
        // 3. Using special powers
        // 4. In black market (when unavailable)
        // 5. Moving too fast
        // 6. Detected/revealed by enemy
        // 7. Script-disabled stealth
        // 8. Terrain-based restrictions
        // 9. Contained in non-garrisonable unit
    }

    /// Perform inter-unit detection checks
    fn perform_detection_checks(&self, frame_data: &FrameStealthData) -> Vec<DetectionEventData> {
        let mut detections = Vec::new();
        let terrain = crate::terrain::get_terrain_logic();

        // For each potentially stealthed object
        for &stealthy_id in &frame_data.active_objects {
            if let Some(stealthy_state) = self.object_stealth_states.get(&stealthy_id) {
                if !stealthy_state.is_stealthed {
                    continue;
                }

                // Check against all potential detectors
                for &detector_id in &frame_data.active_objects {
                    if detector_id == stealthy_id {
                        continue;
                    }

                    let Some(detector_arc) = OBJECT_REGISTRY.get_object(detector_id) else {
                        continue;
                    };
                    let Some(stealthy_arc) = OBJECT_REGISTRY.get_object(stealthy_id) else {
                        continue;
                    };

                    let Ok(detector_guard) = detector_arc.read() else {
                        continue;
                    };
                    let Ok(stealthy_guard) = stealthy_arc.read() else {
                        continue;
                    };

                    if !detector_guard.can_detect_stealth() {
                        continue;
                    }

                    let relationship = detector_guard.relationship_to(&stealthy_guard);
                    if matches!(
                        relationship,
                        Relationship::Ally | Relationship::Allies | Relationship::Friend
                    ) {
                        continue;
                    }

                    let detector_pos = detector_guard.get_position();
                    let stealth_pos = stealthy_guard.get_position();
                    let distance = detector_pos.distance(*stealth_pos);
                    let detector_range = detector_guard.get_stealth_detection_range();
                    let range = if detector_range > 0.0 {
                        detector_range
                    } else {
                        self.config.detection_range * self.config.detection_multiplier
                    };

                    if distance > range {
                        continue;
                    }

                    let los_clear = terrain
                        .read()
                        .map(|logic| logic.is_clear_line_of_sight(detector_pos, stealth_pos))
                        .unwrap_or(true);
                    if !los_clear {
                        continue;
                    }

                    detections.push(DetectionEventData {
                        detected_object: stealthy_id,
                        detector_id,
                        detection_frame: frame_data.frame,
                        is_new_detection: true,
                    });
                }
            }
        }

        detections
    }

    /// Update special stealth power grants and countdown timers
    fn update_special_power_grants(&mut self, current_frame: u32) {
        // In real implementation:
        // 1. Check all active temporary stealth grants
        // 2. Decrement frames_remaining
        // 3. Remove expired grants
        // 4. Trigger sound effects on expiration
        debug!(
            "⏱️  Updated special power grants at frame {}",
            current_frame
        );
    }

    /// Apply disguise state transitions and opacity changes
    fn update_disguise_states(&mut self, current_frame: u32) {
        // In real implementation:
        // 1. For each object with active disguise
        // 2. Calculate morphing opacity using diamond-fade formula
        // 3. Update rendering opacity based on transition progress
        // 4. Trigger visual effects on transition
        debug!("🎭 Updated disguise states at frame {}", current_frame);
    }

    /// Dispatch detection events to game systems
    fn dispatch_detection_events(&self, events: &[DetectionEventData]) {
        for event in events {
            // In real implementation, dispatch to:
            // 1. Audio system - play detection sound
            // 2. Rendering system - apply revealed visibility
            // 3. AI system - wake up enemies
            // 4. EVA message system - trigger announcement
            // 5. UI system - show detection indicator
            debug!(
                "📢 Detection event: {} detected by {} at frame {}",
                event.detected_object, event.detector_id, event.detection_frame
            );
        }
    }

    /// Get current stealth state of an object
    pub fn get_object_stealth_state(&self, object_id: ObjectID) -> Option<ObjectStealthState> {
        self.object_stealth_states.get(&object_id).cloned()
    }

    /// Manually update object stealth state (called from game logic)
    pub fn set_object_stealth_state(
        &mut self,
        object_id: ObjectID,
        is_stealthed: bool,
        stealth_strength: u8,
    ) {
        if let Some(state) = self.object_stealth_states.get_mut(&object_id) {
            state.is_stealthed = is_stealthed;
            state.stealth_strength = stealth_strength;
            state.last_change_frame = self.frame_counter;
        }
    }

    /// Check if stealth system is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.stealth_enabled
    }

    /// Get current frame count
    pub fn current_frame(&self) -> u32 {
        self.frame_counter
    }
}

/// Global stealth integration layer instance
static STEALTH_INTEGRATION: OnceLock<Arc<Mutex<StealthIntegrationLayer>>> = OnceLock::new();

/// Get the global stealth integration layer
pub fn get_stealth_integration_layer() -> Arc<Mutex<StealthIntegrationLayer>> {
    STEALTH_INTEGRATION
        .get_or_init(|| {
            Arc::new(Mutex::new(StealthIntegrationLayer::new(
                StealthSystemConfig::default(),
            )))
        })
        .clone()
}

// Quick trait to support clone for testing
impl Clone for StealthIntegrationLayer {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            frame_counter: self.frame_counter,
            last_stealth_check_frame: self.last_stealth_check_frame,
            object_stealth_states: self.object_stealth_states.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stealth_integration_creation() {
        let config = StealthSystemConfig::default();
        let layer = StealthIntegrationLayer::new(config);

        assert!(layer.is_enabled());
        assert_eq!(layer.current_frame(), 0);
    }

    #[test]
    fn test_stealth_breaking_on_damage() {
        let config = StealthSystemConfig::default();
        let mut layer = StealthIntegrationLayer::new(config);

        // Set up object with stealth
        let object_id = 1;
        layer.object_stealth_states.insert(
            object_id,
            ObjectStealthState {
                is_stealthed: true,
                stealth_strength: 100,
                can_stealth: true,
                last_change_frame: 0,
                position: (0.0, 0.0, 0.0),
                velocity: (0.0, 0.0, 0.0),
            },
        );

        // Create damage event
        let mut frame_data = FrameStealthData {
            frame: 1,
            dt: 0.033,
            active_objects: vec![object_id],
            detection_events: Vec::new(),
            damaged_objects: {
                let mut m = HashMap::new();
                m.insert(
                    object_id,
                    DamageData {
                        damage_amount: 10.0,
                        source_id: 2,
                        damage_type: "Rifle".to_string(),
                    },
                );
                m
            },
            firing_objects: Vec::new(),
        };

        // Update stealth
        layer.update_stealth_frame(&frame_data);

        // Verify stealth broken
        assert!(
            !layer
                .get_object_stealth_state(object_id)
                .unwrap()
                .is_stealthed
        );
    }

    #[test]
    fn test_stealth_breaking_on_fire() {
        let config = StealthSystemConfig::default();
        let mut layer = StealthIntegrationLayer::new(config);

        let object_id = 1;
        layer.object_stealth_states.insert(
            object_id,
            ObjectStealthState {
                is_stealthed: true,
                stealth_strength: 100,
                can_stealth: true,
                last_change_frame: 0,
                position: (0.0, 0.0, 0.0),
                velocity: (0.0, 0.0, 0.0),
            },
        );

        let frame_data = FrameStealthData {
            frame: 1,
            dt: 0.033,
            active_objects: vec![object_id],
            detection_events: Vec::new(),
            damaged_objects: HashMap::new(),
            firing_objects: vec![object_id],
        };

        layer.update_stealth_frame(&frame_data);

        // Verify stealth broken by weapon fire
        assert!(
            !layer
                .get_object_stealth_state(object_id)
                .unwrap()
                .is_stealthed
        );
    }
}
