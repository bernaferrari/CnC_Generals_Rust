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
//! - `PresentationFowGrid` freezes cell-grid state for terrain / minimap overlay
//!   so GPU upload does not re-lock the shroud manager mid-render
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
//!
//! ShroudManager::snapshot_grid_for_player(local)
//!    ↓
//! PresentationFowGrid (owned cells)
//!    ↓
//! FowTerrainOverlay::update_texture / minimap R8-RGBA
//! ```
//!
//! Fail-closed claim: unit FOW + compact cell-grid snapshot for local player.
//! Not full SAGE dirty-rect streaming / multi-player simultaneous grid parity.

use crate::game_logic::ObjectId as ObjectID;
use gamelogic::system::shroud_manager::{get_shroud_manager, ShroudState};
use log::{trace, warn};

fn shroud_runtime_active(
    shroud_mgr: &gamelogic::system::shroud_manager::ShroudManager,
    player_id: u32,
) -> bool {
    // Host residual: ShroudManager::update() queries the gamelogic ObjectManager.
    // Main GameLogic objects are not in that registry on the default host path, so an
    // "update" can clear player_visible_objects and leave them empty while still bumping
    // last_update_frame. That must NOT activate FOW filtering (would hide the whole world).
    // Fail-open unless this player has real visible/explored object membership.
    !shroud_mgr.get_visible_objects(player_id).is_empty()
        || !shroud_mgr.get_explored_objects(player_id).is_empty()
}

/// FOW visibility state for rendering an object
///
/// Snapshot-friendly (Copy + Serialize) so `PresentationFrame` can own unit FOW
/// without re-locking the shroud manager mid-render.
/// Serialize tests that mutate the process-wide shroud manager / FOW bridge.
pub fn shroud_test_isolation_lock() -> &'static std::sync::Mutex<()> {
    use std::sync::{Mutex, OnceLock};
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
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

/// Compact presentation-owned FOW cell grid for the local player.
///
/// Built once per logic frame into `PresentationFrame` so terrain overlay /
/// minimap texture update can consume frozen cells without mid-render shroud
/// re-queries. Values are SAGE-style buckets matching [`ShroudState`].
///
/// Fail-closed: full grid copy (not dirty rects); not full SAGE multi-layer
/// shroud texture streaming parity.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PresentationFowGrid {
    pub width: u32,
    pub height: u32,
    /// World units per cell (matches shroud partition cell size, typically 50).
    pub cell_size: f32,
    /// Row-major `y * width + x`: 0=Hidden, 1=Explored/Fogged, 2=Visible.
    pub cells: Vec<u8>,
    /// True when `cells` came from an initialized shroud grid.
    /// When false, consumers should fail-open (fully visible / no overlay).
    pub active: bool,
}

impl Default for PresentationFowGrid {
    fn default() -> Self {
        Self::inactive()
    }
}

impl PresentationFowGrid {
    pub const CELL_HIDDEN: u8 = 0;
    pub const CELL_EXPLORED: u8 = 1;
    pub const CELL_VISIBLE: u8 = 2;

    /// Terrain overlay R8: shrouded.
    pub const R8_SHROUDED: u8 = 0;
    /// Terrain overlay R8: fogged / explored.
    pub const R8_FOGGED: u8 = 128;
    /// Terrain overlay R8: clear / visible.
    pub const R8_VISIBLE: u8 = 255;

    /// Empty inactive grid — fail-open for consumers (no texture upload).
    pub fn inactive() -> Self {
        Self {
            width: 0,
            height: 0,
            cell_size: 50.0,
            cells: Vec::new(),
            active: false,
        }
    }

    /// Fully visible grid of the given size (shell-map bypass / observer).
    pub fn fully_visible(width: u32, height: u32, cell_size: f32) -> Self {
        let len = (width as usize).saturating_mul(height as usize);
        Self {
            width,
            height,
            cell_size: cell_size.max(1.0),
            cells: vec![Self::CELL_VISIBLE; len],
            active: true,
        }
    }

