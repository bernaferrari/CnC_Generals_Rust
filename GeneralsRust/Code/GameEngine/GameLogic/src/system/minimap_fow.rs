//! Minimap Fog-of-War System
//!
//! Manages the minimap visualization of fog-of-war, including:
//! - Per-pixel visibility state (visible/explored/hidden)
//! - Minimap texture generation from visibility data
//! - Real-time updates as visibility changes
//! - Efficient GPU texture management

use crate::common::UnsignedInt;
use log::{debug, trace, warn};
use std::sync::Mutex;
use std::sync::OnceLock;

/// Maximum number of players
const MAX_PLAYER_COUNT: usize = crate::common::MAX_PLAYER_COUNT;

/// Minimap FOW pixel states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MinimapFowState {
    /// Fully hidden - never seen by player
    Hidden = 0,

    /// Explored - player has seen this area but can't see it now (darkened)
    Explored = 1,

    /// Partially visible - at edge of fog-of-war gradient
    Partial = 2,

    /// Fully visible - currently visible to player (bright)
    Visible = 3,
}

impl MinimapFowState {
    /// Convert state to texture color value
    /// Used for minimap rendering
    pub fn to_color_value(&self) -> u8 {
        match self {
            MinimapFowState::Hidden => 0,    // Black
            MinimapFowState::Explored => 85, // Dark gray (33%)
            MinimapFowState::Partial => 170, // Light gray (67%)
            MinimapFowState::Visible => 255, // White (bright)
        }
    }

    /// Get alpha blending factor for minimap display
    pub fn to_alpha(&self) -> f32 {
        match self {
            MinimapFowState::Hidden => 0.0,   // Fully transparent (hidden)
            MinimapFowState::Explored => 0.3, // Very transparent (darkened)
            MinimapFowState::Partial => 0.7,  // Semi-transparent (gradient)
            MinimapFowState::Visible => 1.0,  // Fully opaque (bright)
        }
    }
}

/// Minimap dimensions (configurable)
#[derive(Debug, Clone, Copy)]
pub struct MinimapDimensions {
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// World units per minimap pixel
    pub scale: f32,
}

impl MinimapDimensions {
    /// Create standard minimap dimensions
    /// Typical 256x256 minimap with scale of 4 world units per pixel
    pub fn standard() -> Self {
        Self {
            width: 256,
            height: 256,
            scale: 4.0,
        }
    }
}

/// Minimap FOW Manager
///
/// Maintains per-player minimap fog-of-war state and generates texture data
/// for rendering the minimap with visibility information.
pub struct MinimapFowManager {
    /// Dimensions of minimap texture
    dimensions: MinimapDimensions,

    /// Per-player minimap FOW state
    /// Stores pixel state for minimap visualization
    player_fow_state: [Vec<MinimapFowState>; MAX_PLAYER_COUNT],

    /// Minimap texture data (RGBA8, ready for GPU upload)
    player_fow_texture: [Vec<u8>; MAX_PLAYER_COUNT],

    /// Last frame state was updated
    last_update_frame: UnsignedInt,
}

impl MinimapFowManager {
    /// Create new MinimapFowManager
    pub fn new(dimensions: MinimapDimensions) -> Self {
        let pixel_count = (dimensions.width * dimensions.height) as usize;

        Self {
            dimensions,
            player_fow_state: std::array::from_fn(|_| vec![MinimapFowState::Hidden; pixel_count]),
            player_fow_texture: std::array::from_fn(|_| vec![0; pixel_count * 4]),
            last_update_frame: 0,
        }
    }

    /// Update minimap FOW state for a specific pixel
    pub fn set_pixel_state(
        &mut self,
        player_id: usize,
        pixel_x: u32,
        pixel_y: u32,
        state: MinimapFowState,
    ) -> Result<(), String> {
        if player_id >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player_id: {}", player_id));
        }

        if pixel_x >= self.dimensions.width || pixel_y >= self.dimensions.height {
            return Err(format!(
                "Pixel ({}, {}) out of bounds (max: {}x{})",
                pixel_x, pixel_y, self.dimensions.width, self.dimensions.height
            ));
        }

        let pixel_idx = (pixel_y * self.dimensions.width + pixel_x) as usize;
        self.player_fow_state[player_id][pixel_idx] = state;

