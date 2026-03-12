////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Debug System Implementation
//!
//! Provides debugging utilities, assertion macros, logging facilities,
//! and performance monitoring for the game engine.
//!
//! Rust conversion: 2025

use once_cell::sync::OnceCell;
use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Debug message severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DebugLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warning = 3,
    Error = 4,
    Fatal = 5,
}

impl fmt::Display for DebugLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DebugLevel::Trace => write!(f, "TRACE"),
            DebugLevel::Debug => write!(f, "DEBUG"),
            DebugLevel::Info => write!(f, "INFO"),
            DebugLevel::Warning => write!(f, "WARN"),
            DebugLevel::Error => write!(f, "ERROR"),
            DebugLevel::Fatal => write!(f, "FATAL"),
        }
    }
}

/// Debug message structure
#[derive(Debug, Clone)]
pub struct DebugMessage {
    pub level: DebugLevel,
    pub message: String,
    pub file: String,
    pub line: u32,
    pub timestamp: u64,
    pub thread_id: String,
}

impl DebugMessage {
    pub fn new(level: DebugLevel, message: String, file: String, line: u32) -> Self {
        Self {
            level,
            message,
            file,
            line,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            thread_id: format!("{:?}", std::thread::current().id()),
        }
    }

    pub fn format(&self) -> String {
        format!(
            "[{}] [{}] [{}:{}] [{}] {}",
            self.timestamp, self.level, self.file, self.line, self.thread_id, self.message
        )
    }
}

/// Debug output handler trait
pub trait DebugHandler: Send + Sync {
    fn handle_message(&self, message: &DebugMessage);
    fn flush(&self);
}

/// Console debug handler
pub struct ConsoleHandler {
    min_level: DebugLevel,
}

impl ConsoleHandler {
    pub fn new(min_level: DebugLevel) -> Self {
        Self { min_level }
    }
}

impl DebugHandler for ConsoleHandler {
    fn handle_message(&self, message: &DebugMessage) {
        if message.level >= self.min_level {
            println!("{}", message.format());
        }
    }

    fn flush(&self) {
        // Console output is typically auto-flushed
    }
}

/// File debug handler
pub struct FileHandler {
    min_level: DebugLevel,
    file_path: String,
}

impl FileHandler {
    pub fn new(min_level: DebugLevel, file_path: String) -> Self {
        Self {
            min_level,
            file_path,
        }
    }
}

impl DebugHandler for FileHandler {
    fn handle_message(&self, message: &DebugMessage) {
        if message.level >= self.min_level {
            // Mock file writing - in real implementation would write to file
            eprintln!("FILE[{}]: {}", self.file_path, message.format());
        }
    }

    fn flush(&self) {
        // Mock file flush
    }
}

/// Memory debug handler (keeps messages in memory)
pub struct MemoryHandler {
    messages: Arc<RwLock<VecDeque<DebugMessage>>>,
    max_messages: usize,
    min_level: DebugLevel,
}

impl MemoryHandler {
    pub fn new(min_level: DebugLevel, max_messages: usize) -> Self {
        Self {
            messages: Arc::new(RwLock::new(VecDeque::with_capacity(max_messages))),
            max_messages,
            min_level,
        }
    }

    pub fn get_messages(&self) -> Vec<DebugMessage> {
        if let Ok(messages) = self.messages.read() {
            messages.iter().cloned().collect()
        } else {
            Vec::new()
        }
    }

    pub fn clear_messages(&self) {
        if let Ok(mut messages) = self.messages.write() {
            messages.clear();
        }
    }
}

impl DebugHandler for MemoryHandler {
    fn handle_message(&self, message: &DebugMessage) {
        if message.level >= self.min_level {
            if let Ok(mut messages) = self.messages.write() {
                if messages.len() >= self.max_messages {
                    messages.pop_front();
                }
                messages.push_back(message.clone());
            }
        }
    }

    fn flush(&self) {
        // Memory handler doesn't need flushing
    }
}

/// Main debug system
pub struct DebugSystem {
    handlers: Vec<Arc<dyn DebugHandler>>,
    enabled: bool,
    min_level: DebugLevel,
}

impl DebugSystem {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            enabled: true,
            min_level: DebugLevel::Trace,
        }
    }

    pub fn add_handler(&mut self, handler: Arc<dyn DebugHandler>) {
        self.handlers.push(handler);
    }

    pub fn remove_all_handlers(&mut self) {
        self.handlers.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_min_level(&mut self, level: DebugLevel) {
        self.min_level = level;
    }

    pub fn log(&self, level: DebugLevel, message: String, file: String, line: u32) {
        if !self.enabled || level < self.min_level {
            return;
        }

        let debug_message = DebugMessage::new(level, message, file, line);

        for handler in &self.handlers {
            handler.handle_message(&debug_message);
        }
    }

    pub fn flush_all(&self) {
        for handler in &self.handlers {
            handler.flush();
        }
    }
}

