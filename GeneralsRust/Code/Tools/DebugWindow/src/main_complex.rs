/*!
 * DebugWindow - System and Game Engine Debug Monitor
 * 
 * Rust implementation of the C++ DebugWindow tool for monitoring game engine
 * performance, memory usage, and system resources. Provides real-time debugging
 * capabilities for the Command & Conquer Generals engine.
 * 
 * Features:
 * - Real-time system resource monitoring
 * - Game engine performance metrics
 * - Memory usage tracking and profiling
 * - Log message filtering and display
 * - Performance graph visualization
 * - Debug command interface
 */

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use eframe::egui::{self, plot::{Line, Plot, PlotPoints}, Color32, RichText};
use env_logger;
use log::{debug, error, info, warn};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::{System, SystemExt, CpuExt, ProcessExt, PidExt};
use tokio::sync::mpsc;
use uuid::Uuid;

mod system_monitor;
mod memory_profiler;
mod performance_graphs;
mod debug_console;

use system_monitor::*;
use memory_profiler::*;
use performance_graphs::*;
use debug_console::*;

/// Configuration for debug window display and monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugConfig {
    /// Update frequency for system monitoring (Hz)
    pub update_frequency: f32,
    /// Maximum number of data points to keep in graphs
    pub max_graph_points: usize,
    /// Which panels to show by default
    pub default_panels: PanelConfig,
    /// Logging configuration
    pub log_config: LogConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelConfig {
    pub system_info: bool,
    pub performance_graphs: bool,
    pub memory_profiler: bool,
    pub debug_console: bool,
    pub log_viewer: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    /// Maximum number of log entries to keep
    pub max_entries: usize,
    /// Minimum log level to display
    pub min_level: String,
    /// Whether to auto-scroll to new entries
    pub auto_scroll: bool,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            update_frequency: 2.0,
            max_graph_points: 100,
            default_panels: PanelConfig {
                system_info: true,
                performance_graphs: true,
                memory_profiler: true,
                debug_console: true,
                log_viewer: true,
            },
            log_config: LogConfig {
                max_entries: 1000,
                min_level: "INFO".to_string(),
                auto_scroll: true,
            },
        }
    }
}

/// Real-time system metrics
#[derive(Debug, Clone)]
pub struct SystemMetrics {
    /// Timestamp when metrics were collected
    pub timestamp: Instant,
    /// CPU usage percentage (0-100)
    pub cpu_usage: f32,
    /// Memory usage in bytes
    pub memory_usage: u64,
    /// Total system memory in bytes  
    pub total_memory: u64,
    /// Memory usage percentage
    pub memory_percent: f32,
    /// Process-specific metrics
    pub process_metrics: Option<ProcessMetrics>,
    /// GPU metrics (if available)
    pub gpu_metrics: Option<GpuMetrics>,
}

#[derive(Debug, Clone)]
pub struct ProcessMetrics {
    /// Process ID
    pub pid: u32,
    /// Process CPU usage
    pub cpu_percent: f32,
    /// Process memory usage in bytes
    pub memory_bytes: u64,
    /// Number of threads
    pub thread_count: usize,
    /// Process uptime
    pub uptime: Duration,
}

#[derive(Debug, Clone)]
pub struct GpuMetrics {
    /// GPU usage percentage
    pub gpu_usage: f32,
    /// GPU memory usage in bytes
    pub gpu_memory_usage: u64,
    /// GPU memory total in bytes
    pub gpu_memory_total: u64,
    /// GPU temperature (if available)
    pub temperature: Option<f32>,
}

/// Log entry for the debug window
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub target: String,
    pub message: String,
}

