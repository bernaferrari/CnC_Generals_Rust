//! Minimap Panel UI
//!
//! This module defines framework-neutral minimap state and coordinate conversion.
//! Rendering is delegated to the active UI backend (GPUI/other), not hardcoded here.

use crate::ui::hud_state::{UiColor, UiPos2, UiTextureId};
use glam::Vec3;

/// Minimap UI state
#[derive(Debug, Clone)]
pub struct MinimapUIState {
    /// Minimap texture ID from renderer
    pub fow_texture_id: Option<UiTextureId>,

    /// Minimap dimensions in screen pixels
    pub width: f32,
    pub height: f32,

    /// Position on screen (top-right corner)
    pub screen_pos: UiPos2,

    /// World bounds for coordinate mapping
    pub world_min: Vec3,
    pub world_max: Vec3,

    /// Camera viewport in world coordinates
    pub camera_bounds: (Vec3, Vec3),

    /// Unit positions to display
    pub unit_positions: Vec<UnitDot>,
    /// Beacon markers to display
    pub beacon_positions: Vec<BeaconDot>,
    /// Radar pings to display (world coords + intensity/age)
    pub radar_pings: Vec<RadarPing>,
    /// Temporary beacon highlight for new pings
    pub beacon_highlight: Option<BeaconHighlight>,

    /// Show FOW overlay
    pub show_fow: bool,

    /// Show terrain base (if no FOW texture)
    pub show_terrain: bool,
}

/// Unit dot on minimap
#[derive(Debug, Clone)]
pub struct UnitDot {
    /// World position of the unit
    pub world_pos: Vec3,

    /// Color of the dot (based on team/type)
    pub color: UiColor,

    /// Size of the dot in pixels
    pub size: f32,

    /// Is this the selected unit
    pub is_selected: bool,
}

/// Beacon marker displayed on the minimap.
#[derive(Debug, Clone)]
pub struct BeaconDot {
    pub world_pos: Vec3,
    pub color: UiColor,
}

/// Radar ping with fade/pulse metadata
#[derive(Debug, Clone, Copy)]
pub struct RadarPing {
    pub world_pos: Vec3,
    pub intensity: f32,
    pub age_seconds: f32,
    pub kind: RadarPingKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadarPingKind {
    Generic,
    Attack,
    Ally,
}

#[derive(Debug, Clone, Copy)]
pub struct BeaconHighlight {
    pub world_pos: Vec3,
    pub timer: f32,
}

impl Default for MinimapUIState {
    fn default() -> Self {
        Self {
            fow_texture_id: None,
            width: 256.0,
            height: 256.0,
            screen_pos: UiPos2::new(10.0, 10.0),
            world_min: Vec3::ZERO,
            world_max: Vec3::new(1024.0, 0.0, 1024.0),
            camera_bounds: (Vec3::ZERO, Vec3::ZERO),
            unit_positions: Vec::new(),
            beacon_positions: Vec::new(),
            radar_pings: Vec::new(),
            beacon_highlight: None,
            show_fow: true,
            show_terrain: true,
        }
    }
}

impl MinimapUIState {
    pub fn set_screen_pos(&mut self, x: f32, y: f32) {
        self.screen_pos = UiPos2::new(x, y);
    }

    /// Convert world position to minimap screen coordinates
    pub fn world_to_minimap(&self, world_pos: Vec3) -> UiPos2 {
        let span_x = (self.world_max.x - self.world_min.x).abs().max(1.0e-4);
        let span_z = (self.world_max.z - self.world_min.z).abs().max(1.0e-4);
        let x_ratio = ((world_pos.x - self.world_min.x) / span_x).clamp(0.0, 1.0);
        let z_ratio = ((world_pos.z - self.world_min.z) / span_z).clamp(0.0, 1.0);

        UiPos2::new(
            self.screen_pos.x + x_ratio * self.width,
            self.screen_pos.y + z_ratio * self.height,
        )
    }

    /// Convert minimap screen coordinates to world position
    pub fn minimap_to_world(&self, minimap_pos: UiPos2) -> Vec3 {
        let width = self.width.max(1.0e-4);
        let height = self.height.max(1.0e-4);
        let x_ratio = ((minimap_pos.x - self.screen_pos.x) / width).clamp(0.0, 1.0);
        let z_ratio = ((minimap_pos.y - self.screen_pos.y) / height).clamp(0.0, 1.0);

        Vec3::new(
            self.world_min.x + x_ratio * (self.world_max.x - self.world_min.x),
            0.0,
            self.world_min.z + z_ratio * (self.world_max.z - self.world_min.z),
        )
    }

    pub fn update_beacons(&mut self, beacons: Vec<BeaconDot>) {
        self.beacon_positions = beacons;
    }

