//! Detection Events System
//!
//! Manages detection events for stealth detection mechanics, including:
//! - RADAR_EVENT_STEALTH_DISCOVERED event generation
//! - Eva messages (enemy detection, own unit detection)
//! - Sound events (quiet ping, loud ping)
//! - Particle system triggers (IR beacon, IR ping, IR grid, IR bright)
//! - Detection feedback per detector type
//! - Audio and UI coordination
//!
//! Faithful to C++ implementation in StealthDetectorUpdate
//!
//! EVA Message Events (matching C++ StealthUpdate.h):
//! - m_enemyDetectionEvaEvent: Fires when YOUR stealthed unit is detected by ENEMY
//! - m_ownDetectionEvaEvent: Fires when YOUR UNIT detects ENEMY stealth
//!
//! This system ensures players receive audio/visual feedback when stealth is involved
//! in detection scenarios, coordinating between game logic, rendering, and audio systems.

use crate::common::{Coord3D, ObjectID, UnsignedInt};
use log::{debug, info, trace, warn};
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::sync::OnceLock;

/// Maximum number of players in game.
const MAX_PLAYER_COUNT: usize = crate::common::MAX_PLAYER_COUNT;

/// Detection event types for radar and UI feedback
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionEventType {
    /// RADAR_EVENT_STEALTH_DISCOVERED - Stealth unit was detected
    RadarEventStealthDiscovered = 0,

    /// Quiet ping sound - subtle detection feedback
    QuietPing = 1,

    /// Loud ping sound - confirmed detection feedback
    LoudPing = 2,

    /// IR Beacon particle system - detection indicator
    IRBeaconActivated = 3,

    /// IR Grid particle system - detection overlay
    IRGridOverlay = 4,

    /// IR Ping particle system - detection pulse
    IRPing = 5,

    /// IR Bright particle system - strong detection indicator
    IRBright = 6,
}

impl DetectionEventType {
    /// Convert event type to description string
    pub fn as_str(&self) -> &str {
        match self {
            DetectionEventType::RadarEventStealthDiscovered => "RadarEventStealthDiscovered",
            DetectionEventType::QuietPing => "QuietPing",
            DetectionEventType::LoudPing => "LoudPing",
            DetectionEventType::IRBeaconActivated => "IRBeaconActivated",
            DetectionEventType::IRGridOverlay => "IRGridOverlay",
            DetectionEventType::IRPing => "IRPing",
            DetectionEventType::IRBright => "IRBright",
        }
    }
}

/// Audio event sound types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioEventType {
    /// Quiet ping - initial detection
    QuietPing = 0,

    /// Loud ping - confirmed detection
    LoudPing = 1,
}

impl AudioEventType {
    /// Convert audio event type to description string
    pub fn as_str(&self) -> &str {
        match self {
            AudioEventType::QuietPing => "QuietPing",
            AudioEventType::LoudPing => "LoudPing",
        }
    }
}

/// Eva message types for player feedback
///
/// These messages provide audio/visual notification to players when stealth detection occurs.
/// Matches C++ StealthUpdate.h Eva event firing mechanisms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaMessageType {
    /// Enemy stealth unit detected by your unit (STEALTH_DISCOVERED)
    /// Played when: YOUR UNIT detects ENEMY stealth
    /// Localized message: "Stealth unit discovered!"
    EnemyDetected = 0,

    /// Own stealth unit was detected by enemy (UNIT_DISCOVERED)
    /// Played when: ENEMY detects YOUR stealthed unit
    /// Localized message: "Unit discovered!"
    OwnUnitDetected = 1,

    /// Stealth discovered - generic stealth detection notification
    /// Localized message: "Stealth discovered!"
    StealthDiscovered = 2,

    /// Unit discovered - generic unit detection notification
    /// Localized message: "Unit discovered!"
    UnitDiscovered = 3,
}

impl EvaMessageType {
    /// Convert message type to description string
    pub fn as_str(&self) -> &str {
        match self {
            EvaMessageType::EnemyDetected => "EnemyDetected",
            EvaMessageType::OwnUnitDetected => "OwnUnitDetected",
            EvaMessageType::StealthDiscovered => "StealthDiscovered",
            EvaMessageType::UnitDiscovered => "UnitDiscovered",
        }
    }

    /// Get localized message string for this Eva event
    pub fn get_localized_message(&self) -> &str {
        match self {
            EvaMessageType::EnemyDetected => "Stealth unit discovered!",
            EvaMessageType::OwnUnitDetected => "Unit discovered!",
            EvaMessageType::StealthDiscovered => "Stealth discovered!",
            EvaMessageType::UnitDiscovered => "Unit discovered!",
        }
    }
}

