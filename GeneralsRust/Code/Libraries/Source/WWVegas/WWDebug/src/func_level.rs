//! Function Level Profiling Module
//!
//! Equivalent to the C++ ProfileFuncLevel class, provides function-level
//! call statistics and timing information.

use crate::timing::ProfileTimer;
use crate::ProfileResult;
use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, ThreadId};

/// Function-level profile ID - equivalent to ProfileFuncLevel::Id
#[derive(Debug, Clone)]
pub struct ProfileFuncId {
    inner: Arc<ProfileFuncIdInner>,
}

#[derive(Debug)]
struct ProfileFuncIdInner {
    source_file: Option<String>,
    function_name: Option<String>,
    line_number: u32,
    address: usize,
    call_counts: RwLock<HashMap<u32, u64>>, // frame -> count
    total_times: RwLock<HashMap<u32, u64>>, // frame -> time (including children)
    function_times: RwLock<HashMap<u32, u64>>, // frame -> time (excluding children)
    caller_lists: RwLock<HashMap<u32, Vec<(ProfileFuncId, u32)>>>, // frame -> [(caller, count)]
    total_calls: AtomicU64,
    total_time: AtomicU64,
    total_function_time: AtomicU64,
}

impl ProfileFuncId {
    fn new(
        source_file: Option<String>,
        function_name: Option<String>,
        line_number: u32,
        address: usize,
    ) -> Self {
        Self {
            inner: Arc::new(ProfileFuncIdInner {
                source_file,
                function_name,
                line_number,
                address,
                call_counts: RwLock::new(HashMap::new()),
                total_times: RwLock::new(HashMap::new()),
                function_times: RwLock::new(HashMap::new()),
                caller_lists: RwLock::new(HashMap::new()),
                total_calls: AtomicU64::new(0),
                total_time: AtomicU64::new(0),
                total_function_time: AtomicU64::new(0),
            }),
        }
    }

    /// Get the source file this ID is in
    pub fn get_source(&self) -> Option<&str> {
        self.inner.source_file.as_deref()
    }

    /// Get the function name for this ID
    pub fn get_function(&self) -> Option<&str> {
        self.inner.function_name.as_deref()
    }

    /// Get the function address
    pub fn get_address(&self) -> usize {
        self.inner.address
    }

    /// Get the line number for this ID
    pub fn get_line(&self) -> u32 {
        self.inner.line_number
    }

    /// Get call count for a specific frame or total
    pub fn get_calls(&self, frame: u32) -> u64 {
        if frame == Self::TOTAL {
            self.inner.total_calls.load(Ordering::Relaxed)
        } else {
            let call_counts = self.inner.call_counts.read();
            call_counts.get(&frame).copied().unwrap_or(0)
        }
    }

    /// Get time spent in this function and its children
    pub fn get_time(&self, frame: u32) -> u64 {
        if frame == Self::TOTAL {
            self.inner.total_time.load(Ordering::Relaxed)
        } else {
            let total_times = self.inner.total_times.read();
            total_times.get(&frame).copied().unwrap_or(0)
        }
    }

    /// Get time spent in this function only (excluding children)
    pub fn get_function_time(&self, frame: u32) -> u64 {
        if frame == Self::TOTAL {
            self.inner.total_function_time.load(Ordering::Relaxed)
        } else {
            let function_times = self.inner.function_times.read();
            function_times.get(&frame).copied().unwrap_or(0)
        }
    }

    /// Get the list of caller IDs for a specific frame
    pub fn get_caller(&self, frame: u32) -> ProfileFuncIdList {
        let caller_lists = self.inner.caller_lists.read();
        let callers = if frame == Self::TOTAL {
            // Aggregate all frames
            let mut all_callers = HashMap::new();
            for frame_callers in caller_lists.values() {
                for (caller, count) in frame_callers {
                    *all_callers.entry(caller.clone()).or_insert(0) += count;
                }
            }
            all_callers.into_iter().collect()
        } else {
            caller_lists.get(&frame).cloned().unwrap_or_default()
        };

        ProfileFuncIdList { callers }
    }

