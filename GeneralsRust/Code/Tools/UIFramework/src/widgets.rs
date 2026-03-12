//! Custom widgets for game development tools

use eframe::egui;

/// Custom color picker with alpha support
pub struct ColorPickerWidget;

impl ColorPickerWidget {
    pub fn show(ui: &mut egui::Ui, color: &mut [f32; 4], label: &str) -> egui::Response {
        ui.horizontal(|ui| {
            ui.label(label);
            ui.color_edit_button_rgba_unmultiplied(color)
        })
        .inner
    }
}

/// File path selector widget
pub struct FilePathWidget;

impl FilePathWidget {
    pub fn show(ui: &mut egui::Ui, path: &mut String, label: &str, filter: &str) -> egui::Response {
        let response = ui.horizontal(|ui| {
            ui.label(label);
            let text_response = ui.text_edit_singleline(path);
            if ui.button("Browse...").clicked() {
                if let Some(file_path) = rfd::FileDialog::new()
                    .add_filter("Supported Files", &[filter])
                    .pick_file()
                {
                    *path = file_path.display().to_string();
                }
            }
            text_response
        });
        response.inner
    }
}

/// Directory selector widget
pub struct DirectoryWidget;

impl DirectoryWidget {
    pub fn show(ui: &mut egui::Ui, path: &mut String, label: &str) -> egui::Response {
        let response = ui.horizontal(|ui| {
            ui.label(label);
            let text_response = ui.text_edit_singleline(path);
            if ui.button("Browse...").clicked() {
                if let Some(dir_path) = rfd::FileDialog::new().pick_folder() {
                    *path = dir_path.display().to_string();
                }
            }
            text_response
        });
        response.inner
    }
}

/// Spinner widget for loading states
pub struct SpinnerWidget;

impl SpinnerWidget {
    pub fn show(ui: &mut egui::Ui, size: f32) {
        let time = ui.input(|i| i.time) as f32;
        let angle = time * 2.0;

        let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(size), egui::Sense::hover());

        let painter = ui.painter();
        let center = rect.center();
        let radius = size * 0.4;

        for i in 0..8 {
            let a = angle + (i as f32 * std::f32::consts::PI / 4.0);
            let alpha = 1.0 - (i as f32 / 8.0);

            let start = center + egui::Vec2::angled(a) * radius * 0.3;
            let end = center + egui::Vec2::angled(a) * radius;

            painter.line_segment(
                [start, end],
                egui::Stroke::new(2.0, egui::Color32::WHITE.gamma_multiply(alpha)),
            );
        }
    }
}

/// Progress bar with custom styling
pub struct ProgressBarWidget;

impl ProgressBarWidget {
    pub fn show(ui: &mut egui::Ui, progress: f32, label: Option<&str>) -> egui::Response {
        let desired_size = egui::Vec2::new(ui.available_width(), 20.0);
        let (rect, response) = ui.allocate_at_least(desired_size, egui::Sense::hover());

        let progress = progress.clamp(0.0, 1.0);

        // Background
        ui.painter()
            .rect_filled(rect, egui::Rounding::same(4), ui.visuals().extreme_bg_color);

        // Progress fill
        let fill_width = rect.width() * progress;
        let fill_rect =
            egui::Rect::from_min_size(rect.min, egui::Vec2::new(fill_width, rect.height()));

        ui.painter().rect_filled(
            fill_rect,
            egui::Rounding::same(4),
            ui.visuals().selection.bg_fill,
        );

        // Text overlay
        if let Some(text) = label {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::default(),
                ui.visuals().text_color(),
            );
        } else {
            let percentage = format!("{:.0}%", progress * 100.0);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                percentage,
                egui::FontId::default(),
                ui.visuals().text_color(),
            );
        }

        response
    }
}

/// Collapsible section widget
pub struct CollapsibleSection;

impl CollapsibleSection {
    pub fn show<R>(
        ui: &mut egui::Ui,
        label: &str,
        open: &mut bool,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<Option<R>> {
        let header_response = ui.horizontal(|ui| {
            let icon = if *open { "▼" } else { "▶" };
            if ui.small_button(icon).clicked() {
                *open = !*open;
            }
            ui.label(label);
        });

        if header_response.response.clicked() {
            *open = !*open;
        }

        if *open {
            ui.indent("collapsible_content", |ui| {
                ui.separator();
                Some(add_contents(ui))
            })
        } else {
            egui::InnerResponse::new(None, header_response.response)
        }
    }
}

/// Custom toolbar widget
pub struct ToolbarWidget {
    tools: Vec<ToolButton>,
}

impl ToolbarWidget {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn add_tool(&mut self, button: ToolButton) {
        self.tools.push(button);
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<String> {
        let mut clicked_tool = None;
        let mut clicked_index = None;

        ui.horizontal(|ui| {
            for (index, tool) in self.tools.iter().enumerate() {
                let response = ui.add(egui::Button::new(&tool.label).selected(tool.active));

                if response.clicked() {
                    clicked_index = Some((index, tool.exclusive, tool.id.clone()));
                }
            }
        });

        // Handle the click after the iteration to avoid double borrow
        if let Some((index, is_exclusive, id)) = clicked_index {
            if is_exclusive {
                // Deactivate all other tools if this is exclusive
                for other_tool in &mut self.tools {
                    other_tool.active = false;
                }
            }

            self.tools[index].active = !self.tools[index].active;
            clicked_tool = Some(id);
        }

        clicked_tool
    }
}

/// Tool button configuration
#[derive(Debug, Clone)]
pub struct ToolButton {
    pub id: String,
    pub label: String,
    pub active: bool,
    pub exclusive: bool, // Only one can be active at a time
}

impl ToolButton {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            active: false,
            exclusive: false,
        }
    }

    pub fn exclusive(mut self) -> Self {
        self.exclusive = true;
        self
    }
}

/// Image viewer widget with zoom and pan
pub struct ImageViewerWidget {
    zoom: f32,
    offset: egui::Vec2,
    dragging: bool,
}

impl Default for ImageViewerWidget {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            offset: egui::Vec2::ZERO,
            dragging: false,
        }
    }
}

impl ImageViewerWidget {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        texture_id: egui::TextureId,
        image_size: egui::Vec2,
    ) -> egui::Response {
        let available_size = ui.available_size();
        let (rect, response) =
            ui.allocate_exact_size(available_size, egui::Sense::click_and_drag());

        // Handle zoom with mouse wheel
        let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
        if response.hovered() && scroll_delta != 0.0 {
            let zoom_delta = 1.0 + scroll_delta * 0.001;
            self.zoom = (self.zoom * zoom_delta).clamp(0.1, 10.0);
        }

        // Handle pan with mouse drag
        if response.dragged() {
            self.offset += response.drag_delta();
        }

        // Reset on double-click
        if response.double_clicked() {
            self.zoom = 1.0;
            self.offset = egui::Vec2::ZERO;
        }

        // Calculate image position and size
        let scaled_size = image_size * self.zoom;
        let center_offset = (available_size - scaled_size) * 0.5;
        let image_rect =
            egui::Rect::from_min_size(rect.min + center_offset + self.offset, scaled_size);

        // Draw the image
        ui.painter().image(
            texture_id,
            image_rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
            egui::Color32::WHITE,
        );

        // Draw zoom info
        let zoom_text = format!("Zoom: {:.0}%", self.zoom * 100.0);
        ui.painter().text(
            rect.left_top() + egui::Vec2::new(10.0, 10.0),
            egui::Align2::LEFT_TOP,
            zoom_text,
            egui::FontId::monospace(12.0),
            ui.visuals().text_color(),
        );

        response
    }
}