impl Default for DebugSystem {
    fn default() -> Self {
        let mut system = Self::new();
        // Add default console handler
        system.add_handler(Arc::new(ConsoleHandler::new(DebugLevel::Debug)));
        system
    }
}

/// Performance profiler for timing code sections
pub struct Profiler {
    timings: Arc<Mutex<std::collections::HashMap<String, Vec<Duration>>>>,
}

impl Profiler {
    pub fn new() -> Self {
        Self {
            timings: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub fn start_timer(&self, name: &str) -> ProfilerTimer {
        ProfilerTimer::new(name.to_string(), self.timings.clone())
    }

    pub fn get_average_time(&self, name: &str) -> Option<Duration> {
        if let Ok(timings) = self.timings.lock() {
            if let Some(times) = timings.get(name) {
                if !times.is_empty() {
                    let total: Duration = times.iter().sum();
                    return Some(total / times.len() as u32);
                }
            }
        }
        None
    }

    pub fn get_total_time(&self, name: &str) -> Option<Duration> {
        if let Ok(timings) = self.timings.lock() {
            if let Some(times) = timings.get(name) {
                return Some(times.iter().sum());
            }
        }
        None
    }

    pub fn get_call_count(&self, name: &str) -> usize {
        if let Ok(timings) = self.timings.lock() {
            if let Some(times) = timings.get(name) {
                return times.len();
            }
        }
        0
    }

    pub fn clear_timings(&self) {
        if let Ok(mut timings) = self.timings.lock() {
            timings.clear();
        }
    }

    pub fn get_all_timings(&self) -> std::collections::HashMap<String, Vec<Duration>> {
        if let Ok(timings) = self.timings.lock() {
            timings.clone()
        } else {
            std::collections::HashMap::new()
        }
    }
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new()
    }
}

/// RAII timer for profiling
pub struct ProfilerTimer {
    name: String,
    start_time: Instant,
    timings: Arc<Mutex<std::collections::HashMap<String, Vec<Duration>>>>,
}

impl ProfilerTimer {
    fn new(
        name: String,
        timings: Arc<Mutex<std::collections::HashMap<String, Vec<Duration>>>>,
    ) -> Self {
        Self {
            name,
            start_time: Instant::now(),
            timings,
        }
    }
}

impl Drop for ProfilerTimer {
    fn drop(&mut self) {
        let elapsed = self.start_time.elapsed();
        if let Ok(mut timings) = self.timings.lock() {
            timings
                .entry(self.name.clone())
                .or_insert_with(Vec::new)
                .push(elapsed);
        }
    }
}

/// Assert helper functions
pub fn debug_assert_impl(condition: bool, message: &str, file: &str, line: u32) {
    if !condition {
        let debug_message = format!("Assertion failed: {}", message);
        if let Some(system) = get_debug_system() {
            if let Ok(debug_system) = system.lock() {
                debug_system.log(DebugLevel::Fatal, debug_message, file.to_string(), line);
            }
        }

        // In debug builds, panic
        #[cfg(debug_assertions)]
        panic!("Assertion failed at {}:{}: {}", file, line, message);
    }
}

/// Global debug system instance
static DEBUG_SYSTEM: OnceCell<Arc<Mutex<DebugSystem>>> = OnceCell::new();

/// Initialize the global debug system
pub fn init_debug_system() {
    if DEBUG_SYSTEM.get().is_none() {
        let _ = DEBUG_SYSTEM.set(Arc::new(Mutex::new(DebugSystem::default())));
    } else if let Some(system) = DEBUG_SYSTEM.get() {
        if let Ok(mut guard) = system.lock() {
            *guard = DebugSystem::default();
        }
    }
}

/// Get reference to the global debug system
pub fn get_debug_system() -> Option<Arc<Mutex<DebugSystem>>> {
    DEBUG_SYSTEM.get().cloned()
}

/// Global profiler instance
static PROFILER: OnceCell<Arc<Profiler>> = OnceCell::new();

/// Initialize the global profiler
pub fn init_profiler() {
    if PROFILER.get().is_none() {
        let _ = PROFILER.set(Arc::new(Profiler::new()));
    } else if let Some(profiler) = PROFILER.get() {
        profiler.clear_timings();
    }
}

/// Get reference to the global profiler
pub fn get_profiler() -> Option<Arc<Profiler>> {
    PROFILER.get().cloned()
}

/// Convenience macros for debug logging
#[macro_export]
macro_rules! debug_trace {
    ($($arg:tt)*) => {
        if let Some(system) = $crate::common::system::debug::get_debug_system() {
            if let Ok(system) = system.lock() {
                system.log(
                    $crate::common::system::debug::DebugLevel::Trace,
                    format!($($arg)*),
                    file!().to_string(),
                    line!()
                );
            }
        }
    };
}

#[macro_export]
macro_rules! debug_info {
    ($($arg:tt)*) => {
        if let Some(system) = $crate::common::system::debug::get_debug_system() {
            if let Ok(system) = system.lock() {
                system.log(
                    $crate::common::system::debug::DebugLevel::Info,
                    format!($($arg)*),
                    file!().to_string(),
                    line!()
                );
            }
        }
    };
}

#[macro_export]
macro_rules! debug_warn {
    ($($arg:tt)*) => {
        if let Some(system) = $crate::common::system::debug::get_debug_system() {
            if let Ok(system) = system.lock() {
                system.log(
                    $crate::common::system::debug::DebugLevel::Warning,
                    format!($($arg)*),
                    file!().to_string(),
                    line!()
                );
            }
        }
    };
}

#[macro_export]
macro_rules! debug_error {
    ($($arg:tt)*) => {
        if let Some(system) = $crate::common::system::debug::get_debug_system() {
            if let Ok(system) = system.lock() {
                system.log(
                    $crate::common::system::debug::DebugLevel::Error,
                    format!($($arg)*),
                    file!().to_string(),
                    line!()
                );
            }
        }
    };
}

#[macro_export]
macro_rules! debug_assert_crash {
    ($condition:expr, $($arg:tt)*) => {
        $crate::common::system::debug::debug_assert_impl(
            $condition,
            &format!($($arg)*),
            file!(),
            line!()
        )
    };
}

#[macro_export]
macro_rules! profile_scope {
    ($name:expr) => {
        let _timer = if let Some(profiler) = $crate::common::system::debug::get_profiler() {
            Some(profiler.start_timer($name))
        } else {
            None
        };
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_debug_message() {
        let message = DebugMessage::new(
            DebugLevel::Info,
            "Test message".to_string(),
            "test.rs".to_string(),
            42,
        );

        assert_eq!(message.level, DebugLevel::Info);
        assert_eq!(message.message, "Test message");
        assert_eq!(message.file, "test.rs");
        assert_eq!(message.line, 42);
        assert!(message.timestamp > 0);
    }

    #[test]
    fn test_console_handler() {
        let handler = ConsoleHandler::new(DebugLevel::Info);
        let message = DebugMessage::new(
            DebugLevel::Info,
            "Test".to_string(),
            "test.rs".to_string(),
            1,
        );

        // This should not panic
        handler.handle_message(&message);
        handler.flush();
    }

    #[test]
    fn test_memory_handler() {
        let handler = MemoryHandler::new(DebugLevel::Debug, 10);
        let message = DebugMessage::new(
            DebugLevel::Info,
            "Test".to_string(),
            "test.rs".to_string(),
            1,
        );

        handler.handle_message(&message);
        let messages = handler.get_messages();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message, "Test");

        handler.clear_messages();
        let messages = handler.get_messages();
        assert_eq!(messages.len(), 0);
    }

    #[test]
    fn test_debug_system() {
        let mut system = DebugSystem::new();
        let memory_handler = Arc::new(MemoryHandler::new(DebugLevel::Debug, 10));
        let memory_handler_clone = memory_handler.clone();

        system.add_handler(memory_handler);
        system.log(
            DebugLevel::Info,
            "Test message".to_string(),
            "test.rs".to_string(),
            1,
        );

        let messages = memory_handler_clone.get_messages();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message, "Test message");
    }

    #[test]
    fn test_profiler() {
        let profiler = Profiler::new();

        {
            let _timer = profiler.start_timer("test_operation");
            thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(profiler.get_call_count("test_operation"), 1);
        let avg_time = profiler.get_average_time("test_operation").unwrap();
        assert!(avg_time >= Duration::from_millis(9)); // Account for timing variations
    }

    #[test]
    fn test_debug_levels() {
        assert!(DebugLevel::Error > DebugLevel::Warning);
        assert!(DebugLevel::Warning > DebugLevel::Info);
        assert!(DebugLevel::Info > DebugLevel::Debug);
        assert!(DebugLevel::Debug > DebugLevel::Trace);
    }
}
