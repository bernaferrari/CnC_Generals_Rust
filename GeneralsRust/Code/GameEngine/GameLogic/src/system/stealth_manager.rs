//! Stealth Manager System
//!
//! Manages stealth state for game objects, including:
//! - Per-object stealth status (hidden, invisible, revealed)
//! - Per-player stealth visibility (different players see different stealth states)
//! - Stealth strength values for detection calculations
//! - Stealth reveal tracking (what broke stealth, who revealed it)
//!
//! Faithful to C++ implementation where stealth affects visibility independent
//! of fog-of-war (can be stealthed even in visible area, or revealed despite FOW)

use crate::common::{ObjectID, UnsignedInt};
use log::{debug, trace, warn};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;

/// Maximum number of players in game.
const MAX_PLAYER_COUNT: usize = crate::common::MAX_PLAYER_COUNT;

/// Stealth status levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum StealthStatus {
    /// Object is not stealthed (normal visibility)
    Hidden = 0,

    /// Object is stealthed/invisible (requires detection to see)
    Invisible = 1,

    /// Object's stealth has been revealed (visible to player regardless of stealth)
    Revealed = 2,
}

impl StealthStatus {
    /// Convert status to description string
    pub fn as_str(&self) -> &str {
        match self {
            StealthStatus::Hidden => "Hidden",
            StealthStatus::Invisible => "Invisible",
            StealthStatus::Revealed => "Revealed",
        }
    }
}

/// Stealth strength value (0.0-100.0)
/// Higher value = harder to detect
#[derive(Debug, Clone, Copy)]
pub struct StealthStrength(f32);

impl StealthStrength {
    /// Create new stealth strength value
    pub fn new(value: f32) -> Self {
        // Clamp to 0.0-100.0 range
        Self(value.max(0.0).min(100.0))
    }

    /// Get raw stealth strength value
    pub fn value(&self) -> f32 {
        self.0
    }

    /// Standard stealth strength for typical cloaked units (60.0)
    pub fn standard_cloak() -> Self {
        Self(60.0)
    }

    /// Very strong stealth (90.0) - GLA stealth
    pub fn strong_stealth() -> Self {
        Self(90.0)
    }

    /// Weak stealth (30.0) - slight concealment
    pub fn weak_stealth() -> Self {
        Self(30.0)
    }

    /// No stealth (0.0)
    pub fn none() -> Self {
        Self(0.0)
    }
}

/// Per-player stealth tracking for an object
#[derive(Debug, Clone)]
struct ObjectStealthState {
    /// Object ID being tracked
    object_id: ObjectID,

    /// Current stealth strength (0.0-100.0)
    stealth_strength: StealthStrength,

    /// Per-player stealth status (what each player sees)
    player_stealth_status: [StealthStatus; MAX_PLAYER_COUNT],

    /// Frame when stealth was revealed (for persistence)
    revealed_frame: UnsignedInt,

    /// Who revealed this object's stealth (ObjectID of revealer)
    revealed_by: ObjectID,
}

impl ObjectStealthState {
    /// Create new stealth state for object
    fn new(object_id: ObjectID) -> Self {
        Self {
            object_id,
            stealth_strength: StealthStrength::none(),
            player_stealth_status: [StealthStatus::Hidden; MAX_PLAYER_COUNT],
            revealed_frame: 0,
            revealed_by: 0,
        }
    }
}

/// Stealth Manager singleton
///
/// Manages stealth state for all game objects. Thread-safe access via mutex.
pub struct StealthManager {
    /// Per-object stealth tracking
    object_stealth: HashMap<ObjectID, ObjectStealthState>,

    /// Last frame stealth was updated
    last_update_frame: UnsignedInt,
}

impl StealthManager {
    /// Create new StealthManager
    pub fn new() -> Self {
        Self {
            object_stealth: HashMap::new(),
            last_update_frame: 0,
        }
    }

    pub(crate) fn object_capacity(&self) -> usize {
        self.object_stealth.capacity()
    }

    pub(crate) fn estimated_heap_bytes(&self) -> usize {
        self.object_stealth
            .capacity()
            .saturating_mul(std::mem::size_of::<(ObjectID, ObjectStealthState)>())
    }

