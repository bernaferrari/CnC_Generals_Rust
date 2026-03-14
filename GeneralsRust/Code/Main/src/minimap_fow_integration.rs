//! Minimap FOW Integration Example
//!
//! This module shows how to integrate the minimap FOW texture rendering system
//! with the game engine and UI.

use anyhow::Result;
use glam::{Vec2, Vec3};
use log::{debug, error, info};
use std::sync::Arc;
use ww3d_engine::FrameTiming;

const LOGIC_FRAME_TIMESTEP: f32 = 1.0 / 30.0;

use crate::game_logic::GameLogic;
use crate::graphics::RenderPipeline;
use crate::ui::{
    MinimapClickEvent, MinimapUIState, UiColor, UiPos2, UiTextureId, update_minimap_state,
};

/// Example integration struct showing how to use the minimap FOW system
pub struct MinimapFowIntegration {
    /// Render pipeline with minimap renderer
    render_pipeline: RenderPipeline,

    /// UI state for minimap
    minimap_ui_state: MinimapUIState,

    /// Camera position for viewport indicator
    camera_position: Vec3,

    /// Camera zoom level (determines viewport size)
    camera_zoom: f32,

    /// Selected units for display on minimap
    selected_units: Vec<u32>,

    /// Performance metrics
    update_count: u32,
    last_frame_number: u64,
    last_total_seconds: f32,
}

