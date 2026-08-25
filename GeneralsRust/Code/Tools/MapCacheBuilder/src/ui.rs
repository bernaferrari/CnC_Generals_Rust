//! Optional egui chrome for MapCacheBuilder (`--features ui`).

use crate::chrome::MapCacheChrome;
use eframe::egui;
use std::path::PathBuf;

pub fn run_ui() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([880.0, 620.0])
            .with_title("MapCacheBuilder"),
        ..Default::default()
    };
    eframe::run_native(
        "MapCacheBuilder",
        options,
        Box::new(|_cc| Ok(Box::new(MapCacheUiApp::new()))),
    )
    .map_err(|err| anyhow::anyhow!("MapCacheBuilder UI failed: {err}"))
}

struct MapCacheUiApp {
    chrome: MapCacheChrome,
    input_edit: String,
    output_edit: String,
}

impl MapCacheUiApp {
    fn new() -> Self {
        let chrome = MapCacheChrome::new();
        Self {
            input_edit: chrome.input_maps_folder.display().to_string(),
            output_edit: chrome.output_ini_path.display().to_string(),
            chrome,
        }
    }

    fn sync_paths_from_edits(&mut self) {
        self.chrome
            .set_input_maps_folder(PathBuf::from(self.input_edit.trim()));
        self.chrome
            .set_output_ini_path(PathBuf::from(self.output_edit.trim()));
    }
}

impl eframe::App for MapCacheUiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("MapCacheBuilder");
                ui.separator();
                if ui.button("Browse maps folder…").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.input_edit = path.display().to_string();
                        self.chrome.set_input_maps_folder(path);
                    }
                }
                if ui.button("Browse output INI…").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("INI", &["ini"])
                        .set_file_name("mapcache.ini")
                        .save_file()
                    {
                        self.output_edit = path.display().to_string();
                        self.chrome.set_output_ini_path(path);
                    }
                }
                if ui.button("Scan").clicked() {
                    self.sync_paths_from_edits();
                    let _ = self.chrome.scan();
                }
                if ui.button("Build").clicked() {
                    self.sync_paths_from_edits();
                    let _ = self.chrome.build();
                }
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Maps: {}", self.chrome.map_count));
                ui.separator();
                if let Some(ext) = &self.chrome.last_extent {
                    ui.label(format!(
                        "Last extent: {}  {:.1}x{:.1}  z=[{:.2},{:.2}]",
                        ext.map_name, ext.width, ext.height, ext.min_z, ext.max_z
                    ));
                } else {
                    ui.label("Last extent: (none)");
                }
                ui.separator();
                if let Some(err) = &self.chrome.last_error {
                    ui.colored_label(
                        egui::Color32::from_rgb(220, 90, 90),
                        format!("Error: {err}"),
                    );
                } else {
                    ui.label(&self.chrome.status);
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Input maps folder:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.input_edit)
                        .desired_width(f32::INFINITY)
                        .hint_text("Maps"),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Output mapcache.ini:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.output_edit)
                        .desired_width(f32::INFINITY)
                        .hint_text("mapcache.ini"),
                );
            });
            ui.separator();
            ui.label("Log");
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in &self.chrome.logs {
                        ui.monospace(line);
                    }
                });
        });
    }
}
