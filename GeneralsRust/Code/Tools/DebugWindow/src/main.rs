/*!
 * DebugWindow - Simplified Version for Build Testing
 *
 * This is a simplified version of the DebugWindow that can build successfully
 * and demonstrates the basic functionality matching the C++ version.
 */

use anyhow::Result;
use eframe::egui;
use env_logger;
use log::info;
use std::collections::VecDeque;

/// Simple DebugWindow application
struct DebugWindowApp {
    show_system_panel: bool,
    show_performance_panel: bool,
    show_memory_panel: bool,
    show_console_panel: bool,
    show_log_panel: bool,
    command_text: String,
    log_entries: VecDeque<LogEntry>,
    console_history: VecDeque<String>,
    cpu_usage: f32,
    memory_usage: f32,
    frame_time: f32,
}

#[derive(Clone)]
struct LogEntry {
    timestamp: String,
    level: String,
    message: String,
}

impl Default for DebugWindowApp {
    fn default() -> Self {
        let mut app = Self {
            show_system_panel: true,
            show_performance_panel: true,
            show_memory_panel: true,
            show_console_panel: true,
            show_log_panel: true,
            command_text: String::new(),
            log_entries: VecDeque::new(),
            console_history: VecDeque::new(),
            cpu_usage: 25.0,
            memory_usage: 45.0,
            frame_time: 16.7,
        };

        // Add some sample log entries
        app.add_log_entry("INFO", "System started successfully");
        app.add_log_entry("DEBUG", "Initialized graphics subsystem");
        app.add_log_entry("WARN", "Low disk space warning");
        app.add_log_entry("INFO", "Game engine ready");

        // Add some sample console history
        app.console_history.push_back("> help".to_string());
        app.console_history
            .push_back("Available commands: help, clear, status, mem, cpu".to_string());
        app.console_history.push_back("> status".to_string());
        app.console_history
            .push_back("All systems operational".to_string());

        app
    }
}

impl DebugWindowApp {
    fn add_log_entry(&mut self, level: &str, message: &str) {
        self.log_entries.push_back(LogEntry {
            timestamp: format!(
                "{:02}:{:02}:{:02}",
                (self.log_entries.len() / 60) % 24,
                (self.log_entries.len()) % 60,
                0
            ),
            level: level.to_string(),
            message: message.to_string(),
        });

        // Keep only last 100 entries
        while self.log_entries.len() > 100 {
            self.log_entries.pop_front();
        }
    }

    fn execute_command(&mut self, command: &str) {
        self.console_history.push_back(format!("> {}", command));

        let response = match command.trim() {
            "help" => "Available commands: help, clear, status, mem, cpu, fps",
            "clear" => {
                self.console_history.clear();
                "Console cleared"
            }
            "status" => "Debug console is running - all systems operational",
            "mem" => "Memory usage: 512MB / 2GB (25%)",
            "cpu" => "CPU usage: 8 cores, 25% average",
            "fps" => "Frame time: 16.7ms (60 FPS)",
            _ => "Unknown command. Type 'help' for available commands.",
        };

        self.console_history.push_back(response.to_string());

        // Keep only last 50 entries
        while self.console_history.len() > 50 {
            self.console_history.pop_front();
        }
    }
}

