//! Selection Renderer - Visual Feedback for Unit Selection
//!
//! This module provides visual feedback for unit selection including:
//! - Selection circles around selected units
//! - Hover highlights for units under the mouse cursor
//! - Team-colored indicators (friendly/enemy)
//! - Health bars and status indicators
//! - Drag selection box rendering
//!
//! Wave 79 residual honesty: selection/HUD color + health-bar + pulse defaults
//! (host-testable; not full W3D Drawable health-bar GPU path).

use crate::game_logic::{ObjectId, Team};
use crate::presentation_frame::PresentationFrame;
use crate::ui::{UIRenderCommand, Vertex};
use crate::unit_control::UnitControlSystem;
use glam::{Vec2, Vec3, Vec4};

// --- Wave 79 selection / HUD residual defaults ---
/// Selection ring residual color (green).
pub const SELECTION_COLOR_RGBA: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
/// Hover highlight residual color (yellow translucent).
pub const SELECTION_HOVER_COLOR_RGBA: [f32; 4] = [1.0, 1.0, 0.0, 0.7];
/// Friendly team residual color (blue).
pub const SELECTION_FRIENDLY_COLOR_RGBA: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
/// Enemy team residual color (red).
pub const SELECTION_ENEMY_COLOR_RGBA: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
/// Neutral residual color (gray).
pub const SELECTION_NEUTRAL_COLOR_RGBA: [f32; 4] = [0.7, 0.7, 0.7, 1.0];
/// Health bar background residual.
pub const SELECTION_HEALTH_BAR_BG_RGBA: [f32; 4] = [0.2, 0.2, 0.2, 0.8];
/// Health bar foreground residual (healthy green).
pub const SELECTION_HEALTH_BAR_FG_RGBA: [f32; 4] = [0.0, 0.8, 0.0, 1.0];
/// Selection circle radius residual (world/UI units).
pub const SELECTION_CIRCLE_RADIUS: f32 = 3.0;
/// Selection circle thickness residual.
pub const SELECTION_CIRCLE_THICKNESS: f32 = 0.2;
/// Health bar width residual.
pub const SELECTION_HEALTH_BAR_WIDTH: f32 = 4.0;
/// Health bar height residual.
pub const SELECTION_HEALTH_BAR_HEIGHT: f32 = 0.3;
/// Health bar vertical offset residual above unit.
pub const SELECTION_HEALTH_BAR_OFFSET: f32 = 2.0;
/// Selection pulse angular speed residual (rad/s scale).
pub const SELECTION_PULSE_SPEED: f32 = 2.0;
/// Hover fade speed residual.
pub const SELECTION_HOVER_FADE_SPEED: f32 = 4.0;
/// Hover circle scale residual vs selection circle.
pub const SELECTION_HOVER_RADIUS_SCALE: f32 = 1.2;
/// Pulse alpha residual: `sin * 0.3 + 0.7` clamped to [0.4, 1.0].
pub const SELECTION_PULSE_ALPHA_AMPLITUDE: f32 = 0.3;
pub const SELECTION_PULSE_ALPHA_BIAS: f32 = 0.7;
pub const SELECTION_PULSE_ALPHA_MIN: f32 = 0.4;
pub const SELECTION_PULSE_ALPHA_MAX: f32 = 1.0;

