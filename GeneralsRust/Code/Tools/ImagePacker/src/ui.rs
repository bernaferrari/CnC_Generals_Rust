/*!
 * User interface for ImagePacker
 */

use crate::{ImagePackerApp, ProcessingStatus};
use eframe::egui::{self, Color32, RichText, Sense};
use std::path::PathBuf;

pub fn render_ui(app: &mut ImagePackerApp, ctx: &egui::Context) {
    // Top menu bar
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Load Config").clicked() {
                    // TODO: Implement config loading
                }
                if ui.button("Save Config").clicked() {
                    // TODO: Implement config saving
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    std::process::exit(0);
                }
            });

            ui.menu_button("View", |ui| {
                ui.checkbox(&mut app.ui_state.show_config_panel, "Configuration");
                ui.checkbox(&mut app.ui_state.show_results_panel, "Results");
                ui.checkbox(&mut app.ui_state.show_log_panel, "Log");
            });

            ui.menu_button("Help", |ui| {
                if ui.button("About").clicked() {
                    // TODO: Show about dialog
                }
                if ui.button("User Guide").clicked() {
                    // TODO: Open help documentation
                }
            });
        });
    });

    // Status bar
    egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            match &app.processing_status {
                ProcessingStatus::Idle => {
                    ui.label("Ready");
                }
                ProcessingStatus::Processing { progress, current_file } => {
                    ui.add(egui::ProgressBar::new(*progress).text(format!("{:.1}%", progress * 100.0)));
                    ui.separator();
                    ui.label(current_file);
                }
                ProcessingStatus::Complete { total_atlases, total_images } => {
                    ui.colored_label(
                        Color32::GREEN,
                        format!("Complete: {} atlases, {} images", total_atlases, total_images)
                    );
                }
                ProcessingStatus::Error(msg) => {
                    ui.colored_label(Color32::RED, format!("Error: {}", msg));
                }
            }
        });
    });

    // Main content area
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.horizontal(|ui| {
            // Left panel - Configuration
            if app.ui_state.show_config_panel {
                ui.vertical(|ui| {
                    render_config_panel(app, ui);
                });
                ui.separator();
            }

            // Right panel - Results and Log
            ui.vertical(|ui| {
                if app.ui_state.show_results_panel {
                    render_results_panel(app, ui);
                    ui.separator();
                }
                if app.ui_state.show_log_panel {
                    render_log_panel(app, ui);
                }
            });
        });
    });
}

fn render_config_panel(app: &mut ImagePackerApp, ui: &mut egui::Ui) {
    ui.heading("Configuration");

    ui.group(|ui| {
        ui.label("Input/Output");
        
        ui.horizontal(|ui| {
            ui.label("Input Dir:");
            if ui.button("Browse").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    app.config.input_dir = path;
                }
            }
        });
        ui.label(format!("📁 {}", app.config.input_dir.display()));

        ui.horizontal(|ui| {
            ui.label("Output Dir:");
            if ui.button("Browse").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    app.config.output_dir = path;
                }
            }
        });
        ui.label(format!("📁 {}", app.config.output_dir.display()));
    });

    ui.group(|ui| {
        ui.label("Texture Settings");
        
        ui.horizontal(|ui| {
            ui.label("Max Size:");
            egui::ComboBox::from_label("")
                .selected_text(format!("{}", app.config.max_texture_size))
                .show_ui(ui, |ui| {
                    for &size in &[512, 1024, 2048, 4096] {
                        ui.selectable_value(&mut app.config.max_texture_size, size, format!("{}", size));
                    }
                });
        });

        ui.horizontal(|ui| {
            ui.label("Padding:");
            ui.add(egui::Slider::new(&mut app.config.padding, 0..=16).suffix(" px"));
        });

        ui.horizontal(|ui| {
            ui.label("Format:");
            egui::ComboBox::from_label("")
                .selected_text(&app.config.output_format)
                .show_ui(ui, |ui| {
                    for format in &["PNG", "TGA", "DDS", "JPG"] {
                        ui.selectable_value(&mut app.config.output_format, format.to_string(), *format);
                    }
                });
        });
    });

    ui.group(|ui| {
        ui.label("Options");
        ui.checkbox(&mut app.config.trim_sprites, "Trim transparent pixels");
        ui.checkbox(&mut app.config.generate_metadata, "Generate metadata files");
    });

    ui.separator();

    // Process button
    let can_process = match app.processing_status {
        ProcessingStatus::Processing { .. } => false,
        _ => app.config.input_dir.exists(),
    };

    let button_text = match app.processing_status {
        ProcessingStatus::Processing { .. } => "Processing...",
        _ => "Process Images",
    };

    if ui.add_enabled(can_process, egui::Button::new(button_text)).clicked() {
        // Process in background
        if let Err(e) = app.process_images() {
            app.processing_status = ProcessingStatus::Error(e.to_string());
        }
    }
}

fn render_results_panel(app: &mut ImagePackerApp, ui: &mut egui::Ui) {
    ui.heading("Results");

    if app.atlas_results.is_empty() {
        ui.label("No atlases generated yet");
        return;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for (i, result) in app.atlas_results.iter().enumerate() {
            ui.group(|ui| {
                let header_response = ui.selectable_label(
                    app.ui_state.selected_atlas == Some(i),
                    RichText::new(&result.group_name).heading()
                );

                if header_response.clicked() {
                    app.ui_state.selected_atlas = if app.ui_state.selected_atlas == Some(i) {
                        None
                    } else {
                        Some(i)
                    };
                }

                if app.ui_state.selected_atlas == Some(i) {
                    ui.separator();
                    ui.label(format!("📊 Sprites: {}", result.sprite_count));
                    ui.label(format!("📐 Size: {}×{}", result.atlas_size.0, result.atlas_size.1));
                    ui.label(format!("📁 Atlas: {}", result.atlas_path.display()));
                    
                    if let Some(ref metadata_path) = result.metadata_path {
                        ui.label(format!("📄 Metadata: {}", metadata_path.display()));
                    }
                    
                    ui.label(format!("🕒 Created: {}", result.created_at.format("%Y-%m-%d %H:%M:%S")));

                    ui.horizontal(|ui| {
                        if ui.button("Open Atlas").clicked() {
                            if let Err(e) = open::that(&result.atlas_path) {
                                log::warn!("Failed to open atlas: {}", e);
                            }
                        }
                        if ui.button("Open Folder").clicked() {
                            if let Some(parent) = result.atlas_path.parent() {
                                if let Err(e) = open::that(parent) {
                                    log::warn!("Failed to open folder: {}", e);
                                }
                            }
                        }
                    });
                }
            });
        }
    });
}

fn render_log_panel(app: &mut ImagePackerApp, ui: &mut egui::Ui) {
    ui.heading("Log");
    
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            ui.label("Log messages would appear here");
            // TODO: Implement proper logging display
            // This would require integrating with a logging system that captures messages
        });
}