        // Mark texture as dirty for regeneration
        Ok(())
    }

    /// Get minimap FOW state for a specific pixel
    pub fn get_pixel_state(
        &self,
        player_id: usize,
        pixel_x: u32,
        pixel_y: u32,
    ) -> Result<MinimapFowState, String> {
        if player_id >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player_id: {}", player_id));
        }

        if pixel_x >= self.dimensions.width || pixel_y >= self.dimensions.height {
            return Err(format!("Pixel ({}, {}) out of bounds", pixel_x, pixel_y));
        }

        let pixel_idx = (pixel_y * self.dimensions.width + pixel_x) as usize;
        Ok(self.player_fow_state[player_id][pixel_idx])
    }

    /// Regenerate texture data from FOW state
    /// Call after updating pixel states to generate GPU-ready texture data
    pub fn regenerate_texture(&mut self, player_id: usize) -> Result<(), String> {
        if player_id >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player_id: {}", player_id));
        }

        let pixel_count = (self.dimensions.width * self.dimensions.height) as usize;

        for pixel_idx in 0..pixel_count {
            let state = self.player_fow_state[player_id][pixel_idx];
            let color = state.to_color_value();

            // RGBA8 format: R, G, B, A
            let tex_idx = pixel_idx * 4;
            self.player_fow_texture[player_id][tex_idx] = color; // Red
            self.player_fow_texture[player_id][tex_idx + 1] = color; // Green
            self.player_fow_texture[player_id][tex_idx + 2] = color; // Blue
            self.player_fow_texture[player_id][tex_idx + 3] = 255; // Alpha (always opaque)
        }

        trace!(
            "Regenerated minimap texture for player {} ({} pixels)",
            player_id,
            pixel_count
        );

        Ok(())
    }

    /// Get texture data for GPU upload (RGBA8 format)
    pub fn get_texture_data(&self, player_id: usize) -> Result<&[u8], String> {
        if player_id >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player_id: {}", player_id));
        }

        Ok(&self.player_fow_texture[player_id])
    }

    /// Get minimap dimensions
    pub fn get_dimensions(&self) -> MinimapDimensions {
        self.dimensions
    }

    /// Clear all FOW state (map reveal for debugging)
    pub fn reveal_all(&mut self, player_id: usize) -> Result<(), String> {
        if player_id >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player_id: {}", player_id));
        }

        let pixel_count = (self.dimensions.width * self.dimensions.height) as usize;
        for state in &mut self.player_fow_state[player_id][0..pixel_count] {
            *state = MinimapFowState::Visible;
        }

        trace!("Revealed all for player {}", player_id);
        Ok(())
    }

    /// Fog entire map (reset to hidden)
    pub fn fog_all(&mut self, player_id: usize) -> Result<(), String> {
        if player_id >= MAX_PLAYER_COUNT {
            return Err(format!("Invalid player_id: {}", player_id));
        }

        let pixel_count = (self.dimensions.width * self.dimensions.height) as usize;
        for state in &mut self.player_fow_state[player_id][0..pixel_count] {
            *state = MinimapFowState::Hidden;
        }

        trace!("Fogged all for player {}", player_id);
        Ok(())
    }

    /// Update last frame state was modified
    pub fn set_update_frame(&mut self, frame: UnsignedInt) {
        self.last_update_frame = frame;
    }

    /// Get last frame state was updated
    pub fn get_last_update_frame(&self) -> UnsignedInt {
        self.last_update_frame
    }
}

impl Default for MinimapFowManager {
    fn default() -> Self {
        Self::new(MinimapDimensions::standard())
    }
}

/// Global singleton accessor for MinimapFowManager
static MINIMAP_FOW_MANAGER: OnceLock<Mutex<MinimapFowManager>> = OnceLock::new();

/// Get the global MinimapFowManager singleton
pub fn get_minimap_fow_manager() -> &'static Mutex<MinimapFowManager> {
    MINIMAP_FOW_MANAGER.get_or_init(|| Mutex::new(MinimapFowManager::default()))
}

#[cfg(test)]
mod minimap_fow_tests {
    use super::*;

