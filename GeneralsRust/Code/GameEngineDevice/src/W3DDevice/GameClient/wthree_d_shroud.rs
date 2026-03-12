//! W3DShroud - 3D Shroud/Fog-of-War Rendering System
//!
//! Corresponds to C++ file: GameEngineDevice/Include/W3DDevice/GameClient/W3DShroud.h
//!
//! This module provides the rendering interface for the fog-of-war system,
//! integrating the logical shroud grid from GameLogic with the GPU rendering pipeline.
//!
//! ## Architecture
//!
//! ```text
//! ShroudManager (GameLogic) → W3DShroud (GameClient) → FowTerrainOverlay (GPU)
//!       ↓                            ↓                        ↓
//! Per-cell state            Texture generation       Hardware-accelerated
//! (Hidden/Explored/         (R8 format)              overlay rendering
//!  Visible)                                          (wgpu pipeline)
//! ```
//!
//! ## C++ Reference
//!
//! Ports behavior from:
//! - `W3DShroud.cpp` - Shroud texture management and rendering
//! - `Display.cpp` - setShroudLevel() interface
//! - `TerrainRenderObject.cpp` - Terrain/shroud blending

use std::sync::Arc;
use log::{debug, trace, warn};

/// Maximum number of players
const MAX_PLAYER_COUNT: usize = 8;

/// Shroud cell size in world units
/// Matches C++ SHROUD_GRID_CELL_SIZE from PartitionManager
pub const SHROUD_CELL_SIZE: f32 = 50.0;

/// Shroud texture size (powers of 2 for optimal GPU performance)
/// Typical values: 256x256, 512x512, 1024x1024
pub const SHROUD_TEXTURE_WIDTH: u32 = 256;
pub const SHROUD_TEXTURE_HEIGHT: u32 = 256;

/// Shroud status per cell
/// Matches C++ CellShroudStatus from GameCommon.h
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellShroudStatus {
    /// Cell has never been seen (black shroud)
    Shrouded = 0,
    /// Cell has been seen but is now fogged (darkened)
    Fogged = 1,
    /// Cell is currently visible (no overlay)
    Clear = 2,
}

impl CellShroudStatus {
    /// Convert shroud status to texture value (R8 format)
    /// Matches C++ texture encoding
    pub fn to_texture_value(&self) -> u8 {
        match self {
            CellShroudStatus::Shrouded => 0,   // 0.0 in normalized form
            CellShroudStatus::Fogged => 128,   // 0.5 in normalized form
            CellShroudStatus::Clear => 255,    // 1.0 in normalized form
        }
    }

    /// Create from texture value
    pub fn from_texture_value(value: u8) -> Self {
        match value {
            0..=42 => CellShroudStatus::Shrouded,
            43..=213 => CellShroudStatus::Fogged,
            214..=255 => CellShroudStatus::Clear,
        }
    }
}

/// W3D Shroud Renderer
///
/// Manages per-player shroud textures and rendering state for the 3D view.
/// Synchronizes with ShroudManager to provide visual feedback of fog-of-war state.
pub struct W3DShroud {
    /// Per-player shroud texture data (R8 format)
    /// Stored as [player_id][cell_index]
    player_shroud_data: Vec<Vec<u8>>,

    /// Texture dimensions
    texture_width: u32,
    texture_height: u32,

    /// World bounds (for coordinate mapping)
    world_min_x: f32,
    world_min_z: f32,
    world_max_x: f32,
    world_max_z: f32,

    /// Current player ID being rendered
    current_player_id: usize,

    /// Frame counter for update tracking
    last_update_frame: u32,

    /// Dirty flags per player (needs GPU upload)
    dirty_flags: [bool; MAX_PLAYER_COUNT],

    /// Observer mode (bypasses FOW)
    observer_mode: bool,
}