/// Detection event to be processed by rendering, audio, and UI systems
#[derive(Debug, Clone)]
pub enum DetectionEvent {
    /// Stealth discovery event with detection details
    StealthDiscovered {
        /// Object that was detected
        object_id: ObjectID,

        /// Object that detected the stealth
        detector_id: ObjectID,

        /// Frame when detection occurred
        frame: UnsignedInt,

        /// Player who owns the detector
        player_id: usize,
    },

    /// Radar event for UI feedback
    RadarEvent {
        /// Object involved in event
        object_id: ObjectID,

        /// Type of radar event
        event_type: DetectionEventType,
    },

    /// Audio event for sound playback
    AudioEvent {
        /// Type of sound to play
        sound_type: AudioEventType,

        /// Position to play sound at
        position: Coord3D,
    },

    /// Eva message for player notification
    EvaMessage {
        /// Type of message
        message_type: EvaMessageType,

        /// Player who receives the message
        player_id: usize,

        /// The detected unit ID (for context in UI/audio systems)
        detected_unit_id: ObjectID,
    },
}

/// Per-object detection event history
#[derive(Debug, Clone)]
struct ObjectDetectionHistory {
    /// Object ID being tracked
    object_id: ObjectID,

    /// List of detection events for this object
    events: Vec<DetectionEvent>,

    /// Last frame when detection occurred
    last_detection_frame: UnsignedInt,
}

impl ObjectDetectionHistory {
    /// Create new detection history for object
    fn new(object_id: ObjectID) -> Self {
        Self {
            object_id,
            events: Vec::new(),
            last_detection_frame: 0,
        }
    }
}

/// Eva event entry in the pending queue
#[derive(Debug, Clone)]
pub struct PendingEvaEvent {
    /// Message type to fire
    message_type: EvaMessageType,

    /// Player who receives this message
    player_id: usize,

    /// Unit that triggered this detection
    detected_unit_id: ObjectID,

    /// Frame when Eva event was queued
    frame: UnsignedInt,
}

/// Detection Events Manager singleton
///
/// Manages detection events and coordinates between detection system,
/// rendering, audio, and UI systems.
///
/// Integrates Eva message firing for stealth detection feedback, matching
/// C++ StealthUpdate.h implementation (m_enemyDetectionEvaEvent, m_ownDetectionEvaEvent).
pub struct DetectionEventManager {
    /// Queue of pending events to process
    pending_events: VecDeque<DetectionEvent>,

    /// Per-object detection event history
    event_history: HashMap<ObjectID, ObjectDetectionHistory>,

    /// Queue of pending Eva message events
    pending_eva_events: VecDeque<PendingEvaEvent>,

    /// Last frame events were processed
    last_process_frame: UnsignedInt,

    /// Mapping of (detector_player_id, detected_object_id) to track whether Eva message was sent
    /// This prevents duplicate Eva messages for the same detection pair
    eva_event_cache: HashMap<(usize, ObjectID), UnsignedInt>,
}

impl DetectionEventManager {
    /// Create new DetectionEventManager
    pub fn new() -> Self {
        Self {
            pending_events: VecDeque::new(),
            event_history: HashMap::new(),
            pending_eva_events: VecDeque::new(),
            last_process_frame: 0,
            eva_event_cache: HashMap::new(),
        }
    }

    /// Register detection between detector and target
    ///
    /// This is the main entry point for detection system integration.
    /// Validates inputs and creates appropriate events including Eva messages.
    ///
    /// # Arguments
    /// * `detector_id` - Object that is detecting
    /// * `object_id` - Object being detected
    /// * `frame` - Current game frame
    /// * `detector_player_id` - Player who owns the detector
    /// * `detected_player_id` - Player who owns the detected object
    pub fn register_detection(
        &mut self,
        detector_id: ObjectID,
        object_id: ObjectID,
        frame: UnsignedInt,
        detector_player_id: usize,
        detected_player_id: usize,
    ) -> Result<(), String> {
        if detector_id == 0 {
            return Err("Invalid detector_id: 0".to_string());
        }
        if object_id == 0 {
            return Err("Invalid object_id: 0".to_string());
        }
        if detector_player_id >= MAX_PLAYER_COUNT {
            return Err(format!(
                "Invalid detector_player_id: {}",
                detector_player_id
            ));
        }
        if detected_player_id >= MAX_PLAYER_COUNT {
            return Err(format!(
                "Invalid detected_player_id: {}",
                detected_player_id
            ));
        }

        // Create stealth discovered event
        let event = DetectionEvent::StealthDiscovered {
            object_id,
            detector_id,
            frame,
            player_id: detector_player_id,
        };

        self.queue_event(event.clone())?;

        // Create radar event
        let radar_event = DetectionEvent::RadarEvent {
            object_id,
            event_type: DetectionEventType::RadarEventStealthDiscovered,
        };
        self.queue_event(radar_event)?;

        // Fire Eva messages if different players (hostile detection)
        if detector_player_id != detected_player_id {
            self.fire_detection_eva_messages(
                detector_id,
                object_id,
                detector_player_id,
                detected_player_id,
                frame,
            )?;
        }

        debug!(
            "Registered detection: object {} (player {}) detected by {} (player {}) at frame {}",
            object_id, detected_player_id, detector_id, detector_player_id, frame
        );

        Ok(())
    }

