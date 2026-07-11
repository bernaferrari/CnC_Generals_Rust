////////////////////////////////////////////////////////////////////////////////
//
//  (c) 2001-2003 Electronic Arts Inc.
//
////////////////////////////////////////////////////////////////////////////////

// FILE: debug_system.rs
//
// Debug logging and crash reporting system
// Matches the C++ DebugLogger functionality
//
// Author: Colin Day, April 2001 (Converted to Rust)
//
///////////////////////////////////////////////////////////////////////////////

use anyhow::{Context, Result};
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Debug system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugConfig {
    pub log_to_file: bool,
    pub log_to_console: bool,
    pub log_level: String,
    pub log_file_path: PathBuf,
    pub crash_dump_path: PathBuf,
    pub max_log_file_size: u64,
    pub max_log_files: u32,
    pub flush_frequency: u64,
    pub enable_crash_handler: bool,
    pub debug_ui_enabled: bool,
    pub performance_logging: bool,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            log_to_file: true,
            log_to_console: cfg!(debug_assertions),
            log_level: if cfg!(debug_assertions) {
                "debug".to_string()
            } else {
                "info".to_string()
            },
            log_file_path: PathBuf::from("generals_debug.log"),
            crash_dump_path: PathBuf::from("crashes"),
            max_log_file_size: 50 * 1024 * 1024, // 50MB
            max_log_files: 5,
            flush_frequency: 100, // Flush every 100 log entries
            enable_crash_handler: true,
            debug_ui_enabled: cfg!(debug_assertions),
            performance_logging: cfg!(debug_assertions),
        }
    }
}

/// Debug system state and statistics
#[derive(Debug, Clone)]
pub struct DebugStats {
    pub log_entries_written: u64,
    pub crashes_handled: u32,
    pub last_flush_time: SystemTime,
    pub total_log_size: u64,
    pub performance_samples: u32,
}

impl Default for DebugStats {
    fn default() -> Self {
        Self {
            log_entries_written: 0,
            crashes_handled: 0,
            last_flush_time: SystemTime::now(),
            total_log_size: 0,
            performance_samples: 0,
        }
    }
}

/// Performance timing data
#[derive(Debug, Clone)]
pub struct PerformanceTimer {
    pub name: String,
    pub start_time: SystemTime,
    pub samples: Vec<f64>,
    pub avg_time: f64,
    pub min_time: f64,
    pub max_time: f64,
}

impl PerformanceTimer {
    pub fn new(name: String) -> Self {
        Self {
            name,
            start_time: SystemTime::now(),
            samples: Vec::new(),
            avg_time: 0.0,
            min_time: f64::MAX,
            max_time: 0.0,
        }
    }

    pub fn start(&mut self) {
        self.start_time = SystemTime::now();
    }

    pub fn stop(&mut self) {
        let elapsed = self.start_time.elapsed().unwrap_or_default().as_secs_f64() * 1000.0; // ms
        self.samples.push(elapsed);

        if self.samples.len() > 1000 {
            self.samples.remove(0); // Keep only last 1000 samples
        }

        self.min_time = self.min_time.min(elapsed);
        self.max_time = self.max_time.max(elapsed);
        self.avg_time = self.samples.iter().sum::<f64>() / self.samples.len() as f64;
    }
}

/// Main debug system
pub struct DebugSystem {
    config: DebugConfig,
    stats: Arc<Mutex<DebugStats>>,
    log_writer: Option<Arc<Mutex<BufWriter<File>>>>,
    performance_timers: Arc<Mutex<std::collections::HashMap<String, PerformanceTimer>>>,
}

impl DebugSystem {
    /// Create a new debug system with the given configuration
    pub fn new(config: DebugConfig) -> Result<Self> {
        let mut system = Self {
            config: config.clone(),
            stats: Arc::new(Mutex::new(DebugStats::default())),
            log_writer: None,
            performance_timers: Arc::new(Mutex::new(std::collections::HashMap::new())),
        };

        // Initialize log file if enabled
        if config.log_to_file {
            system.init_log_file()?;
        }

        // Set up crash handler if enabled
        if config.enable_crash_handler {
            system.setup_crash_handler()?;
        }

        // Create crash dump directory
        if !config.crash_dump_path.exists() {
            std::fs::create_dir_all(&config.crash_dump_path)
                .context("Failed to create crash dump directory")?;
        }

        info!("Debug system initialized");
        info!("Log file: {:?}", config.log_file_path);
        info!("Crash dumps: {:?}", config.crash_dump_path);

        Ok(system)
    }

    /// Initialize the log file writer
    fn init_log_file(&mut self) -> Result<()> {
        // Rotate log files if necessary
        self.rotate_log_files()?;

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.config.log_file_path)
            .context("Failed to open log file")?;