impl W3DShroud {
    /// Create new W3DShroud instance
    ///
    /// # Arguments
    ///
    /// * `world_bounds` - (min_x, min_z, max_x, max_z) in world units
    pub fn new(world_bounds: (f32, f32, f32, f32)) -> Self {
        let (world_min_x, world_min_z, world_max_x, world_max_z) = world_bounds;

        let texture_size = (SHROUD_TEXTURE_WIDTH * SHROUD_TEXTURE_HEIGHT) as usize;
        let mut player_shroud_data = Vec::with_capacity(MAX_PLAYER_COUNT);

        // Initialize all players with fully shrouded map
        for _ in 0..MAX_PLAYER_COUNT {
            player_shroud_data.push(vec![
                CellShroudStatus::Shrouded.to_texture_value();
                texture_size
            ]);
        }

        debug!(
            "W3DShroud initialized: {}x{} texture, world bounds ({}, {}) to ({}, {})",
            SHROUD_TEXTURE_WIDTH,
            SHROUD_TEXTURE_HEIGHT,
            world_min_x,
            world_min_z,
            world_max_x,
            world_max_z
        );

        Self {
            player_shroud_data,
            texture_width: SHROUD_TEXTURE_WIDTH,
            texture_height: SHROUD_TEXTURE_HEIGHT,
            world_min_x,
            world_min_z,
            world_max_x,
            world_max_z,
            current_player_id: 0,
            last_update_frame: 0,
            dirty_flags: [false; MAX_PLAYER_COUNT],
            observer_mode: false,
        }
    }

    /// Set shroud level for a specific cell
    ///
    /// Matches C++ Display::setShroudLevel() from Display.cpp
    ///
    /// # Arguments
    ///
    /// * `player_id` - Player ID (0-7)
    /// * `cell_x` - Cell X coordinate in partition grid
    /// * `cell_y` - Cell Y coordinate in partition grid
    /// * `status` - New shroud status for this cell
    pub fn set_shroud_level(
        &mut self,
        player_id: usize,
        cell_x: u32,
        cell_y: u32,
        status: CellShroudStatus,
    ) -> Result<(), String> {
        if player_id >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player_id: {}", player_id));
        }

        if cell_x >= self.texture_width || cell_y >= self.texture_height {
            return Err(format!(
                "Cell ({}, {}) out of bounds (max: {}x{})",
                cell_x, cell_y, self.texture_width, self.texture_height
            ));
        }

        let cell_index = (cell_y * self.texture_width + cell_x) as usize;
        let texture_value = status.to_texture_value();

        // Only update if changed
        if self.player_shroud_data[player_id][cell_index] != texture_value {
            self.player_shroud_data[player_id][cell_index] = texture_value;
            self.dirty_flags[player_id] = true;

            trace!(
                "setShroudLevel: player={}, cell=({}, {}), status={:?}",
                player_id,
                cell_x,
                cell_y,
                status
            );
        }