    /// Fire appropriate Eva messages for stealth detection
    ///
    /// Implements C++ StealthUpdate.h behavior:
    /// - When enemy detects your stealthed unit → fire m_enemyDetectionEvaEvent to enemy
    /// - When your unit detects enemy stealth → fire m_ownDetectionEvaEvent to you
    fn fire_detection_eva_messages(
        &mut self,
        detector_id: ObjectID,
        detected_object_id: ObjectID,
        detector_player_id: usize,
        detected_player_id: usize,
        frame: UnsignedInt,
    ) -> Result<(), String> {
        // Check if we've already sent an Eva event for this pair (prevent duplicates)
        let cache_key = (detector_player_id, detected_object_id);
        if let Some(cached_frame) = self.eva_event_cache.get(&cache_key) {
            // If we already sent this within the same frame, don't duplicate
            if *cached_frame == frame {
                return Ok(());
            }
        }

        // Update cache
        self.eva_event_cache.insert(cache_key, frame);

        // Fire Eva message to detector's player (your unit detected enemy stealth)
        self.queue_eva_event(
            detector_player_id,
            EvaMessageType::EnemyDetected,
            detected_object_id,
        )?;

        // Fire Eva message to detected object's player (your unit was discovered)
        self.queue_eva_event(
            detected_player_id,
            EvaMessageType::OwnUnitDetected,
            detected_object_id,
        )?;

        info!(
            "Fired Eva messages for detection: detector {} (player {}) detected object {} (player {})",
            detector_id, detector_player_id, detected_object_id, detected_player_id
        );

        Ok(())
    }

    /// Create a radar event for detection feedback
    pub fn create_radar_event(
        &self,
        object_id: ObjectID,
        event_type: DetectionEventType,
    ) -> Result<DetectionEvent, String> {
        if object_id == 0 {
            return Err("Invalid object_id: 0".to_string());
        }

        Ok(DetectionEvent::RadarEvent {
            object_id,
            event_type,
        })
    }

    /// Create an audio event for ping sound playback
    pub fn create_audio_event(
        &self,
        sound_type: AudioEventType,
        position: Coord3D,
    ) -> Result<DetectionEvent, String> {
        Ok(DetectionEvent::AudioEvent {
            sound_type,
            position,
        })
    }

    /// Create an Eva message for player notification
    pub fn create_eva_message(
        &self,
        message_type: EvaMessageType,
        player_id: usize,
        detected_unit_id: ObjectID,
    ) -> Result<DetectionEvent, String> {
        if player_id >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player_id: {}", player_id));
        }
        if detected_unit_id == 0 {
            return Err("Invalid detected_unit_id: 0".to_string());
        }

