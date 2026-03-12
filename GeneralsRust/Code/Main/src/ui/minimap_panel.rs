//! Minimap Panel UI
//!
//! This module handles the minimap panel UI rendering, including:
//! - FOW texture overlay
//! - Unit dots
//! - Camera viewport indicator
//! - Click handling for camera panning

use crate::localization;
use egui::{Color32, Context, Image, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, Window};
use glam::Vec3;
use log::{debug, trace};

/// Minimap UI state
#[derive(Debug, Clone)]
pub struct MinimapUIState {
    /// Minimap texture ID from renderer
    pub fow_texture_id: Option<egui::TextureId>,

    /// Minimap dimensions in screen pixels
    pub width: f32,
    pub height: f32,

    /// Position on screen (top-right corner)
    pub screen_pos: Pos2,

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
    pub color: Color32,

    /// Size of the dot in pixels
    pub size: f32,

    /// Is this the selected unit
    pub is_selected: bool,
}

/// Beacon marker displayed on the minimap.
#[derive(Debug, Clone)]
pub struct BeaconDot {
    pub world_pos: Vec3,
    pub color: Color32,
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
            screen_pos: Pos2::new(10.0, 10.0),
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
    /// Convert world position to minimap screen coordinates
    pub fn world_to_minimap(&self, world_pos: Vec3) -> Pos2 {
        let span_x = (self.world_max.x - self.world_min.x).abs().max(1.0e-4);
        let span_z = (self.world_max.z - self.world_min.z).abs().max(1.0e-4);
        let x_ratio = ((world_pos.x - self.world_min.x) / span_x).clamp(0.0, 1.0);
        let z_ratio = ((world_pos.z - self.world_min.z) / span_z).clamp(0.0, 1.0);

        Pos2::new(
            self.screen_pos.x + x_ratio * self.width,
            self.screen_pos.y + z_ratio * self.height,
        )
    }

    /// Convert minimap screen coordinates to world position
    pub fn minimap_to_world(&self, minimap_pos: Pos2) -> Vec3 {
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
    pub fn contains(&self, pos: Pos2) -> bool {
        pos.x >= self.screen_pos.x
            && pos.x <= self.screen_pos.x + self.width
            && pos.y >= self.screen_pos.y
            && pos.y <= self.screen_pos.y + self.height
    }
}

pub fn color_for_player(index: u8) -> Color32 {
    const COLORS: [Color32; 8] = [
        Color32::RED,
        Color32::from_rgb(50, 160, 255), // Blue
        Color32::from_rgb(80, 200, 120), // Green
        Color32::YELLOW,
        Color32::from_rgb(255, 120, 0),   // Orange
        Color32::from_rgb(200, 80, 255),  // Purple
        Color32::from_rgb(255, 255, 255), // White
        Color32::from_rgb(120, 120, 120), // Gray fallback
    ];
    COLORS[(index as usize) % COLORS.len()]
}

/// Render the minimap panel
pub fn render_minimap_panel(
    ctx: &Context,
    ui_state: &mut MinimapUIState,
) -> Option<MinimapClickEvent> {
    let mut click_event = None;

    // Create minimap window
    Window::new(localization::localize("minimap.window.title", "Minimap"))
        .fixed_pos(ui_state.screen_pos)
        .fixed_size(Vec2::new(ui_state.width + 20.0, ui_state.height + 40.0))
        .collapsible(false)
        .resizable(false)
        .title_bar(true)
        .show(ctx, |ui| {
            // Create minimap area
            let minimap_rect =
                Rect::from_min_size(ui.cursor().min, Vec2::new(ui_state.width, ui_state.height));

            // Allocate the space and make it interactive
            let response = ui.allocate_rect(minimap_rect, Sense::click_and_drag());

            // Draw base terrain or background
            if ui_state.show_terrain && ui_state.fow_texture_id.is_none() {
                // Draw a simple terrain background
                ui.painter().rect_filled(
                    minimap_rect,
                    0.0,
                    Color32::from_rgb(139, 119, 70), // Desert brown
                );
            }

            // Draw FOW texture if available
            if let Some(texture_id) = ui_state.fow_texture_id {
                if ui_state.show_fow {
                    // Draw the FOW texture
                    let image = Image::new((texture_id, minimap_rect.size()));
                    image.paint_at(ui, minimap_rect);

                    trace!("Drew minimap FOW texture at {:?}", minimap_rect);
                }
            } else {
                draw_fallback_minimap_background(ui.painter(), minimap_rect);
            }

            // Draw camera viewport rectangle
            draw_camera_viewport(ui.painter(), ui_state);

            // Draw beacon markers
            draw_beacon_dots(ui.painter(), ui_state);

            // Draw radar pings
            draw_radar_dots(ui.painter(), ui_state);

            // Draw unit dots
            draw_unit_dots(ui.painter(), ui_state);

            // Handle click events
            if response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let world_pos = ui_state.minimap_to_world(pos);
                    click_event = Some(MinimapClickEvent {
                        world_position: world_pos,
                        screen_position: pos,
                        is_right_click: false,
                    });

                    debug!("Minimap clicked at screen {:?}, world {:?}", pos, world_pos);
                }
            }

            // Handle right-click for move commands
            if response.secondary_clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let world_pos = ui_state.minimap_to_world(pos);
                    click_event = Some(MinimapClickEvent {
                        world_position: world_pos,
                        screen_position: pos,
                        is_right_click: true,
                    });
                }
            }

            // Show coordinates on hover
            if let Some(pos) = response.hover_pos() {
                let world_pos = ui_state.minimap_to_world(pos);
                let world_x = format!("{:.0}", world_pos.x);
                let world_z = format!("{:.0}", world_pos.z);
                let hover_text = localization::localize_with_args(
                    "minimap.tooltip.world_coords",
                    "World: ({x}, {z})",
                    &[("x", world_x.as_str()), ("z", world_z.as_str())],
                );
                response.on_hover_text(hover_text);
            }
        });

    click_event
}

