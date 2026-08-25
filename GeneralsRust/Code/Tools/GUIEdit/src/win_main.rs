//! GUIEdit - GUI Layout Editor for C&C Generals Zero Hour
//!
//! Corresponds to C++ file: Tools/GUIEdit/Source/WinMain.cpp
//!
//! Layout-editor chrome shell: File/Edit/View/Layout menus, gadget toolbox,
//! canvas, properties, status bar. Save/load uses shipped `gui_edit::save`.

use anyhow::Result;
use eframe::egui;
use gui_edit::chrome::{
    ChromeEditor, EDIT_MENU_LABELS, FILE_MENU_LABELS, GadgetType, LAYOUT_MENU_LABELS,
};
use log::{error, info};

/// Main GUIEdit application (C++ `GUIEdit` + `WinMain` chrome).
struct GUIEditApp {
    editor: ChromeEditor,
}

impl Default for GUIEditApp {
    fn default() -> Self {
        Self {
            editor: ChromeEditor::new(),
        }
    }
}

impl eframe::App for GUIEditApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    for label in FILE_MENU_LABELS {
                        if ui.button(*label).clicked() {
                            match *label {
                                "New" => self.editor.new_layout(),
                                "Open" => self.open_layout(),
                                "Save" => self.save_layout(),
                                "Save As" => self.save_layout_as(),
                                "Exit" => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                                _ => {}
                            }
                        }
                    }
                });

                ui.menu_button("Edit", |ui| {
                    for label in EDIT_MENU_LABELS {
                        if ui.button(*label).clicked() {
                            match *label {
                                "Undo" => self.editor.undo(),
                                "Redo" => self.editor.redo(),
                                "Delete" => self.editor.delete_selected(),
                                _ => {}
                            }
                        }
                    }
                });

                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.editor.show_hierarchy, "Hierarchy");
                    ui.checkbox(&mut self.editor.show_properties, "Properties");
                    ui.checkbox(&mut self.editor.show_toolbox, "Toolbox");
                    ui.checkbox(&mut self.editor.show_grid, "Grid");
                    ui.separator();
                    ui.checkbox(&mut self.editor.snap_to_grid, "Snap to Grid");
                    ui.horizontal(|ui| {
                        ui.label("Grid Size:");
                        ui.add(egui::DragValue::new(&mut self.editor.grid_size).speed(1.0));
                    });
                    ui.separator();
                    if ui.button("Zoom In").clicked() {
                        self.editor.zoom *= 1.2;
                    }
                    if ui.button("Zoom Out").clicked() {
                        self.editor.zoom /= 1.2;
                    }
                    if ui.button("Reset Zoom").clicked() {
                        self.editor.zoom = 1.0;
                    }
                });

                ui.menu_button("Layout", |ui| {
                    for label in LAYOUT_MENU_LABELS {
                        if ui.button(*label).clicked() {
                            self.editor.apply_layout_command(label);
                        }
                    }
                });
            });
        });

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(self.editor.status_line());
                ui.separator();
                if let Some(path) = &self.editor.current_path {
                    ui.label(path.display().to_string());
                } else {
                    ui.label("Untitled.wnd");
                }
                ui.separator();
                ui.label(format!("Zoom: {:.0}%", self.editor.zoom * 100.0));
            });
        });

        if self.editor.show_toolbox {
            egui::SidePanel::left("toolbox")
                .resizable(true)
                .default_width(180.0)
                .show(ctx, |ui| {
                    ui.heading("Toolbox");
                    ui.separator();
                    ui.label("Gadgets:");
                    for gadget in GadgetType::toolbox_types() {
                        if ui.button(gadget.as_str()).clicked() {
                            self.editor.add_gadget(*gadget);
                        }
                    }
                });
        }

        if self.editor.show_hierarchy {
            egui::SidePanel::left("hierarchy")
                .resizable(true)
                .default_width(180.0)
                .show(ctx, |ui| {
                    ui.heading("Hierarchy");
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let mut clicked = None;
                        for widget in &self.editor.widgets {
                            let is_selected = self.editor.selected == Some(widget.id);
                            let label = format!("{} ({})", widget.name, widget.gadget.as_str());
                            if ui.selectable_label(is_selected, label).clicked() {
                                clicked = Some(widget.id);
                            }
                        }
                        if let Some(id) = clicked {
                            self.editor.select(Some(id));
                        }
                    });
                });
        }

        if self.editor.show_properties {
            egui::SidePanel::right("properties")
                .resizable(true)
                .default_width(250.0)
                .show(ctx, |ui| {
                    ui.heading("Properties");
                    ui.separator();
                    if let Some(widget) = self.editor.selected_widget_mut() {
                        ui.horizontal(|ui| {
                            ui.label("Type:");
                            egui::ComboBox::from_id_salt("prop_gadget_type")
                                .selected_text(widget.gadget.as_str())
                                .show_ui(ui, |ui| {
                                    for gadget in GadgetType::toolbox_types() {
                                        ui.selectable_value(
                                            &mut widget.gadget,
                                            *gadget,
                                            gadget.as_str(),
                                        );
                                    }
                                });
                        });
                        ui.horizontal(|ui| {
                            ui.label("WINDOWTYPE:");
                            ui.monospace(widget.window_type());
                        });
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
                            ui.label("W:");
                            ui.add(egui::DragValue::new(&mut widget.width).speed(1.0));
                        });
                        ui.horizontal(|ui| {
                            ui.label("H:");
                            ui.add(egui::DragValue::new(&mut widget.height).speed(1.0));
                        });
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
            let zoom = self.editor.zoom.max(0.1);

            painter.rect_filled(canvas_rect, 0.0, egui::Color32::from_gray(40));

            if self.editor.show_grid {
                let grid_spacing = (self.editor.grid_size as f32 * zoom).max(2.0);
                let mut x = 0.0;
                while x < canvas_rect.width() {
                    painter.line_segment(
                        [
                            egui::pos2(canvas_rect.left() + x, canvas_rect.top()),
                            egui::pos2(canvas_rect.left() + x, canvas_rect.bottom()),
                        ],
                        egui::Stroke::new(1.0, egui::Color32::from_gray(50)),
                    );
                    x += grid_spacing;
                }
                let mut y = 0.0;
                while y < canvas_rect.height() {
                    painter.line_segment(
                        [
                            egui::pos2(canvas_rect.left(), canvas_rect.top() + y),
                            egui::pos2(canvas_rect.right(), canvas_rect.top() + y),
                        ],
                        egui::Stroke::new(1.0, egui::Color32::from_gray(50)),
                    );
                    y += grid_spacing;
                }
            }

            for widget in &self.editor.widgets {
                let is_selected = self.editor.selected == Some(widget.id);
                let stroke_color = if is_selected {
                    egui::Color32::YELLOW
                } else {
                    egui::Color32::LIGHT_BLUE
                };
                let fill = if is_selected {
                    egui::Color32::from_rgba_unmultiplied(80, 80, 20, 80)
                } else {
                    egui::Color32::from_rgba_unmultiplied(40, 60, 90, 80)
                };

                let widget_rect = egui::Rect::from_min_size(
                    egui::pos2(
                        canvas_rect.left() + widget.x as f32 * zoom,
                        canvas_rect.top() + widget.y as f32 * zoom,
                    ),
                    egui::vec2(widget.width as f32 * zoom, widget.height as f32 * zoom),
                );

                painter.rect_filled(widget_rect, 2.0, fill);
                painter.rect_stroke(
                    widget_rect,
                    2.0,
                    egui::Stroke::new(2.0, stroke_color),
                    egui::StrokeKind::Middle,
                );
                painter.text(
                    widget_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("{}\n{}", widget.name, widget.gadget.as_str()),
                    egui::FontId::default(),
                    egui::Color32::WHITE,
                );
            }

            if response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let local_x = (pos.x - canvas_rect.left()) / zoom;
                    let local_y = (pos.y - canvas_rect.top()) / zoom;
                    self.editor.select_at_point(local_x, local_y);
                }
            }
        });

        let mut new_layout = false;
        let mut delete_selected = false;
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Delete) {
                delete_selected = true;
            }
            if i.modifiers.command && i.key_pressed(egui::Key::N) {
                new_layout = true;
            }
        });
        if delete_selected {
            self.editor.delete_selected();
        }
        if new_layout {
            self.editor.new_layout();
        }
    }
}

impl GUIEditApp {
    fn open_layout(&mut self) {
        info!("Opening layout");
        let picked = rfd::FileDialog::new()
            .add_filter("Window Layout", &["wnd"])
            .add_filter("All Files", &["*"])
            .pick_file();
        let Some(path) = picked else {
            return;
        };
        match self.editor.read_from_path(&path) {
            Ok(()) => info!("Loaded {}", path.display()),
            Err(e) => error!("Failed to open {}: {e}", path.display()),
        }
    }

    fn save_layout(&mut self) {
        info!("Saving layout");
        if let Some(path) = self.editor.current_path.clone() {
            if let Err(e) = self.editor.write_to_path(&path) {
                error!("Failed to save {}: {e}", path.display());
            }
        } else {
            self.save_layout_as();
        }
    }

    fn save_layout_as(&mut self) {
        info!("Save layout as");
        let picked = rfd::FileDialog::new()
            .add_filter("Window Layout", &["wnd"])
            .set_file_name("Untitled.wnd")
            .save_file();
        let Some(path) = picked else {
            return;
        };
        if let Err(e) = self.editor.write_to_path(&path) {
            error!("Failed to save {}: {e}", path.display());
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