/// Main DebugWindow application
pub struct DebugWindowApp {
    config: DebugConfig,
    system: System,
    system_monitor: SystemMonitor,
    memory_profiler: MemoryProfiler,
    performance_graphs: PerformanceGraphs,
    debug_console: DebugConsole,
    log_entries: Arc<RwLock<VecDeque<LogEntry>>>,
    ui_state: UiState,
    last_update: Instant,
    metrics_history: VecDeque<SystemMetrics>,
    selected_process: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct UiState {
    pub show_system_panel: bool,
    pub show_performance_panel: bool,
    pub show_memory_panel: bool,
    pub show_console_panel: bool,
    pub show_log_panel: bool,
    pub log_filter: String,
    pub selected_log_level: String,
    pub graph_time_range: f32, // seconds
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            show_system_panel: true,
            show_performance_panel: true,
            show_memory_panel: true,
            show_console_panel: true,
            show_log_panel: true,
            log_filter: String::new(),
            selected_log_level: "INFO".to_string(),
            graph_time_range: 60.0,
        }
    }
}

impl DebugWindowApp {
    pub fn new() -> Result<Self> {
        let config = DebugConfig::default();
        let mut system = System::new_all();
        system.refresh_all();

        Ok(Self {
            system_monitor: SystemMonitor::new()?,
            memory_profiler: MemoryProfiler::new()?,
            performance_graphs: PerformanceGraphs::new(),
            debug_console: DebugConsole::new(),
            log_entries: Arc::new(RwLock::new(VecDeque::new())),
            ui_state: UiState::default(),
            last_update: Instant::now(),
            metrics_history: VecDeque::new(),
            selected_process: None,
            config,
            system,
        })
    }

    /// Update system metrics
    pub fn update_metrics(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_update).as_secs_f32() < 1.0 / self.config.update_frequency {
            return;
        }

        self.system.refresh_all();
        self.last_update = now;

        // Collect current metrics
        let metrics = SystemMetrics {
            timestamp: now,
            cpu_usage: self.system.global_cpu_info().cpu_usage(),
            memory_usage: self.system.used_memory(),
            total_memory: self.system.total_memory(),
            memory_percent: (self.system.used_memory() as f32 / self.system.total_memory() as f32) * 100.0,
            process_metrics: self.get_process_metrics(),
            gpu_metrics: self.get_gpu_metrics(),
        };

        // Add to history
        self.metrics_history.push_back(metrics);
        
        // Keep only the last N data points
        while self.metrics_history.len() > self.config.max_graph_points {
            self.metrics_history.pop_front();
        }

        // Update sub-components
        self.system_monitor.update(&self.system);
        self.memory_profiler.update(&self.system);
        self.performance_graphs.update(&self.metrics_history);
    }

    fn get_process_metrics(&self) -> Option<ProcessMetrics> {
        if let Some(pid) = self.selected_process {
            if let Some(process) = self.system.process(sysinfo::Pid::from_u32(pid)) {
                return Some(ProcessMetrics {
                    pid,
                    cpu_percent: process.cpu_usage(),
                    memory_bytes: process.memory(),
                    thread_count: process.tasks().len(),
                    uptime: Duration::from_secs(process.run_time()),
                });
            }
        }
        None
    }

    fn get_gpu_metrics(&self) -> Option<GpuMetrics> {
        // TODO: Implement GPU metrics collection
        // This would require platform-specific GPU monitoring libraries
        None
    }

    /// Add a log entry to the debug window
    pub fn add_log_entry(&mut self, level: &str, target: &str, message: &str) {
        let entry = LogEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            level: level.to_string(),
            target: target.to_string(),
            message: message.to_string(),
        };

        let mut log_entries = self.log_entries.write();
        log_entries.push_back(entry);

        // Keep only the last N entries
        while log_entries.len() > self.config.log_config.max_entries {
            log_entries.pop_front();
        }
    }

    /// Get filtered log entries
    pub fn get_filtered_log_entries(&self) -> Vec<LogEntry> {
        let log_entries = self.log_entries.read();
        log_entries
            .iter()
            .filter(|entry| {
                // Filter by level
                if !self.ui_state.selected_log_level.is_empty() && 
                   entry.level != self.ui_state.selected_log_level {
                    return false;
                }

                // Filter by search text
                if !self.ui_state.log_filter.is_empty() {
                    let filter = self.ui_state.log_filter.to_lowercase();
                    return entry.message.to_lowercase().contains(&filter) ||
                           entry.target.to_lowercase().contains(&filter);
                }

                true
            })
            .cloned()
            .collect()
    }
}