        Ok(DetectionEvent::EvaMessage {
            message_type,
            player_id,
            detected_unit_id,
        })
    }

    /// Fire enemy detection Eva message (your unit detected enemy stealth)
    ///
    /// This corresponds to m_ownDetectionEvaEvent in C++ StealthUpdate.h
    /// Fires to the player whose unit performed the detection.
    pub fn fire_enemy_detection_eva(
        &mut self,
        player_id: usize,
        detected_unit_id: ObjectID,
    ) -> Result<(), String> {
        self.queue_eva_event(player_id, EvaMessageType::EnemyDetected, detected_unit_id)
    }

    /// Fire own detection Eva message (your stealthed unit was detected)
    ///
    /// This corresponds to m_enemyDetectionEvaEvent in C++ StealthUpdate.h
    /// Fires to the player whose unit was detected.
    pub fn fire_own_detection_eva(
        &mut self,
        player_id: usize,
        detected_unit_id: ObjectID,
    ) -> Result<(), String> {
        self.queue_eva_event(player_id, EvaMessageType::OwnUnitDetected, detected_unit_id)
    }

    /// Queue an Eva message event for later dispatch
    ///
    /// Eva messages are queued separately and processed by the audio/UI systems.
    /// This allows batching of Eva messages for efficient processing.
    pub fn queue_eva_event(
        &mut self,
        player_id: usize,
        message_type: EvaMessageType,
        detected_unit_id: ObjectID,
    ) -> Result<(), String> {
        if player_id >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player_id: {}", player_id));
        }
        if detected_unit_id == 0 {
            return Err("Invalid detected_unit_id: 0".to_string());
        }

        let eva_event = PendingEvaEvent {
            message_type,
            player_id,
            detected_unit_id,
            frame: self.last_process_frame,
        };

        self.pending_eva_events.push_back(eva_event);

        // Also queue as a detection event for processing
        let event = DetectionEvent::EvaMessage {
            message_type,
            player_id,
            detected_unit_id,
        };
        self.pending_events.push_back(event);

        trace!(
            "Queued Eva event: {} for player {} (unit: {})",
            message_type.as_str(),
            player_id,
            detected_unit_id
        );

        Ok(())
    }

    /// Get all pending Eva messages without removing them
    ///
    /// Returns pending Eva messages that need to be dispatched to the audio/UI systems.
    pub fn get_pending_eva_events(&self) -> Vec<PendingEvaEvent> {
        self.pending_eva_events.iter().cloned().collect()
    }

    /// Dequeue the next pending Eva message event
    pub fn dequeue_eva_event(&mut self) -> Option<PendingEvaEvent> {
        self.pending_eva_events.pop_front()
    }

    /// Clear all pending Eva events
    pub fn clear_pending_eva_events(&mut self) {
        self.pending_eva_events.clear();
        trace!("Cleared all pending Eva events");
    }

    /// Get number of pending Eva events
    pub fn pending_eva_event_count(&self) -> usize {
        self.pending_eva_events.len()
    }

    /// Queue an event for processing
    pub fn queue_event(&mut self, event: DetectionEvent) -> Result<(), String> {
        // Track in history if it's a stealth discovered event
        if let DetectionEvent::StealthDiscovered {
            object_id, frame, ..
        } = &event
        {
            let history = self
                .event_history
                .entry(*object_id)
                .or_insert_with(|| ObjectDetectionHistory::new(*object_id));
            history.events.push(event.clone());
            history.last_detection_frame = *frame;
        }

        self.pending_events.push_back(event);
        trace!("Queued detection event");
        Ok(())
    }

    /// Dequeue the next pending event
    pub fn dequeue_event(&mut self) -> Option<DetectionEvent> {
        self.pending_events.pop_front()
    }

    /// Get all pending events without removing them
    pub fn peek_all_events(&self) -> Vec<DetectionEvent> {
        self.pending_events.iter().cloned().collect()
    }

    /// Get detection history for a specific object
    pub fn get_detection_history(&self, object_id: ObjectID) -> Vec<DetectionEvent> {
        self.event_history
            .get(&object_id)
            .map(|h| h.events.clone())
            .unwrap_or_default()
    }

    /// Get last detection frame for an object
    pub fn get_last_detection_frame(&self, object_id: ObjectID) -> UnsignedInt {
        self.event_history
            .get(&object_id)
            .map(|h| h.last_detection_frame)
            .unwrap_or(0)
    }

    /// Clear detection history for a specific object
    pub fn clear_history(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.event_history.remove(&object_id);
        trace!("Cleared detection history for object {}", object_id);
        Ok(())
    }

    /// Clear all pending events
    pub fn clear_pending_events(&mut self) {
        self.pending_events.clear();
        trace!("Cleared all pending detection events");
    }

    /// Process all pending events and return them
    ///
    /// Consumes all queued events and returns them for system processing.
    /// This is called by the game logic update loop to dispatch events.
    pub fn process_all_events(&mut self) -> Vec<DetectionEvent> {
        let mut events = Vec::new();
        while let Some(event) = self.dequeue_event() {
            events.push(event);
        }
        self.last_process_frame = 0; // Reset for next frame
        trace!("Processed {} detection events", events.len());
        events
    }

    /// Get number of pending events
    pub fn pending_event_count(&self) -> usize {
        self.pending_events.len()
    }

    /// Check if there are any pending events
    pub fn has_pending_events(&self) -> bool {
        !self.pending_events.is_empty()
    }

    /// Get total number of detection histories
    pub fn history_count(&self) -> usize {
        self.event_history.len()
    }

    /// Update last process frame
    pub fn set_process_frame(&mut self, frame: UnsignedInt) {
        self.last_process_frame = frame;
    }

    /// Get last process frame
    pub fn get_last_process_frame(&self) -> UnsignedInt {
        self.last_process_frame
    }
}