    /// Build from shroud manager snapshot bytes (0/1/2 per cell).
    pub fn from_snapshot(width: u32, height: u32, cell_size: f32, cells: Vec<u8>) -> Self {
        let expected = (width as usize).saturating_mul(height as usize);
        let mut cells = cells;
        if cells.len() != expected {
            // Fail-closed sizing: pad/truncate rather than panic at snapshot time.
            cells.resize(expected, Self::CELL_HIDDEN);
        }
        Self {
            width,
            height,
            cell_size: cell_size.max(1.0),
            cells,
            active: expected > 0,
        }
    }

    #[inline]
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Cell state at grid coords, or Hidden when OOB / inactive.
    #[inline]
    pub fn cell_at(&self, x: u32, y: u32) -> u8 {
        if !self.active || x >= self.width || y >= self.height {
            return if self.active {
                Self::CELL_HIDDEN
            } else {
                Self::CELL_VISIBLE
            };
        }
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        self.cells.get(idx).copied().unwrap_or(Self::CELL_HIDDEN)
    }

    /// Sample cell using shroud world axes (X,Y) — matches `ShroudGrid::world_to_grid`.
    pub fn state_at_world_xy(&self, world_x: f32, world_y: f32) -> u8 {
        if !self.active || self.cell_size <= 0.0 {
            return Self::CELL_VISIBLE;
        }
        let gx = (world_x / self.cell_size).floor() as i32;
        let gy = (world_y / self.cell_size).floor() as i32;
        if gx < 0 || gy < 0 {
            return Self::CELL_HIDDEN;
        }
        self.cell_at(gx as u32, gy as u32)
    }

    /// Encode one cell to R8 for `FowTerrainOverlay::update_texture`.
    #[inline]
    pub fn cell_to_r8(cell: u8) -> u8 {
        match cell {
            Self::CELL_VISIBLE => Self::R8_VISIBLE,
            Self::CELL_EXPLORED => Self::R8_FOGGED,
            _ => Self::R8_SHROUDED,
        }
    }

    /// Full R8 texture payload (length = width * height) for terrain FOW overlay.
    ///
    /// Inactive grids return empty — callers should skip upload / fail-open.
    pub fn to_r8_texture(&self) -> Vec<u8> {
        if !self.active || self.cells.is_empty() {
            return Vec::new();
        }
        self.cells.iter().map(|&c| Self::cell_to_r8(c)).collect()
    }

    /// Map cell state to object-style visibility (for tests / shared encoding).
    pub fn cell_to_object_visibility(cell: u8) -> ObjectVisibility {
        match cell {
            Self::CELL_VISIBLE => ObjectVisibility::VISIBLE,
            Self::CELL_EXPLORED => ObjectVisibility::FOGGED,
            _ => ObjectVisibility::HIDDEN,
        }
    }

    /// Lightweight fingerprint for dual-run presentation determinism.
    pub fn content_fingerprint(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        self.width.hash(&mut h);
        self.height.hash(&mut h);
        self.active.hash(&mut h);
        self.cell_size.to_bits().hash(&mut h);
        self.cells.len().hash(&mut h);
        // Hash all cells for strict grid consistency (compact maps stay cheap).
        for c in &self.cells {
            c.hash(&mut h);
        }
        h.finish()
    }
}

impl ObjectVisibility {
    /// Fully visible (no FOW darkening / shell-map bypass).
    pub const FULLY_VISIBLE: Self = Self {
        visibility_alpha: 1.0,
        is_explored: 1.0,
        visibility_falloff: 1.0,
    };

    /// Currently visible.
    pub const VISIBLE: Self = Self::FULLY_VISIBLE;

    /// Explored earlier, not currently in vision (darkened).
    pub const FOGGED: Self = Self {
        visibility_alpha: 0.3,
        is_explored: 1.0,
        visibility_falloff: 1.0,
    };

