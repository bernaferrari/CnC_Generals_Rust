//! Preview Module
//!
//! Handles real-time particle system preview and rendering.

use crate::particles::ParticleSystem;
use anyhow::Result;

/// Particle preview system
#[derive(Debug, Clone)]
pub struct ParticlePreview {
    pub is_enabled: bool,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub camera_distance: f32,
    pub camera_rotation: f32,
    pub show_grid: bool,
    pub show_axes: bool,
}

impl ParticlePreview {
    pub fn new() -> Self {
        Self {
            is_enabled: true,
            viewport_width: 800,
            viewport_height: 600,
            camera_distance: 10.0,
            camera_rotation: 0.0,
            show_grid: true,
            show_axes: true,
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        log::info!("Initializing particle preview");
        Ok(())
    }

    pub fn set_system(&mut self, system: Option<&ParticleSystem>) -> Result<()> {
        if system.is_some() {
            log::debug!("Preview system set");
        }
        Ok(())
    }

    pub fn update(&mut self, _dt: f32) -> Result<()> {
        if self.is_enabled {
            // Update preview state
        }
        Ok(())
    }

    pub fn render(&mut self) -> Result<()> {
        if self.is_enabled {
            // Render particle preview
            log::debug!("Rendering particle preview");
        }
        Ok(())
    }

    pub fn show(
        &mut self,
        ui: &mut eframe::egui::Ui,
        system: Option<&ParticleSystem>,
        current_time: f32,
    ) {
        ui.group(|ui| {
            ui.heading("Particle Preview");

            if let Some(system) = system {
                // Preview controls
                ui.horizontal(|ui| {
                    ui.label("Camera Distance:");
                    ui.add(eframe::egui::Slider::new(
                        &mut self.camera_distance,
                        1.0..=50.0,
                    ));
                });

                ui.horizontal(|ui| {
                    ui.label("Camera Rotation:");
                    ui.add(
                        eframe::egui::Slider::new(&mut self.camera_rotation, 0.0..=360.0)
                            .suffix("°"),
                    );
                });

                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.show_grid, "Show Grid");
                    ui.checkbox(&mut self.show_axes, "Show Axes");
                });

                ui.separator();

                // System info
                ui.label(format!("System: {}", system.info.name));
                ui.label(format!("Particles: {}", system.particles.len()));
                ui.label(format!("Time: {:.2}s", current_time));
                ui.label(format!("Active: {}", system.is_active));

                ui.separator();

                // Preview viewport
                let preview_size =
                    eframe::egui::vec2(ui.available_width(), ui.available_height().max(300.0));

                let (response, painter) =
                    ui.allocate_painter(preview_size, eframe::egui::Sense::click_and_drag());

                let rect = response.rect;

                // Draw viewport background
                painter.rect_filled(rect, 0.0, eframe::egui::Color32::from_rgb(20, 20, 25));

                // Draw grid if enabled
                if self.show_grid {
                    self.draw_grid(&painter, rect, ui);
                }

                // Draw axes if enabled
                if self.show_axes {
                    self.draw_axes(&painter, rect, ui);
                }

                // Draw particles (simplified 2D representation)
                self.draw_particles(&painter, rect, system, ui);

                // Handle camera controls
                if response.dragged() {
                    let delta = response.drag_delta();
                    self.camera_rotation += delta.x * 0.5;
                    self.camera_distance =
                        (self.camera_distance - delta.y * 0.1).max(1.0).min(50.0);
                }

                if response.hovered() {
                    ui.ctx().input(|i| {
                        let scroll_delta = i.smooth_scroll_delta;
                        self.camera_distance -= scroll_delta.y * 0.1;
                        self.camera_distance = self.camera_distance.max(1.0).min(50.0);
                    });
                }
            } else {
                ui.label("No particle system loaded");
                ui.allocate_space(eframe::egui::vec2(ui.available_width(), 300.0));
            }
        });
    }

    fn draw_grid(
        &self,
        painter: &eframe::egui::Painter,
        rect: eframe::egui::Rect,
        ui: &eframe::egui::Ui,
    ) {
        let center = rect.center();
        let grid_size = 10;
        let cell_size = 20.0;

        // Draw grid lines
        for i in -grid_size..=grid_size {
            let offset = i as f32 * cell_size;

            // Vertical lines
            painter.line_segment(
                [
                    eframe::egui::pos2(center.x + offset, rect.top()),
                    eframe::egui::pos2(center.x + offset, rect.bottom()),
                ],
                eframe::egui::Stroke::new(0.5, eframe::egui::Color32::from_gray(40)),
            );

            // Horizontal lines
            painter.line_segment(
                [
                    eframe::egui::pos2(rect.left(), center.y + offset),
                    eframe::egui::pos2(rect.right(), center.y + offset),
                ],
                eframe::egui::Stroke::new(0.5, eframe::egui::Color32::from_gray(40)),
            );
        }

        // Draw center lines (thicker)
        painter.line_segment(
            [
                eframe::egui::pos2(center.x, rect.top()),
                eframe::egui::pos2(center.x, rect.bottom()),
            ],
            eframe::egui::Stroke::new(1.0, eframe::egui::Color32::from_gray(60)),
        );
        painter.line_segment(
            [
                eframe::egui::pos2(rect.left(), center.y),
                eframe::egui::pos2(rect.right(), center.y),
            ],
            eframe::egui::Stroke::new(1.0, eframe::egui::Color32::from_gray(60)),
        );
    }

    fn draw_axes(
        &self,
        painter: &eframe::egui::Painter,
        rect: eframe::egui::Rect,
        ui: &eframe::egui::Ui,
    ) {
        let center = rect.center();
        let axis_length = 50.0;

        // X axis (red)
        painter.line_segment(
            [center, eframe::egui::pos2(center.x + axis_length, center.y)],
            eframe::egui::Stroke::new(2.0, eframe::egui::Color32::RED),
        );
        painter.text(
            eframe::egui::pos2(center.x + axis_length + 5.0, center.y),
            eframe::egui::Align2::LEFT_CENTER,
            "X",
            eframe::egui::FontId::proportional(12.0),
            eframe::egui::Color32::RED,
        );

        // Y axis (green)
        painter.line_segment(
            [center, eframe::egui::pos2(center.x, center.y - axis_length)],
            eframe::egui::Stroke::new(2.0, eframe::egui::Color32::GREEN),
        );
        painter.text(
            eframe::egui::pos2(center.x, center.y - axis_length - 5.0),
            eframe::egui::Align2::CENTER_BOTTOM,
            "Y",
            eframe::egui::FontId::proportional(12.0),
            eframe::egui::Color32::GREEN,
        );

        // Z axis (blue) - represented as diagonal for 2D view
        let z_offset_x = axis_length * 0.707;
        let z_offset_y = axis_length * 0.707;
        painter.line_segment(
            [
                center,
                eframe::egui::pos2(center.x + z_offset_x, center.y + z_offset_y),
            ],
            eframe::egui::Stroke::new(2.0, eframe::egui::Color32::BLUE),
        );
        painter.text(
            eframe::egui::pos2(center.x + z_offset_x + 5.0, center.y + z_offset_y + 5.0),
            eframe::egui::Align2::LEFT_TOP,
            "Z",
            eframe::egui::FontId::proportional(12.0),
            eframe::egui::Color32::BLUE,
        );
    }

    fn draw_particles(
        &self,
        painter: &eframe::egui::Painter,
        rect: eframe::egui::Rect,
        system: &ParticleSystem,
        ui: &eframe::egui::Ui,
    ) {
        let center = rect.center();
        let scale = 20.0 / self.camera_distance;

        // Draw each particle
        for particle in &system.particles {
            // Project 3D position to 2D screen space (simple orthographic projection)
            let screen_x = center.x + particle.info.pos.x * scale;
            let screen_y = center.y - particle.info.pos.y * scale; // Y is inverted in screen space

            // Particle size
            let size = particle.info.size * scale;

            // Particle color (simplified - use first color keyframe)
            let color = particle.current_color;
            let alpha = (particle.current_alpha * 255.0) as u8;

            let color32 = eframe::egui::Color32::from_rgba_unmultiplied(
                (color.red * 255.0) as u8,
                (color.green * 255.0) as u8,
                (color.blue * 255.0) as u8,
                alpha,
            );

            // Draw particle as a circle
            painter.circle_filled(
                eframe::egui::pos2(screen_x, screen_y),
                size.max(1.0),
                color32,
            );
        }
    }
}

impl Default for ParticlePreview {
    fn default() -> Self {
        Self::new()
    }
}