impl eframe::App for DebugWindowApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update metrics
        self.update_metrics();

        // Top menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("View", |ui| {
                    ui.checkbox(&mut self.ui_state.show_system_panel, "System Info");
                    ui.checkbox(&mut self.ui_state.show_performance_panel, "Performance");
                    ui.checkbox(&mut self.ui_state.show_memory_panel, "Memory");
                    ui.checkbox(&mut self.ui_state.show_console_panel, "Console");
                    ui.checkbox(&mut self.ui_state.show_log_panel, "Log");
                });

                ui.menu_button("Tools", |ui| {
                    if ui.button("Clear Logs").clicked() {
                        self.log_entries.write().clear();
                    }
                    if ui.button("Reset Metrics").clicked() {
                        self.metrics_history.clear();
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        // TODO: Show about dialog
                    }
                });
            });
        });

        // Main panels
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                if self.ui_state.show_system_panel {
                    self.render_system_panel(ui);
                    ui.separator();
                }

                if self.ui_state.show_performance_panel {
                    self.render_performance_panel(ui);
                    ui.separator();
                }

                if self.ui_state.show_memory_panel {
                    self.render_memory_panel(ui);
                    ui.separator();
                }

                if self.ui_state.show_console_panel {
                    self.render_console_panel(ui);
                    ui.separator();
                }

                if self.ui_state.show_log_panel {
                    self.render_log_panel(ui);
                }
            });
        });

        // Request repaint for real-time updates
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

impl DebugWindowApp {
    fn render_system_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("System Information");

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label("CPU Information");
                ui.separator();
                
                if let Some(metrics) = self.metrics_history.back() {
                    ui.label(format!("Usage: {:.1}%", metrics.cpu_usage));
                }
                