impl Default for DetectionEventManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global singleton accessor for DetectionEventManager
static DETECTION_EVENTS_MANAGER: OnceLock<Mutex<DetectionEventManager>> = OnceLock::new();

/// Get the global DetectionEventManager singleton
pub fn get_detection_events_manager() -> &'static Mutex<DetectionEventManager> {
    DETECTION_EVENTS_MANAGER.get_or_init(|| Mutex::new(DetectionEventManager::new()))
}

#[cfg(test)]
mod detection_event_tests {
    use super::*;

    #[test]
    fn test_detection_event_basic() {
        let mut manager = DetectionEventManager::new();

        // Register a basic detection between different players
        assert!(manager.register_detection(1, 2, 100, 0, 1).is_ok());
        // Should have: StealthDiscovered + RadarEvent + 2 Eva messages
        assert!(manager.pending_event_count() >= 2);
    }

    #[test]
    fn test_radar_event_creation() {
        let manager = DetectionEventManager::new();

        // Create radar event
        let event = manager.create_radar_event(1, DetectionEventType::RadarEventStealthDiscovered);
        assert!(event.is_ok());

        if let DetectionEvent::RadarEvent {
            object_id,
            event_type,
        } = event.unwrap()
        {
            assert_eq!(object_id, 1);
            assert_eq!(event_type, DetectionEventType::RadarEventStealthDiscovered);
        } else {
            panic!("Expected RadarEvent");
        }
    }

    #[test]
    fn test_audio_event_types() {
        let manager = DetectionEventManager::new();
        let position = glam::Vec3::new(10.0, 20.0, 30.0);

        // Test quiet ping
        let quiet = manager.create_audio_event(AudioEventType::QuietPing, position);
        assert!(quiet.is_ok());

        if let DetectionEvent::AudioEvent { sound_type, .. } = quiet.unwrap() {
            assert_eq!(sound_type, AudioEventType::QuietPing);
        } else {
            panic!("Expected AudioEvent");
        }

        // Test loud ping
        let loud = manager.create_audio_event(AudioEventType::LoudPing, position);
        assert!(loud.is_ok());

        if let DetectionEvent::AudioEvent { sound_type, .. } = loud.unwrap() {
            assert_eq!(sound_type, AudioEventType::LoudPing);
        } else {
            panic!("Expected AudioEvent");
        }
    }

    #[test]
    fn test_eva_message_events() {
        let manager = DetectionEventManager::new();

        // Create enemy detected message
        let enemy_msg = manager.create_eva_message(EvaMessageType::EnemyDetected, 0, 5);
        assert!(enemy_msg.is_ok());

        if let DetectionEvent::EvaMessage {
            message_type,
            player_id,
            detected_unit_id,
        } = enemy_msg.unwrap()
        {
            assert_eq!(message_type, EvaMessageType::EnemyDetected);
            assert_eq!(player_id, 0);
            assert_eq!(detected_unit_id, 5);
        } else {
            panic!("Expected EvaMessage");
        }

        // Create own unit detected message
        let own_msg = manager.create_eva_message(EvaMessageType::OwnUnitDetected, 3, 10);
        assert!(own_msg.is_ok());

        if let DetectionEvent::EvaMessage {
            message_type,
            player_id,
            detected_unit_id,
        } = own_msg.unwrap()
        {
            assert_eq!(message_type, EvaMessageType::OwnUnitDetected);
            assert_eq!(player_id, 3);
            assert_eq!(detected_unit_id, 10);
        } else {
            panic!("Expected EvaMessage");
        }
    }

    #[test]
    fn test_event_queuing() {
        let mut manager = DetectionEventManager::new();

        // Queue multiple events
        let event1 = DetectionEvent::RadarEvent {
            object_id: 1,
            event_type: DetectionEventType::QuietPing,
        };
        let event2 = DetectionEvent::RadarEvent {
            object_id: 2,
            event_type: DetectionEventType::LoudPing,
        };

        assert!(manager.queue_event(event1).is_ok());
        assert!(manager.queue_event(event2).is_ok());

        assert_eq!(manager.pending_event_count(), 2);

        // Dequeue in order
        let first = manager.dequeue_event();
        assert!(first.is_some());
        assert_eq!(manager.pending_event_count(), 1);

        let second = manager.dequeue_event();
        assert!(second.is_some());
        assert_eq!(manager.pending_event_count(), 0);
    }

