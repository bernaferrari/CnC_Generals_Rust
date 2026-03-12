//! FOW (Fog of War) Rendering Integration
//!
//! This module bridges the FOW system (shroud manager) with the rendering pipeline.
//! It provides visibility queries for objects and updates shader uniforms with
//! visibility information for per-object rendering.
//!
//! ## Integration Points:
//! - During rendering, query `get_object_visibility()` to get FOW state
//! - Pass `visibility_alpha` and `is_explored` to shader uniforms
//! - Supports per-player and per-object visibility queries
//!
//! ## Architecture:
//! ```text
//! ShroudManager::can_see_object(player, obj_id)
//!    ↓
//! FOWRenderingBridge::get_object_visibility()
//!    ↓
//! Shader uniform (visibility_alpha, is_explored)
//!    ↓
//! Fragment shader applies FOW effects
//! ```

use crate::game_logic::ObjectId as ObjectID;
use gamelogic::system::shroud_manager::get_shroud_manager;
use log::{trace, warn};

fn shroud_runtime_active(
    shroud_mgr: &gamelogic::system::shroud_manager::ShroudManager,
    player_id: u32,
) -> bool {
    // C++ parity safeguard: when shroud has not been updated yet, fail open to avoid hiding
    // the whole world in single-player startup paths.
    shroud_mgr.get_last_update_frame() > 0 || !shroud_mgr.get_visible_objects(player_id).is_empty()
}

/// FOW visibility state for rendering an object
#[derive(Debug, Clone, Copy)]
pub struct ObjectVisibility {
    /// Alpha blend factor (0.0 = hidden, 1.0 = fully visible)
    pub visibility_alpha: f32,
    /// Explored state (1.0 = explored, 0.0 = unexplored)
    pub is_explored: f32,
    /// Falloff/gradient strength for smooth transitions
    pub visibility_falloff: f32,
}

impl Default for ObjectVisibility {
    fn default() -> Self {
        Self {
            visibility_alpha: 1.0,   // Default: fully visible
            is_explored: 1.0,        // Default: explored
            visibility_falloff: 1.0, // Default: sharp transition
        }
    }
}

/// FOW rendering bridge - connects shroud system to rendering pipeline
pub struct FOWRenderingBridge;

impl FOWRenderingBridge {
    /// Get visibility state for an object from the shroud manager
    ///
    /// This method queries the current FOW state and returns visibility
    /// parameters that should be passed to the shader for this object.
    ///
    /// # Arguments
    ///
    /// * `player_id` - Which player is viewing (0-7)
    /// * `object_id` - Which object to check visibility for
    ///
    /// # Returns
    ///
    /// ObjectVisibility with:
    /// - `visibility_alpha`: 0.0 (hidden) to 1.0 (fully visible)
    /// - `is_explored`: 1.0 (explored) or 0.0 (never seen)
    /// - `visibility_falloff`: Gradient strength (1.0 for sharp, lower for smoother)
    pub fn get_object_visibility(player_id: u32, object_id: ObjectID) -> ObjectVisibility {
        // Default to fully visible if shroud manager not available
        // This ensures the game continues to work even without FOW
        let mut visibility = ObjectVisibility::default();

        // Query ShroudManager for visibility state
        if let Ok(shroud_mgr) = get_shroud_manager().lock() {
            if !shroud_runtime_active(&shroud_mgr, player_id) {
                return visibility;
            }

            // Check if object is visible to this player
            let is_visible = shroud_mgr.can_see_object(player_id, object_id.0);

            // Check if object has been explored by this player
            let is_explored = shroud_mgr.has_explored_object(player_id, object_id.0);

            if is_visible {
                // Object is currently visible
                visibility.visibility_alpha = 1.0;
                visibility.is_explored = 1.0;
                visibility.visibility_falloff = 1.0;
            } else if is_explored {
                // Object was seen before but is not currently visible
                // Apply fog-of-war darkening effect
                visibility.visibility_alpha = 0.3;
                visibility.is_explored = 1.0;
                visibility.visibility_falloff = 1.0;
            } else {
                // Object has never been seen - completely hidden
                visibility.visibility_alpha = 0.0;
                visibility.is_explored = 0.0;
                visibility.visibility_falloff = 1.0;
            }

            trace!(
                "FOW visibility for object {}: alpha={}, explored={}, visible={}",
                object_id,
                visibility.visibility_alpha,
                is_explored,
                is_visible
            );
        } else {
            trace!(
                "Shroud manager unavailable, using default visibility for object {}",
                object_id
            );
        }

        visibility
    }