        let writer = BufWriter::new(file);
        self.log_writer = Some(Arc::new(Mutex::new(writer)));

        Ok(())
    }

    /// Rotate log files to prevent them from getting too large
    fn rotate_log_files(&self) -> Result<()> {
        let log_file = &self.config.log_file_path;

        if log_file.exists() {
            let metadata = std::fs::metadata(log_file)?;
            if metadata.len() > self.config.max_log_file_size {
                // Rotate files
                for i in (1..self.config.max_log_files).rev() {
                    let old_file = log_file.with_extension(format!("log.{}", i));
                    let new_file = log_file.with_extension(format!("log.{}", i + 1));

                    if old_file.exists() {
                        if new_file.exists() {
                            std::fs::remove_file(&new_file)?;
                        }
                        std::fs::rename(&old_file, &new_file)?;
                    }
                }

                // Move current log to .1
                let backup_file = log_file.with_extension("log.1");
                if backup_file.exists() {
                    std::fs::remove_file(&backup_file)?;
                }
                std::fs::rename(log_file, backup_file)?;
            }
        }

        Ok(())
    }

    /// Set up crash handler (cross-platform stub)
    fn setup_crash_handler(&self) -> Result<()> {
        // On Unix systems, we could use signal handlers
        // On Windows, we could use SetUnhandledExceptionFilter
        // For now, we'll use Rust's panic hook

        let crash_path = self.config.crash_dump_path.clone();
        std::panic::set_hook(Box::new(move |panic_info| {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let crash_file = crash_path.join(format!("crash_{}.txt", timestamp));

            if let Ok(mut file) = File::create(&crash_file) {
                writeln!(file, "Command & Conquer Generals Zero Hour - Crash Report").unwrap_or(());
                writeln!(file, "Time: {}", timestamp).unwrap_or(());
                writeln!(file, "Panic: {}", panic_info).unwrap_or(());
                writeln!(file, "Location: {:?}", panic_info.location()).unwrap_or(());

                // Add backtrace if available
                #[cfg(debug_assertions)]
                {
                    let backtrace = std::backtrace::Backtrace::capture();
                    writeln!(file, "Backtrace:\n{}", backtrace).unwrap_or(());
                }

                file.flush().unwrap_or(());
            }

            eprintln!("CRASH: Panic occurred, dump written to: {:?}", crash_file);
            eprintln!("Panic: {}", panic_info);
        }));

        Ok(())
    }

    /// Write a debug log entry to file
    pub fn log_to_file(&self, level: &str, message: &str) -> Result<()> {
        if let Some(ref writer) = self.log_writer {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();

            let formatted = format!("[{}] [{}] {}\n", timestamp, level.to_uppercase(), message);

            {
                let mut writer_guard = writer.lock().unwrap_or_else(|e| e.into_inner());
                writer_guard.write_all(formatted.as_bytes())?;

                // Update stats
                {
                    let mut stats = self.stats.lock().unwrap_or_else(|e| e.into_inner());
                    stats.log_entries_written += 1;
                    stats.total_log_size += formatted.len() as u64;

                    // Flush periodically
                    if stats.log_entries_written.is_multiple_of(self.config.flush_frequency) {
                        writer_guard.flush()?;
                        stats.last_flush_time = SystemTime::now();
                    }
                }
            }
        }

        Ok(())
    }

    /// Start a performance timer
    pub fn start_timer(&self, name: &str) {
        if !self.config.performance_logging {
            return;
        }

        let mut timers = self
            .performance_timers
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let timer = timers
            .entry(name.to_string())
            .or_insert_with(|| PerformanceTimer::new(name.to_string()));
        timer.start();
    }

    /// Stop a performance timer and record the result
    pub fn stop_timer(&self, name: &str) {
        if !self.config.performance_logging {
            return;
        }

        let mut timers = self
            .performance_timers
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(timer) = timers.get_mut(name) {
            timer.stop();

            // Log performance data periodically
            if timer.samples.len() % 100 == 0 {
                info!(
                    "PERF [{}]: avg={:.2}ms, min={:.2}ms, max={:.2}ms",
                    name, timer.avg_time, timer.min_time, timer.max_time
                );
            }
        }
    }

    /// Get performance statistics
    pub fn get_performance_stats(&self) -> std::collections::HashMap<String, PerformanceTimer> {
        self.performance_timers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Get debug system statistics
    pub fn get_stats(&self) -> DebugStats {
        self.stats.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Flush log buffers to disk
    pub fn flush(&self) -> Result<()> {
        if let Some(ref writer) = self.log_writer {
            let mut writer_guard = writer.lock().unwrap_or_else(|e| e.into_inner());
            writer_guard.flush()?;

            let mut stats = self.stats.lock().unwrap_or_else(|e| e.into_inner());
            stats.last_flush_time = SystemTime::now();
        }
        Ok(())
    }

    /// Handle a critical error (creates crash dump)
    pub fn handle_critical_error(&self, error: &str, details: Option<&str>) -> Result<()> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let crash_file = self
            .config
            .crash_dump_path
            .join(format!("critical_error_{}.txt", timestamp));

        let mut file = File::create(&crash_file)?;
        writeln!(
            file,
            "Command & Conquer Generals Zero Hour - Critical Error"
        )?;
        writeln!(file, "Time: {}", timestamp)?;
        writeln!(file, "Error: {}", error)?;

        if let Some(details) = details {
            writeln!(file, "Details: {}", details)?;
        }

        // Add system information
        writeln!(file, "Platform: {}", std::env::consts::OS)?;
        writeln!(file, "Architecture: {}", std::env::consts::ARCH)?;

        file.flush()?;

        {
            let mut stats = self.stats.lock().unwrap_or_else(|e| e.into_inner());
            stats.crashes_handled += 1;
        }

        error!("CRITICAL ERROR: {} (dump: {:?})", error, crash_file);
        Ok(())
    }
}