    pub fn set_beacon_highlight(&mut self, world_pos: Vec3) {
        self.beacon_highlight = Some(BeaconHighlight {
            world_pos,
            timer: 1.0,
        });
    }

    /// Check if a position is within minimap bounds
    pub fn contains(&self, pos: UiPos2) -> bool {
        pos.x >= self.screen_pos.x
            && pos.x <= self.screen_pos.x + self.width
            && pos.y >= self.screen_pos.y
            && pos.y <= self.screen_pos.y + self.height
    }
}

/// Minimap click event
#[derive(Debug, Clone)]
pub struct MinimapClickEvent {
    /// World position clicked
    pub world_position: Vec3,

    /// Screen position of click
    pub screen_position: UiPos2,

    /// Was this a right-click (for move commands)
    pub is_right_click: bool,
}

/// Update minimap with game state
pub fn update_minimap_state(
    ui_state: &mut MinimapUIState,
    camera_pos: Vec3,
    camera_size: f32,
    units: &[(Vec3, UiColor, bool)], // (position, color, selected)
    radar_pings: &[RadarPing],
    delta_time: f32,
    new_beacons: &[Vec3],
) {
    // Update camera bounds
    ui_state.camera_bounds = (
        Vec3::new(camera_pos.x - camera_size, 0.0, camera_pos.z - camera_size),
        Vec3::new(camera_pos.x + camera_size, 0.0, camera_pos.z + camera_size),
    );

    // Update unit positions
    ui_state.unit_positions.clear();
    for (pos, color, selected) in units {
        ui_state.unit_positions.push(UnitDot {
            world_pos: *pos,
            color: *color,
            size: if *selected { 3.0 } else { 2.0 },
            is_selected: *selected,
        });
    }

    ui_state.radar_pings.clear();
    ui_state.radar_pings.extend_from_slice(radar_pings);

    if let Some(highlight) = &mut ui_state.beacon_highlight {
        let new_timer = (highlight.timer - delta_time).max(0.0);
        if new_timer <= 0.0 {
            ui_state.beacon_highlight = None;
        } else {
            highlight.timer = new_timer;
        }
    }

    if !new_beacons.is_empty() {
        // Start a highlight on the most recent beacon location
        if let Some(last) = new_beacons.last().copied() {
            ui_state.beacon_highlight = Some(BeaconHighlight {
                world_pos: last,
                timer: 1.0,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinate_conversion() {
        let mut ui_state = MinimapUIState::default();
        ui_state.world_min = Vec3::new(0.0, 0.0, 0.0);
        ui_state.world_max = Vec3::new(1000.0, 0.0, 1000.0);
        ui_state.width = 200.0;
        ui_state.height = 200.0;
        ui_state.screen_pos = UiPos2::new(10.0, 10.0);

        // Test world to minimap
        let world_pos = Vec3::new(500.0, 0.0, 500.0);
        let minimap_pos = ui_state.world_to_minimap(world_pos);
        assert_eq!(minimap_pos.x, 110.0); // 10 + 200*0.5
        assert_eq!(minimap_pos.y, 110.0);

        // Test minimap to world
        let minimap_pos = UiPos2::new(110.0, 110.0);
        let world_pos = ui_state.minimap_to_world(minimap_pos);
        assert!((world_pos.x - 500.0).abs() < 0.1);
        assert!((world_pos.z - 500.0).abs() < 0.1);
    }

    #[test]
    fn test_contains() {
        let ui_state = MinimapUIState {
            screen_pos: UiPos2::new(10.0, 10.0),
            width: 256.0,
            height: 256.0,
            ..Default::default()
        };

        assert!(ui_state.contains(UiPos2::new(100.0, 100.0)));
        assert!(!ui_state.contains(UiPos2::new(300.0, 100.0)));
        assert!(!ui_state.contains(UiPos2::new(5.0, 100.0)));
    }

    #[test]
    fn test_minimap_conversions_clamp_out_of_bounds() {
        let mut ui_state = MinimapUIState::default();
        ui_state.world_min = Vec3::new(0.0, 0.0, 0.0);
        ui_state.world_max = Vec3::new(1000.0, 0.0, 1000.0);
        ui_state.width = 200.0;
        ui_state.height = 200.0;
        ui_state.screen_pos = UiPos2::new(10.0, 10.0);

        let world = ui_state.minimap_to_world(UiPos2::new(-100.0, 9999.0));
        assert!((world.x - 0.0).abs() < 0.1);
        assert!((world.z - 1000.0).abs() < 0.1);

        let pos = ui_state.world_to_minimap(Vec3::new(-500.0, 0.0, 5000.0));
        assert!((pos.x - 10.0).abs() < 0.1);
        assert!((pos.y - 210.0).abs() < 0.1);
    }
}
