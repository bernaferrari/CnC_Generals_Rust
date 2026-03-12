//! Per-Player Visibility System
//!
//! Manages which players can see which stealthed units

use crate::common::*;
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

/// Per-player visibility information
#[derive(Debug, Clone)]
pub struct PlayerVisibility {
    /// Whether this player can see the object
    pub visible: bool,
    /// Frame when visibility was gained
    pub visible_since_frame: u32,
    /// Objects that are detecting this one for this player
    pub detecting_objects: HashSet<ObjectID>,
}

impl Default for PlayerVisibility {
    fn default() -> Self {
        Self {
            visible: true,
            visible_since_frame: 0,
            detecting_objects: HashSet::new(),
        }
    }
}

/// Tracks visibility of one object to all players
#[derive(Debug)]
pub struct PerPlayerVisibility {
    object_id: ObjectID,
    /// Map of player ID to visibility state
    player_visibility: HashMap<ObjectID, PlayerVisibility>,
    /// Default visibility for new players
    default_visible: bool,
}

impl PerPlayerVisibility {
    pub fn new(object_id: ObjectID, default_visible: bool) -> Self {
        Self {
            object_id,
            player_visibility: HashMap::new(),
            default_visible,
        }
    }

    /// Check if object is visible to a specific player
    pub fn is_visible_to_player(&self, player_id: ObjectID) -> bool {
        self.player_visibility
            .get(&player_id)
            .map(|v| v.visible)
            .unwrap_or(self.default_visible)
    }

    /// Set visibility for a specific player
    pub fn set_visible_to_player(&mut self, player_id: ObjectID, visible: bool, frame: u32) {
        self.player_visibility
            .entry(player_id)
            .or_insert_with(PlayerVisibility::default)
            .visible = visible;

        if visible {
            self.player_visibility
                .get_mut(&player_id)
                .unwrap()
                .visible_since_frame = frame;
        }
    }

    /// Add a detector for a player
    pub fn add_detector_for_player(
        &mut self,
        player_id: ObjectID,
        detector_id: ObjectID,
        frame: u32,
    ) {
        let visibility = self
            .player_visibility
            .entry(player_id)
            .or_insert_with(PlayerVisibility::default);

        visibility.detecting_objects.insert(detector_id);
        visibility.visible = true;
        visibility.visible_since_frame = frame;
    }

    /// Remove a detector for a player
    pub fn remove_detector_for_player(&mut self, player_id: ObjectID, detector_id: ObjectID) {
        if let Some(visibility) = self.player_visibility.get_mut(&player_id) {
            visibility.detecting_objects.remove(&detector_id);

            // If no more detectors, hide the object
            if visibility.detecting_objects.is_empty() {
                visibility.visible = false;
            }
        }
    }

