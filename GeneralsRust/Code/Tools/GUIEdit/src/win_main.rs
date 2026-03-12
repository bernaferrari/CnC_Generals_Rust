//! GUIEdit - GUI Layout Editor for C&C Generals Zero Hour
//!
//! Corresponds to C++ file: Tools/GUIEdit/Source/WinMain.cpp
//!
//! This tool provides a visual editor for creating and editing game UI layouts.

use anyhow::Result;
use eframe::egui;
use env_logger;
use log::info;
use std::path::PathBuf;

/// Main GUIEdit application
struct GUIEditApp {
    current_layout: Option<PathBuf>,
    show_properties: bool,
    show_hierarchy: bool,
    show_toolbox: bool,
    selected_widget: Option<String>,
    widgets: Vec<WidgetInfo>,
    zoom: f32,
    grid_size: i32,
    snap_to_grid: bool,
}

#[derive(Clone, Debug)]
struct WidgetInfo {
    name: String,
    widget_type: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Default for GUIEditApp {
    fn default() -> Self {
        Self {
            current_layout: None,
            show_properties: true,
            show_hierarchy: true,
            show_toolbox: true,
            selected_widget: None,
            widgets: Vec::new(),
            zoom: 1.0,
            grid_size: 10,
            snap_to_grid: true,
        }
    }
}

impl eframe::App for GUIEditApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Layout").clicked() {
                        self.new_layout();
                    }
                    if ui.button("Open Layout...").clicked() {
                        self.open_layout();
                    }
                    if ui.button("Save Layout").clicked() {
                        self.save_layout();
                    }
                    if ui.button("Save Layout As...").clicked() {
                        self.save_layout_as();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo").clicked() {
                        info!("Undo");
                    }
                    if ui.button("Redo").clicked() {
                        info!("Redo");
                    }
                    ui.separator();
                    if ui.button("Delete Selected").clicked() {
                        self.delete_selected();
                    }
                });

                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.show_toolbox, "Toolbox");
                    ui.checkbox(&mut self.show_hierarchy, "Hierarchy");
                    ui.checkbox(&mut self.show_properties, "Properties");
                    ui.separator();
                    if ui.button("Zoom In").clicked() {
                        self.zoom *= 1.2;
                    }
                    if ui.button("Zoom Out").clicked() {
                        self.zoom /= 1.2;
                    }
                    if ui.button("Reset Zoom").clicked() {
                        self.zoom = 1.0;
                    }
                });

                ui.menu_button("Tools", |ui| {
                    ui.checkbox(&mut self.snap_to_grid, "Snap to Grid");
                    ui.horizontal(|ui| {
                        ui.label("Grid Size:");
                        ui.add(egui::DragValue::new(&mut self.grid_size).speed(1.0));
                    });
                });
            });
        });

        if self.show_toolbox {
            egui::SidePanel::left("toolbox")
                .resizable(true)
                .default_width(200.0)
                .show(ctx, |ui| {
                    ui.heading("Toolbox");
                    ui.separator();

                    ui.label("Widgets:");
                    if ui.button("Button").clicked() {
                        self.add_widget("Button");
                    }
                    if ui.button("Label").clicked() {
                        self.add_widget("Label");
                    }
                    if ui.button("TextBox").clicked() {
                        self.add_widget("TextBox");
                    }
                    if ui.button("CheckBox").clicked() {
                        self.add_widget("CheckBox");
                    }
                    if ui.button("RadioButton").clicked() {
                        self.add_widget("RadioButton");
                    }
                    if ui.button("ComboBox").clicked() {
                        self.add_widget("ComboBox");
                    }
                    if ui.button("ListBox").clicked() {
                        self.add_widget("ListBox");
                    }
                    if ui.button("ProgressBar").clicked() {
                        self.add_widget("ProgressBar");
                    }
                    if ui.button("Slider").clicked() {
                        self.add_widget("Slider");
                    }
                    if ui.button("TabControl").clicked() {
                        self.add_widget("TabControl");
                    }
                });
        }

        if self.show_hierarchy {
            egui::SidePanel::right("hierarchy")
                .resizable(true)
                .default_width(200.0)
                .show(ctx, |ui| {
                    ui.heading("Hierarchy");
                    ui.separator();

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for widget in &self.widgets {
                            let is_selected = self.selected_widget.as_ref() == Some(&widget.name);
                            if ui.selectable_label(is_selected, &widget.name).clicked() {
                                self.selected_widget = Some(widget.name.clone());
                            }
                        }
                    });
                });
        }

        if self.show_properties {
            egui::SidePanel::right("properties")
                .resizable(true)
                .default_width(250.0)
                .show(ctx, |ui| {
                    ui.heading("Properties");
                    ui.separator();

                    if let Some(selected) = &self.selected_widget {
                        if let Some(widget) = self.widgets.iter_mut().find(|w| &w.name == selected)
                        {
                            ui.label(format!("Type: {}", widget.widget_type));
                            ui.separator();

                            ui.horizontal(|ui| {
                                ui.label("Name:");
                                ui.text_edit_singleline(&mut widget.name);
                            });

                            ui.horizontal(|ui| {
                                ui.label("X:");
                                ui.add(egui::DragValue::new(&mut widget.x).speed(1.0));
                            });

                            ui.horizontal(|ui| {
                                ui.label("Y:");
                                ui.add(egui::DragValue::new(&mut widget.y).speed(1.0));
                            });

                            ui.horizontal(|ui| {
                                ui.label("Width:");
                                ui.add(egui::DragValue::new(&mut widget.width).speed(1.0));
                            });

                            ui.horizontal(|ui| {
                                ui.label("Height:");
                                ui.add(egui::DragValue::new(&mut widget.height).speed(1.0));
                            });
                        }
                    } else {
                        ui.label("No widget selected");
                    }
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Layout Canvas");
            ui.separator();

            let available_size = ui.available_size();
            let (response, painter) =
                ui.allocate_painter(available_size, egui::Sense::click_and_drag());

            let canvas_rect = response.rect;

            painter.rect_filled(canvas_rect, 0.0, egui::Color32::from_gray(40));

            if self.snap_to_grid {
                let grid_spacing = self.grid_size as f32 * self.zoom;
                for x in (0..canvas_rect.width() as i32).step_by(grid_spacing as usize) {
                    painter.line_segment(
                        [
                            egui::pos2(canvas_rect.left() + x as f32, canvas_rect.top()),
                            egui::pos2(canvas_rect.left() + x as f32, canvas_rect.bottom()),
                        ],
                        egui::Stroke::new(1.0, egui::Color32::from_gray(50)),
                    );
                }
                for y in (0..canvas_rect.height() as i32).step_by(grid_spacing as usize) {
                    painter.line_segment(
                        [
                            egui::pos2(canvas_rect.left(), canvas_rect.top() + y as f32),
                            egui::pos2(canvas_rect.right(), canvas_rect.top() + y as f32),
                        ],
                        egui::Stroke::new(1.0, egui::Color32::from_gray(50)),
                    );
                }
            }

            for widget in &self.widgets {
                let is_selected = self.selected_widget.as_ref() == Some(&widget.name);
                let color = if is_selected {
                    egui::Color32::YELLOW
                } else {
                    egui::Color32::LIGHT_BLUE
                };

                let widget_rect = egui::Rect::from_min_size(
                    egui::pos2(
                        canvas_rect.left() + widget.x * self.zoom,
                        canvas_rect.top() + widget.y * self.zoom,
                    ),
                    egui::vec2(widget.width * self.zoom, widget.height * self.zoom),
                );

                painter.rect_stroke(
                    widget_rect,
                    2.0,
                    egui::Stroke::new(2.0, color),
                    egui::StrokeKind::Middle,
                );
                painter.text(
                    widget_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &widget.name,
                    egui::FontId::default(),
                    egui::Color32::WHITE,
                );
            }

            ui.label(format!("Zoom: {:.0}%", self.zoom * 100.0));
        });
    }
}

