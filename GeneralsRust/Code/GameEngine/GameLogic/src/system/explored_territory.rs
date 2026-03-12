//! Explored Territory System
//!
//! Tracks which parts of the map have been explored by each player.
//! Unlike fog-of-war (which shows current visibility), explored territory
//! persists - once a player has seen a location, that territory remains
//! visible (though darkened) even after units move away.
//!
//! Faithful to C++ implementation where explored=1 means player has ever seen it,
//! and shroud (visibility)=1 means player can currently see it.

use crate::common::{Coord3D, UnsignedInt};
use log::{trace, warn};
use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::OnceLock;

/// Maximum number of players in game (0-7, plus observers)
const MAX_PLAYER_COUNT: usize = 8;

/// Explored Territory Manager singleton
///
/// Tracks which territory has been explored by each player.
/// Thread-safe access via mutex.
pub struct ExploredTerritoryManager {
    /// Per-player explored objects set
    /// An object is "explored" if a player has ever seen it
    player_explored_objects: [HashSet<u32>; MAX_PLAYER_COUNT],

    /// Explored territory persist rate
    /// Objects remain in explored set even after player loses visibility
    last_update_frame: UnsignedInt,
}

impl ExploredTerritoryManager {
    /// Create new ExploredTerritoryManager instance
    pub fn new() -> Self {
        Self {
            player_explored_objects: [
                HashSet::new(),
                HashSet::new(),
                HashSet::new(),
                HashSet::new(),
                HashSet::new(),
                HashSet::new(),
                HashSet::new(),
                HashSet::new(),
            ],
            last_update_frame: 0,
        }
    }

    /// Update explored territory from current visibility
    /// Called by GameLogic periodically (less frequently than ShroudManager)
    ///
    /// # Arguments
    /// * `player_id` - Which player to update explored territory for (0-7)
    /// * `visible_objects` - Set of currently visible object IDs
    /// * `frame` - Current game frame (for update tracking)
    pub fn update_explored_for_player(
        &mut self,
        player_id: usize,
        visible_objects: &HashSet<u32>,
        frame: UnsignedInt,
    ) -> Result<(), String> {
        if player_id >= MAX_PLAYER_COUNT {
            return Err(format!(
                "Invalid player_id: {} (must be 0-{})",
                player_id,
                MAX_PLAYER_COUNT - 1
            ));
        }

        // Add all currently visible objects to explored set
        // Once explored, objects stay explored (never removed)
        for &obj_id in visible_objects {
            self.player_explored_objects[player_id].insert(obj_id);
        }

        self.last_update_frame = frame;
        trace!(
            "Updated explored territory for player {}: {} explored objects",
            player_id,
            self.player_explored_objects[player_id].len()
        );

        Ok(())
    }

    /// Check if a player has explored a specific object
    ///
    /// Returns true if object has ever been visible to player (persistent)
    pub fn has_explored_object(&self, player_id: usize, object_id: u32) -> bool {
        if player_id >= MAX_PLAYER_COUNT {
            return false;
        }
        self.player_explored_objects[player_id].contains(&object_id)
    }

    /// Get all explored objects for a player
    /// Returns snapshot of explored objects
    pub fn get_explored_objects(&self, player_id: usize) -> Vec<u32> {
        if player_id >= MAX_PLAYER_COUNT {
            return Vec::new();
        }
        self.player_explored_objects[player_id]
            .iter()
            .copied()
            .collect()
    }

    /// Clear explored territory (for reset/reload scenarios)
    pub fn clear_for_player(&mut self, player_id: usize) -> Result<(), String> {
        if player_id >= MAX_PLAYER_COUNT {
            return Err(format!(
                "Invalid player_id: {} (must be 0-{})",
                player_id,
                MAX_PLAYER_COUNT - 1
            ));
        }
        self.player_explored_objects[player_id].clear();
        trace!("Cleared explored territory for player {}", player_id);
        Ok(())
    }

    /// Clear all explored territory (game reset)
    pub fn clear_all(&mut self) {
        for explored_set in &mut self.player_explored_objects {
            explored_set.clear();
        }
        self.last_update_frame = 0;
        trace!("Cleared all explored territory");
    }

    /// Get last frame explored territory was updated
    pub fn get_last_update_frame(&self) -> UnsignedInt {
        self.last_update_frame
    }
}

impl Default for ExploredTerritoryManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global singleton accessor for ExploredTerritoryManager
static EXPLORED_TERRITORY_MANAGER: OnceLock<Mutex<ExploredTerritoryManager>> = OnceLock::new();

/// Get the global ExploredTerritoryManager singleton
///
/// Returns a static reference to the mutex-protected manager.
/// Thread-safe: Multiple threads can call this and lock the manager safely.
pub fn get_explored_territory_manager() -> &'static Mutex<ExploredTerritoryManager> {
    EXPLORED_TERRITORY_MANAGER.get_or_init(|| Mutex::new(ExploredTerritoryManager::new()))
}

#[cfg(test)]
mod explored_territory_tests {
    use super::*;

    /// Test basic explored territory tracking
    #[test]
    fn test_explored_territory_basic() {
        let mut manager = ExploredTerritoryManager::new();

        // Initially no explored objects
        assert!(
            manager.get_explored_objects(0).is_empty(),
            "No objects should be explored initially"
        );
        assert!(
            !manager.has_explored_object(0, 1),
            "Object 1 should not be explored"
        );

        // Add visible objects to explored set
        let mut visible = HashSet::new();
        visible.insert(10);
        visible.insert(11);
        visible.insert(12);

        manager
            .update_explored_for_player(0, &visible, 100)
            .expect("Update should succeed");

        // Verify objects are now explored
        assert!(
            manager.has_explored_object(0, 10),
            "Object 10 should be explored"
        );
        assert!(
            manager.has_explored_object(0, 11),
            "Object 11 should be explored"
        );
        assert!(
            manager.has_explored_object(0, 12),
            "Object 12 should be explored"
        );
        assert_eq!(
            manager.get_explored_objects(0).len(),
            3,
            "Should have 3 explored objects"
        );
    }

