//! # Profile Module - Rust Implementation
//!
//! This is a Rust conversion of the C++ profiling library from Command & Conquer Generals Zero Hour.
//!
//! The profile module provides:
//! - High level hierarchical timer and logical profiling (frame based and global)
//! - Function level hierarchical timer based profiling (frame based and global)
//! - Pattern-based enabling/disabling of profiling ranges
//! - Result output functions for generating profile reports
//!
//! ## Usage
//!
//! ```rust
//! use profile_rust::{Profile, ProfileHighLevel};
//!
//! // Start a profiling range
//! Profile::start_range(Some("my_range"));
//!
//! // Do some work...
//! {
//!     let _block = ProfileHighLevel::block("expensive_operation");
//!     // ... work happens here
//! }
//!
//! // Stop the range
//! Profile::stop_range(Some("my_range"));
//! ```

use once_cell::sync::{Lazy, OnceCell};
use parking_lot::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use thiserror::Error;

pub mod cmd;
pub mod func_level;
pub mod high_level;
pub mod memory;
pub mod pattern;
pub mod result;
pub mod timing;

use func_level::ProfileFuncLevel;
pub mod _pch;
pub mod debug_cmd;
pub mod debug_debug;
pub mod debug_dlg;
pub mod debug_doc;
pub mod debug_except;
pub mod debug_getdefaultcommands;
pub mod debug_internal;
pub mod debug_io;
pub mod debug_io_con;
pub mod debug_io_flat;
pub mod debug_io_net;
pub mod debug_io_ods;
pub mod debug_macro;
pub mod debug_purecall;
pub mod debug_stack;
pub mod internal;
pub mod internal_cmd;
pub mod internal_except;
pub mod internal_funclevel;
pub mod internal_highlevel;
pub mod internal_io;
pub mod internal_result;
pub mod netserv;
pub mod profile_cmd;
pub mod profile_doc;
pub mod profile_funclevel;
pub mod profile_highlevel;
pub mod profile_result;
pub mod rc_exception;
pub mod test2;
pub mod test2_stdafx;
pub mod test3;
pub mod test4;
pub mod test5;
pub mod test6;
pub mod wwdebug;
pub mod wwhack;
pub mod wwmemlog;
pub mod wwprofile;
pub use high_level::ProfileHighLevel;
use pattern::PatternMatcher;
use result::ProfileResultInterface;
use timing::ProfileTimer;

/// Error types for the profile system
#[derive(Error, Debug)]
pub enum ProfileError {
    #[error("Range '{0}' not found")]
    RangeNotFound(String),
    #[error("Memory allocation failed")]
    MemoryAllocation,
    #[error("Pattern matching error: {0}")]
    PatternError(String),
    #[error("Clock initialization failed")]
    ClockError,
}

/// Result type for profile operations
pub type ProfileResult<T> = std::result::Result<T, ProfileError>;

/// Pattern list entry for enabling/disabling profiling ranges
#[derive(Debug, Clone)]
struct PatternListEntry {
    is_active: bool,
    pattern: String,
}

/// Frame name tracking information
#[derive(Debug, Clone)]
struct FrameName {
    name: String,
    frames: usize,
    is_recording: bool,
    do_append: bool,
    func_index: Option<i32>,
    high_index: Option<i32>,
    last_global_index: Option<i32>,
}

/// Global profiler state
struct ProfilerState {
    /// Recorded frame count
    rec_count: AtomicUsize,
    /// Recorded frame names
    rec_names: Mutex<Vec<String>>,
    /// Known frame names
    frame_names: RwLock<Vec<FrameName>>,
    /// Pattern list for enabling/disabling ranges
    pattern_list: RwLock<Vec<PatternListEntry>>,
    /// Result functions to execute on shutdown
    result_functions: Mutex<Vec<Box<dyn ProfileResultInterface>>>,
    /// High-level profiler instance
    high_level: ProfileHighLevel,
    /// Function-level profiler instance
    func_level: ProfileFuncLevel,
    /// CPU clock cycles per second (cached)
    clock_cycles_per_second: OnceCell<u64>,
}

/// Global profiler state instance
static PROFILER_STATE: Lazy<ProfilerState> = Lazy::new(|| ProfilerState {
    rec_count: AtomicUsize::new(0),
    rec_names: Mutex::new(Vec::new()),
    frame_names: RwLock::new(Vec::new()),
    pattern_list: RwLock::new(Vec::new()),
    result_functions: Mutex::new(Vec::new()),
    high_level: ProfileHighLevel::new(),
    func_level: ProfileFuncLevel::new(),
    clock_cycles_per_second: OnceCell::new(),
});

/// Main Profile class - equivalent to C++ Profile static class
pub struct Profile;