impl MinimapFowIntegration {
    /// Create new minimap FOW integration
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        graphics_system: &crate::graphics::GraphicsSystem,
    ) -> Result<Self> {
        // Initialize render pipeline
        let mut render_pipeline = RenderPipeline::initialize(graphics_system)?;

        // Set world bounds (example: 2048x2048 map)
        let world_bounds = (
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(2048.0, 0.0, 2048.0),
        );

        // Initialize minimap renderer in the pipeline
        render_pipeline.initialize_minimap_renderer(
            device.clone(),
            queue.clone(),
            world_bounds,
        )?;

        // Create UI state for minimap
        let minimap_ui_state = MinimapUIState {
            fow_texture_id: None,
            width: 256.0,
            height: 256.0,
            screen_pos: UiPos2::new(10.0, 10.0), // Top-left corner
            world_min: world_bounds.0,
            world_max: world_bounds.1,
            camera_bounds: (Vec3::ZERO, Vec3::ZERO),
            unit_positions: Vec::new(),
            beacon_positions: Vec::new(),
            radar_pings: Vec::new(),
            show_fow: true,
            show_terrain: true,
        };

        Ok(Self {
            render_pipeline,
            minimap_ui_state,
            camera_position: Vec3::new(1024.0, 0.0, 1024.0), // Center of map
            camera_zoom: 1.0,
            selected_units: Vec::new(),
            update_count: 0,
            last_frame_number: 0,
            last_total_seconds: 0.0,
        })
    }

    /// Attach a UI texture id that the active UI backend can present.
    pub fn set_ui_texture_id(&mut self, texture_id: UiTextureId) {
        self.minimap_ui_state.fow_texture_id = Some(texture_id);
    }

    /// Update minimap each frame
    pub fn update_with_timing(
        &mut self,
        game_logic: &GameLogic,
        current_player: u32,
        timing: &FrameTiming,
    ) -> Result<()> {
        self.update_internal(
            game_logic,
            current_player,
            timing.frame_number,
            timing.total_seconds(),
        )
    }

    /// Update minimap with explicit frame index (legacy fallback)
    pub fn update(
        &mut self,
        game_logic: &GameLogic,
        current_player: u32,
        frame_number: u64,
    ) -> Result<()> {
        let approximate_seconds = frame_number as f32 * LOGIC_FRAME_TIMESTEP;
        self.update_internal(
            game_logic,
            current_player,
            frame_number,
            approximate_seconds,
        )
    }

    fn update_internal(
        &mut self,
        game_logic: &GameLogic,
        current_player: u32,
        frame_number: u64,
        total_seconds: f32,
    ) -> Result<()> {
        let delta_time = (total_seconds - self.last_total_seconds).max(0.0);
        // Set current player for FOW visibility
        self.render_pipeline.set_current_player(current_player);

        // Update FOW texture (automatically called in render pipeline)
        // This happens in render_pipeline.execute() but we can also call it manually:
        self.render_pipeline.update_minimap_fow_texture()?;

        // Collect unit positions for minimap dots
        let mut unit_positions = Vec::new();
        for (object_id, object) in game_logic.get_objects() {
            if object.is_alive() {
                let position = object.get_position();

                // Determine color based on ownership
                let color = if object.get_owner() == current_player {
                    UiColor::from_rgb(0, 255, 0) // Friendly units
                } else if object.get_owner() == 0 {
                    UiColor::from_rgb(255, 255, 255) // Neutral
                } else {
                    UiColor::from_rgb(255, 0, 0) // Enemy units (if visible)
                };

                // Check if unit is selected
                let is_selected = self.selected_units.contains(object_id);

                unit_positions.push((position, color, is_selected));
            }
        }

        // Calculate camera viewport size based on zoom
        let viewport_size = 200.0 / self.camera_zoom;

        // Update minimap UI state
        update_minimap_state(
            &mut self.minimap_ui_state,
            self.camera_position,
            viewport_size,
            &unit_positions,
            &[] as &[crate::ui::minimap_panel::RadarPing],
            delta_time,
            &[],
        );

        // Track performance
        self.update_count += 1;
        self.last_frame_number = frame_number;
        self.last_total_seconds = total_seconds.max(0.0);

        Ok(())
    }

    /// Handle one minimap click event coming from the active UI backend.
    pub fn process_click_event(
        &mut self,
        event: &MinimapClickEvent,
        game_logic: &mut GameLogic,
        current_player: u32,
    ) {
        self.handle_minimap_click(event, game_logic, current_player);
    }

    /// Handle minimap click event
    fn handle_minimap_click(
        &mut self,
        event: &MinimapClickEvent,
        game_logic: &mut GameLogic,
        current_player: u32,
    ) {
        // Check if the clicked area is visible/explored
        if let Some(world_pos) = self.render_pipeline.handle_minimap_click(
            Vec2::new(event.screen_position.x, event.screen_position.y)
        ) {
            if event.is_right_click {
                // Right-click: Move selected units
                info!("Move units to world position: {:?}", world_pos);
                let unit_ids: Vec<crate::game_logic::ObjectId> = self
                    .selected_units
                    .iter()
                    .copied()
                    .map(crate::game_logic::ObjectId)
                    .collect();
                for unit_id in &unit_ids {
                    let _ = game_logic.assign_unit_path(*unit_id, world_pos, &[]);
                }
                if let Some(player) = game_logic.get_player_mut(current_player) {
                    player.selected_objects = unit_ids;
                }
            } else {
                // Left-click: Pan camera
                info!("Pan camera to world position: {:?}", world_pos);
                self.camera_position = world_pos;
            }
        } else {
            // Area is not explored - show warning
            debug!("Cannot click on unexplored area");
        }
    }

    /// Set selected units
    pub fn set_selected_units(&mut self, units: Vec<u32>) {
        self.selected_units = units;
    }

    /// Set camera position
    pub fn set_camera_position(&mut self, position: Vec3) {
        self.camera_position = position;
    }

    /// Set camera zoom
    pub fn set_camera_zoom(&mut self, zoom: f32) {
        self.camera_zoom = zoom.clamp(0.5, 3.0);
    }

    /// Get performance metrics
    pub fn get_performance_metrics(&self) -> (u32, f32) {
        let updates_per_second = if self.last_total_seconds > 0.0 {
            self.update_count as f32 / self.last_total_seconds
        } else {
            0.0
        };
        (self.update_count, updates_per_second)
    }
}

/// Test the minimap FOW system
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minimap_integration_creation() {
        // This would need actual wgpu device/queue in a real test
        // For now, just verify the structure compiles
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn test_coordinate_mapping() {
        let ui_state = MinimapUIState {
            world_min: Vec3::new(0.0, 0.0, 0.0),
            world_max: Vec3::new(1000.0, 0.0, 1000.0),
            width: 200.0,
            height: 200.0,
            screen_pos: UiPos2::new(10.0, 10.0),
            ..Default::default()
        };

        // Test world to minimap conversion
        let world_pos = Vec3::new(500.0, 0.0, 500.0);
        let minimap_pos = ui_state.world_to_minimap(world_pos);
        assert_eq!(minimap_pos.x, 110.0); // 10 + 200*0.5
        assert_eq!(minimap_pos.y, 110.0);
    }
}