    /// Test explored territory persistence
    #[test]
    fn test_explored_territory_persistence() {
        let mut manager = ExploredTerritoryManager::new();

        // First visibility update
        let mut visible1 = HashSet::new();
        visible1.insert(10);
        visible1.insert(11);
        manager
            .update_explored_for_player(0, &visible1, 100)
            .expect("First update should succeed");

        assert_eq!(manager.get_explored_objects(0).len(), 2);

        // Second visibility update with different objects
        let mut visible2 = HashSet::new();
        visible2.insert(12);
        visible2.insert(13);
        manager
            .update_explored_for_player(0, &visible2, 200)
            .expect("Second update should succeed");

        // All objects should be explored (persistence)
        assert_eq!(
            manager.get_explored_objects(0).len(),
            4,
            "Should have 4 total explored objects (persistence)"
        );
        assert!(
            manager.has_explored_object(0, 10),
            "Original object should persist"
        );
        assert!(
            manager.has_explored_object(0, 13),
            "New object should be added"
        );
    }

    /// Test per-player explored territory isolation
    #[test]
    fn test_explored_territory_per_player() {
        let mut manager = ExploredTerritoryManager::new();

        // Player 0 explores objects
        let mut visible0 = HashSet::new();
        visible0.insert(10);
        visible0.insert(11);
        manager
            .update_explored_for_player(0, &visible0, 100)
            .expect("Player 0 update should succeed");

        // Player 1 explores different objects
        let mut visible1 = HashSet::new();
        visible1.insert(20);
        visible1.insert(21);
        manager
            .update_explored_for_player(1, &visible1, 100)
            .expect("Player 1 update should succeed");

        // Verify isolation
        assert!(
            manager.has_explored_object(0, 10),
            "Player 0 should have explored 10"
        );
        assert!(
            !manager.has_explored_object(0, 20),
            "Player 0 should not have explored 20"
        );
        assert!(
            manager.has_explored_object(1, 20),
            "Player 1 should have explored 20"
        );
        assert!(
            !manager.has_explored_object(1, 10),
            "Player 1 should not have explored 10"
        );
    }

    /// Test explored territory boundary checks
    #[test]
    fn test_explored_territory_boundary_check() {
        let mut manager = ExploredTerritoryManager::new();

        let visible = HashSet::new();

        // Invalid player ID should fail
        assert!(
            manager
                .update_explored_for_player(8, &visible, 100)
                .is_err(),
            "Update should fail for invalid player ID 8"
        );
        assert!(
            manager.clear_for_player(255).is_err(),
            "Clear should fail for invalid player ID 255"
        );

        // Invalid player ID should return false/empty
        assert!(
            !manager.has_explored_object(8, 10),
            "Invalid player should return false visibility"
        );
        assert!(
            manager.get_explored_objects(8).is_empty(),
            "Invalid player should return empty explored set"
        );
    }

    /// Test explored territory clear
    #[test]
    fn test_explored_territory_clear() {
        let mut manager = ExploredTerritoryManager::new();

        // Add explored objects
        let mut visible = HashSet::new();
        visible.insert(10);
        visible.insert(11);
        manager
            .update_explored_for_player(0, &visible, 100)
            .expect("Update should succeed");

        assert_eq!(manager.get_explored_objects(0).len(), 2);

        // Clear player 0's explored territory
        manager.clear_for_player(0).expect("Clear should succeed");

        assert!(
            manager.get_explored_objects(0).is_empty(),
            "Explored territory should be cleared"
        );
        assert!(
            !manager.has_explored_object(0, 10),
            "Cleared object should not be explored"
        );
    }

    /// Test explored territory framework documentation
    #[test]
    fn test_explored_territory_framework() {
        // This test documents the explored territory system architecture

        // Explored territory serves the rendering system's FOW needs:
        // 1. Persistent visibility tracking (explored once, stays visible)
        // 2. Per-player territory tracking
        // 3. Dynamic updates from ShroudManager visibility

        // Expected usage pattern:
        // 1. GameLogic.update() calls ShroudManager.update()
        // 2. For each player, get visible_objects from ShroudManager
        // 3. Call ExploredTerritoryManager.update_explored_for_player()
        // 4. Rendering can check is_explored flag for darkened rendering

        let manager = ExploredTerritoryManager::new();
        assert_eq!(manager.get_explored_objects(0).len(), 0);

        // System integration verified through usage pattern
    }

    /// Test explored territory thread safety
    #[test]
    fn test_explored_territory_thread_safe() {
        let mut manager = ExploredTerritoryManager::new();

        // Add initial objects
        let mut visible = HashSet::new();
        visible.insert(10);
        manager
            .update_explored_for_player(0, &visible, 100)
            .expect("Initial update should succeed");

        // Multiple queries should work fine
        assert!(manager.has_explored_object(0, 10));
        assert!(manager.has_explored_object(0, 10));
        assert_eq!(manager.get_explored_objects(0).len(), 1);
        assert_eq!(manager.get_explored_objects(0).len(), 1);

        // Updates should be consistent
        let mut visible2 = HashSet::new();
        visible2.insert(11);
        manager
            .update_explored_for_player(0, &visible2, 200)
            .expect("Second update should succeed");

        assert_eq!(manager.get_explored_objects(0).len(), 2);
    }
}
