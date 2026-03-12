//! Selection Renderer - Visual Feedback for Unit Selection
//!
//! This module provides visual feedback for unit selection including:
//! - Selection circles around selected units
//! - Hover highlights for units under the mouse cursor
//! - Team-colored indicators (friendly/enemy)
//! - Health bars and status indicators
//! - Drag selection box rendering

use crate::game_logic::{GameLogic, Object, Team};
use crate::ui::{UIRenderCommand, Vertex};
use crate::unit_control::UnitControlSystem;
use glam::{Vec2, Vec3, Vec4};
use std::sync::Arc;
use std::sync::Mutex as AsyncMutex;

/// Visual feedback for unit selection and highlighting
pub struct SelectionRenderer {
    /// Colors for different selection states
    pub selection_color: [f32; 4],
    pub hover_color: [f32; 4],
    pub friendly_color: [f32; 4],
    pub enemy_color: [f32; 4],
    pub neutral_color: [f32; 4],
    pub health_bar_bg_color: [f32; 4],
    pub health_bar_fg_color: [f32; 4],

    /// Rendering settings
    pub selection_circle_radius: f32,
    pub selection_circle_thickness: f32,
    pub health_bar_width: f32,
    pub health_bar_height: f32,
    pub health_bar_offset: f32,

    /// Animation settings
    pub selection_pulse_speed: f32,
    pub hover_fade_speed: f32,

    /// Animation state
    selection_pulse_time: f32,
    hover_fade_time: f32,
}

impl Default for SelectionRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectionRenderer {
    pub fn new() -> Self {
        Self {
            selection_color: [0.0, 1.0, 0.0, 1.0],     // Green
            hover_color: [1.0, 1.0, 0.0, 0.7],         // Yellow with transparency
            friendly_color: [0.0, 0.0, 1.0, 1.0],      // Blue
            enemy_color: [1.0, 0.0, 0.0, 1.0],         // Red
            neutral_color: [0.7, 0.7, 0.7, 1.0],       // Gray
            health_bar_bg_color: [0.2, 0.2, 0.2, 0.8], // Dark gray
            health_bar_fg_color: [0.0, 0.8, 0.0, 1.0], // Green

            selection_circle_radius: 3.0,
            selection_circle_thickness: 0.2,
            health_bar_width: 4.0,
            health_bar_height: 0.3,
            health_bar_offset: 2.0,

            selection_pulse_speed: 2.0,
            hover_fade_speed: 4.0,

            selection_pulse_time: 0.0,
            hover_fade_time: 0.0,
        }
    }

    /// Update animation timers
    pub fn update(&mut self, dt: f32) {
        self.selection_pulse_time += dt * self.selection_pulse_speed;
        self.hover_fade_time += dt * self.hover_fade_speed;

        // Keep times in reasonable range to prevent overflow
        if self.selection_pulse_time > std::f32::consts::TAU {
            self.selection_pulse_time -= std::f32::consts::TAU;
        }
        if self.hover_fade_time > std::f32::consts::TAU {
            self.hover_fade_time -= std::f32::consts::TAU;
        }
    }

    /// Generate UI render commands for selection visualization
    pub async fn render_selection(
        &self,
        unit_control: &UnitControlSystem,
        game_logic: &Arc<AsyncMutex<GameLogic>>,
        camera_view_matrix: &glam::Mat4,
        camera_proj_matrix: &glam::Mat4,
        window_size: (f32, f32),
    ) -> Vec<UIRenderCommand> {
        let mut commands = Vec::new();
        let logic = game_logic.lock().unwrap();

        // Render selection circles for selected objects
        for &object_id in unit_control.get_selected_objects() {
            if let Some(object) = logic.get_object(object_id) {
                if let Some(screen_pos) = self.world_to_screen(
                    object.get_position(),
                    camera_view_matrix,
                    camera_proj_matrix,
                    window_size,
                ) {
                    let color = self.get_selection_color_animated();
                    commands.push(self.create_selection_circle(screen_pos, color));

                    // Add health bar
                    if object.is_alive() {
                        commands.push(self.create_health_bar(screen_pos, object));
                    }
                }
            }
        }

        // Render hover highlight
        if let Some(hovered_id) = unit_control.get_hovered_object() {
            if let Some(object) = logic.get_object(hovered_id) {
                if let Some(screen_pos) = self.world_to_screen(
                    object.get_position(),
                    camera_view_matrix,
                    camera_proj_matrix,
                    window_size,
                ) {
                    let color = self.get_hover_color_animated();
                    commands.push(self.create_hover_highlight(screen_pos, color));
                }
            }
        }

        // Render team indicators for all visible objects
        for (object_id, object) in logic.get_objects().iter() {
            // Skip selected objects (already have selection circles)
            if unit_control.is_object_selected(*object_id) {
                continue;
            }

            if let Some(screen_pos) = self.world_to_screen(
                object.get_position(),
                camera_view_matrix,
                camera_proj_matrix,
                window_size,
            ) {
                let team_color = self.get_team_color(object.team, unit_control.local_player_team);
                commands.push(self.create_team_indicator(screen_pos, team_color));
            }
        }

        commands
    }

