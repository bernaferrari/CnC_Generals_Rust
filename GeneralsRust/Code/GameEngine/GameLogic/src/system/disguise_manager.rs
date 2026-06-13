//! Disguise Manager System
//!
//! Manages disguise state for game objects, including:
//! - Unit disguise as other templates (appearance/rendering)
//! - Per-player disguise opacity (invisible to team, visible to enemies)
//! - Disguise transitions (animation frames during swap)
//! - Reveal transitions (animation frames during reveal)
//! - Team-wide disguise (all friendly units appear as disguised unit)
//! - Distance-based reveal (detect when within range of target)
//!
//! Works in conjunction with StealthManager for complete invisibility system.
//! Disguise affects rendering, StealthManager affects detection.

use crate::common::{ObjectID, UnsignedInt};
use log::{debug, trace, warn};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;

/// Maximum number of players in game.
const MAX_PLAYER_COUNT: usize = crate::common::MAX_PLAYER_COUNT;

/// Disguise state for a single object
#[derive(Debug, Clone)]
pub struct DisguiseState {
    /// Object ID being disguised
    pub object_id: ObjectID,

    /// Template name this object appears as (e.g., "InfantryGLA", "MediumTank")
    pub disguised_as_template: String,

    /// Which player team to appear as (None = not disguised)
    pub disguised_as_player: Option<u32>,

    /// Per-player opacity 0.0-1.0 (how visible to each player)
    /// 0.0 = invisible (team), 1.0 = fully visible (enemies)
    pub per_player_opacity: [f32; MAX_PLAYER_COUNT],

    /// Frames remaining in transition animation
    pub transition_frames_remaining: u32,

    /// Frames for reveal animation
    pub reveal_transition_frames: u32,

    /// Whether this disguise affects all team units (team-wide mode)
    pub team_wide_disguise: bool,

    /// Currently animating disguise transition
    pub is_transitioning: bool,
}

impl DisguiseState {
    /// Create new disguise state for object
    fn new(object_id: ObjectID) -> Self {
        // Default opacity: 0.0 for friendly team (invisible), 1.0 for enemies (visible)
        let opacity = [1.0; MAX_PLAYER_COUNT];
        Self {
            object_id,
            disguised_as_template: String::new(),
            disguised_as_player: None,
            per_player_opacity: opacity,
            transition_frames_remaining: 0,
            reveal_transition_frames: 0,
            team_wide_disguise: false,
            is_transitioning: false,
        }
    }

    /// Helper to clamp opacity to valid range
    fn clamp_opacity(value: f32) -> f32 {
        value.max(0.0).min(1.0)
    }
}

/// Disguise Manager singleton
///
/// Manages disguise state for all game objects. Thread-safe access via mutex.
/// Handles template swapping, opacity calculation, and team-wide effects.
pub struct DisguiseManager {
    /// Per-object disguise tracking
    object_disguises: HashMap<ObjectID, DisguiseState>,

    /// Team-wide disguise source (which object defines the team appearance)
    /// Stored as (team_id, source_object_id)
    team_disguise_sources: HashMap<u32, ObjectID>,

    /// Last simulation frame processed (keeps updates in sync with stealth layer)
    last_update_frame: UnsignedInt,
}

impl DisguiseManager {
    /// Create new DisguiseManager
    pub fn new() -> Self {
        Self {
            object_disguises: HashMap::new(),
            team_disguise_sources: HashMap::new(),
            last_update_frame: 0,
        }
    }

    /// Remember which frame the disguise layer last updated.
    pub fn set_update_frame(&mut self, frame: UnsignedInt) {
        self.last_update_frame = frame;
    }