    /// Record a function call (internal)
    pub(crate) fn record_call(
        &self,
        frame: Option<u32>,
        total_time: u64,
        function_time: u64,
        caller: Option<ProfileFuncId>,
    ) {
        // Update totals
        self.inner.total_calls.fetch_add(1, Ordering::Relaxed);
        self.inner
            .total_time
            .fetch_add(total_time, Ordering::Relaxed);
        self.inner
            .total_function_time
            .fetch_add(function_time, Ordering::Relaxed);

        // Update frame-specific data if frame is provided
        if let Some(frame_num) = frame {
            {
                let mut call_counts = self.inner.call_counts.write();
                *call_counts.entry(frame_num).or_insert(0) += 1;
            }

            {
                let mut total_times = self.inner.total_times.write();
                *total_times.entry(frame_num).or_insert(0) += total_time;
            }

            {
                let mut function_times = self.inner.function_times.write();
                *function_times.entry(frame_num).or_insert(0) += function_time;
            }

            // Record caller information if provided
            if let Some(caller_id) = caller {
                let mut caller_lists = self.inner.caller_lists.write();
                let frame_callers = caller_lists.entry(frame_num).or_insert_with(Vec::new);

                // Find existing caller or add new one
                if let Some(pos) = frame_callers
                    .iter()
                    .position(|(c, _)| c.inner.address == caller_id.inner.address)
                {
                    frame_callers[pos].1 += 1;
                } else {
                    frame_callers.push((caller_id, 1));
                }
            }
        }
    }

    /// Clear totals
    pub(crate) fn clear_totals(&self) {
        self.inner.total_calls.store(0, Ordering::Relaxed);
        self.inner.total_time.store(0, Ordering::Relaxed);
        self.inner.total_function_time.store(0, Ordering::Relaxed);
    }

    /// Special frame number for totals
    pub const TOTAL: u32 = 0xffffffff;
}

// Implement Hash and Eq based on address for ProfileFuncId
impl std::hash::Hash for ProfileFuncId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.address.hash(state);
    }
}

impl PartialEq for ProfileFuncId {
    fn eq(&self, other: &Self) -> bool {
        self.inner.address == other.inner.address
    }
}

impl Eq for ProfileFuncId {}

/// List of function level profile IDs - equivalent to ProfileFuncLevel::IdList
pub struct ProfileFuncIdList {
    callers: Vec<(ProfileFuncId, u32)>,
}

impl ProfileFuncIdList {
    /// Enumerate the list of IDs
    pub fn enumerate(&self, index: usize) -> Option<(ProfileFuncId, Option<u32>)> {
        self.callers
            .get(index)
            .map(|(id, count)| (id.clone(), Some(*count)))
    }

    /// Get the count of IDs in this list
    pub fn len(&self) -> usize {
        self.callers.len()
    }

    /// Check if the list is empty
    pub fn is_empty(&self) -> bool {
        self.callers.is_empty()
    }
}

/// Thread tracking for function-level profiling - equivalent to ProfileFuncLevel::Thread
#[derive(Debug, Clone)]
pub struct ProfileFuncThread {
    thread_id: ThreadId,
    profile_id: usize,
    functions: Arc<RwLock<HashMap<usize, ProfileFuncId>>>,
}