    #[test]
    fn test_event_history() {
        let mut manager = DetectionEventManager::new();

        // Register multiple detections for same object (same player = no Eva messages)
        assert!(manager.register_detection(1, 2, 100, 0, 0).is_ok());
        assert!(manager.register_detection(1, 2, 200, 0, 0).is_ok());

        // Check history
        let history = manager.get_detection_history(2);
        assert_eq!(history.len(), 2);

        // Check last detection frame
        let last_frame = manager.get_last_detection_frame(2);
        assert_eq!(last_frame, 200);
    }

    #[test]
    fn test_multiple_events() {
        let mut manager = DetectionEventManager::new();

        // Register detection between different players
        assert!(manager.register_detection(1, 2, 100, 0, 1).is_ok());

        // Queue additional events
        let audio = manager.create_audio_event(AudioEventType::LoudPing, glam::Vec3::ZERO);
        assert!(manager.queue_event(audio.unwrap()).is_ok());

        let msg = manager.create_eva_message(EvaMessageType::EnemyDetected, 0, 5);
        assert!(manager.queue_event(msg.unwrap()).is_ok());

        // Should have multiple events
        assert!(manager.pending_event_count() > 2);
    }

    #[test]
    fn test_event_clearing() {
        let mut manager = DetectionEventManager::new();

        // Register detection
        assert!(manager.register_detection(1, 2, 100, 0, 1).is_ok());
        assert!(manager.pending_event_count() > 0);

        // Clear pending events
        manager.clear_pending_events();
        assert_eq!(manager.pending_event_count(), 0);

        // History should still exist
        assert!(!manager.get_detection_history(2).is_empty());

        // Clear history
        assert!(manager.clear_history(2).is_ok());
        assert!(manager.get_detection_history(2).is_empty());
    }

    #[test]
    fn test_loud_vs_quiet_ping() {
        let manager = DetectionEventManager::new();

        let quiet_ping = manager.create_audio_event(AudioEventType::QuietPing, glam::Vec3::ZERO);
        let loud_ping = manager.create_audio_event(AudioEventType::LoudPing, glam::Vec3::ZERO);

        assert!(quiet_ping.is_ok());
        assert!(loud_ping.is_ok());

        if let DetectionEvent::AudioEvent {
            sound_type: quiet_type,
            ..
        } = quiet_ping.unwrap()
        {
            if let DetectionEvent::AudioEvent {
                sound_type: loud_type,
                ..
            } = loud_ping.unwrap()
            {
                assert_ne!(quiet_type, loud_type);
                assert_eq!(quiet_type, AudioEventType::QuietPing);
                assert_eq!(loud_type, AudioEventType::LoudPing);
            } else {
                panic!("Expected AudioEvent for loud ping");
            }
        } else {
            panic!("Expected AudioEvent for quiet ping");
        }
    }

    #[test]
    fn test_detection_feedback_coordination() {
        let mut manager = DetectionEventManager::new();

        // Register detection (creates stealth discovered and radar event + Eva messages)
        assert!(manager.register_detection(1, 2, 100, 0, 1).is_ok());

        // Process all events
        let events = manager.process_all_events();

        // Should have stealth discovered, radar event, and Eva messages
        let has_stealth = events
            .iter()
            .any(|e| matches!(e, DetectionEvent::StealthDiscovered { .. }));
        let has_radar = events
            .iter()
            .any(|e| matches!(e, DetectionEvent::RadarEvent { .. }));
        let has_eva = events
            .iter()
            .any(|e| matches!(e, DetectionEvent::EvaMessage { .. }));

        assert!(has_stealth, "Should have stealth discovered event");
        assert!(has_radar, "Should have radar event");
        assert!(has_eva, "Should have Eva message event");

        // Queue should be empty after processing
        assert_eq!(manager.pending_event_count(), 0);
    }

    #[test]
    fn test_invalid_inputs() {
        let mut manager = DetectionEventManager::new();

        // Invalid detector ID (0)
        assert!(manager.register_detection(0, 2, 100, 0, 1).is_err());

        // Invalid object ID (0)
        assert!(manager.register_detection(1, 0, 100, 0, 1).is_err());

        // Invalid detector player ID (8, max is 7)
        assert!(manager.register_detection(1, 2, 100, 8, 1).is_err());

        // Invalid detected player ID (8, max is 7)
        assert!(manager.register_detection(1, 2, 100, 0, 8).is_err());

        // Invalid player ID for Eva message
        assert!(manager
            .create_eva_message(EvaMessageType::EnemyDetected, 8, 5)
            .is_err());

        // Invalid detected unit ID (0)
        assert!(manager
            .create_eva_message(EvaMessageType::EnemyDetected, 0, 0)
            .is_err());
    }