/// Wave 79 selection/HUD residual honesty pack.
///
/// Fail-closed: not full Drawable::drawIcon UI / health-bar bone attach GPU.
pub fn honesty_selection_hud_residual_pack_wave79() -> bool {
    let r = SelectionRenderer::new();
    r.selection_color == SELECTION_COLOR_RGBA
        && r.hover_color == SELECTION_HOVER_COLOR_RGBA
        && r.friendly_color == SELECTION_FRIENDLY_COLOR_RGBA
        && r.enemy_color == SELECTION_ENEMY_COLOR_RGBA
        && r.neutral_color == SELECTION_NEUTRAL_COLOR_RGBA
        && r.health_bar_bg_color == SELECTION_HEALTH_BAR_BG_RGBA
        && r.health_bar_fg_color == SELECTION_HEALTH_BAR_FG_RGBA
        && (r.selection_circle_radius - SELECTION_CIRCLE_RADIUS).abs() < 0.001
        && (r.selection_circle_thickness - SELECTION_CIRCLE_THICKNESS).abs() < 0.001
        && (r.health_bar_width - SELECTION_HEALTH_BAR_WIDTH).abs() < 0.001
        && (r.health_bar_height - SELECTION_HEALTH_BAR_HEIGHT).abs() < 0.001
        && (r.health_bar_offset - SELECTION_HEALTH_BAR_OFFSET).abs() < 0.001
        && (r.selection_pulse_speed - SELECTION_PULSE_SPEED).abs() < 0.001
        && (r.hover_fade_speed - SELECTION_HOVER_FADE_SPEED).abs() < 0.001
        && (SELECTION_HOVER_RADIUS_SCALE - 1.2).abs() < 0.001
        && (SELECTION_PULSE_ALPHA_AMPLITUDE - 0.3).abs() < 0.001
        && (SELECTION_PULSE_ALPHA_BIAS - 0.7).abs() < 0.001
}

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
    /// Options residual: master toggle for selection health bars.
    pub show_health_bars: bool,

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
    pub fn set_show_health_bars(&mut self, enabled: bool) {
        self.show_health_bars = enabled;
    }

    pub fn new() -> Self {
        Self {
            selection_color: SELECTION_COLOR_RGBA,
            hover_color: SELECTION_HOVER_COLOR_RGBA,
            friendly_color: SELECTION_FRIENDLY_COLOR_RGBA,
            enemy_color: SELECTION_ENEMY_COLOR_RGBA,
            neutral_color: SELECTION_NEUTRAL_COLOR_RGBA,
            health_bar_bg_color: SELECTION_HEALTH_BAR_BG_RGBA,
            health_bar_fg_color: SELECTION_HEALTH_BAR_FG_RGBA,

            selection_circle_radius: SELECTION_CIRCLE_RADIUS,
            selection_circle_thickness: SELECTION_CIRCLE_THICKNESS,
            health_bar_width: SELECTION_HEALTH_BAR_WIDTH,
            health_bar_height: SELECTION_HEALTH_BAR_HEIGHT,
            health_bar_offset: SELECTION_HEALTH_BAR_OFFSET,
            show_health_bars: true,

            selection_pulse_speed: SELECTION_PULSE_SPEED,
            hover_fade_speed: SELECTION_HOVER_FADE_SPEED,

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

    /// Generate UI render commands for selection visualization.
    ///
    /// Presentation-only: identity (position/team/health/selected/aliveness) comes
    /// from the immutable snapshot. Returns empty when no frame is provided.

    pub fn render_selection(
        &self,
        unit_control: &UnitControlSystem,
        camera_view_matrix: &glam::Mat4,
        camera_proj_matrix: &glam::Mat4,
        window_size: (f32, f32),
        presentation: Option<&PresentationFrame>,
    ) -> Vec<UIRenderCommand> {
        let Some(frame) = presentation else {
            return Vec::new();
        };
        self.render_selection_from_presentation(
            unit_control,
            frame,
            camera_view_matrix,
            camera_proj_matrix,
            window_size,
        )
    }

    /// Production presentation path: selection/health/team identity from snapshot only.
    pub fn render_selection_from_presentation(
        &self,
        unit_control: &UnitControlSystem,
        frame: &PresentationFrame,
        camera_view_matrix: &glam::Mat4,
        camera_proj_matrix: &glam::Mat4,
        window_size: (f32, f32),
    ) -> Vec<UIRenderCommand> {
        let mut commands = Vec::new();
        let by_id: std::collections::HashMap<
            ObjectId,
            &crate::presentation_frame::RenderableObject,
        > = frame.objects.iter().map(|o| (o.id, o)).collect();

        // Selected units: prefer unit_control list, then frame.selected, then
        // snapshot object.selected flags (presentation-owned identity).
        let selected_ids: Vec<ObjectId> = {
            let from_control: Vec<_> = unit_control.get_selected_objects().to_vec();
            if !from_control.is_empty() {
                from_control
            } else if !frame.selected.is_empty() {
                frame.selected.clone()
            } else {
                frame
                    .objects
                    .iter()
                    .filter(|o| o.selected && !o.destroyed)
                    .map(|o| o.id)
                    .collect()
            }
        };

        for object_id in &selected_ids {
            let Some(ro) = by_id.get(object_id) else {
                continue;
            };
            if ro.destroyed {
                continue;
            }
            if let Some(screen_pos) = self.world_to_screen(
                ro.position,
                camera_view_matrix,
                camera_proj_matrix,
                window_size,
            ) {
                let color = self.get_selection_color_animated();
                commands.push(self.create_selection_circle(screen_pos, color));
                if self.show_health_bars && ro.show_health_bar && ro.health_current > 0.0 {
                    commands.push(self.create_health_bar(
                        screen_pos,
                        ro.health_current,
                        ro.health_max.max(1.0),
                    ));
                }
            }
        }

        if let Some(hovered_id) = unit_control.get_hovered_object() {
            if let Some(ro) = by_id.get(&hovered_id) {
                if !ro.destroyed {
                    if let Some(screen_pos) = self.world_to_screen(
                        ro.position,
                        camera_view_matrix,
                        camera_proj_matrix,
                        window_size,
                    ) {
                        let color = self.get_hover_color_animated();
                        commands.push(self.create_hover_highlight(screen_pos, color));
                    }
                }
            }
        }

        for ro in &frame.objects {
            if ro.destroyed {
                continue;
            }
            if unit_control.is_object_selected(ro.id) || selected_ids.contains(&ro.id) {
                continue;
            }
            if let Some(screen_pos) = self.world_to_screen(
                ro.position,
                camera_view_matrix,
                camera_proj_matrix,
                window_size,
            ) {
                let team_color = self.get_team_color(ro.team, unit_control.local_player_team);
                commands.push(self.create_team_indicator(screen_pos, team_color));
            }
        }

        // Control-group badges from snapshot positions (no live GameLogic).
        commands.extend(self.render_control_group_numbers_from_presentation(unit_control, frame));
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
        let radius = self.selection_circle_radius * SELECTION_HOVER_RADIUS_SCALE;
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

    /// Create health bar render command from snapshot or live health values.
    fn create_health_bar(
        &self,
        center: Vec2,
        health_current: f32,
        health_maximum: f32,
    ) -> UIRenderCommand {
        let health_percent = if health_maximum > 0.0 {
            (health_current / health_maximum).clamp(0.0, 1.0)
        } else {
            0.0
        };
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
        let pulse = (self.selection_pulse_time.sin() * SELECTION_PULSE_ALPHA_AMPLITUDE
            + SELECTION_PULSE_ALPHA_BIAS)
            .clamp(SELECTION_PULSE_ALPHA_MIN, SELECTION_PULSE_ALPHA_MAX);
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
    /// Control-group badges from presentation identity (preferred path).
    ///
    /// Positions come from the frozen `PresentationFrame` — not a live
    /// `GameLogic` re-read. Unit→group membership stays client-side in
    /// `UnitControlSystem`.
    pub fn render_control_group_numbers_from_presentation(
        &self,
        unit_control: &UnitControlSystem,
        frame: &PresentationFrame,
    ) -> Vec<UIRenderCommand> {
        let mut commands = Vec::new();
        let by_id: std::collections::HashMap<
            ObjectId,
            &crate::presentation_frame::RenderableObject,
        > = frame.objects.iter().map(|o| (o.id, o)).collect();

        // Prefer control-system membership walk; fall back to every snapshot object.
        let mut seen = std::collections::HashSet::new();
        let mut candidates: Vec<ObjectId> = unit_control
            .get_selected_objects()
            .iter()
            .copied()
            .collect();
        for o in &frame.objects {
            candidates.push(o.id);
        }
        for object_id in candidates {
            if !seen.insert(object_id) {
                continue;
            }
            let groups = unit_control.get_unit_control_groups(object_id);
            if groups.is_empty() {
                continue;
            }
            let Some(obj) = by_id.get(&object_id) else {
                continue;
            };
            if obj.destroyed || obj.health_current <= 0.0 {
                continue;
            }
            let position = obj.position;
            for (index, &group_num) in groups.iter().enumerate() {
                let offset_x = index as f32 * 0.5;
                let indicator_pos = Vec2::new(
                    position.x + offset_x - (groups.len() as f32 * 0.25),
                    position.z - 3.0,
                );
                let group_color = self.get_group_color(group_num);
                commands.push(self.create_rectangle(
                    indicator_pos,
                    Vec2::new(0.4, 0.4),
                    group_color,
                ));
            }
        }
        commands
    }

    /// Presentation-only control-group badges (no live GameLogic dual-read).

    pub fn render_control_group_numbers(
        &self,
        unit_control: &UnitControlSystem,
        presentation: Option<&PresentationFrame>,
    ) -> Vec<UIRenderCommand> {
        let Some(frame) = presentation else {
            return Vec::new();
        };
        self.render_control_group_numbers_from_presentation(unit_control, frame)
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

#[cfg(test)]
mod presentation_identity_tests {
    use super::*;
    use crate::game_logic::{GameLogic, KindOf, ThingTemplate};
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::{Mat4, Vec3};

    #[test]
    fn selection_hud_residual_pack_wave79_honesty() {
        assert!(honesty_selection_hud_residual_pack_wave79());
    }

    #[test]
    fn legacy_selection_renderer_uses_presentation_identity_not_live_reread() {
        // Criterion 2: shipped legacy SelectionRenderer path prefers PresentationFrame.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("LegacySelPres");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("LegacySelUnit");
        t.set_health(80.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("LegacySelUnit".into(), t);
        // Near origin so identity matrices project into NDC frustum.
        let id = logic
            .create_object("LegacySelUnit", Team::USA, Vec3::ZERO)
            .expect("unit");
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
        }
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        assert!(
            snap.objects.iter().any(|o| o.id == id && !o.destroyed),
            "snapshot must contain unit"
        );
        assert!(
            snap.selected.contains(&id) || snap.objects.iter().any(|o| o.id == id && o.selected),
            "snapshot must record selection identity"
        );

        // Mutate live world after snapshot: position leaves NDC frustum under identity matrices.
        if let Some(o) = logic.get_object_mut(id) {
            o.set_position(Vec3::new(999.0, 0.0, 999.0));
            o.health.current = 1.0;
            o.selected = false;
        }

        // Empty unit_control selection → consumer falls back to frame.selected / object.selected.
        let unit_control = UnitControlSystem::new((800.0, 600.0), Team::USA, 0);
        let identity = Mat4::IDENTITY;
        let renderer = SelectionRenderer::new();
        let cmds = renderer.render_selection_from_presentation(
            &unit_control,
            &snap,
            &identity,
            &identity,
            (800.0, 600.0),
        );
        assert!(
            !cmds.is_empty(),
            "presentation path must draw selection/health from snapshot at origin"
        );

        // If consumer re-read live position (999), world_to_screen would cull — empty.
        // Snapshot at ZERO projects with identity matrices → non-empty commands.
        let triangle_cmds = cmds
            .iter()
            .filter(|c| c.primitive_type == crate::ui::PrimitiveType::Triangles)
            .count();
        assert!(
            triangle_cmds > 0,
            "health/team triangles expected from snapshot health"
        );
    }

    #[test]
    fn control_group_numbers_use_presentation_positions() {
        use crate::game_logic::{KindOf, Team, ThingTemplate};
        use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
        use crate::unit_control::UnitControlSystem;
        use glam::Vec3;
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("CtrlGrpPres");
        apply_skirmish_config(&mut logic, &cfg).expect("cfg");
        let mut t = ThingTemplate::new("CtrlGrpUnit");
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        t.set_health(100.0);
        logic.templates.insert("CtrlGrpUnit".into(), t);
        let id = logic
            .create_object("CtrlGrpUnit", Team::USA, Vec3::new(10.0, 0.0, 20.0))
            .expect("u");
        let snap = PresentationFrame::build_from_logic(&logic, 0);
        if let Some(o) = logic.get_object_mut(id) {
            o.set_position(Vec3::new(999.0, 0.0, 999.0));
        }
        let logic_arc = std::sync::Arc::new(std::sync::Mutex::new(logic));
        let mut uc = UnitControlSystem::new((800.0, 600.0), Team::USA, 0);
        uc.selected_objects = vec![id];
        uc.set_presentation_frame(Some(snap.clone()));
        futures::executor::block_on(uc.assign_control_group(1, &logic_arc));
        assert!(
            !uc.get_unit_control_groups(id).is_empty(),
            "unit must be in control group 1"
        );

        let renderer = SelectionRenderer::new();
        let cmds = renderer.render_control_group_numbers_from_presentation(&uc, &snap);
        assert!(
            !cmds.is_empty(),
            "control group badges must render from presentation"
        );
        let any_near_snapshot = cmds.iter().any(|c| {
            c.vertices
                .iter()
                .any(|v| (v.position[0] - 10.0).abs() < 5.0)
        });
        assert!(
            any_near_snapshot,
            "badge position must come from snapshot, not live 999"
        );
    }

    #[test]
    fn golden_skirmish_source_has_no_engine_object_id_force_clear() {
        let src = include_str!("golden_skirmish.rs");
        // Match assignment residual only (comments may still mention the field).
        let force_clear = src.lines().any(|line| {
            let t = line.trim();
            !t.starts_with("//")
                && (t.contains("engine_object_id = None")
                    || t.contains("engine_object_id=None")
                    || t.contains(".engine_object_id = None"))
        });
        assert!(
            !force_clear,
            "golden success path must not mid-scenario force-clear engine_object_id"
        );
    }
}