/// RAII performance timer helper
pub struct ScopedTimer {
    debug_system: Arc<DebugSystem>,
    timer_name: String,
}

impl ScopedTimer {
    pub fn new(debug_system: Arc<DebugSystem>, name: String) -> Self {
        debug_system.start_timer(&name);
        Self {
            debug_system,
            timer_name: name,
        }
    }
}

impl Drop for ScopedTimer {
    fn drop(&mut self) {
        self.debug_system.stop_timer(&self.timer_name);
    }
}

/// Global debug system instance
static DEBUG_SYSTEM: std::sync::LazyLock<std::sync::Mutex<Option<Arc<DebugSystem>>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

/// Initialize the global debug system
pub fn initialize_debug_system(config: Option<DebugConfig>) -> Result<Arc<DebugSystem>> {
    let config = config.unwrap_or_default();
    let system = Arc::new(DebugSystem::new(config)?);

    {
        let mut global_system = DEBUG_SYSTEM.lock().unwrap_or_else(|e| e.into_inner());
        *global_system = Some(system.clone());
    }

    info!("Global debug system initialized");
    Ok(system)
}

/// Get the global debug system
pub fn get_debug_system() -> Option<Arc<DebugSystem>> {
    DEBUG_SYSTEM
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Start a performance timer on the global debug system
pub fn start_performance_timer(name: &str) {
    if let Some(system) = get_debug_system() {
        system.start_timer(name);
    }
}

/// Stop a performance timer on the global debug system
pub fn stop_performance_timer(name: &str) {
    if let Some(system) = get_debug_system() {
        system.stop_timer(name);
    }
}

/// Create a scoped timer that automatically stops when dropped
pub fn scoped_timer(name: &str) -> Option<ScopedTimer> {
    get_debug_system().map(|system| ScopedTimer::new(system, name.to_string()))
}

/// Log a critical error to the global debug system
pub fn log_critical_error(error: &str, details: Option<&str>) {
    if let Some(system) = get_debug_system() {
        if let Err(e) = system.handle_critical_error(error, details) {
            eprintln!("Failed to log critical error: {}", e);
        }
    }
}

/// Flush all debug logs
pub fn flush_debug_logs() {
    if let Some(system) = get_debug_system() {
        if let Err(e) = system.flush() {
            eprintln!("Failed to flush debug logs: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_debug_system_creation() {
        let config = DebugConfig {
            log_to_file: false, // Don't create files in tests
            ..Default::default()
        };

        let system = DebugSystem::new(config).unwrap();
        let stats = system.get_stats();
        assert_eq!(stats.log_entries_written, 0);
    }

    #[test]
    fn test_performance_timer() {
        let mut timer = PerformanceTimer::new("test".to_string());
        timer.start();
        std::thread::sleep(Duration::from_millis(1));
        timer.stop();

        assert!(timer.avg_time > 0.0);
        assert_eq!(timer.samples.len(), 1);
    }

    #[test]
    fn test_scoped_timer() {
        let config = DebugConfig {
            log_to_file: false,
            performance_logging: true,
            ..Default::default()
        };

        let system = Arc::new(DebugSystem::new(config).unwrap());

        {
            let _timer = ScopedTimer::new(system.clone(), "test_scope".to_string());
            std::thread::sleep(Duration::from_millis(1));
        } // Timer stops here

        let stats = system.get_performance_stats();
        assert!(stats.contains_key("test_scope"));
    }
}