impl eframe::App for DebugWindowApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Simulate changing values
        self.cpu_usage = 20.0 + 10.0 * (ctx.input(|i| i.time) as f32).sin();
        self.memory_usage = 40.0 + 5.0 * (ctx.input(|i| i.time) as f32 * 0.5).cos();
        self.frame_time = 16.0 + 2.0 * (ctx.input(|i| i.time) as f32 * 2.0).sin();

        // Top menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.show_system_panel, "System Info");
                    ui.checkbox(&mut self.show_performance_panel, "Performance");
                    ui.checkbox(&mut self.show_memory_panel, "Memory");
                    ui.checkbox(&mut self.show_console_panel, "Console");
                    ui.checkbox(&mut self.show_log_panel, "Log");
                });

                ui.menu_button("Tools", |ui| {
                    if ui.button("Clear Logs").clicked() {
                        self.log_entries.clear();
                    }
                    if ui.button("Clear Console").clicked() {
                        self.console_history.clear();
                    }
                    if ui.button("Reset Counters").clicked() {
                        info!("Resetting performance counters");
                    }
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("DebugWindow - C&C Generals Zero Hour");

            egui::ScrollArea::vertical().show(ui, |ui| {
                if self.show_system_panel {
                    ui.group(|ui| {
                        ui.heading("System Information");
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label("CPU Information");
                                ui.separator();
                                ui.label(format!("Usage: {:.1}%", self.cpu_usage));
                                ui.label("Cores: 8");
                                ui.label("Brand: Intel Core i7");
                                ui.label("Frequency: 3200 MHz");
                            });

                            ui.separator();

                            ui.vertical(|ui| {
                                ui.label("Memory Information");
                                ui.separator();
                                ui.label(format!("Used: {:.1}%", self.memory_usage));
                                ui.label("Total: 16 GB");
                                ui.label("Available: 8.5 GB");
                                ui.label("Swap: 2 GB");
                            });

                            ui.separator();

                            ui.vertical(|ui| {
                                ui.label("Graphics Information");
                                ui.separator();
                                ui.label("GPU: NVIDIA GTX 1080");
                                ui.label("VRAM: 8 GB");
                                ui.label("Driver: 461.92");
                                ui.label(format!("Frame Time: {:.1}ms", self.frame_time));
                            });
                        });
                    });
                    ui.separator();
                }

                if self.show_performance_panel {
                    ui.group(|ui| {
                        ui.heading("Performance Monitoring");
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label("Real-time Metrics");
                                ui.add(
                                    egui::ProgressBar::new(self.cpu_usage / 100.0)
                                        .text(format!("CPU: {:.1}%", self.cpu_usage)),
                                );
                                ui.add(
                                    egui::ProgressBar::new(self.memory_usage / 100.0)
                                        .text(format!("Memory: {:.1}%", self.memory_usage)),
                                );
                                ui.label(format!("FPS: {:.0}", 1000.0 / self.frame_time));
                            });

                            ui.separator();

                            ui.vertical(|ui| {
                                ui.label("Performance Graphs");
                                ui.colored_label(
                                    egui::Color32::GRAY,
                                    "[CPU usage graph would be here]",
                                );
                                ui.colored_label(
                                    egui::Color32::GRAY,
                                    "[Memory usage graph would be here]",
                                );
                                ui.colored_label(
                                    egui::Color32::GRAY,
                                    "[Frame time graph would be here]",
                                );
                            });
                        });
                    });
                    ui.separator();
                }

                if self.show_memory_panel {
                    ui.group(|ui| {
                        ui.heading("Memory Profiler");
                        ui.label("Process Memory Details");
                        ui.label("• Game Engine: 512 MB");
                        ui.label("• Graphics: 256 MB");
                        ui.label("• Audio: 64 MB");
                        ui.label("• Scripts: 32 MB");
                        ui.label("• Assets: 1.2 GB");
                        ui.separator();
                        ui.label("Memory Allocations: 2,847 active");
                        ui.label("Peak Memory Usage: 2.1 GB");
                    });
                    ui.separator();
                }

                if self.show_console_panel {
                    ui.group(|ui| {
                        ui.heading("Debug Console");

                        // Command input
                        ui.horizontal(|ui| {
                            ui.label("Command:");
                            let response = ui.text_edit_singleline(&mut self.command_text);

                            if response.lost_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            {
                                self.execute_command(&self.command_text.clone());
                                self.command_text.clear();
                            }

                            if ui.button("Execute").clicked() {
                                self.execute_command(&self.command_text.clone());
                                self.command_text.clear();
                            }
                        });

                        // Command history
                        egui::ScrollArea::vertical()
                            .max_height(200.0)
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                for entry in &self.console_history {
                                    ui.label(entry);
                                }
                            });
                    });
                    ui.separator();
                }

                if self.show_log_panel {
                    ui.group(|ui| {
                        ui.heading("Log Viewer");

                        // Log entries
                        egui::ScrollArea::vertical()
                            .max_height(250.0)
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                for entry in &self.log_entries {
                                    let color = match entry.level.as_str() {
                                        "ERROR" => egui::Color32::RED,
                                        "WARN" => egui::Color32::YELLOW,
                                        "INFO" => egui::Color32::WHITE,
                                        "DEBUG" => egui::Color32::LIGHT_BLUE,
                                        _ => egui::Color32::GRAY,
                                    };

                                    ui.horizontal(|ui| {
                                        ui.colored_label(color, format!("[{}]", entry.level));
                                        ui.label(&entry.timestamp);
                                        ui.separator();
                                        ui.label(&entry.message);
                                    });
                                }
                            });
                    });
                }
            });
        });

        // Request repaint for real-time updates
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

/// Main entry point matching C++ DebugWindow interface
fn main() -> Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    info!("Starting DebugWindow...");

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("DebugWindow - System Monitor"),
        ..Default::default()
    };

    eframe::run_native(
        "DebugWindow",
        native_options,
        Box::new(|_cc| Ok(Box::new(DebugWindowApp::default()))),
    )
    .map_err(|e| anyhow::anyhow!("Failed to run DebugWindow: {}", e))
}