impl ProfileFuncThread {
    fn new(thread_id: ThreadId, profile_id: usize) -> Self {
        Self {
            thread_id,
            profile_id,
            functions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Enumerate known function level profile values for this thread
    pub fn enum_profile(&self, index: usize) -> Option<ProfileFuncId> {
        let functions = self.functions.read();
        functions.values().nth(index).cloned()
    }

    /// Get unique thread ID
    pub fn get_id(&self) -> usize {
        self.profile_id
    }

    /// Add a function to this thread's profile (internal)
    pub(crate) fn add_function(&self, address: usize, func_id: ProfileFuncId) {
        let mut functions = self.functions.write();
        functions.insert(address, func_id);
    }

    /// Get function by address (internal)
    pub(crate) fn get_function(&self, address: usize) -> Option<ProfileFuncId> {
        let functions = self.functions.read();
        functions.get(&address).cloned()
    }
}

/// Function call stack entry for tracking nested calls
#[derive(Debug)]
struct CallStackEntry {
    func_id: ProfileFuncId,
    start_time: u64,
    child_time: u64, // Time spent in child functions
}

/// Per-thread call stack
struct ThreadCallStack {
    stack: Vec<CallStackEntry>,
    caller_tracking_enabled: bool,
}

impl ThreadCallStack {
    fn new() -> Self {
        Self {
            stack: Vec::new(),
            caller_tracking_enabled: false,
        }
    }

    fn push(&mut self, func_id: ProfileFuncId, start_time: u64) {
        self.stack.push(CallStackEntry {
            func_id,
            start_time,
            child_time: 0,
        });
    }

    fn pop(&mut self, end_time: u64, frame: Option<u32>) -> Option<()> {
        let entry = self.stack.pop()?;

        let total_time = end_time.saturating_sub(entry.start_time);
        let function_time = total_time.saturating_sub(entry.child_time);

        // Get caller if caller tracking is enabled
        let caller = if self.caller_tracking_enabled {
            self.stack.last().map(|parent| parent.func_id.clone())
        } else {
            None
        };

        // Record the call
        entry
            .func_id
            .record_call(frame, total_time, function_time, caller);

        // Add this call's total time to parent's child time
        if let Some(parent) = self.stack.last_mut() {
            parent.child_time += total_time;
        }

        Some(())
    }
}

/// Function level profiler - equivalent to ProfileFuncLevel class
pub struct ProfileFuncLevel {
    threads: DashMap<ThreadId, ProfileFuncThread>,
    thread_stacks: DashMap<ThreadId, Mutex<ThreadCallStack>>,
    next_thread_id: AtomicUsize,
    caller_tracking_enabled: AtomicBool,
}

impl ProfileFuncLevel {
    pub fn new() -> Self {
        Self {
            threads: DashMap::new(),
            thread_stacks: DashMap::new(),
            next_thread_id: AtomicUsize::new(0),
            caller_tracking_enabled: AtomicBool::new(false),
        }
    }

    /// Enable or disable caller tracking
    pub fn set_caller_tracking(&self, enabled: bool) {
        self.caller_tracking_enabled
            .store(enabled, Ordering::Relaxed);

        // Update existing thread stacks
        for mut entry in self.thread_stacks.iter_mut() {
            if let Ok(mut stack) = entry.value().lock() {
                stack.caller_tracking_enabled = enabled;
            }
        }
    }

    /// Check if caller tracking is enabled
    pub fn is_caller_tracking_enabled(&self) -> bool {
        self.caller_tracking_enabled.load(Ordering::Relaxed)
    }

    /// Enumerate known and profiled threads
    pub fn enum_threads(&self, index: usize) -> Option<ProfileFuncThread> {
        self.threads
            .iter()
            .nth(index)
            .map(|entry| entry.value().clone())
    }

    /// Get thread count
    pub fn get_thread_count(&self) -> usize {
        self.threads.len()
    }

    /// Register a function entry (would be called by _penter hook in C++)
    pub fn function_enter(
        &self,
        address: usize,
        source_file: Option<&str>,
        function_name: Option<&str>,
        line: u32,
    ) -> ProfileResult<()> {
        let thread_id = thread::current().id();

        // Get or create thread profile
        let thread_profile = self.get_or_create_thread(thread_id);

        // Get or create function ID
        let func_id = match thread_profile.get_function(address) {
            Some(existing) => existing,
            None => {
                let new_func = ProfileFuncId::new(
                    source_file.map(|s| s.to_string()),
                    function_name.map(|s| s.to_string()),
                    line,
                    address,
                );
                thread_profile.add_function(address, new_func.clone());
                new_func
            }
        };

        // Get call stack for this thread
        let call_stack = self.thread_stacks.entry(thread_id).or_insert_with(|| {
            let mut stack = Mutex::new(ThreadCallStack::new());
            if let Ok(mut guard) = stack.lock() {
                guard.caller_tracking_enabled =
                    self.caller_tracking_enabled.load(Ordering::Relaxed);
            }
            stack
        });

        // Record function entry
        let start_time = ProfileTimer::get_cpu_cycles()?;
        if let Ok(mut stack) = call_stack.lock() {
            stack.push(func_id, start_time);
        }

        Ok(())
    }

    /// Register a function exit (would be called by _pexit hook in C++)
    pub fn function_exit(&self, frame: Option<u32>) -> ProfileResult<()> {
        let thread_id = thread::current().id();

        if let Some(call_stack) = self.thread_stacks.get(&thread_id) {
            let end_time = ProfileTimer::get_cpu_cycles()?;
            if let Ok(mut stack) = call_stack.lock() {
                stack.pop(end_time, frame);
            }
        }

        Ok(())
    }

    /// Clear all totals
    pub fn clear_totals(&self) {
        for thread_entry in self.threads.iter() {
            let functions = thread_entry.functions.read();
            for func_id in functions.values() {
                func_id.clear_totals();
            }
        }
    }

    /// Frame start (internal)
    pub(crate) fn frame_start(&self) -> ProfileResult<i32> {
        // In Rust implementation, we don't need explicit frame tracking
        // as the original C++ version did
        Ok(0) // Dummy index
    }

    /// Frame end (internal)
    pub(crate) fn frame_end(&self, _index: i32, _frame: Option<i32>) -> ProfileResult<()> {
        // Frame end handling is done in function_exit calls
        Ok(())
    }

    /// Get or create a thread profile
    fn get_or_create_thread(&self, thread_id: ThreadId) -> ProfileFuncThread {
        if let Some(existing) = self.threads.get(&thread_id) {
            existing.clone()
        } else {
            let profile_id = self.next_thread_id.fetch_add(1, Ordering::Relaxed);
            let thread_profile = ProfileFuncThread::new(thread_id, profile_id);

            self.threads.insert(thread_id, thread_profile.clone());
            thread_profile
        }
    }

    /// Shutdown function-level profiling
    pub fn shutdown(&self) {
        // Clear all data
        self.threads.clear();
        self.thread_stacks.clear();
    }
}

impl Default for ProfileFuncLevel {
    fn default() -> Self {
        Self::new()
    }
}

/// Automatic function profiling helper
/// Use this in functions you want to profile manually
pub struct FunctionProfiler {
    _address: usize,
}

impl FunctionProfiler {
    /// Create a new function profiler
    pub fn new(source_file: &str, function_name: &str, line_param: u32) -> ProfileResult<Self> {
        let address = Self::get_return_address();

        #[cfg(feature = "function-level")]
        {
            let state = &*crate::PROFILER_STATE;
            state.func_level.function_enter(
                address,
                Some(source_file),
                Some(function_name),
                line_param,
            )?;
        }

        Ok(Self { _address: address })
    }

    /// Get approximate return address for profiling
    /// This is a simplified version - real implementation would need inline assembly
    fn get_return_address() -> usize {
        // In a real implementation, this would use inline assembly to get the return address
        // For now, we use the address of this function as a placeholder
        Self::get_return_address as *const () as usize
    }
}

impl Drop for FunctionProfiler {
    fn drop(&mut self) {
        #[cfg(feature = "function-level")]
        {
            let state = &*crate::PROFILER_STATE;
            let _ = state.func_level.function_exit(None);
        }
    }
}

/// Macro to automatically profile a function
#[macro_export]
macro_rules! profile_function {
    () => {
        let _func_profiler = $crate::func_level::FunctionProfiler::new(
            file!(),
            concat!(module_path!(), "::", line!()),
            line!(),
        )
        .ok();
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_function_profiling() {
        let func_level = ProfileFuncLevel::new();

        // Enable caller tracking
        func_level.set_caller_tracking(true);
        assert!(func_level.is_caller_tracking_enabled());

        // Simulate function calls
        let address1 = 0x1000;
        let address2 = 0x2000;

        func_level
            .function_enter(address1, Some("test.rs"), Some("test_func"), 42)
            .unwrap();
        thread::sleep(Duration::from_millis(1));

        func_level
            .function_enter(address2, Some("test.rs"), Some("inner_func"), 100)
            .unwrap();
        thread::sleep(Duration::from_millis(1));
        func_level.function_exit(Some(1)).unwrap();

        func_level.function_exit(Some(1)).unwrap();

        // Check that we have one thread
        assert_eq!(func_level.get_thread_count(), 1);

        let thread_profile = func_level.enum_threads(0).unwrap();
        assert_eq!(thread_profile.get_id(), 0);

        // Check that functions were recorded
        let func1 = thread_profile.get_function(address1).unwrap();
        assert_eq!(func1.get_function(), Some("test_func"));
        assert_eq!(func1.get_line(), 42);
        assert_eq!(func1.get_calls(ProfileFuncId::TOTAL), 1);

        let func2 = thread_profile.get_function(address2).unwrap();
        assert_eq!(func2.get_function(), Some("inner_func"));
        assert_eq!(func2.get_line(), 100);
        assert_eq!(func2.get_calls(ProfileFuncId::TOTAL), 1);

        // Check caller relationship
        let callers = func2.get_caller(ProfileFuncId::TOTAL);
        assert_eq!(callers.len(), 1);

        let (caller, count) = callers.enumerate(0).unwrap();
        assert_eq!(caller.get_address(), address1);
        assert_eq!(count, Some(1));
    }

    #[test]
    fn test_multiple_threads() {
        let func_level = Arc::new(ProfileFuncLevel::new());

        let handles: Vec<_> = (0..3)
            .map(|i| {
                let func_level = func_level.clone();
                thread::spawn(move || {
                    let address = 0x1000 + i * 0x100;
                    func_level
                        .function_enter(
                            address,
                            Some("test.rs"),
                            Some(&format!("thread_func_{}", i)),
                            10 + i as u32,
                        )
                        .unwrap();
                    thread::sleep(Duration::from_millis(1));
                    func_level.function_exit(None).unwrap();
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // Should have 3 threads
        assert_eq!(func_level.get_thread_count(), 3);

        let mut functions: Vec<String> = (0..func_level.get_thread_count())
            .filter_map(|index| {
                let thread_profile = func_level.enum_threads(index)?;
                let func = thread_profile.enum_profile(0)?;
                assert_eq!(func.get_calls(ProfileFuncId::TOTAL), 1);
                func.get_function().map(|name| name.to_string())
            })
            .collect();

        functions.sort();
        assert_eq!(
            functions,
            vec![
                "thread_func_0".to_string(),
                "thread_func_1".to_string(),
                "thread_func_2".to_string()
            ]
        );
    }

    #[test]
    fn test_clear_totals() {
        let func_level = ProfileFuncLevel::new();
        let address = 0x1000;

        // Record some calls
        func_level
            .function_enter(address, Some("test.rs"), Some("test_func"), 42)
            .unwrap();
        func_level.function_exit(None).unwrap();
        func_level
            .function_enter(address, Some("test.rs"), Some("test_func"), 42)
            .unwrap();
        func_level.function_exit(None).unwrap();

        let thread_profile = func_level.enum_threads(0).unwrap();
        let func = thread_profile.get_function(address).unwrap();
        assert_eq!(func.get_calls(ProfileFuncId::TOTAL), 2);

        // Clear totals
        func_level.clear_totals();

        assert_eq!(func.get_calls(ProfileFuncId::TOTAL), 0);
        assert_eq!(func.get_time(ProfileFuncId::TOTAL), 0);
        assert_eq!(func.get_function_time(ProfileFuncId::TOTAL), 0);
    }
}