    /// Get all players that can see this object
    pub fn get_visible_to_players(&self) -> Vec<ObjectID> {
        self.player_visibility
            .iter()
            .filter(|(_, v)| v.visible)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get all players that cannot see this object
    pub fn get_hidden_from_players(&self) -> Vec<ObjectID> {
        self.player_visibility
            .iter()
            .filter(|(_, v)| !v.visible)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Clear all visibility (hide from all players)
    pub fn clear_all_visibility(&mut self) {
        self.default_visible = false;
        for visibility in self.player_visibility.values_mut() {
            visibility.visible = false;
            visibility.detecting_objects.clear();
        }
    }

    /// Set all visibility (show to all players)
    pub fn set_all_visible(&mut self, frame: u32) {
        self.default_visible = true;
        for visibility in self.player_visibility.values_mut() {
            visibility.visible = true;
            visibility.visible_since_frame = frame;
        }
    }
}

/// Global visibility manager for all objects
pub struct VisibilityManager {
    /// Map of object ID to per-player visibility
    object_visibility: RwLock<HashMap<ObjectID, PerPlayerVisibility>>,
}

impl VisibilityManager {
    pub fn new() -> Self {
        Self {
            object_visibility: RwLock::new(HashMap::new()),
        }
    }

    /// Register an object with the visibility system
    pub fn register_object(&self, object_id: ObjectID, default_visible: bool) {
        let mut map = self.object_visibility.write().unwrap();
        map.insert(
            object_id,
            PerPlayerVisibility::new(object_id, default_visible),
        );
    }

    /// Unregister an object from the visibility system
    pub fn unregister_object(&self, object_id: ObjectID) {
        let mut map = self.object_visibility.write().unwrap();
        map.remove(&object_id);
    }

    /// Check if object is visible to player
    pub fn is_visible(&self, object_id: ObjectID, player_id: ObjectID) -> bool {
        let map = self.object_visibility.read().unwrap();
        map.get(&object_id)
            .map(|v| v.is_visible_to_player(player_id))
            .unwrap_or(true) // Default to visible if not registered
    }

    /// Set object visibility for a specific player
    pub fn set_visible(&self, object_id: ObjectID, player_id: ObjectID, visible: bool, frame: u32) {
        let mut map = self.object_visibility.write().unwrap();
        if let Some(vis) = map.get_mut(&object_id) {
            vis.set_visible_to_player(player_id, visible, frame);
        }
    }

    /// Add detector relationship
    pub fn add_detector(
        &self,
        object_id: ObjectID,
        player_id: ObjectID,
        detector_id: ObjectID,
        frame: u32,
    ) {
        let mut map = self.object_visibility.write().unwrap();
        if let Some(vis) = map.get_mut(&object_id) {
            vis.add_detector_for_player(player_id, detector_id, frame);
        }
    }

    /// Remove detector relationship
    pub fn remove_detector(&self, object_id: ObjectID, player_id: ObjectID, detector_id: ObjectID) {
        let mut map = self.object_visibility.write().unwrap();
        if let Some(vis) = map.get_mut(&object_id) {
            vis.remove_detector_for_player(player_id, detector_id);
        }
    }

    /// Get all players that can see an object
    pub fn get_visible_to_players(&self, object_id: ObjectID) -> Vec<ObjectID> {
        let map = self.object_visibility.read().unwrap();
        map.get(&object_id)
            .map(|v| v.get_visible_to_players())
            .unwrap_or_default()
    }

    /// Get all objects visible to a specific player
    pub fn get_visible_objects_for_player(&self, player_id: ObjectID) -> Vec<ObjectID> {
        let map = self.object_visibility.read().unwrap();
        map.iter()
            .filter(|(_, vis)| vis.is_visible_to_player(player_id))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Hide object from all players (enter stealth)
    pub fn hide_from_all(&self, object_id: ObjectID) {
        let mut map = self.object_visibility.write().unwrap();
        if let Some(vis) = map.get_mut(&object_id) {
            vis.clear_all_visibility();
        }
    }

    /// Show object to all players (exit stealth)
    pub fn show_to_all(&self, object_id: ObjectID, frame: u32) {
        let mut map = self.object_visibility.write().unwrap();
        if let Some(vis) = map.get_mut(&object_id) {
            vis.set_all_visible(frame);
        }
    }

    /// Team-based visibility: set visible to all team members
    pub fn set_visible_to_team(
        &self,
        object_id: ObjectID,
        team_player_ids: &[ObjectID],
        frame: u32,
    ) {
        let mut map = self.object_visibility.write().unwrap();
        if let Some(vis) = map.get_mut(&object_id) {
            for &player_id in team_player_ids {
                vis.set_visible_to_player(player_id, true, frame);
            }
        }
    }

    /// Get count of registered objects
    pub fn get_registered_count(&self) -> usize {
        let map = self.object_visibility.read().unwrap();
        map.len()
    }
}

impl Default for VisibilityManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_per_player_visibility() {
        let mut vis = PerPlayerVisibility::new(1, false);
        assert!(!vis.is_visible_to_player(2));

        vis.set_visible_to_player(2, true, 0);
        assert!(vis.is_visible_to_player(2));

        vis.set_visible_to_player(2, false, 10);
        assert!(!vis.is_visible_to_player(2));
    }

    #[test]
    fn test_detector_tracking() {
        let mut vis = PerPlayerVisibility::new(1, false);

        vis.add_detector_for_player(2, 10, 0);
        assert!(vis.is_visible_to_player(2));

        vis.add_detector_for_player(2, 11, 0);
        assert!(vis.is_visible_to_player(2));

        vis.remove_detector_for_player(2, 10);
        assert!(vis.is_visible_to_player(2)); // Still detected by 11

        vis.remove_detector_for_player(2, 11);
        assert!(!vis.is_visible_to_player(2)); // No more detectors
    }

    #[test]
    fn test_visibility_manager() {
        let manager = VisibilityManager::new();

        manager.register_object(1, false);
        assert!(!manager.is_visible(1, 2));

        manager.set_visible(1, 2, true, 0);
        assert!(manager.is_visible(1, 2));

        manager.hide_from_all(1);
        assert!(!manager.is_visible(1, 2));
    }

    #[test]
    fn test_visible_objects_for_player() {
        let manager = VisibilityManager::new();

        manager.register_object(1, true);
        manager.register_object(2, false);
        manager.register_object(3, true);

        let visible = manager.get_visible_objects_for_player(100);
        assert_eq!(visible.len(), 2);
        assert!(visible.contains(&1));
        assert!(visible.contains(&3));
    }

    #[test]
    fn test_team_visibility() {
        let manager = VisibilityManager::new();
        manager.register_object(1, false);

        let team_players = vec![2, 3, 4];
        manager.set_visible_to_team(1, &team_players, 0);

        assert!(manager.is_visible(1, 2));
        assert!(manager.is_visible(1, 3));
        assert!(manager.is_visible(1, 4));
        assert!(!manager.is_visible(1, 5));
    }
}