                ui.label(format!("Cores: {}", self.system.cpus().len()));
                ui.label(format!("Brand: {}", self.system.global_cpu_info().brand()));
                ui.label(format!("Frequency: {} MHz", self.system.global_cpu_info().frequency()));
            });

            ui.separator();

            ui.vertical(|ui| {
                ui.label("Memory Information");
                ui.separator();
                
                if let Some(metrics) = self.metrics_history.back() {
                    ui.label(format!("Used: {:.1} GB ({:.1}%)", 
                        metrics.memory_usage as f64 / 1024.0 / 1024.0 / 1024.0,
                        metrics.memory_percent));
                    ui.label(format!("Total: {:.1} GB", 
                        metrics.total_memory as f64 / 1024.0 / 1024.0 / 1024.0));
                }
                
                ui.label(format!("Swap Used: {:.1} GB", 
                    self.system.used_swap() as f64 / 1024.0 / 1024.0 / 1024.0));
            });

            ui.separator();

            ui.vertical(|ui| {
                ui.label("Process Selection");
                ui.separator();
                
                egui::ComboBox::from_label("Target Process")
                    .selected_text(if let Some(pid) = self.selected_process {
                        format!("PID: {}", pid)
                    } else {
                        "None".to_string()
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.selected_process, None, "None");
                        for (pid, process) in self.system.processes() {
                            ui.selectable_value(
                                &mut self.selected_process, 
                                Some(pid.as_u32()), 
                                format!("{}: {}", pid.as_u32(), process.name())
                            );
                        }
                    });
            });
        });
    }

    fn render_performance_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Performance Graphs");

        let plot_height = 150.0;

        // CPU Usage Graph
        ui.label("CPU Usage");
        Plot::new("cpu_plot")
            .height(plot_height)
            .show(ui, |plot_ui| {
                let cpu_points: PlotPoints = self.metrics_history
                    .iter()
                    .enumerate()
                    .map(|(i, metrics)| [i as f64, metrics.cpu_usage as f64])
                    .collect();
                plot_ui.line(Line::new(cpu_points).color(Color32::RED));
            });

        // Memory Usage Graph
        ui.label("Memory Usage");
        Plot::new("memory_plot")
            .height(plot_height)
            .show(ui, |plot_ui| {
                let memory_points: PlotPoints = self.metrics_history
                    .iter()
                    .enumerate()
                    .map(|(i, metrics)| [i as f64, metrics.memory_percent as f64])
                    .collect();
                plot_ui.line(Line::new(memory_points).color(Color32::BLUE));
            });
    }

    fn render_memory_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Memory Profiler");
        
        if let Some(process_metrics) = self.metrics_history.back()
            .and_then(|m| m.process_metrics.as_ref()) {
            ui.label(format!("Process Memory: {:.2} MB", 
                process_metrics.memory_bytes as f64 / 1024.0 / 1024.0));
            ui.label(format!("Thread Count: {}", process_metrics.thread_count));
            ui.label(format!("Uptime: {:?}", process_metrics.uptime));
        } else {
            ui.label("Select a process to view detailed memory information");
        }
    }

    fn render_console_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Debug Console");
        
        // Command input
        ui.horizontal(|ui| {
            ui.label("Command:");
            let mut command_text = String::new();
            let response = ui.text_edit_singleline(&mut command_text);
            
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.debug_console.execute_command(&command_text);
            }
            
            if ui.button("Execute").clicked() {
                self.debug_console.execute_command(&command_text);
            }
        });

        // Command history/output
        egui::ScrollArea::vertical()
            .height(200.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for entry in self.debug_console.get_history() {
                    ui.label(&entry.text);
                }
            });
    }

    fn render_log_panel(&mut self, ui: &mut egui::Ui) {
        ui.heading("Log Viewer");

        // Log controls
        ui.horizontal(|ui| {
            ui.label("Filter:");
            ui.text_edit_singleline(&mut self.ui_state.log_filter);
            
            ui.separator();
            
            ui.label("Level:");
            egui::ComboBox::from_label("")
                .selected_text(&self.ui_state.selected_log_level)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.ui_state.selected_log_level, String::new(), "All");
                    for level in &["ERROR", "WARN", "INFO", "DEBUG", "TRACE"] {
                        ui.selectable_value(&mut self.ui_state.selected_log_level, level.to_string(), *level);
                    }
                });
            
            if ui.button("Clear").clicked() {
                self.log_entries.write().clear();
            }
        });

        // Log entries
        egui::ScrollArea::vertical()
            .height(300.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                let entries = self.get_filtered_log_entries();
                for entry in entries {
                    let color = match entry.level.as_str() {
                        "ERROR" => Color32::RED,
                        "WARN" => Color32::YELLOW,
                        "INFO" => Color32::WHITE,
                        "DEBUG" => Color32::LIGHT_BLUE,
                        _ => Color32::GRAY,
                    };
                    
                    ui.horizontal(|ui| {
                        ui.colored_label(color, format!("[{}]", entry.level));
                        ui.label(format!("{}", entry.timestamp.format("%H:%M:%S")));
                        ui.separator();
                        ui.label(&entry.message);
                    });
                }
            });
    }
}

/// Main entry point matching C++ DebugWindow interface
fn main() -> Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    info!("Starting DebugWindow...");

    let app = DebugWindowApp::new()
        .context("Failed to create DebugWindow application")?;

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title("DebugWindow - System Monitor"),
        ..Default::default()
    };

    eframe::run_native(
        "DebugWindow",
        native_options,
        Box::new(|_cc| Ok(Box::new(app))),
    ).map_err(|e| anyhow::anyhow!("Failed to run DebugWindow: {}", e))
}