    #[test]
    fn test_minimap_fow_basic() {
        let mut manager = MinimapFowManager::default();

        // Check initial state (all hidden)
        assert_eq!(
            manager.get_pixel_state(0, 0, 0).unwrap(),
            MinimapFowState::Hidden
        );

        // Set to visible
        manager
            .set_pixel_state(0, 0, 0, MinimapFowState::Visible)
            .unwrap();
        assert_eq!(
            manager.get_pixel_state(0, 0, 0).unwrap(),
            MinimapFowState::Visible
        );
    }

    #[test]
    fn test_minimap_fow_state_colors() {
        assert_eq!(MinimapFowState::Hidden.to_color_value(), 0);
        assert_eq!(MinimapFowState::Explored.to_color_value(), 85);
        assert_eq!(MinimapFowState::Partial.to_color_value(), 170);
        assert_eq!(MinimapFowState::Visible.to_color_value(), 255);
    }

    #[test]
    fn test_minimap_fow_state_alpha() {
        assert_eq!(MinimapFowState::Hidden.to_alpha(), 0.0);
        assert!((MinimapFowState::Explored.to_alpha() - 0.3).abs() < 0.001);
        assert!((MinimapFowState::Partial.to_alpha() - 0.7).abs() < 0.001);
        assert_eq!(MinimapFowState::Visible.to_alpha(), 1.0);
    }

    #[test]
    fn test_minimap_fow_texture_generation() {
        let mut manager = MinimapFowManager::default();

        // Set some pixels to different states
        manager
            .set_pixel_state(0, 0, 0, MinimapFowState::Visible)
            .unwrap();
        manager
            .set_pixel_state(0, 1, 0, MinimapFowState::Explored)
            .unwrap();
        manager
            .set_pixel_state(0, 2, 0, MinimapFowState::Hidden)
            .unwrap();

        // Regenerate texture
        manager.regenerate_texture(0).unwrap();

        // Check texture data (RGBA8 format)
        let texture = manager.get_texture_data(0).unwrap();

        // Pixel 0: Visible (white)
        assert_eq!(texture[0], 255); // R
        assert_eq!(texture[1], 255); // G
        assert_eq!(texture[2], 255); // B
        assert_eq!(texture[3], 255); // A

        // Pixel 1: Explored (dark gray)
        assert_eq!(texture[4], 85); // R
        assert_eq!(texture[5], 85); // G
        assert_eq!(texture[6], 85); // B
        assert_eq!(texture[7], 255); // A

        // Pixel 2: Hidden (black)
        assert_eq!(texture[8], 0); // R
        assert_eq!(texture[9], 0); // G
        assert_eq!(texture[10], 0); // B
        assert_eq!(texture[11], 255); // A
    }

    #[test]
    fn test_minimap_fow_boundaries() {
        let manager = MinimapFowManager::default();

        // Valid boundaries should work
        assert!(manager.get_pixel_state(0, 0, 0).is_ok());
        assert!(manager.get_pixel_state(0, 255, 255).is_ok());

        // Out of bounds should fail
        assert!(manager.get_pixel_state(0, 256, 0).is_err());
        assert!(manager.get_pixel_state(0, 0, 256).is_err());
    }

    #[test]
    fn test_minimap_fow_reveal_fog() {
        let mut manager = MinimapFowManager::default();

        // Initially all hidden
        assert_eq!(
            manager.get_pixel_state(0, 10, 10).unwrap(),
            MinimapFowState::Hidden
        );

        // Reveal all
        manager.reveal_all(0).unwrap();
        assert_eq!(
            manager.get_pixel_state(0, 10, 10).unwrap(),
            MinimapFowState::Visible
        );

        // Fog all
        manager.fog_all(0).unwrap();
        assert_eq!(
            manager.get_pixel_state(0, 10, 10).unwrap(),
            MinimapFowState::Hidden
        );
    }

    #[test]
    fn test_minimap_fow_per_player() {
        let mut manager = MinimapFowManager::default();

        // Set different states for different players
        manager
            .set_pixel_state(0, 0, 0, MinimapFowState::Visible)
            .unwrap();
        manager
            .set_pixel_state(1, 0, 0, MinimapFowState::Hidden)
            .unwrap();

        assert_eq!(
            manager.get_pixel_state(0, 0, 0).unwrap(),
            MinimapFowState::Visible
        );
        assert_eq!(
            manager.get_pixel_state(1, 0, 0).unwrap(),
            MinimapFowState::Hidden
        );
    }
}