impl Profile {
    /// Starts range recording.
    ///
    /// # Arguments
    /// * `range` - Name of range to record, None for "frame"
    pub fn start_range(range: Option<&str>) -> ProfileResult<()> {
        let range_name = range.unwrap_or("frame");
        let state = &*PROFILER_STATE;

        // Find or create the frame name entry
        let mut frame_names = state.frame_names.write();
        let frame_idx = match frame_names.iter().position(|f| f.name == range_name) {
            Some(idx) => idx,
            None => {
                frame_names.push(FrameName {
                    name: range_name.to_string(),
                    frames: 0,
                    is_recording: false,
                    do_append: false,
                    func_index: None,
                    high_index: None,
                    last_global_index: None,
                });
                frame_names.len() - 1
            }
        };

        // Stop old recording if already recording
        if frame_names[frame_idx].is_recording {
            drop(frame_names);
            Self::stop_range(Some(range_name))?;
            frame_names = state.frame_names.write();
        }

        // Start new recording
        frame_names[frame_idx].is_recording = true;
        frame_names[frame_idx].do_append = false;

        // Check if recording is enabled by pattern matching
        let pattern_list = state.pattern_list.read();
        let mut active = false;
        for pattern_entry in pattern_list.iter() {
            if PatternMatcher::simple_match(range_name, &pattern_entry.pattern) {
                active = pattern_entry.is_active;
            }
        }
        drop(pattern_list);

        if active {
            // Start function level profiling if enabled
            #[cfg(feature = "function-level")]
            {
                frame_names[frame_idx].func_index = Some(state.func_level.frame_start()?);
            }

            // Start high level profiling
            frame_names[frame_idx].high_index = Some(state.high_level.frame_start()?);
        } else {
            frame_names[frame_idx].func_index = None;
            frame_names[frame_idx].high_index = None;
        }

        Ok(())
    }

    /// Appends profile data to the last recorded frame of the given range.
    ///
    /// # Arguments
    /// * `range` - Name of range to record, None for "frame"
    pub fn append_range(range: Option<&str>) -> ProfileResult<()> {
        let range_name = range.unwrap_or("frame");
        let state = &*PROFILER_STATE;

        let mut frame_names = state.frame_names.write();
        let frame_idx = match frame_names.iter().position(|f| f.name == range_name) {
            Some(idx) => idx,
            None => {
                // Range doesn't exist, so StartRange will handle it
                drop(frame_names);
                return Self::start_range(Some(range_name));
            }
        };

        // If still recording, don't do anything
        if frame_names[frame_idx].is_recording {
            return Ok(());
        }

        // Start new recording
        frame_names[frame_idx].is_recording = true;
        frame_names[frame_idx].do_append = true;

        // Check if recording is enabled by pattern matching
        let pattern_list = state.pattern_list.read();
        let mut active = false;
        for pattern_entry in pattern_list.iter() {
            if PatternMatcher::simple_match(range_name, &pattern_entry.pattern) {
                active = pattern_entry.is_active;
            }
        }
        drop(pattern_list);

        if active {
            // Start function level profiling if enabled
            #[cfg(feature = "function-level")]
            {
                frame_names[frame_idx].func_index = Some(state.func_level.frame_start()?);
            }

            // Start high level profiling
            frame_names[frame_idx].high_index = Some(state.high_level.frame_start()?);
        } else {
            frame_names[frame_idx].func_index = None;
            frame_names[frame_idx].high_index = None;
        }

        Ok(())
    }

    /// Stops range recording.
    /// After this call the recorded range data will be available as a new range frame.
    ///
    /// # Arguments
    /// * `range` - Name of range to record, None for "frame"
    pub fn stop_range(range: Option<&str>) -> ProfileResult<()> {
        let range_name = range.unwrap_or("frame");
        let state = &*PROFILER_STATE;

        let mut frame_names = state.frame_names.write();
        let frame_idx = match frame_names.iter().position(|f| f.name == range_name) {
            Some(idx) => idx,
            None => return Err(ProfileError::RangeNotFound(range_name.to_string())),
        };

        if !frame_names[frame_idx].is_recording {
            return Ok(()); // Not recording, nothing to do
        }

        // Stop recording
        frame_names[frame_idx].is_recording = false;

        let has_active_profiling = frame_names[frame_idx].func_index.is_some()
            || frame_names[frame_idx].high_index.is_some();

        if has_active_profiling {
            let at_index = if !frame_names[frame_idx].do_append
                || frame_names[frame_idx].last_global_index.is_none()
            {
                // Create new frame record
                frame_names[frame_idx].frames += 1;
                let global_index = state.rec_count.load(Ordering::Relaxed);
                frame_names[frame_idx].last_global_index = Some(global_index as i32);

                let frame_name = format!("{}:{}", range_name, frame_names[frame_idx].frames);
                let mut rec_names = state.rec_names.lock().unwrap();
                rec_names.push(frame_name);
                state.rec_count.store(rec_names.len(), Ordering::Relaxed);

                None // New frame
            } else {
                frame_names[frame_idx].last_global_index // Append to existing frame
            };

            // End function level profiling if it was started
            #[cfg(feature = "function-level")]
            if let Some(func_index) = frame_names[frame_idx].func_index {
                state.func_level.frame_end(func_index, at_index)?;
            }

            // End high level profiling if it was started
            if let Some(high_index) = frame_names[frame_idx].high_index {
                state.high_level.frame_end(high_index, at_index)?;
            }
        }

        Ok(())
    }