    #[test]
    fn test_event_type_strings() {
        assert_eq!(
            DetectionEventType::RadarEventStealthDiscovered.as_str(),
            "RadarEventStealthDiscovered"
        );
        assert_eq!(DetectionEventType::QuietPing.as_str(), "QuietPing");
        assert_eq!(DetectionEventType::LoudPing.as_str(), "LoudPing");
        assert_eq!(
            DetectionEventType::IRBeaconActivated.as_str(),
            "IRBeaconActivated"
        );
        assert_eq!(DetectionEventType::IRGridOverlay.as_str(), "IRGridOverlay");
        assert_eq!(DetectionEventType::IRPing.as_str(), "IRPing");
        assert_eq!(DetectionEventType::IRBright.as_str(), "IRBright");

        assert_eq!(AudioEventType::QuietPing.as_str(), "QuietPing");
        assert_eq!(AudioEventType::LoudPing.as_str(), "LoudPing");

        assert_eq!(EvaMessageType::EnemyDetected.as_str(), "EnemyDetected");
        assert_eq!(EvaMessageType::OwnUnitDetected.as_str(), "OwnUnitDetected");
        assert_eq!(
            EvaMessageType::StealthDiscovered.as_str(),
            "StealthDiscovered"
        );
        assert_eq!(EvaMessageType::UnitDiscovered.as_str(), "UnitDiscovered");

        // Test localized messages
        assert_eq!(
            EvaMessageType::EnemyDetected.get_localized_message(),
            "Stealth unit discovered!"
        );
        assert_eq!(
            EvaMessageType::OwnUnitDetected.get_localized_message(),
            "Unit discovered!"
        );
        assert_eq!(
            EvaMessageType::StealthDiscovered.get_localized_message(),
            "Stealth discovered!"
        );
        assert_eq!(
            EvaMessageType::UnitDiscovered.get_localized_message(),
            "Unit discovered!"
        );
    }

    #[test]
    fn test_singleton_access() {
        let manager1 = get_detection_events_manager();
        let manager2 = get_detection_events_manager();

        // Should be the same singleton
        assert!(
            std::ptr::eq(manager1, manager2),
            "Should return the same singleton instance"
        );
    }

    #[test]
    fn test_peek_all_events() {
        let mut manager = DetectionEventManager::new();

        // Queue some events
        assert!(manager.register_detection(1, 2, 100, 0, 1).is_ok());
        let count_before = manager.pending_event_count();

        // Peek all events
        let peeked = manager.peek_all_events();
        assert_eq!(peeked.len(), count_before);

        // Should not remove from queue
        assert_eq!(manager.pending_event_count(), count_before);
    }

    // New Eva message tests

    #[test]
    fn test_enemy_detection_eva_fires_to_correct_player() {
        let mut manager = DetectionEventManager::new();

        // Enemy detects your stealth unit
        assert!(manager.register_detection(10, 20, 100, 1, 0).is_ok());

        // Should queue Eva events for both players
        let events = manager.process_all_events();

        // Find Eva messages in events
        let eva_messages: Vec<_> = events
            .iter()
            .filter_map(|e| {
                if let DetectionEvent::EvaMessage {
                    message_type,
                    player_id,
                    detected_unit_id,
                } = e
                {
                    Some((*message_type, *player_id, *detected_unit_id))
                } else {
                    None
                }
            })
            .collect();

        // Should have 2 Eva messages: one for detector (player 1), one for detected (player 0)
        assert!(
            eva_messages.len() >= 2,
            "Should have at least 2 Eva messages"
        );

        // Verify both players receive messages
        assert!(
            eva_messages.iter().any(|(_, player_id, _)| *player_id == 0),
            "Player 0 should receive message"
        );
        assert!(
            eva_messages.iter().any(|(_, player_id, _)| *player_id == 1),
            "Player 1 should receive message"
        );
    }

    #[test]
    fn test_own_detection_eva_not_fired_for_allies() {
        let mut manager = DetectionEventManager::new();

        // Ally detects another ally (same player)
        assert!(manager.register_detection(10, 20, 100, 0, 0).is_ok());

        // Should NOT queue Eva events for allies
        let eva_count = manager.pending_eva_event_count();
        assert_eq!(
            eva_count, 0,
            "Should not fire Eva messages for friendly detections"
        );
    }