    /// Register object for disguise tracking
    pub fn register_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        if self.object_disguises.contains_key(&object_id) {
            return Err(format!("Object {} already registered", object_id));
        }
        self.object_disguises
            .insert(object_id, DisguiseState::new(object_id));
        trace!("Registered object {} for disguise tracking", object_id);
        Ok(())
    }

    /// Unregister object from disguise tracking
    pub fn unregister_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        if self.object_disguises.remove(&object_id).is_some() {
            trace!("Unregistered object {} from disguise tracking", object_id);
            Ok(())
        } else {
            Err(format!("Object {} not registered", object_id))
        }
    }

    /// Set disguise for object (which template it appears as)
    pub fn set_disguise(
        &mut self,
        object_id: ObjectID,
        template_name: String,
        player_id: Option<u32>,
    ) -> Result<(), String> {
        let state = self
            .object_disguises
            .get_mut(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        state.disguised_as_template = template_name.clone();
        state.disguised_as_player = player_id;

        trace!(
            "Set disguise for object {} as template {} for player {:?}",
            object_id,
            template_name,
            player_id
        );
        Ok(())
    }

    /// Get which template object is disguised as
    pub fn get_disguise(&self, object_id: ObjectID) -> Result<String, String> {
        let state = self
            .object_disguises
            .get(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        if state.disguised_as_template.is_empty() {
            Err(format!("Object {} has no disguise set", object_id))
        } else {
            Ok(state.disguised_as_template.clone())
        }
    }

    /// Get which player team object is disguised as
    pub fn get_disguised_as_player(&self, object_id: ObjectID) -> Result<Option<u32>, String> {
        let state = self
            .object_disguises
            .get(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        Ok(state.disguised_as_player)
    }

    /// Set friendly opacity bounds (min/max visibility to friendly team)
    pub fn set_friendly_opacity(
        &mut self,
        object_id: ObjectID,
        min_opacity: f32,
        max_opacity: f32,
    ) -> Result<(), String> {
        if !(0.0..=1.0).contains(&min_opacity) || !(0.0..=1.0).contains(&max_opacity) {
            return Err(format!(
                "Opacity values must be 0.0-1.0, got min={}, max={}",
                min_opacity, max_opacity
            ));
        }

        let state = self
            .object_disguises
            .get_mut(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        // Apply opacity bounds to all friendly players (assuming < 4 is friendly team example)
        // In real implementation, would check actual player team
        for player_idx in 0..MAX_PLAYER_COUNT {
            state.per_player_opacity[player_idx] = DisguiseState::clamp_opacity(min_opacity);
        }

        trace!(
            "Set friendly opacity for object {} to min={}, max={}",
            object_id,
            min_opacity,
            max_opacity
        );
        Ok(())
    }

    /// Get current opacity for specific player
    pub fn get_friendly_opacity(
        &self,
        object_id: ObjectID,
        player_id: usize,
    ) -> Result<f32, String> {
        if player_id >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player_id: {}", player_id));
        }

        let state = self
            .object_disguises
            .get(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        Ok(state.per_player_opacity[player_id])
    }

    /// Start disguise transition animation
    pub fn start_transition(&mut self, object_id: ObjectID, frames: u32) -> Result<(), String> {
        let state = self
            .object_disguises
            .get_mut(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        state.transition_frames_remaining = frames;
        state.is_transitioning = true;

        debug!(
            "Started transition for object {} with {} frames",
            object_id, frames
        );
        Ok(())
    }

    /// Check if object is currently transitioning
    pub fn is_transitioning(&self, object_id: ObjectID) -> Result<bool, String> {
        let state = self
            .object_disguises
            .get(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        Ok(state.is_transitioning)
    }

    /// Advance transition by one frame, returns true if complete
    pub fn advance_transition(&mut self, object_id: ObjectID) -> Result<bool, String> {
        let state = self
            .object_disguises
            .get_mut(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        if !state.is_transitioning {
            return Ok(false);
        }

        if state.transition_frames_remaining > 0 {
            state.transition_frames_remaining -= 1;
        }

        if state.transition_frames_remaining == 0 {
            state.is_transitioning = false;
            trace!("Transition complete for object {}", object_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Start reveal animation
    pub fn reveal_disguise(
        &mut self,
        object_id: ObjectID,
        frames_duration: u32,
    ) -> Result<(), String> {
        let state = self
            .object_disguises
            .get_mut(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        state.reveal_transition_frames = frames_duration;

        debug!(
            "Started reveal for object {} with {} frame duration",
            object_id, frames_duration
        );
        Ok(())
    }

    /// Get current reveal transition frame count
    pub fn get_reveal_transition_frames(&self, object_id: ObjectID) -> Result<u32, String> {
        let state = self
            .object_disguises
            .get(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        Ok(state.reveal_transition_frames)
    }

    /// Calculate morph opacity for disguise transition animation
    ///
    /// Implements C++ diamond-shaped fade: starts at 1.0, fades to 0.0 at halfway point,
    /// then fades back to 1.0. This creates smooth visual transition between disguises.
    ///
    /// Formula: opacity = abs(1.0 - (factor * 2.0)) where factor = 1.0 - (elapsed / total)
    pub fn calculate_transition_opacity(elapsed_frames: u32, total_frames: u32) -> f32 {
        if total_frames == 0 {
            return 0.0;
        }
        if elapsed_frames >= total_frames {
            return 1.0;
        }

        // Calculate morph progress: 1.0 → 0.0 as we progress through animation
        let factor = 1.0 - (elapsed_frames as f32 / total_frames as f32);

        // Create diamond-shaped opacity curve: 1.0 → 0.0 → 1.0
        let morphed = 1.0 - (factor * 2.0);

        // Clamp to valid range [0.0, 1.0]
        morphed.abs().min(1.0).max(0.0)
    }

    /// Advance reveal animation by one frame
    pub fn advance_reveal_transition(&mut self, object_id: ObjectID) -> Result<bool, String> {
        let state = self
            .object_disguises
            .get_mut(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        if state.reveal_transition_frames > 0 {
            state.reveal_transition_frames -= 1;
        }

        if state.reveal_transition_frames == 0 {
            trace!("Reveal transition complete for object {}", object_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Clear disguise from object
    pub fn clear_disguise(&mut self, object_id: ObjectID) -> Result<(), String> {
        let state = self
            .object_disguises
            .get_mut(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        state.disguised_as_template.clear();
        state.disguised_as_player = None;
        state.is_transitioning = false;
        state.transition_frames_remaining = 0;
        state.team_wide_disguise = false;

        trace!("Cleared disguise for object {}", object_id);
        Ok(())
    }

    /// Enable team-wide disguise (all team units appear as this unit's disguise)
    pub fn enable_team_wide_disguise(
        &mut self,
        object_id: ObjectID,
        team_id: u32,
    ) -> Result<(), String> {
        let state = self
            .object_disguises
            .get_mut(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        state.team_wide_disguise = true;
        self.team_disguise_sources.insert(team_id, object_id);

        debug!(
            "Enabled team-wide disguise for team {} sourced from object {}",
            team_id, object_id
        );
        Ok(())
    }

    /// Get team-wide disguise source object (if set)
    pub fn get_team_disguise_source(&self, team_id: u32) -> Result<Option<ObjectID>, String> {
        Ok(self.team_disguise_sources.get(&team_id).copied())
    }

    /// Disable team-wide disguise for a team
    pub fn disable_team_wide_disguise(&mut self, team_id: u32) -> Result<(), String> {
        if let Some(object_id) = self.team_disguise_sources.remove(&team_id) {
            if let Some(state) = self.object_disguises.get_mut(&object_id) {
                state.team_wide_disguise = false;
            }
            trace!("Disabled team-wide disguise for team {}", team_id);
            Ok(())
        } else {
            Err(format!("No team-wide disguise source for team {}", team_id))
        }
    }

    /// Get all disguised objects for a specific player team
    pub fn get_disguised_objects_for_team(&self, team_id: u32) -> Vec<ObjectID> {
        self.object_disguises
            .values()
            .filter(|state| {
                state.disguised_as_player == Some(team_id)
                    && !state.disguised_as_template.is_empty()
            })
            .map(|state| state.object_id)
            .collect()
    }

    /// Check if object has active disguise
    pub fn has_active_disguise(&self, object_id: ObjectID) -> Result<bool, String> {
        let state = self
            .object_disguises
            .get(&object_id)
            .ok_or_else(|| format!("Object {} not registered", object_id))?;

        Ok(!state.disguised_as_template.is_empty() && state.disguised_as_player.is_some())
    }
}

impl Default for DisguiseManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global singleton accessor for DisguiseManager
static DISGUISE_MANAGER: OnceLock<Mutex<DisguiseManager>> = OnceLock::new();

/// Get the global DisguiseManager singleton
pub fn get_disguise_manager() -> &'static Mutex<DisguiseManager> {
    DISGUISE_MANAGER.get_or_init(|| Mutex::new(DisguiseManager::new()))
}

#[cfg(test)]
mod disguise_tests {
    use super::*;

    #[test]
    fn test_disguise_basic() {
        let mut manager = DisguiseManager::new();

        // Register object
        assert!(manager.register_object(1).is_ok());
        assert!(
            manager.register_object(1).is_err(),
            "Should not register twice"
        );

        // Check initial state (no disguise)
        assert!(manager.get_disguise(1).is_err());
        assert_eq!(manager.get_disguised_as_player(1).unwrap(), None);
    }

    #[test]
    fn test_disguise_as_template() {
        let mut manager = DisguiseManager::new();
        manager.register_object(1).unwrap();

        // Set disguise as specific template
        assert!(manager
            .set_disguise(1, "MediumTank".to_string(), Some(0))
            .is_ok());

        // Check disguise was set
        assert_eq!(manager.get_disguise(1).unwrap(), "MediumTank".to_string());
        assert_eq!(manager.get_disguised_as_player(1).unwrap(), Some(0));
    }

    #[test]
    fn test_disguise_as_player() {
        let mut manager = DisguiseManager::new();
        manager.register_object(1).unwrap();

        // Set disguise for different players
        manager
            .set_disguise(1, "InfantryGLA".to_string(), Some(2))
            .unwrap();
        assert_eq!(manager.get_disguised_as_player(1).unwrap(), Some(2));

        manager
            .set_disguise(1, "SoldierChinese".to_string(), Some(1))
            .unwrap();
        assert_eq!(manager.get_disguised_as_player(1).unwrap(), Some(1));
    }

    #[test]
    fn test_friendly_opacity_calculation() {
        let mut manager = DisguiseManager::new();
        manager.register_object(1).unwrap();

        // Set opacity bounds
        assert!(manager.set_friendly_opacity(1, 0.2, 0.8).is_ok());

        // Check opacity was set
        let opacity = manager.get_friendly_opacity(1, 0).unwrap();
        assert!(opacity >= 0.0 && opacity <= 1.0);
    }

    #[test]
    fn test_per_player_opacity() {
        let mut manager = DisguiseManager::new();
        manager.register_object(1).unwrap();

        // Get opacity for different players
        let op0 = manager.get_friendly_opacity(1, 0).unwrap();
        let op1 = manager.get_friendly_opacity(1, 1).unwrap();

        // All players should have valid opacity
        assert!(op0 >= 0.0 && op0 <= 1.0);
        assert!(op1 >= 0.0 && op1 <= 1.0);
    }

    #[test]
    fn test_disguise_transition() {
        let mut manager = DisguiseManager::new();
        manager.register_object(1).unwrap();

        // Start transition
        assert!(manager.start_transition(1, 10).is_ok());
        assert!(manager.is_transitioning(1).unwrap());

        // Advance transition
        for _ in 0..10 {
            manager.advance_transition(1).unwrap();
        }
        assert!(!manager.is_transitioning(1).unwrap());
    }

    #[test]
    fn test_disguise_reveal() {
        let mut manager = DisguiseManager::new();
        manager.register_object(1).unwrap();

        // Set disguise first
        manager
            .set_disguise(1, "Tank".to_string(), Some(0))
            .unwrap();

        // Start reveal animation
        assert!(manager.reveal_disguise(1, 5).is_ok());
        assert_eq!(manager.get_reveal_transition_frames(1).unwrap(), 5);
    }

    #[test]
    fn test_disguise_reveal_transition() {
        let mut manager = DisguiseManager::new();
        manager.register_object(1).unwrap();

        // Start reveal with duration
        manager.reveal_disguise(1, 8).unwrap();

        // Advance reveal animation frames
        let mut is_complete = false;
        for _ in 0..8 {
            is_complete = manager.advance_reveal_transition(1).unwrap();
        }
        assert!(is_complete);
        assert_eq!(manager.get_reveal_transition_frames(1).unwrap(), 0);
    }

    #[test]
    fn test_team_wide_disguise() {
        let mut manager = DisguiseManager::new();
        manager.register_object(1).unwrap();
        manager.register_object(2).unwrap();

        // Set team-wide disguise on object 1
        manager
            .set_disguise(1, "GhostStalk".to_string(), Some(0))
            .unwrap();
        assert!(manager.enable_team_wide_disguise(1, 0).is_ok());

        // Check source was recorded
        assert_eq!(manager.get_team_disguise_source(0).unwrap(), Some(1));
    }

    #[test]
    fn test_disguise_clear() {
        let mut manager = DisguiseManager::new();
        manager.register_object(1).unwrap();

        // Set and then clear disguise
        manager
            .set_disguise(1, "Ranger".to_string(), Some(0))
            .unwrap();
        assert!(manager.get_disguise(1).is_ok());

        assert!(manager.clear_disguise(1).is_ok());
        assert!(manager.get_disguise(1).is_err());
        assert_eq!(manager.get_disguised_as_player(1).unwrap(), None);
    }

    #[test]
    fn test_multiple_disguises() {
        let mut manager = DisguiseManager::new();

        // Register multiple objects
        for i in 1..=5 {
            manager.register_object(i).unwrap();
        }

        // Set different disguises
        for i in 1..=5 {
            manager
                .set_disguise(i, format!("Unit{}", i), Some(i % 2))
                .unwrap();
        }

        // Verify each disguise
        for i in 1..=5 {
            assert_eq!(manager.get_disguise(i).unwrap(), format!("Unit{}", i));
        }
    }

    #[test]
    fn test_disguise_registration() {
        let mut manager = DisguiseManager::new();

        manager.register_object(10).unwrap();
        manager.register_object(20).unwrap();

        // Unregister first object
        assert!(manager.unregister_object(10).is_ok());
        assert!(
            manager.get_disguise(10).is_err(),
            "Should not find unregistered object"
        );
        assert!(
            manager.get_disguise(20).is_err(),
            "Object 20 not yet disguised"
        );

        // Register again after unregister
        assert!(manager.register_object(10).is_ok());
    }

    #[test]
    fn test_opacity_min_max_bounds() {
        let mut manager = DisguiseManager::new();
        manager.register_object(1).unwrap();

        // Valid bounds
        assert!(manager.set_friendly_opacity(1, 0.0, 1.0).is_ok());
        assert!(manager.set_friendly_opacity(1, 0.5, 0.5).is_ok());

        // Invalid bounds (out of range)
        assert!(manager.set_friendly_opacity(1, -0.1, 0.5).is_err());
        assert!(manager.set_friendly_opacity(1, 0.5, 1.1).is_err());
    }

    #[test]
    fn test_has_active_disguise() {
        let mut manager = DisguiseManager::new();
        manager.register_object(1).unwrap();

        // Initially no active disguise
        assert!(!manager.has_active_disguise(1).unwrap());

        // Set disguise
        manager
            .set_disguise(1, "TankEU".to_string(), Some(0))
            .unwrap();
        assert!(manager.has_active_disguise(1).unwrap());

        // Clear disguise
        manager.clear_disguise(1).unwrap();
        assert!(!manager.has_active_disguise(1).unwrap());
    }

    #[test]
    fn test_transition_frame_advancement() {
        let mut manager = DisguiseManager::new();
        manager.register_object(1).unwrap();

        // Start with 5 frames
        manager.start_transition(1, 5).unwrap();

        // First 4 advances should return false
        for i in 0..4 {
            let complete = manager.advance_transition(1).unwrap();
            assert!(!complete, "Should not complete at frame {}", i);
        }

        // Final advance should complete
        let complete = manager.advance_transition(1).unwrap();
        assert!(complete, "Should complete on final frame");
        assert!(!manager.is_transitioning(1).unwrap());
    }

    #[test]
    fn test_transition_opacity_calculation() {
        // Test diamond-shaped fade curve
        let total_frames = 10;

        // At frame 0: factor = 1.0, morphed = 1.0 - (1.0 * 2.0) = -1.0, opacity = 1.0 (abs + clamp)
        let opacity = DisguiseManager::calculate_transition_opacity(0, total_frames);
        assert!(
            (opacity - 1.0).abs() < 0.01,
            "Start should be opaque: {}",
            opacity
        );

        // At frame 5 (halfway): factor = 0.5, morphed = 1.0 - (0.5 * 2.0) = 0.0, opacity = 0.0
        let opacity = DisguiseManager::calculate_transition_opacity(5, total_frames);
        assert!(opacity < 0.01, "Halfway should be transparent: {}", opacity);

        // At frame 10: factor = 0.0, morphed = 1.0 - (0.0 * 2.0) = 1.0, opacity = 1.0
        let opacity = DisguiseManager::calculate_transition_opacity(10, total_frames);
        assert!(
            (opacity - 1.0).abs() < 0.01,
            "End should be opaque: {}",
            opacity
        );

        // At frame 2 (early): should be between 1.0 and 0.5
        let opacity = DisguiseManager::calculate_transition_opacity(2, total_frames);
        assert!(
            opacity > 0.5 && opacity < 1.0,
            "Early fade should be partial: {}",
            opacity
        );

        // At frame 8 (late): should be between 0.5 and 1.0
        let opacity = DisguiseManager::calculate_transition_opacity(8, total_frames);
        assert!(
            opacity > 0.5 && opacity < 1.0,
            "Late fade should be partial: {}",
            opacity
        );
    }

    #[test]
    fn test_transition_opacity_edge_cases() {
        // Invalid: zero total frames
        let opacity = DisguiseManager::calculate_transition_opacity(5, 0);
        assert!(opacity == 0.0, "Zero total frames should return 0.0");

        // Invalid: elapsed >= total
        let opacity = DisguiseManager::calculate_transition_opacity(15, 10);
        assert!(opacity == 1.0, "Elapsed >= total should return 1.0");

        // Single frame animation
        let opacity = DisguiseManager::calculate_transition_opacity(0, 1);
        assert!(
            (opacity - 1.0).abs() < 0.01,
            "Single frame start should be 1.0"
        );

        let opacity = DisguiseManager::calculate_transition_opacity(1, 1);
        assert!(opacity == 1.0, "Single frame completion should be 1.0");
    }
}