        Ok(())
    }

    /// Get shroud level for a specific cell
    ///
    /// # Arguments
    ///
    /// * `player_id` - Player ID (0-7)
    /// * `cell_x` - Cell X coordinate
    /// * `cell_y` - Cell Y coordinate
    ///
    /// # Returns
    ///
    /// Current shroud status for the cell
    pub fn get_shroud_level(
        &self,
        player_id: usize,
        cell_x: u32,
        cell_y: u32,
    ) -> Result<CellShroudStatus, String> {
        if player_id >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player_id: {}", player_id));
        }

        if cell_x >= self.texture_width || cell_y >= self.texture_height {
            return Err(format!("Cell ({}, {}) out of bounds", cell_x, cell_y));
        }

        let cell_index = (cell_y * self.texture_width + cell_x) as usize;
        let value = self.player_shroud_data[player_id][cell_index];

        Ok(CellShroudStatus::from_texture_value(value))
    }

    /// Get texture data for a specific player (R8 format, ready for GPU upload)
    ///
    /// # Arguments
    ///
    /// * `player_id` - Player ID (0-7)
    ///
    /// # Returns
    ///
    /// Slice of texture data (width * height bytes)
    pub fn get_texture_data(&self, player_id: usize) -> Result<&[u8], String> {
        if player_id >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player_id: {}", player_id));
        }

        Ok(&self.player_shroud_data[player_id])
    }

    /// Check if texture needs GPU upload
    ///
    /// # Arguments
    ///
    /// * `player_id` - Player ID to check
    ///
    /// # Returns
    ///
    /// `true` if texture data has changed since last GPU upload
    pub fn is_dirty(&self, player_id: usize) -> bool {
        player_id < MAX_PLAYER_COUNT && self.dirty_flags[player_id]
    }

    /// Clear dirty flag after GPU upload
    ///
    /// # Arguments
    ///
    /// * `player_id` - Player ID to mark as clean
    pub fn clear_dirty(&mut self, player_id: usize) {
        if player_id < MAX_PLAYER_COUNT {
            self.dirty_flags[player_id] = false;
        }
    }

    /// Set current player ID for rendering
    pub fn set_current_player(&mut self, player_id: usize) {
        if player_id < MAX_PLAYER_COUNT {
            self.current_player_id = player_id;
        }
    }

    /// Get current player ID
    pub fn get_current_player(&self) -> usize {
        self.current_player_id
    }

    /// Get texture dimensions
    pub fn get_texture_size(&self) -> (u32, u32) {
        (self.texture_width, self.texture_height)
    }

    /// Get world bounds
    pub fn get_world_bounds(&self) -> (f32, f32, f32, f32) {
        (
            self.world_min_x,
            self.world_min_z,
            self.world_max_x,
            self.world_max_z,
        )
    }

    /// Convert world position to cell coordinates
    ///
    /// # Arguments
    ///
    /// * `world_x` - World X coordinate
    /// * `world_z` - World Z coordinate
    ///
    /// # Returns
    ///
    /// (cell_x, cell_y) or None if out of bounds
    pub fn world_to_cell(&self, world_x: f32, world_z: f32) -> Option<(u32, u32)> {
        let world_width = self.world_max_x - self.world_min_x;
        let world_height = self.world_max_z - self.world_min_z;

        let norm_x = (world_x - self.world_min_x) / world_width;
        let norm_z = (world_z - self.world_min_z) / world_height;

        if norm_x < 0.0 || norm_x > 1.0 || norm_z < 0.0 || norm_z > 1.0 {
            return None;
        }

        let cell_x = (norm_x * self.texture_width as f32) as u32;
        let cell_y = (norm_z * self.texture_height as f32) as u32;

        // Clamp to valid range
        let cell_x = cell_x.min(self.texture_width - 1);
        let cell_y = cell_y.min(self.texture_height - 1);

        Some((cell_x, cell_y))
    }

    /// Reveal entire map for a player (debugging/cheats)
    ///
    /// Matches C++ revealMap() functionality
    pub fn reveal_all(&mut self, player_id: usize) -> Result<(), String> {
        if player_id >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player_id: {}", player_id));
        }

        let clear_value = CellShroudStatus::Clear.to_texture_value();
        for cell in &mut self.player_shroud_data[player_id] {
            *cell = clear_value;
        }

        self.dirty_flags[player_id] = true;

        debug!("Revealed entire map for player {}", player_id);
        Ok(())
    }

    /// Shroud entire map for a player (reset to hidden)
    ///
    /// Matches C++ shroudMap() functionality
    pub fn shroud_all(&mut self, player_id: usize) -> Result<(), String> {
        if player_id >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player_id: {}", player_id));
        }

        let shrouded_value = CellShroudStatus::Shrouded.to_texture_value();
        for cell in &mut self.player_shroud_data[player_id] {
            *cell = shrouded_value;
        }

        self.dirty_flags[player_id] = true;

        debug!("Shrouded entire map for player {}", player_id);
        Ok(())
    }

    /// Enable/disable observer mode (bypasses FOW rendering)
    ///
    /// Used for spectators, replays, and debugging
    pub fn set_observer_mode(&mut self, enabled: bool) {
        if self.observer_mode != enabled {
            self.observer_mode = enabled;
            debug!(
                "Observer mode {}",
                if enabled { "enabled" } else { "disabled" }
            );
        }
    }

    /// Check if observer mode is enabled
    pub fn is_observer_mode(&self) -> bool {
        self.observer_mode
    }

    /// Update frame counter
    pub fn set_frame(&mut self, frame: u32) {
        self.last_update_frame = frame;
    }

    /// Get last update frame
    pub fn get_frame(&self) -> u32 {
        self.last_update_frame
    }
}