    #[test]
    fn test_queue_eva_event() {
        let mut manager = DetectionEventManager::new();

        // Directly queue an Eva event
        assert!(manager
            .queue_eva_event(0, EvaMessageType::EnemyDetected, 5)
            .is_ok());

        // Should appear in both queues
        assert_eq!(manager.pending_eva_event_count(), 1);
        assert!(manager.pending_event_count() > 0);

        // Verify content
        let eva = manager.dequeue_eva_event();
        assert!(eva.is_some());
        if let Some(event) = eva {
            assert_eq!(event.player_id, 0);
            assert_eq!(event.message_type, EvaMessageType::EnemyDetected);
            assert_eq!(event.detected_unit_id, 5);
        }
    }

    #[test]
    fn test_fire_enemy_detection_eva() {
        let mut manager = DetectionEventManager::new();

        // Fire enemy detected message
        assert!(manager.fire_enemy_detection_eva(0, 10).is_ok());

        // Should be queued
        assert_eq!(manager.pending_eva_event_count(), 1);

        let events = manager.process_all_events();
        let has_enemy_detected = events.iter().any(|e| {
            if let DetectionEvent::EvaMessage {
                message_type,
                player_id,
                detected_unit_id,
            } = e
            {
                *message_type == EvaMessageType::EnemyDetected
                    && *player_id == 0
                    && *detected_unit_id == 10
            } else {
                false
            }
        });

        assert!(
            has_enemy_detected,
            "Should have EnemyDetected message for player 0"
        );
    }

    #[test]
    fn test_fire_own_detection_eva() {
        let mut manager = DetectionEventManager::new();

        // Fire own unit detected message
        assert!(manager.fire_own_detection_eva(2, 15).is_ok());

        // Should be queued
        assert_eq!(manager.pending_eva_event_count(), 1);

        let events = manager.process_all_events();
        let has_own_detected = events.iter().any(|e| {
            if let DetectionEvent::EvaMessage {
                message_type,
                player_id,
                detected_unit_id,
            } = e
            {
                *message_type == EvaMessageType::OwnUnitDetected
                    && *player_id == 2
                    && *detected_unit_id == 15
            } else {
                false
            }
        });

        assert!(
            has_own_detected,
            "Should have OwnUnitDetected message for player 2"
        );
    }

    #[test]
    fn test_multiple_eva_events_queue() {
        let mut manager = DetectionEventManager::new();

        // Queue multiple Eva events
        assert!(manager
            .queue_eva_event(0, EvaMessageType::EnemyDetected, 1)
            .is_ok());
        assert!(manager
            .queue_eva_event(1, EvaMessageType::OwnUnitDetected, 2)
            .is_ok());
        assert!(manager
            .queue_eva_event(2, EvaMessageType::StealthDiscovered, 3)
            .is_ok());

        // Should all be queued
        assert_eq!(manager.pending_eva_event_count(), 3);

        // Process and verify all are present
        let events = manager.process_all_events();
        let eva_messages: Vec<_> = events
            .iter()
            .filter_map(|e| {
                if let DetectionEvent::EvaMessage { .. } = e {
                    Some(e)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(eva_messages.len(), 3, "Should have all 3 Eva messages");
    }

    #[test]
    fn test_get_pending_eva_events() {
        let mut manager = DetectionEventManager::new();

        // Queue some Eva events
        assert!(manager
            .queue_eva_event(0, EvaMessageType::EnemyDetected, 5)
            .is_ok());
        assert!(manager
            .queue_eva_event(1, EvaMessageType::OwnUnitDetected, 10)
            .is_ok());

        // Get without removing
        let pending = manager.get_pending_eva_events();
        assert_eq!(pending.len(), 2);
        assert_eq!(manager.pending_eva_event_count(), 2);

        // Verify content
        assert!(pending
            .iter()
            .any(|e| e.player_id == 0 && e.detected_unit_id == 5));
        assert!(pending
            .iter()
            .any(|e| e.player_id == 1 && e.detected_unit_id == 10));
    }

    #[test]
    fn test_eva_event_cache_prevents_duplicates() {
        let mut manager = DetectionEventManager::new();

        // Same detection on same frame should be cached
        assert!(manager.register_detection(10, 20, 100, 0, 1).is_ok());
        let count_after_first = manager.pending_eva_event_count();

        // Try to register same detection again on same frame
        assert!(manager.register_detection(10, 20, 100, 0, 1).is_ok());
        let count_after_second = manager.pending_eva_event_count();

        // Cache should prevent duplicate
        assert_eq!(
            count_after_first, count_after_second,
            "Cache should prevent duplicate Eva messages"
        );
    }
}