    /// Never explored — must not be drawn for the local player.
    pub const HIDDEN: Self = Self {
        visibility_alpha: 0.0,
        is_explored: 0.0,
        visibility_falloff: 1.0,
    };

    /// Encode shroud flags into render visibility (parity with FOW bridge states).
    pub fn from_shroud_flags(is_visible: bool, is_explored: bool) -> Self {
        if is_visible {
            Self::VISIBLE
        } else if is_explored {
            Self::FOGGED
        } else {
            Self::HIDDEN
        }
    }

    /// True when the object should enter the mesh pass (visible or fogged, not never-seen).
    #[inline]
    pub fn should_render(&self) -> bool {
        self.visibility_alpha > 0.0 || self.is_explored > 0.0
    }

    /// C++ GameClient drawable shroud residual: Fogged|Shrouded|InvalidButPreviousValid
    /// → `setFullyObscuredByShroud(true)`. Only currently-visible cells keep models lit.
    #[inline]
    pub fn fully_obscures_drawable(&self) -> bool {
        self.visibility_alpha < 1.0
    }

    /// True when never explored (skip mesh entirely for local player).
    #[inline]
    pub fn never_explored(&self) -> bool {
        self.visibility_alpha <= 0.0 && self.is_explored <= 0.0
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

            visibility = ObjectVisibility::from_shroud_flags(is_visible, is_explored);

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

    /// Snapshot the partition cell grid for `player_id` into a presentation-owned buffer.
    ///
    /// Returns an inactive empty grid when the shroud manager is unavailable or the
    /// grid is not initialized (fail-open for terrain overlay). Shell-map callers
    /// should pass `shell_bypass=true` to force fully-visible cells when dimensions
    /// are known.
    pub fn snapshot_terrain_grid(player_id: u32, shell_bypass: bool) -> PresentationFowGrid {
        let Ok(shroud_mgr) = get_shroud_manager().lock() else {
            return PresentationFowGrid::inactive();
        };

        let Some((width, height, cell_size)) = shroud_mgr.grid_dimensions() else {
            return PresentationFowGrid::inactive();
        };
        let width_u = width as u32;
        let height_u = height as u32;

        if shell_bypass {
            return PresentationFowGrid::fully_visible(width_u, height_u, cell_size);
        }

        // When shroud has never updated, fail-open (match unit FOW startup safeguard)
        // so terrain is not painted fully black during boot.
        if !shroud_runtime_active(&shroud_mgr, player_id) {
            return PresentationFowGrid::fully_visible(width_u, height_u, cell_size);
        }

        match shroud_mgr.snapshot_grid_for_player(player_id) {
            Some(cells) => {
                trace!(
                    "FOW terrain grid snapshot player={} {}x{} cells={}",
                    player_id,
                    width_u,
                    height_u,
                    cells.len()
                );
                PresentationFowGrid::from_snapshot(width_u, height_u, cell_size, cells)
            }
            None => PresentationFowGrid::inactive(),
        }
    }

    /// Encode a live [`ShroudState`] into the compact presentation cell value.
    #[inline]
    pub fn shroud_state_to_cell(state: ShroudState) -> u8 {
        state as u8
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

// --- Wave 77: FOW residual honesty pack ---

/// Retail / SAGE shroud partition cell size residual (world units).
///
/// Matches host `PresentationFowGrid::inactive` default and typical
/// `ShroudGrid` cell size. Fail-closed: not full multi-layer streaming.
pub const PRESENTATION_FOW_DEFAULT_CELL_SIZE: f32 = 50.0;

/// Honesty: FOW cell encoding / R8 terrain overlay / inactive fail-open residual.
///
/// Host-testable pack for presentation-owned FOW grid residual (Wave 77).
/// Fail-closed: not full SAGE dirty-rect / multi-layer shroud texture streaming.
pub fn honesty_fow_residual_pack_wave77() -> bool {
    // SAGE-style cell buckets residual (0/1/2).
    PresentationFowGrid::CELL_HIDDEN == 0
        && PresentationFowGrid::CELL_EXPLORED == 1
        && PresentationFowGrid::CELL_VISIBLE == 2
        // Terrain overlay R8 residual (shrouded / fogged / clear).
        && PresentationFowGrid::R8_SHROUDED == 0
        && PresentationFowGrid::R8_FOGGED == 128
        && PresentationFowGrid::R8_VISIBLE == 255
        && PresentationFowGrid::cell_to_r8(PresentationFowGrid::CELL_HIDDEN)
            == PresentationFowGrid::R8_SHROUDED
        && PresentationFowGrid::cell_to_r8(PresentationFowGrid::CELL_EXPLORED)
            == PresentationFowGrid::R8_FOGGED
        && PresentationFowGrid::cell_to_r8(PresentationFowGrid::CELL_VISIBLE)
            == PresentationFowGrid::R8_VISIBLE
        // Default cell size residual.
        && (PRESENTATION_FOW_DEFAULT_CELL_SIZE - 50.0).abs() < 0.01
        && {
            let inactive = PresentationFowGrid::inactive();
            !inactive.active
                && inactive.cells.is_empty()
                && (inactive.cell_size - PRESENTATION_FOW_DEFAULT_CELL_SIZE).abs() < 0.01
                // Inactive fail-open: sample as visible, empty R8 payload.
                && inactive.cell_at(0, 0) == PresentationFowGrid::CELL_VISIBLE
                && inactive.to_r8_texture().is_empty()
        }
        && {
            // Fully-visible residual (shell-map / observer bypass).
            let full = PresentationFowGrid::fully_visible(4, 3, PRESENTATION_FOW_DEFAULT_CELL_SIZE);
            full.active
                && full.cell_count() == 12
                && full.cells.iter().all(|&c| c == PresentationFowGrid::CELL_VISIBLE)
                && full.to_r8_texture().iter().all(|&v| v == PresentationFowGrid::R8_VISIBLE)
        }
        && {
            // from_snapshot resize residual (pad with Hidden when undersized).
            let g = PresentationFowGrid::from_snapshot(
                2,
                2,
                PRESENTATION_FOW_DEFAULT_CELL_SIZE,
                vec![PresentationFowGrid::CELL_VISIBLE],
            );
            g.active
                && g.cell_count() == 4
                && g.cell_at(0, 0) == PresentationFowGrid::CELL_VISIBLE
                && g.cell_at(1, 1) == PresentationFowGrid::CELL_HIDDEN
                && g.content_fingerprint() != 0
        }
        && {
            // ObjectVisibility residual encoding for FOW consumers.
            ObjectVisibility::from_shroud_flags(true, true) == ObjectVisibility::VISIBLE
                && ObjectVisibility::from_shroud_flags(false, true) == ObjectVisibility::FOGGED
                && ObjectVisibility::from_shroud_flags(false, false) == ObjectVisibility::HIDDEN
                && ObjectVisibility::HIDDEN.never_explored()
                && !ObjectVisibility::HIDDEN.should_render()
                && ObjectVisibility::FOGGED.should_render()
                && (ObjectVisibility::FOGGED.visibility_alpha - 0.3).abs() < 0.01
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
        assert!(vis.should_render());
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

    #[test]
    fn test_object_visibility_from_shroud_flags() {
        assert_eq!(
            ObjectVisibility::from_shroud_flags(true, true),
            ObjectVisibility::VISIBLE
        );
        assert_eq!(
            ObjectVisibility::from_shroud_flags(false, true),
            ObjectVisibility::FOGGED
        );
        assert_eq!(
            ObjectVisibility::from_shroud_flags(false, false),
            ObjectVisibility::HIDDEN
        );
        assert!(ObjectVisibility::HIDDEN.never_explored());
        assert!(!ObjectVisibility::HIDDEN.should_render());
        assert!(ObjectVisibility::FOGGED.should_render());
    }

    #[test]
    fn presentation_fow_grid_r8_encoding_and_sample() {
        let mut cells = vec![PresentationFowGrid::CELL_HIDDEN; 4];
        cells[0] = PresentationFowGrid::CELL_VISIBLE; // (0,0)
        cells[1] = PresentationFowGrid::CELL_EXPLORED; // (1,0)
        cells[2] = PresentationFowGrid::CELL_HIDDEN; // (0,1)
        cells[3] = PresentationFowGrid::CELL_VISIBLE; // (1,1)
        let grid = PresentationFowGrid::from_snapshot(2, 2, 50.0, cells);
        assert!(grid.active);
        assert_eq!(grid.cell_at(0, 0), PresentationFowGrid::CELL_VISIBLE);
        assert_eq!(grid.cell_at(1, 0), PresentationFowGrid::CELL_EXPLORED);
        assert_eq!(
            grid.state_at_world_xy(10.0, 10.0),
            PresentationFowGrid::CELL_VISIBLE
        );
        assert_eq!(
            grid.state_at_world_xy(60.0, 10.0),
            PresentationFowGrid::CELL_EXPLORED
        );

        let r8 = grid.to_r8_texture();
        assert_eq!(
            r8,
            vec![
                PresentationFowGrid::R8_VISIBLE,
                PresentationFowGrid::R8_FOGGED,
                PresentationFowGrid::R8_SHROUDED,
                PresentationFowGrid::R8_VISIBLE,
            ]
        );
        assert_eq!(
            PresentationFowGrid::cell_to_object_visibility(PresentationFowGrid::CELL_EXPLORED),
            ObjectVisibility::FOGGED
        );
        assert_eq!(
            FOWRenderingBridge::shroud_state_to_cell(ShroudState::Visible),
            PresentationFowGrid::CELL_VISIBLE
        );
        assert_eq!(
            FOWRenderingBridge::shroud_state_to_cell(ShroudState::Explored),
            PresentationFowGrid::CELL_EXPLORED
        );
        assert_eq!(
            FOWRenderingBridge::shroud_state_to_cell(ShroudState::Hidden),
            PresentationFowGrid::CELL_HIDDEN
        );
    }

    #[test]
    fn presentation_fow_grid_inactive_fail_open() {
        let g = PresentationFowGrid::inactive();
        assert!(!g.active);
        assert!(g.to_r8_texture().is_empty());
        assert_eq!(g.cell_at(0, 0), PresentationFowGrid::CELL_VISIBLE);
        assert_eq!(
            g.state_at_world_xy(999.0, 999.0),
            PresentationFowGrid::CELL_VISIBLE
        );
    }

    /// Wave 77 residual: FOW cell/R8/inactive/fail-open honesty pack.
    #[test]
    fn fow_residual_pack_wave77_honesty() {
        assert!(honesty_fow_residual_pack_wave77());
        assert_eq!(PRESENTATION_FOW_DEFAULT_CELL_SIZE, 50.0);
        assert_eq!(PresentationFowGrid::CELL_HIDDEN, 0);
        assert_eq!(PresentationFowGrid::CELL_EXPLORED, 1);
        assert_eq!(PresentationFowGrid::CELL_VISIBLE, 2);
        assert_eq!(PresentationFowGrid::R8_SHROUDED, 0);
        assert_eq!(PresentationFowGrid::R8_FOGGED, 128);
        assert_eq!(PresentationFowGrid::R8_VISIBLE, 255);
    }
}

#[cfg(test)]
mod host_fow_fail_open_tests {
    #[test]
    fn host_fow_fail_open_without_object_membership() {
        let src = include_str!("fow_rendering.rs");
        let start = src.find("fn shroud_runtime_active").expect("fn");
        let body = &src[start..src.len().min(start + 900)];
        assert!(
            body.contains("get_visible_objects(player_id)"),
            "must require visible membership"
        );
        assert!(
            body.contains("get_explored_objects(player_id)"),
            "must require explored membership"
        );
        assert!(
            !body.contains("get_last_update_frame() > 0"),
            "last_update_frame alone must not activate FOW object filtering"
        );
    }
}