impl Default for W3DShroud {
    fn default() -> Self {
        // Default world bounds (1000x1000 map)
        Self::new((0.0, 0.0, 1000.0, 1000.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_shroud_status_texture_values() {
        assert_eq!(CellShroudStatus::Shrouded.to_texture_value(), 0);
        assert_eq!(CellShroudStatus::Fogged.to_texture_value(), 128);
        assert_eq!(CellShroudStatus::Clear.to_texture_value(), 255);
    }

    #[test]
    fn test_cell_shroud_status_from_texture() {
        assert_eq!(
            CellShroudStatus::from_texture_value(0),
            CellShroudStatus::Shrouded
        );
        assert_eq!(
            CellShroudStatus::from_texture_value(128),
            CellShroudStatus::Fogged
        );
        assert_eq!(
            CellShroudStatus::from_texture_value(255),
            CellShroudStatus::Clear
        );
    }

    #[test]
    fn test_w3d_shroud_creation() {
        let shroud = W3DShroud::new((0.0, 0.0, 1000.0, 1000.0));

        assert_eq!(shroud.get_texture_size(), (256, 256));
        assert_eq!(shroud.get_world_bounds(), (0.0, 0.0, 1000.0, 1000.0));
        assert_eq!(shroud.get_current_player(), 0);
        assert!(!shroud.is_observer_mode());
    }

    #[test]
    fn test_set_shroud_level() {
        let mut shroud = W3DShroud::default();

        // Initially shrouded
        assert_eq!(
            shroud.get_shroud_level(0, 0, 0).unwrap(),
            CellShroudStatus::Shrouded
        );

        // Set to visible
        shroud
            .set_shroud_level(0, 0, 0, CellShroudStatus::Clear)
            .unwrap();
        assert_eq!(
            shroud.get_shroud_level(0, 0, 0).unwrap(),
            CellShroudStatus::Clear
        );
        assert!(shroud.is_dirty(0));

        // Clear dirty flag
        shroud.clear_dirty(0);
        assert!(!shroud.is_dirty(0));
    }

    #[test]
    fn test_world_to_cell() {
        let shroud = W3DShroud::new((0.0, 0.0, 1000.0, 1000.0));

        // Center of world
        let (cell_x, cell_y) = shroud.world_to_cell(500.0, 500.0).unwrap();
        assert_eq!(cell_x, 128); // Middle of 256-cell grid
        assert_eq!(cell_y, 128);

        // Corner
        let (cell_x, cell_y) = shroud.world_to_cell(0.0, 0.0).unwrap();
        assert_eq!(cell_x, 0);
        assert_eq!(cell_y, 0);

        // Out of bounds
        assert!(shroud.world_to_cell(-10.0, -10.0).is_none());
        assert!(shroud.world_to_cell(1010.0, 1010.0).is_none());
    }

    #[test]
    fn test_reveal_all() {
        let mut shroud = W3DShroud::default();

        shroud.reveal_all(0).unwrap();

        // Check a few cells
        assert_eq!(
            shroud.get_shroud_level(0, 0, 0).unwrap(),
            CellShroudStatus::Clear
        );
        assert_eq!(
            shroud.get_shroud_level(0, 128, 128).unwrap(),
            CellShroudStatus::Clear
        );
        assert_eq!(
            shroud.get_shroud_level(0, 255, 255).unwrap(),
            CellShroudStatus::Clear
        );

        assert!(shroud.is_dirty(0));
    }

    #[test]
    fn test_shroud_all() {
        let mut shroud = W3DShroud::default();

        // Reveal first
        shroud.reveal_all(0).unwrap();
        shroud.clear_dirty(0);

        // Then shroud
        shroud.shroud_all(0).unwrap();

        assert_eq!(
            shroud.get_shroud_level(0, 128, 128).unwrap(),
            CellShroudStatus::Shrouded
        );
        assert!(shroud.is_dirty(0));
    }

    #[test]
    fn test_observer_mode() {
        let mut shroud = W3DShroud::default();

        assert!(!shroud.is_observer_mode());

        shroud.set_observer_mode(true);
        assert!(shroud.is_observer_mode());

        shroud.set_observer_mode(false);
        assert!(!shroud.is_observer_mode());
    }

    #[test]
    fn test_texture_data() {
        let shroud = W3DShroud::default();

        let data = shroud.get_texture_data(0).unwrap();
        assert_eq!(data.len(), (256 * 256) as usize);

        // All cells should be shrouded initially
        assert!(data.iter().all(|&v| v == 0));
    }

    #[test]
    fn test_per_player_independence() {
        let mut shroud = W3DShroud::default();

        // Reveal for player 0, shrouded for player 1
        shroud
            .set_shroud_level(0, 100, 100, CellShroudStatus::Clear)
            .unwrap();

        assert_eq!(
            shroud.get_shroud_level(0, 100, 100).unwrap(),
            CellShroudStatus::Clear
        );
        assert_eq!(
            shroud.get_shroud_level(1, 100, 100).unwrap(),
            CellShroudStatus::Shrouded
        );
    }
}
