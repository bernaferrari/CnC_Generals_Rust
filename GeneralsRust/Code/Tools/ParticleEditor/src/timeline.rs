//! Timeline Module
//!
//! Handles timeline-based editing for particle systems.

use crate::particles::ParticleSystem;
use anyhow::Result;

/// Timeline for particle system editing
#[derive(Debug, Clone)]
pub struct Timeline {
    pub current_time: f32,
    pub duration: f32,
    pub is_playing: bool,
    pub is_looping: bool,
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            current_time: 0.0,
            duration: 10.0,
            is_playing: false,
            is_looping: true,
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        log::info!("Initializing timeline");
        Ok(())
    }

    pub fn update(&mut self, delta_time: f32) -> Result<()> {
        if self.is_playing {
            self.current_time += delta_time;
            if self.current_time >= self.duration {
                if self.is_looping {
                    self.current_time = 0.0;
                } else {
                    self.current_time = self.duration;
                    self.is_playing = false;
                }
            }
        }
        Ok(())
    }

    pub fn set_system(&mut self, system: Option<&ParticleSystem>) -> Result<()> {
        if let Some(system) = system {
            // Set timeline duration based on system properties
            if system.info.system_lifetime > 0 {
                self.duration = system.info.system_lifetime as f32 / 30.0; // Convert frames to seconds at 30fps
            } else {
                self.duration = 10.0; // Default duration
            }
            log::debug!("Timeline duration set to {} seconds", self.duration);
        }
        Ok(())
    }

    pub fn duration(&self) -> f32 {
        self.duration
    }

    pub fn is_looping(&self) -> bool {
        self.is_looping
    }

    pub fn set_current_time(&mut self, time: f32) {
        self.current_time = time.max(0.0).min(self.duration);
    }

    pub fn show(
        &mut self,
        ui: &mut eframe::egui::Ui,
        current_time: &mut f32,
        is_playing: &mut bool,
    ) {
        ui.group(|ui| {
            ui.heading("Timeline");

            // Time slider
            ui.horizontal(|ui| {
                ui.label("Time:");
                if ui
                    .add(eframe::egui::Slider::new(current_time, 0.0..=self.duration).text("s"))
                    .changed()
                {
                    self.current_time = *current_time;
                }
            });

            // Timeline visualization
            let timeline_height = 60.0;
            let (response, painter) = ui.allocate_painter(
                eframe::egui::vec2(ui.available_width(), timeline_height),
                eframe::egui::Sense::click_and_drag(),
            );

            let rect = response.rect;

            // Draw timeline background
            painter.rect_filled(rect, 0.0, ui.style().visuals.extreme_bg_color);

            // Draw time markers
            let num_markers = 10;
            for i in 0..=num_markers {
                let t = i as f32 / num_markers as f32;
                let x = rect.left() + rect.width() * t;
                let y = rect.bottom();

                painter.line_segment(
                    [eframe::egui::pos2(x, y - 5.0), eframe::egui::pos2(x, y)],
                    eframe::egui::Stroke::new(1.0, ui.style().visuals.text_color()),
                );

                let time_text = format!("{:.1}s", self.duration * t);
                painter.text(
                    eframe::egui::pos2(x, y + 10.0),
                    eframe::egui::Align2::CENTER_TOP,
                    time_text,
                    eframe::egui::FontId::proportional(10.0),
                    ui.style().visuals.text_color(),
                );
            }

            // Draw current time indicator
            let progress = self.current_time / self.duration;
            let indicator_x = rect.left() + rect.width() * progress;

            painter.line_segment(
                [
                    eframe::egui::pos2(indicator_x, rect.top()),
                    eframe::egui::pos2(indicator_x, rect.bottom()),
                ],
                eframe::egui::Stroke::new(2.0, eframe::egui::Color32::RED),
            );

            // Handle click/drag on timeline
            if response.clicked() || response.dragged() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let t = (pos.x - rect.left()) / rect.width();
                    let new_time = (t * self.duration).max(0.0).min(self.duration);
                    self.current_time = new_time;
                    *current_time = new_time;
                }
            }

            // Playback controls
            ui.horizontal(|ui| {
                if ui
                    .button(if *is_playing { "⏸ Pause" } else { "▶ Play" })
                    .clicked()
                {
                    *is_playing = !*is_playing;
                    self.is_playing = *is_playing;
                }

                if ui.button("⏹ Stop").clicked() {
                    *is_playing = false;
                    self.is_playing = false;
                    self.current_time = 0.0;
                    *current_time = 0.0;
                }

                ui.separator();

                ui.checkbox(&mut self.is_looping, "Loop");
            });
        });
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}