fn draw_fallback_minimap_background(painter: &Painter, minimap_rect: Rect) {
    painter.rect_filled(minimap_rect, 0.0, Color32::from_rgb(84, 79, 61));
    painter.rect_stroke(
        minimap_rect,
        0.0,
        Stroke::new(1.0, Color32::from_gray(170)),
        StrokeKind::Outside,
    );

    // Keep a usable minimap when FOW texture is not yet available.
    let grid_color = Color32::from_rgba_unmultiplied(255, 255, 255, 28);
    let cols = 8;
    let rows = 8;
    for i in 1..cols {
        let t = i as f32 / cols as f32;
        let x = minimap_rect.left() + t * minimap_rect.width();
        painter.line_segment(
            [
                Pos2::new(x, minimap_rect.top()),
                Pos2::new(x, minimap_rect.bottom()),
            ],
            Stroke::new(1.0, grid_color),
        );
    }
    for i in 1..rows {
        let t = i as f32 / rows as f32;
        let y = minimap_rect.top() + t * minimap_rect.height();
        painter.line_segment(
            [
                Pos2::new(minimap_rect.left(), y),
                Pos2::new(minimap_rect.right(), y),
            ],
            Stroke::new(1.0, grid_color),
        );
    }
}

/// Draw camera viewport indicator on minimap
fn draw_camera_viewport(painter: &Painter, ui_state: &MinimapUIState) {
    let camera_min = ui_state.world_to_minimap(ui_state.camera_bounds.0);
    let camera_max = ui_state.world_to_minimap(ui_state.camera_bounds.1);

    let viewport_rect = Rect::from_two_pos(camera_min, camera_max);

    // Draw viewport rectangle
    painter.rect_stroke(
        viewport_rect,
        0.0,
        Stroke::new(2.0, Color32::WHITE),
        StrokeKind::Outside,
    );

    // Draw viewport corners for visibility
    let corner_size = 3.0;
    for corner in &[
        viewport_rect.left_top(),
        viewport_rect.right_top(),
        viewport_rect.left_bottom(),
        viewport_rect.right_bottom(),
    ] {
        painter.circle_filled(*corner, corner_size, Color32::WHITE);
    }
}

/// Draw unit dots on minimap
fn draw_unit_dots(painter: &Painter, ui_state: &MinimapUIState) {
    for unit in &ui_state.unit_positions {
        let pos = ui_state.world_to_minimap(unit.world_pos);

        // Draw dot
        painter.circle_filled(pos, unit.size, unit.color);

        // Draw selection indicator
        if unit.is_selected {
            painter.circle_stroke(pos, unit.size + 2.0, Stroke::new(1.0, Color32::YELLOW));
        }
    }
}

/// Draw beacon markers on minimap
fn draw_beacon_dots(painter: &Painter, ui_state: &MinimapUIState) {
    for beacon in &ui_state.beacon_positions {
        let pos = ui_state.world_to_minimap(beacon.world_pos);
        painter.circle_filled(pos, 4.0, beacon.color);
        painter.circle_stroke(pos, 6.0, Stroke::new(1.5, Color32::WHITE));
    }

    if let Some(highlight) = ui_state.beacon_highlight {
        let pos = ui_state.world_to_minimap(highlight.world_pos);
        let alpha = ((highlight.timer / 1.0) * 200.0).clamp(0.0, 200.0) as u8;
        painter.circle_stroke(
            pos,
            10.0,
            Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 255, 160, alpha)),
        );
    }
}