    /// Register object for stealth tracking
    pub fn register_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        if self.object_stealth.contains_key(&object_id) {
            return Err(format!("Object {} already registered", object_id));
        }
        self.object_stealth
            .insert(object_id, ObjectStealthState::new(object_id));
        trace!("Registered object {} for stealth tracking", object_id);
        Ok(())
    }

    /// Unregister object from stealth tracking
    pub fn unregister_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        if self.object_stealth.remove(&object_id).is_some() {
            trace!("Unregistered object {} from stealth tracking", object_id);
            Ok(())
        } else {
            Err(format!("Object {} not registered", object_id))
        }
    }

    /// Set stealth strength for object
    pub fn set_stealth_strength(
        &mut self,
        object_id: ObjectID,
        strength: StealthStrength,
    ) -> Result<(), String> {
        let state = self
            .object_stealth
            .get_mut(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        state.stealth_strength = strength;
        trace!(
            "Set stealth strength for object {}: {:.1}",
            object_id,
            strength.value()
        );
        Ok(())
    }

    /// Get stealth strength for object
    pub fn get_stealth_strength(&self, object_id: ObjectID) -> Result<StealthStrength, String> {
        let state = self
            .object_stealth
            .get(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        Ok(state.stealth_strength)
    }

    /// Set stealth status for specific player
    pub fn set_stealth_status(
        &mut self,
        object_id: ObjectID,
        player_id: usize,
        status: StealthStatus,
    ) -> Result<(), String> {
        if player_id >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player_id: {}", player_id));
        }

        let state = self
            .object_stealth
            .get_mut(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        state.player_stealth_status[player_id] = status;
        trace!(
            "Set stealth status for object {} to player {}: {}",
            object_id,
            player_id,
            status.as_str()
        );
        Ok(())
    }

    /// Get stealth status for specific player
    pub fn get_stealth_status(
        &self,
        object_id: ObjectID,
        player_id: usize,
    ) -> Result<StealthStatus, String> {
        if player_id >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player_id: {}", player_id));
        }

        let state = self
            .object_stealth
            .get(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        Ok(state.player_stealth_status[player_id])
    }

    /// Check if object is invisible to specific player
    pub fn is_invisible_to_player(
        &self,
        object_id: ObjectID,
        player_id: usize,
    ) -> Result<bool, String> {
        if player_id >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player_id: {}", player_id));
        }

        let status = self.get_stealth_status(object_id, player_id)?;
        Ok(status == StealthStatus::Invisible)
    }

    /// Reveal object's stealth to specific player
    pub fn reveal_stealth(
        &mut self,
        object_id: ObjectID,
        player_id: usize,
        revealed_by: ObjectID,
        frame: UnsignedInt,
    ) -> Result<(), String> {
        if player_id >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player_id: {}", player_id));
        }

        let state = self
            .object_stealth
            .get_mut(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        state.player_stealth_status[player_id] = StealthStatus::Revealed;
        state.revealed_frame = frame;
        state.revealed_by = revealed_by;

        debug!(
            "Revealed stealth for object {} to player {} by {} at frame {}",
            object_id, player_id, revealed_by, frame
        );
        Ok(())
    }

    /// Break stealth for all players (detection successful)
    pub fn break_stealth_all(
        &mut self,
        object_id: ObjectID,
        frame: UnsignedInt,
    ) -> Result<(), String> {
        let state = self
            .object_stealth
            .get_mut(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        for status in &mut state.player_stealth_status {
            *status = StealthStatus::Revealed;
        }
        state.revealed_frame = frame;

        debug!("Broke stealth for object {} to all players", object_id);
        Ok(())
    }

    /// Reset stealth for all players (goes back to invisible)
    pub fn reset_stealth_all(&mut self, object_id: ObjectID) -> Result<(), String> {
        let state = self
            .object_stealth
            .get_mut(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        for status in &mut state.player_stealth_status {
            *status = StealthStatus::Invisible;
        }

        trace!("Reset stealth for object {} to invisible", object_id);
        Ok(())
    }

    /// Update last frame stealth was modified
    pub fn set_update_frame(&mut self, frame: UnsignedInt) {
        self.last_update_frame = frame;
    }

    /// Get last frame stealth was updated
    pub fn get_last_update_frame(&self) -> UnsignedInt {
        self.last_update_frame
    }
}

impl Default for StealthManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global singleton accessor for StealthManager
static STEALTH_MANAGER: OnceLock<Mutex<StealthManager>> = OnceLock::new();

/// Get the global StealthManager singleton
pub fn get_stealth_manager() -> &'static Mutex<StealthManager> {
    STEALTH_MANAGER.get_or_init(|| Mutex::new(StealthManager::new()))
}

#[cfg(test)]
mod stealth_tests {
    use super::*;

    #[test]
    fn test_stealth_basic() {
        let mut manager = StealthManager::new();

        // Register object
        assert!(manager.register_object(1).is_ok());
        assert!(
            manager.register_object(1).is_err(),
            "Should not register twice"
        );

        // Check initial state (Hidden)
        assert_eq!(
            manager.get_stealth_status(1, 0).unwrap(),
            StealthStatus::Hidden
        );
    }

    #[test]
    fn test_stealth_strength() {
        let mut manager = StealthManager::new();
        manager.register_object(1).unwrap();

        let strength = StealthStrength::standard_cloak();
        manager.set_stealth_strength(1, strength).unwrap();

        assert_eq!(
            manager.get_stealth_strength(1).unwrap().value(),
            strength.value()
        );
    }

    #[test]
    fn test_stealth_status_per_player() {
        let mut manager = StealthManager::new();
        manager.register_object(1).unwrap();

        // Different players see different stealth status
        manager
            .set_stealth_status(1, 0, StealthStatus::Invisible)
            .unwrap();
        manager
            .set_stealth_status(1, 1, StealthStatus::Revealed)
            .unwrap();

        assert_eq!(
            manager.get_stealth_status(1, 0).unwrap(),
            StealthStatus::Invisible
        );
        assert_eq!(
            manager.get_stealth_status(1, 1).unwrap(),
            StealthStatus::Revealed
        );
    }

    #[test]
    fn test_stealth_invisible_check() {
        let mut manager = StealthManager::new();
        manager.register_object(1).unwrap();

        manager
            .set_stealth_status(1, 0, StealthStatus::Invisible)
            .unwrap();
        assert!(manager.is_invisible_to_player(1, 0).unwrap());

        manager
            .set_stealth_status(1, 0, StealthStatus::Revealed)
            .unwrap();
        assert!(!manager.is_invisible_to_player(1, 0).unwrap());
    }

    #[test]
    fn test_stealth_reveal() {
        let mut manager = StealthManager::new();
        manager.register_object(1).unwrap();

        manager
            .set_stealth_status(1, 0, StealthStatus::Invisible)
            .unwrap();
        manager.reveal_stealth(1, 0, 2, 100).unwrap();

        assert_eq!(
            manager.get_stealth_status(1, 0).unwrap(),
            StealthStatus::Revealed
        );
    }

    #[test]
    fn test_stealth_break_all() {
        let mut manager = StealthManager::new();
        manager.register_object(1).unwrap();

        // Set invisible to multiple players
        for player in 0..3 {
            manager
                .set_stealth_status(1, player, StealthStatus::Invisible)
                .unwrap();
        }

        // Break stealth for all
        manager.break_stealth_all(1, 100).unwrap();

        for player in 0..3 {
            assert_eq!(
                manager.get_stealth_status(1, player).unwrap(),
                StealthStatus::Revealed
            );
        }
    }

    #[test]
    fn test_stealth_strength_values() {
        assert_eq!(StealthStrength::none().value(), 0.0);
        assert_eq!(StealthStrength::weak_stealth().value(), 30.0);
        assert_eq!(StealthStrength::standard_cloak().value(), 60.0);
        assert_eq!(StealthStrength::strong_stealth().value(), 90.0);
    }

    #[test]
    fn test_stealth_strength_clamping() {
        let weak = StealthStrength::new(-10.0);
        assert_eq!(weak.value(), 0.0);

        let strong = StealthStrength::new(150.0);
        assert_eq!(strong.value(), 100.0);
    }

    #[test]
    fn test_stealth_registration() {
        let mut manager = StealthManager::new();

        manager.register_object(1).unwrap();
        manager.register_object(2).unwrap();

        // Unregister first object
        assert!(manager.unregister_object(1).is_ok());
        assert!(
            manager.get_stealth_strength(1).is_err(),
            "Should not find unregistered object"
        );
        assert!(
            manager.get_stealth_strength(2).is_ok(),
            "Should still find other object"
        );
    }

    #[test]
    fn test_stealth_invalid_player() {
        let mut manager = StealthManager::new();
        manager.register_object(1).unwrap();

        // Invalid player ID should fail
        assert!(manager
            .set_stealth_status(1, 8, StealthStatus::Invisible)
            .is_err());
        assert!(manager.get_stealth_status(1, 255).is_err());
    }
}