    /// Create selection circle render command
    fn create_selection_circle(&self, center: Vec2, color: [f32; 4]) -> UIRenderCommand {
        let segments = 32;
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // Create circle vertices
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let x = center.x + self.selection_circle_radius * angle.cos();
            let y = center.y + self.selection_circle_radius * angle.sin();

            vertices.push(Vertex {
                position: [x, y, 0.0],
                color,
                tex_coords: [0.0, 0.0],
            });
        }

        // Create line indices for circle outline
        for i in 0..segments {
            indices.push(i as u16);
            indices.push(((i + 1) % segments) as u16);
        }

        UIRenderCommand {
            vertices,
            indices,
            texture_id: None,
            blend_mode: crate::ui::BlendMode::Alpha,
            primitive_type: crate::ui::PrimitiveType::Lines,
            clip_rect: None,
        }
    }

    /// Create hover highlight render command
    fn create_hover_highlight(&self, center: Vec2, color: [f32; 4]) -> UIRenderCommand {
        let radius = self.selection_circle_radius * 1.2; // Slightly larger than selection
        let segments = 16;
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // Center vertex
        vertices.push(Vertex {
            position: [center.x, center.y, 0.0],
            color: [color[0], color[1], color[2], 0.0], // Transparent center
            tex_coords: [0.5, 0.5],
        });

        // Circle vertices
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let x = center.x + radius * angle.cos();
            let y = center.y + radius * angle.sin();

            vertices.push(Vertex {
                position: [x, y, 0.0],
                color,
                tex_coords: [0.0, 0.0],
            });
        }

        // Create fan indices for filled circle
        for i in 0..segments {
            indices.push(0); // Center
            indices.push((i + 1) as u16);
            indices.push(((i + 1) % segments + 1) as u16);
        }

        UIRenderCommand {
            vertices,
            indices,
            texture_id: None,
            blend_mode: crate::ui::BlendMode::Alpha,
            primitive_type: crate::ui::PrimitiveType::Triangles,
            clip_rect: None,
        }
    }

    /// Create team indicator render command
    fn create_team_indicator(&self, center: Vec2, color: [f32; 4]) -> UIRenderCommand {
        let size = 2.0;
        let half_size = size * 0.5;

        let vertices = vec![
            Vertex {
                position: [center.x - half_size, center.y - half_size, 0.0],
                color,
                tex_coords: [0.0, 0.0],
            },
            Vertex {
                position: [center.x + half_size, center.y - half_size, 0.0],
                color,
                tex_coords: [1.0, 0.0],
            },
            Vertex {
                position: [center.x + half_size, center.y + half_size, 0.0],
                color,
                tex_coords: [1.0, 1.0],
            },
            Vertex {
                position: [center.x - half_size, center.y + half_size, 0.0],
                color,
                tex_coords: [0.0, 1.0],
            },
        ];

        let indices = vec![0, 1, 2, 0, 2, 3];

        UIRenderCommand {
            vertices,
            indices,
            texture_id: None,
            blend_mode: crate::ui::BlendMode::Alpha,
            primitive_type: crate::ui::PrimitiveType::Triangles,
            clip_rect: None,
        }
    }

    /// Create health bar render command
    fn create_health_bar(&self, center: Vec2, object: &Object) -> UIRenderCommand {
        let health_percent = object.health.current / object.health.maximum;
        let bar_y = center.y - self.health_bar_offset;
        let half_width = self.health_bar_width * 0.5;
        let half_height = self.health_bar_height * 0.5;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // Background bar
        vertices.extend_from_slice(&[
            Vertex {
                position: [center.x - half_width, bar_y - half_height, 0.0],
                color: self.health_bar_bg_color,
                tex_coords: [0.0, 0.0],
            },
            Vertex {
                position: [center.x + half_width, bar_y - half_height, 0.0],
                color: self.health_bar_bg_color,
                tex_coords: [1.0, 0.0],
            },
            Vertex {
                position: [center.x + half_width, bar_y + half_height, 0.0],
                color: self.health_bar_bg_color,
                tex_coords: [1.0, 1.0],
            },
            Vertex {
                position: [center.x - half_width, bar_y + half_height, 0.0],
                color: self.health_bar_bg_color,
                tex_coords: [0.0, 1.0],
            },
        ]);

        // Health bar
        let health_width = self.health_bar_width * health_percent;
        let _health_half_width = health_width * 0.5;
        let health_color = self.get_health_color(health_percent);

        vertices.extend_from_slice(&[
            Vertex {
                position: [center.x - half_width, bar_y - half_height, 0.0],
                color: health_color,
                tex_coords: [0.0, 0.0],
            },
            Vertex {
                position: [
                    center.x - half_width + health_width,
                    bar_y - half_height,
                    0.0,
                ],
                color: health_color,
                tex_coords: [1.0, 0.0],
            },
            Vertex {
                position: [
                    center.x - half_width + health_width,
                    bar_y + half_height,
                    0.0,
                ],
                color: health_color,
                tex_coords: [1.0, 1.0],
            },
            Vertex {
                position: [center.x - half_width, bar_y + half_height, 0.0],
                color: health_color,
                tex_coords: [0.0, 1.0],
            },
        ]);

        // Background indices
        indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        // Health bar indices
        indices.extend_from_slice(&[4, 5, 6, 4, 6, 7]);

        UIRenderCommand {
            vertices,
            indices,
            texture_id: None,
            blend_mode: crate::ui::BlendMode::Alpha,
            primitive_type: crate::ui::PrimitiveType::Triangles,
            clip_rect: None,
        }
    }

    /// Create drag selection box render command
    pub fn render_selection_box(&self, start: Vec2, end: Vec2) -> UIRenderCommand {
        let min_x = start.x.min(end.x);
        let max_x = start.x.max(end.x);
        let min_y = start.y.min(end.y);
        let max_y = start.y.max(end.y);

        let color = [
            self.selection_color[0],
            self.selection_color[1],
            self.selection_color[2],
            0.3,
        ];

        let vertices = vec![
            // Fill
            Vertex {
                position: [min_x, min_y, 0.0],
                color,
                tex_coords: [0.0, 0.0],
            },
            Vertex {
                position: [max_x, min_y, 0.0],
                color,
                tex_coords: [1.0, 0.0],
            },
            Vertex {
                position: [max_x, max_y, 0.0],
                color,
                tex_coords: [1.0, 1.0],
            },
            Vertex {
                position: [min_x, max_y, 0.0],
                color,
                tex_coords: [0.0, 1.0],
            },
        ];

        let indices = vec![0, 1, 2, 0, 2, 3];

        UIRenderCommand {
            vertices,
            indices,
            texture_id: None,
            blend_mode: crate::ui::BlendMode::Alpha,
            primitive_type: crate::ui::PrimitiveType::Triangles,
            clip_rect: None,
        }
    }

    /// Convert world position to screen coordinates
    fn world_to_screen(
        &self,
        world_pos: Vec3,
        view_matrix: &glam::Mat4,
        proj_matrix: &glam::Mat4,
        window_size: (f32, f32),
    ) -> Option<Vec2> {
        // Transform to clip space
        let world_pos_4 = Vec4::new(world_pos.x, world_pos.y, world_pos.z, 1.0);
        let view_pos = *view_matrix * world_pos_4;
        let clip_pos = *proj_matrix * view_pos;

        // Check if behind camera
        if clip_pos.w <= 0.0 {
            return None;
        }

        // Perspective divide
        let ndc = Vec3::new(
            clip_pos.x / clip_pos.w,
            clip_pos.y / clip_pos.w,
            clip_pos.z / clip_pos.w,
        );

        // Check if outside view frustum
        if ndc.x < -1.0 || ndc.x > 1.0 || ndc.y < -1.0 || ndc.y > 1.0 {
            return None;
        }

        // Convert to screen coordinates
        let screen_x = (ndc.x + 1.0) * 0.5 * window_size.0;
        let screen_y = (1.0 - ndc.y) * 0.5 * window_size.1;

        Some(Vec2::new(screen_x, screen_y))
    }

    /// Get animated selection color
    fn get_selection_color_animated(&self) -> [f32; 4] {
        let pulse = (self.selection_pulse_time.sin() * 0.3 + 0.7).clamp(0.4, 1.0);
        [
            self.selection_color[0],
            self.selection_color[1],
            self.selection_color[2],
            self.selection_color[3] * pulse,
        ]
    }

    /// Get animated hover color
    fn get_hover_color_animated(&self) -> [f32; 4] {
        let fade = (self.hover_fade_time.sin() * 0.3 + 0.7).clamp(0.4, 1.0);
        [
            self.hover_color[0],
            self.hover_color[1],
            self.hover_color[2],
            self.hover_color[3] * fade,
        ]
    }

    /// Get team color based on relationship to local player
    fn get_team_color(&self, team: Team, local_player_team: Team) -> [f32; 4] {
        if team == local_player_team {
            self.friendly_color
        } else {
            match team {
                Team::GLA | Team::USA | Team::China => self.enemy_color,
                _ => self.neutral_color,
            }
        }
    }

    /// Get health bar color based on health percentage
    fn get_health_color(&self, health_percent: f32) -> [f32; 4] {
        if health_percent > 0.6 {
            [0.0, 1.0, 0.0, 1.0] // Green
        } else if health_percent > 0.3 {
            [1.0, 1.0, 0.0, 1.0] // Yellow
        } else {
            [1.0, 0.0, 0.0, 1.0] // Red
        }
    }

    /// Render control group numbers on units (C++ Generals style)
    pub fn render_control_group_numbers(
        &self,
        unit_control: &UnitControlSystem,
        game_logic: &GameLogic,
    ) -> Vec<UIRenderCommand> {
        let mut commands = Vec::new();

        // Get all objects in the game
        for (object_id, object) in game_logic.get_objects() {
            // Check if this unit belongs to any control groups
            let groups = unit_control.get_unit_control_groups(*object_id);

            if !groups.is_empty() {
                let position = object.get_position();

                // Render control group numbers above the unit
                // In a full implementation, this would render text/sprites showing "1", "2", etc.
                // For now, we'll create a visual indicator (colored square)
                for (index, &group_num) in groups.iter().enumerate() {
                    let offset_x = index as f32 * 0.5;
                    let indicator_pos = Vec2::new(
                        position.x + offset_x - (groups.len() as f32 * 0.25),
                        position.z - 3.0, // Above the unit
                    );

                    // Create a small colored rectangle to indicate group membership
                    let group_color = self.get_group_color(group_num);
                    let rect_command =
                        self.create_rectangle(indicator_pos, Vec2::new(0.4, 0.4), group_color);
                    commands.push(rect_command);
                }
            }
        }

        commands
    }

    /// Get color for control group indicator (matching C++ Generals group colors)
    /// Create a colored rectangle UI command
    fn create_rectangle(&self, position: Vec2, size: Vec2, color: [f32; 4]) -> UIRenderCommand {
        let half_width = size.x * 0.5;
        let half_height = size.y * 0.5;

        let vertices = vec![
            Vertex {
                position: [position.x - half_width, position.y - half_height, 0.0],
                color,
                tex_coords: [0.0, 0.0],
            },
            Vertex {
                position: [position.x + half_width, position.y - half_height, 0.0],
                color,
                tex_coords: [1.0, 0.0],
            },
            Vertex {
                position: [position.x + half_width, position.y + half_height, 0.0],
                color,
                tex_coords: [1.0, 1.0],
            },
            Vertex {
                position: [position.x - half_width, position.y + half_height, 0.0],
                color,
                tex_coords: [0.0, 1.0],
            },
        ];

        let indices = vec![0, 1, 2, 0, 2, 3];

        UIRenderCommand {
            vertices,
            indices,
            texture_id: None,
            blend_mode: crate::ui::BlendMode::Alpha,
            primitive_type: crate::ui::PrimitiveType::Triangles,
            clip_rect: None,
        }
    }

    fn get_group_color(&self, group_num: u8) -> [f32; 4] {
        // Use different colors for different groups (similar to C++ Generals)
        match group_num {
            0 => [1.0, 1.0, 1.0, 1.0], // White
            1 => [1.0, 0.0, 0.0, 1.0], // Red
            2 => [0.0, 1.0, 0.0, 1.0], // Green
            3 => [0.0, 0.0, 1.0, 1.0], // Blue
            4 => [1.0, 1.0, 0.0, 1.0], // Yellow
            5 => [1.0, 0.0, 1.0, 1.0], // Magenta
            6 => [0.0, 1.0, 1.0, 1.0], // Cyan
            7 => [1.0, 0.5, 0.0, 1.0], // Orange
            8 => [0.5, 0.0, 1.0, 1.0], // Purple
            9 => [0.5, 1.0, 0.5, 1.0], // Light green
            _ => [0.7, 0.7, 0.7, 1.0], // Gray (fallback)
        }
    }
}