/// Draw radar pings on minimap (small white circles)
fn draw_radar_dots(painter: &Painter, ui_state: &MinimapUIState) {
    for ping in &ui_state.radar_pings {
        let pos = ui_state.world_to_minimap(ping.world_pos);
        // Pulse size as a function of intensity and age.
        let radius = 3.0 + 3.0 * ping.intensity;
        let alpha = (ping.intensity * 255.0).clamp(0.0, 255.0) as u8;
        let (fill, stroke) = match ping.kind {
            RadarPingKind::Attack => (
                Color32::from_rgba_unmultiplied(255, 80, 80, alpha),
                Color32::from_rgba_unmultiplied(255, 160, 160, alpha),
            ),
            RadarPingKind::Ally => (
                Color32::from_rgba_unmultiplied(80, 180, 255, alpha),
                Color32::from_rgba_unmultiplied(160, 220, 255, alpha),
            ),
            RadarPingKind::Generic => (
                Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
                Color32::from_rgba_unmultiplied(200, 200, 200, alpha),
            ),
        };
        painter.circle_filled(pos, radius, fill);
        painter.circle_stroke(pos, radius + 2.0, Stroke::new(1.0, stroke));
        // Add a brief bloom on very fresh pings.
        if ping.age_seconds < 0.75 {
            let bloom_alpha = ((0.75 - ping.age_seconds) / 0.75 * 180.0).clamp(0.0, 180.0) as u8;
            let bloom_radius = radius + 6.0;
            painter.circle_stroke(
                pos,
                bloom_radius,
                Stroke::new(
                    2.0,
                    Color32::from_rgba_unmultiplied(fill.r(), fill.g(), fill.b(), bloom_alpha),
                ),
            );
        }
    }
}

/// Minimap click event
#[derive(Debug, Clone)]
pub struct MinimapClickEvent {
    /// World position clicked
    pub world_position: Vec3,

    /// Screen position of click
    pub screen_position: Pos2,

    /// Was this a right-click (for move commands)
    pub is_right_click: bool,
}

/// Update minimap with game state
pub fn update_minimap_state(
    ui_state: &mut MinimapUIState,
    camera_pos: Vec3,
    camera_size: f32,
    units: &[(Vec3, Color32, bool)], // (position, color, selected)
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
        let last = new_beacons.last().copied().unwrap();
        ui_state.beacon_highlight = Some(BeaconHighlight {
            world_pos: last,
            timer: 1.0,
        });
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
        ui_state.screen_pos = Pos2::new(10.0, 10.0);

        // Test world to minimap
        let world_pos = Vec3::new(500.0, 0.0, 500.0);
        let minimap_pos = ui_state.world_to_minimap(world_pos);
        assert_eq!(minimap_pos.x, 110.0); // 10 + 200*0.5
        assert_eq!(minimap_pos.y, 110.0);

        // Test minimap to world
        let minimap_pos = Pos2::new(110.0, 110.0);
        let world_pos = ui_state.minimap_to_world(minimap_pos);
        assert!((world_pos.x - 500.0).abs() < 0.1);
        assert!((world_pos.z - 500.0).abs() < 0.1);
    }

    #[test]
    fn test_contains() {
        let ui_state = MinimapUIState {
            screen_pos: Pos2::new(10.0, 10.0),
            width: 256.0,
            height: 256.0,
            ..Default::default()
        };

        assert!(ui_state.contains(Pos2::new(100.0, 100.0)));
        assert!(!ui_state.contains(Pos2::new(300.0, 100.0)));
        assert!(!ui_state.contains(Pos2::new(5.0, 100.0)));
    }

    #[test]
    fn test_minimap_conversions_clamp_out_of_bounds() {
        let mut ui_state = MinimapUIState::default();
        ui_state.world_min = Vec3::new(0.0, 0.0, 0.0);
        ui_state.world_max = Vec3::new(1000.0, 0.0, 1000.0);
        ui_state.width = 200.0;
        ui_state.height = 200.0;
        ui_state.screen_pos = Pos2::new(10.0, 10.0);

        let world = ui_state.minimap_to_world(Pos2::new(-100.0, 9999.0));
        assert!((world.x - 0.0).abs() < 0.1);
        assert!((world.z - 1000.0).abs() < 0.1);

        let pos = ui_state.world_to_minimap(Vec3::new(-500.0, 0.0, 5000.0));
        assert!((pos.x - 10.0).abs() < 0.1);
        assert!((pos.y - 210.0).abs() < 0.1);
    }
}