impl GUIEditApp {
    fn new_layout(&mut self) {
        info!("Creating new layout");
        self.current_layout = None;
        self.widgets.clear();
        self.selected_widget = None;
    }

    fn open_layout(&mut self) {
        info!("Opening layout");
    }

    fn save_layout(&mut self) {
        info!("Saving layout");
    }

    fn save_layout_as(&mut self) {
        info!("Save layout as");
    }

    fn add_widget(&mut self, widget_type: &str) {
        let count = self
            .widgets
            .iter()
            .filter(|w| w.widget_type == widget_type)
            .count();
        let widget = WidgetInfo {
            name: format!("{}{}", widget_type, count + 1),
            widget_type: widget_type.to_string(),
            x: 50.0,
            y: 50.0,
            width: 100.0,
            height: 30.0,
        };
        self.widgets.push(widget);
    }

    fn delete_selected(&mut self) {
        if let Some(selected) = &self.selected_widget {
            self.widgets.retain(|w| &w.name != selected);
            self.selected_widget = None;
        }
    }
}

/// Main entry point for GUIEdit
fn main() -> Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    info!("Starting GUIEdit...");

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_resizable(true)
            .with_decorations(true)
            .with_title("GUIEdit - GUI Layout Editor"),
        ..Default::default()
    };

    eframe::run_native(
        "GUIEdit",
        native_options,
        Box::new(|_cc| Ok(Box::new(GUIEditApp::default()))),
    )
    .map_err(|e| anyhow::anyhow!("Failed to run GUIEdit: {}", e))
}