    /// Determines if any range recording is enabled or not.
    ///
    /// # Returns
    /// `true` if range profiling is enabled, `false` if not
    pub fn is_enabled() -> bool {
        let state = &*PROFILER_STATE;
        let frame_names = state.frame_names.read();
        frame_names.iter().any(|f| f.is_recording)
    }

    /// Determines the number of known (recorded) range frames.
    ///
    /// # Returns
    /// Number of recorded range frames
    pub fn get_frame_count() -> usize {
        PROFILER_STATE.rec_count.load(Ordering::Relaxed)
    }

    /// Determines the range name of a recorded range frame.
    /// A unique number will be added to the frame name, separated by a ':', e.g. 'frame:3'
    ///
    /// # Arguments
    /// * `frame` - Number of recorded frame
    ///
    /// # Returns
    /// Range name, or None if frame not found
    pub fn get_frame_name(frame: usize) -> Option<String> {
        let state = &*PROFILER_STATE;
        let rec_names = state.rec_names.lock().unwrap();
        rec_names.get(frame).cloned()
    }

    /// Resets all 'total' counter values to 0.
    /// This function does not change any recorded frames.
    pub fn clear_totals() {
        let state = &*PROFILER_STATE;
        state.high_level.clear_totals();
        #[cfg(feature = "function-level")]
        state.func_level.clear_totals();
    }

    /// Determines number of CPU clock cycles per second.
    /// This value is cached internally so this function is quite fast.
    ///
    /// # Returns
    /// Number of CPU clock cycles per second
    pub fn get_clock_cycles_per_second() -> ProfileResult<u64> {
        let state = &*PROFILER_STATE;
        match state.clock_cycles_per_second.get() {
            Some(&cycles) => Ok(cycles),
            None => {
                let cycles = ProfileTimer::measure_cpu_frequency()?;
                match state.clock_cycles_per_second.set(cycles) {
                    Ok(()) => Ok(cycles),
                    Err(_) => Ok(*state
                        .clock_cycles_per_second
                        .get()
                        .expect("clock frequency set concurrently")),
                }
            }
        }
    }

    /// Add the given result function interface.
    ///
    /// # Arguments
    /// * `result_fn` - Result function to add
    pub fn add_result_function(result_fn: Box<dyn ProfileResultInterface>) {
        let state = &*PROFILER_STATE;
        let mut result_functions = state.result_functions.lock().unwrap();
        result_functions.push(result_fn);
    }

    /// Add a pattern to enable/disable profiling ranges
    ///
    /// # Arguments
    /// * `pattern` - Pattern to match against range names (supports '*' wildcard)
    /// * `active` - Whether matched ranges should be active or inactive
    pub fn add_pattern(pattern: &str, active: bool) -> ProfileResult<()> {
        let state = &*PROFILER_STATE;
        let mut pattern_list = state.pattern_list.write();
        pattern_list.push(PatternListEntry {
            is_active: active,
            pattern: pattern.to_string(),
        });
        Ok(())
    }

    /// Clear all patterns
    pub fn clear_patterns() {
        let state = &*PROFILER_STATE;
        let mut pattern_list = state.pattern_list.write();
        pattern_list.clear();
    }

    /// Clear patterns matching the provided pattern (supports '*' wildcard)
    pub fn clear_patterns_matching(pattern: &str) {
        let state = &*PROFILER_STATE;
        let mut pattern_list = state.pattern_list.write();
        pattern_list.retain(|entry| !PatternMatcher::simple_match(&entry.pattern, pattern));
    }

    /// Get a snapshot of the current pattern list
    pub fn get_patterns() -> Vec<(bool, String)> {
        let state = &*PROFILER_STATE;
        let pattern_list = state.pattern_list.read();
        pattern_list
            .iter()
            .map(|entry| (entry.is_active, entry.pattern.clone()))
            .collect()
    }