    /// Get visibility state considering stealth/detection systems
    ///
    /// This variant checks stealth systems in addition to basic FOW.
    /// Objects may be visible in FOW but invisible due to stealth.
    ///
    /// # Arguments
    ///
    /// * `player_id` - Which player is viewing
    /// * `object_id` - Which object to check
    ///
    /// # Returns
    ///
    /// ObjectVisibility with stealth considerations applied
    pub fn get_object_visibility_with_stealth(
        player_id: u32,
        object_id: ObjectID,
    ) -> ObjectVisibility {
        // Start with basic FOW visibility
        let mut visibility = Self::get_object_visibility(player_id, object_id);

        // If not visible due to FOW, stealth doesn't matter
        if visibility.visibility_alpha <= 0.0 {
            return visibility;
        }

        // Check stealth system - this would check if object is stealthed
        // and whether the player has detection capability
        if let Ok(shroud_mgr) = get_shroud_manager().lock() {
            if !shroud_runtime_active(&shroud_mgr, player_id) {
                return visibility;
            }

            match shroud_mgr.can_see_object_with_stealth(player_id, object_id.0) {
                Ok(can_see_with_stealth) => {
                    if !can_see_with_stealth {
                        // Object is stealthed and not detected
                        visibility.visibility_alpha = 0.0;
                    }
                }
                Err(_) => {
                    // On error, keep current visibility
                    // (fail-open for gameplay)
                }
            }
        }

        visibility
    }

    /// Update all object visibilities for a player
    ///
    /// Batch query for all visible objects. Used during rendering to
    /// efficiently determine which objects to render and with what visibility.
    ///
    /// # Arguments
    ///
    /// * `player_id` - Which player is viewing
    /// * `object_ids` - List of objects to check
    ///
    /// # Returns
    ///
    /// Map of object_id to visibility state
    pub fn get_all_object_visibilities(
        player_id: u32,
        object_ids: &[ObjectID],
    ) -> std::collections::HashMap<ObjectID, ObjectVisibility> {
        let mut visibilities = std::collections::HashMap::with_capacity(object_ids.len());

        for &object_id in object_ids {
            let visibility = Self::get_object_visibility(player_id, object_id);
            visibilities.insert(object_id, visibility);
        }

        visibilities
    }

    /// Check if an object should be rendered at all for a player
    ///
    /// Returns true if object is either visible or explored (darkened).
    /// Objects that have never been seen return false.
    ///
    /// # Arguments
    ///
    /// * `player_id` - Which player is viewing
    /// * `object_id` - Which object to check
    ///
    /// # Returns
    ///
    /// true if object should be rendered (even if darkened)
    pub fn should_render_object(player_id: u32, object_id: ObjectID) -> bool {
        if let Ok(shroud_mgr) = get_shroud_manager().lock() {
            if !shroud_runtime_active(&shroud_mgr, player_id) {
                return true;
            }
            // Render if visible or explored (explored objects show as darkened)
            shroud_mgr.can_see_object(player_id, object_id.0)
                || shroud_mgr.has_explored_object(player_id, object_id.0)
        } else {
            // No shroud manager, render everything
            true
        }
    }

    /// Force visibility recalculation for next frame
    ///
    /// Called when significant events occur:
    /// - Units created or destroyed
    /// - Vision upgrades completed
    /// - Special powers used
    pub fn force_visibility_update() {
        if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
            shroud_mgr.force_update();
            trace!("FOW visibility recalculation forced");
        }
    }
}

/// Reveal the entire map for the specified player (used on defeat/observer transitions).
pub fn reveal_entire_map_for_player(player_id: u32) {
    if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
        if let Err(err) = shroud_mgr.reveal_map_for_player_permanently(player_id) {
            warn!("Failed to permanently reveal map for player {player_id}: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_visibility_default() {
        let vis = ObjectVisibility::default();
        assert_eq!(vis.visibility_alpha, 1.0);
        assert_eq!(vis.is_explored, 1.0);
        assert_eq!(vis.visibility_falloff, 1.0);
    }

    #[test]
    fn test_object_visibility_custom() {
        let vis = ObjectVisibility {
            visibility_alpha: 0.5,
            is_explored: 1.0,
            visibility_falloff: 0.8,
        };
        assert_eq!(vis.visibility_alpha, 0.5);
        assert_eq!(vis.is_explored, 1.0);
        assert_eq!(vis.visibility_falloff, 0.8);
    }
}