    /// Get access to the high-level profiler
    pub fn high_level() -> &'static ProfileHighLevel {
        &PROFILER_STATE.high_level
    }

    /// Get access to the function-level profiler
    #[cfg(feature = "function-level")]
    pub fn func_level() -> &'static ProfileFuncLevel {
        &PROFILER_STATE.func_level
    }

    /// Execute all registered result functions (typically called on shutdown)
    pub fn execute_result_functions() {
        let state = &*PROFILER_STATE;
        let mut result_functions = state.result_functions.lock().unwrap();
        for result_fn in result_functions.drain(..) {
            result_fn.write_results();
        }
    }

    /// Compatibility helper for legacy examples/tests.
    pub fn write_results() {
        Self::execute_result_functions();
    }
}

/// Convenience macro for creating a profiling block that automatically ends when it goes out of scope
#[macro_export]
macro_rules! profile_block {
    ($name:expr) => {
        let _profile_block = $crate::ProfileHighLevel::block($name);
    };
}

/// Convenience macro for profiling a range
#[macro_export]
macro_rules! profile_range {
    ($name:expr, $code:block) => {{
        $crate::Profile::start_range(Some($name)).ok();
        let result = $code;
        $crate::Profile::stop_range(Some($name)).ok();
        result
    }};
}

// Automatic shutdown handling
static SHUTDOWN_REGISTERED: std::sync::Once = std::sync::Once::new();

/// Register automatic shutdown to execute result functions
fn register_shutdown() {
    SHUTDOWN_REGISTERED.call_once(|| {
        extern "C" fn shutdown_handler() {
            log::info!(
                "CPU speed is {} Hz",
                Profile::get_clock_cycles_per_second().unwrap_or(0)
            );
            crate::cmd::ProfileCmdInterface::run_result_functions();
        }

        unsafe {
            libc::atexit(shutdown_handler);
        }
    });
}

/// Initialize the profiler (called automatically via lazy initialization)
pub fn init() {
    Lazy::force(&PROFILER_STATE);
    register_shutdown();
}

// Re-export main types
pub use cmd::{
    execute_command_to_string, execute_command_with_stdout, CommandMode, ProfileCommandExecutor,
    ProfileCommandParser,
};
#[cfg(feature = "function-level")]
pub use func_level::ProfileFuncId;
pub use high_level::ProfileId as HighLevelId;

#[cfg(test)]
pub(crate) fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .expect("profile test lock")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_basic_range_recording() {
        let _lock = crate::test_lock();
        Profile::clear_patterns();
        Profile::add_pattern("test*", true).unwrap();

        let start_frame = Profile::get_frame_count();
        assert!(!Profile::is_enabled());

        Profile::start_range(Some("test_range")).unwrap();
        assert!(Profile::is_enabled());

        thread::sleep(Duration::from_millis(10));

        Profile::stop_range(Some("test_range")).unwrap();
        assert!(!Profile::is_enabled());

        assert_eq!(Profile::get_frame_count(), start_frame + 1);
        let frame_name = Profile::get_frame_name(start_frame).unwrap();
        assert_eq!(frame_name, "test_range:1");
    }

    #[test]
    fn test_pattern_matching() {
        let _lock = crate::test_lock();
        Profile::clear_patterns();

        // Start with no patterns - should be inactive
        Profile::start_range(Some("inactive_range")).unwrap();
        Profile::stop_range(Some("inactive_range")).unwrap();

        // Add active pattern
        Profile::add_pattern("active*", true).unwrap();
        Profile::start_range(Some("active_range")).unwrap();
        assert!(Profile::is_enabled());
        Profile::stop_range(Some("active_range")).unwrap();
    }

    #[test]
    fn test_high_level_profiling() {
        let _lock = crate::test_lock();
        let high_level = Profile::high_level();

        let id = high_level
            .add_profile("test.counter", "Test counter", "count", 0, 0)
            .unwrap();

        id.increment(1.0);
        id.increment(2.0);

        // The specific value depends on implementation, but we can test that it doesn't panic
        let _name = id.get_name();
        let _desc = id.get_description();
        let _unit = id.get_unit();
    }

    #[test]
    fn test_cpu_frequency_measurement() {
        let _lock = crate::test_lock();
        let freq = Profile::get_clock_cycles_per_second().unwrap();
        assert!(freq > 1000000); // Should be at least 1 MHz

        // Second call should return cached value
        let freq2 = Profile::get_clock_cycles_per_second().unwrap();
        assert_eq!(freq, freq2);
    }

    #[test]
    fn test_macros() {
        let _lock = crate::test_lock();
        Profile::clear_patterns();
        Profile::add_pattern("macro*", true).unwrap();
        let start_frame = Profile::get_frame_count();

        let result = profile_range!("macro_test", {
            thread::sleep(Duration::from_millis(1));
            42
        });

        assert_eq!(result, 42);
        assert_eq!(Profile::get_frame_count(), start_frame + 1);
    }
}
