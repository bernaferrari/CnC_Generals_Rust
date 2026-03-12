//! High Level Profiling Module
//!
//! Equivalent to the C++ ProfileHighLevel class, provides timer-based profiling
//! and logical profiling with hierarchical naming schemes.

use crate::timing::ProfileTimer;
use crate::ProfileResult;
use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// High level profile ID - equivalent to ProfileHighLevel::Id
#[derive(Debug, Clone)]
pub struct ProfileId {
    inner: Arc<ProfileIdInner>,
}

#[derive(Debug)]
struct ProfileIdInner {
    name: String,
    description: String,
    unit: String,
    precision: i32,
    exp10: i32,
    current_value: AtomicU64,
    total_value: AtomicU64,
    frame_values: RwLock<HashMap<usize, f64>>,
    is_max_value: bool,
}

impl ProfileId {
    fn new(name: String, description: String, unit: String, precision: i32, exp10: i32) -> Self {
        Self {
            inner: Arc::new(ProfileIdInner {
                name,
                description,
                unit,
                precision,
                exp10,
                current_value: AtomicU64::new(0),
                total_value: AtomicU64::new(0),
                frame_values: RwLock::new(HashMap::new()),
                is_max_value: false,
            }),
        }
    }

    fn new_max(
        name: String,
        description: String,
        unit: String,
        precision: i32,
        exp10: i32,
    ) -> Self {
        Self {
            inner: Arc::new(ProfileIdInner {
                name,
                description,
                unit,
                precision,
                exp10,
                current_value: AtomicU64::new(0),
                total_value: AtomicU64::new(0),
                frame_values: RwLock::new(HashMap::new()),
                is_max_value: true,
            }),
        }
    }

    /// Increment the internal profile value
    pub fn increment(&self, add: f64) {
        let add_bits = add.to_bits();
        self.inner
            .current_value
            .fetch_add(add_bits, Ordering::Relaxed);

        if !self.inner.is_max_value {
            let current_total = f64::from_bits(self.inner.total_value.load(Ordering::Relaxed));
            let new_total = current_total + add;
            self.inner
                .total_value
                .store(new_total.to_bits(), Ordering::Relaxed);
        }
    }

    /// Set a new maximum value
    pub fn set_max(&self, max: f64) {
        if !self.inner.is_max_value {
            return; // Only for max-value profiles
        }

        let max_bits = max.to_bits();

        loop {
            let current = self.inner.current_value.load(Ordering::Relaxed);
            let current_val = f64::from_bits(current);

            if max <= current_val {
                break; // Current value is already higher
            }

            if self
                .inner
                .current_value
                .compare_exchange_weak(current, max_bits, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }

        // Also update total if this is a new max
        let current_total = f64::from_bits(self.inner.total_value.load(Ordering::Relaxed));
        if max > current_total {
            self.inner.total_value.store(max_bits, Ordering::Relaxed);
        }
    }

    /// Get the internal ID name
    pub fn get_name(&self) -> &str {
        &self.inner.name
    }

    /// Get the descriptive name
    pub fn get_description(&self) -> &str {
        &self.inner.description
    }

    /// Get the unit text
    pub fn get_unit(&self) -> &str {
        &self.inner.unit
    }

    /// Get the current value (since last call)
    pub fn get_current_value(&self) -> String {
        let current_bits = self.inner.current_value.swap(0, Ordering::Relaxed);
        let current = f64::from_bits(current_bits);
        self.format_value(current)
    }

    /// Get the value for a specific recorded frame
    pub fn get_value(&self, frame: usize) -> Option<String> {
        let frame_values = self.inner.frame_values.read();
        frame_values
            .get(&frame)
            .map(|&value| self.format_value(value))
    }

    /// Get the total value for all frames
    pub fn get_total_value(&self) -> String {
        let total_bits = self.inner.total_value.load(Ordering::Relaxed);
        let total = f64::from_bits(total_bits);
        self.format_value(total)
    }

    /// Format a value according to precision and exp10 settings
    fn format_value(&self, value: f64) -> String {
        let scaled_value = value * 10.0_f64.powi(self.inner.exp10);
        format!(
            "{:.prec$}",
            scaled_value,
            prec = self.inner.precision as usize
        )
    }

    /// Record frame value (internal use)
    pub(crate) fn record_frame_value(&self, frame: usize) {
        let current_bits = self.inner.current_value.load(Ordering::Relaxed);
        let current = f64::from_bits(current_bits);

        let mut frame_values = self.inner.frame_values.write();
        frame_values.insert(frame, current);
    }

    /// Clear totals
    pub(crate) fn clear_total(&self) {
        self.inner.total_value.store(0, Ordering::Relaxed);
    }
}

/// Timer-based profiling block - equivalent to ProfileHighLevel::Block
pub struct ProfileBlock {
    id: ProfileId,
    start_time: u64,
}

impl ProfileBlock {
    fn new(name: &str) -> ProfileResult<Self> {
        let timer_name = format!("{}.time", name);
        let state = &*crate::PROFILER_STATE;
        let high_level = &state.high_level;

        let id = high_level.add_profile(
            &timer_name,
            &format!("Time spent in {}", name),
            "sec",
            6,  // 6 decimal places for seconds
            -9, // Convert nanoseconds to seconds
        )?;

        let start_time = ProfileTimer::get_cpu_cycles()?;

        Ok(Self { id, start_time })
    }
}

impl Drop for ProfileBlock {
    fn drop(&mut self) {
        if let Ok(end_time) = ProfileTimer::get_cpu_cycles() {
            let elapsed_cycles = end_time.saturating_sub(self.start_time);

            // Convert cycles to nanoseconds
            if let Ok(cycles_per_sec) = crate::Profile::get_clock_cycles_per_second() {
                let elapsed_ns = (elapsed_cycles as f64 * 1_000_000_000.0) / cycles_per_sec as f64;
                self.id.increment(elapsed_ns);
            }
        }
    }
}

/// High level profiler - equivalent to ProfileHighLevel class
pub struct ProfileHighLevel {
    profiles: DashMap<String, ProfileId>,
    profile_list: RwLock<Vec<ProfileId>>,
}

impl ProfileHighLevel {
    pub fn new() -> Self {
        Self {
            profiles: DashMap::new(),
            profile_list: RwLock::new(Vec::new()),
        }
    }

    /// Register a new high level profile value
    pub fn add_profile(
        &self,
        name: &str,
        description: &str,
        unit: &str,
        precision: i32,
        exp10: i32,
    ) -> ProfileResult<ProfileId> {
        // Check if profile already exists
        if let Some(existing) = self.profiles.get(name) {
            return Ok(existing.clone());
        }

        let profile_id = ProfileId::new(
            name.to_string(),
            description.to_string(),
            unit.to_string(),
            precision,
            exp10,
        );

        // Insert into map
        self.profiles.insert(name.to_string(), profile_id.clone());

        // Add to sorted list
        let mut profile_list = self.profile_list.write();

        // Insert in sorted order by name
        let insert_pos = profile_list
            .binary_search_by(|probe| probe.get_name().cmp(name))
            .unwrap_or_else(|pos| pos);

        profile_list.insert(insert_pos, profile_id.clone());

        Ok(profile_id)
    }

    /// Create a timer-based profiling block
    pub fn block(name: &str) -> ProfileResult<ProfileBlock> {
        ProfileBlock::new(name)
    }

    /// Enumerate known high level profile values
    pub fn enum_profile(&self, index: usize) -> Option<ProfileId> {
        let profile_list = self.profile_list.read();
        profile_list.get(index).cloned()
    }

    /// Find a high level profile by name
    pub fn find_profile(&self, name: &str) -> Option<ProfileId> {
        let profile_list = self.profile_list.read();

        // Binary search for first profile with name >= search name
        match profile_list.binary_search_by(|probe| probe.get_name().cmp(name)) {
            Ok(index) => profile_list.get(index).cloned(),
            Err(index) => profile_list.get(index).cloned(),
        }
    }

    /// Get profile count
    pub fn get_profile_count(&self) -> usize {
        self.profile_list.read().len()
    }

    /// Clear all totals
    pub fn clear_totals(&self) {
        let profile_list = self.profile_list.read();
        for profile in profile_list.iter() {
            profile.clear_total();
        }
    }

    /// Frame start (internal)
    pub(crate) fn frame_start(&self) -> ProfileResult<i32> {
        // In the Rust version, we don't need to track frame starts/ends
        // as explicitly as the C++ version since we use different synchronization
        Ok(0) // Dummy index
    }

    /// Frame end (internal)
    pub(crate) fn frame_end(&self, _index: i32, frame: Option<i32>) -> ProfileResult<()> {
        if let Some(frame_num) = frame {
            // Record current values for all profiles to this frame
            let profile_list = self.profile_list.read();
            for profile in profile_list.iter() {
                profile.record_frame_value(frame_num as usize);
            }
        }
        Ok(())
    }
}

impl Default for ProfileHighLevel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_profile_id_increment() {
        let high_level = ProfileHighLevel::new();

        let id = high_level
            .add_profile("test.counter", "Test Counter", "count", 0, 0)
            .unwrap();

        id.increment(5.0);
        id.increment(3.0);

        let total = id.get_total_value();
        assert_eq!(total, "8");
    }

    #[test]
    fn test_profile_id_max() {
        let id = ProfileId::new_max(
            "test.max".to_string(),
            "Test Max".to_string(),
            "units".to_string(),
            1,
            0,
        );

        id.set_max(10.0);
        id.set_max(5.0); // Should not change
        id.set_max(15.0); // Should update

        let total = id.get_total_value();
        assert_eq!(total, "15.0");
    }

    #[test]
    fn test_profile_block() {
        {
            let _block = ProfileHighLevel::block("test_operation").unwrap();
            thread::sleep(Duration::from_millis(10));
        }

        // Find the timer profile that was created
        let timer_id = crate::PROFILER_STATE
            .high_level
            .find_profile("test_operation.time");
        assert!(timer_id.is_some());

        let timer_id = timer_id.unwrap();
        let total = timer_id.get_total_value();

        // Should have recorded some positive time
        let total_val: f64 = total.parse().unwrap_or(0.0);
        assert!(total_val > 0.0);
    }

    #[test]
    fn test_profile_enumeration() {
        let high_level = ProfileHighLevel::new();

        // Add profiles in non-alphabetical order
        high_level
            .add_profile("zebra", "Z Profile", "count", 0, 0)
            .unwrap();
        high_level
            .add_profile("alpha", "A Profile", "count", 0, 0)
            .unwrap();
        high_level
            .add_profile("beta", "B Profile", "count", 0, 0)
            .unwrap();

        // Should be returned in alphabetical order
        let first = high_level.enum_profile(0).unwrap();
        assert_eq!(first.get_name(), "alpha");

        let second = high_level.enum_profile(1).unwrap();
        assert_eq!(second.get_name(), "beta");

        let third = high_level.enum_profile(2).unwrap();
        assert_eq!(third.get_name(), "zebra");

        assert!(high_level.enum_profile(3).is_none());
    }

    #[test]
    fn test_profile_find() {
        let high_level = ProfileHighLevel::new();

        high_level
            .add_profile("test.alpha", "Alpha", "count", 0, 0)
            .unwrap();
        high_level
            .add_profile("test.gamma", "Gamma", "count", 0, 0)
            .unwrap();

        // Exact match
        let found = high_level.find_profile("test.alpha").unwrap();
        assert_eq!(found.get_name(), "test.alpha");

        // Should find first profile >= search term
        let found = high_level.find_profile("test.beta").unwrap();
        assert_eq!(found.get_name(), "test.gamma");

        // No match returns None or last available
        let found = high_level.find_profile("test.zzzz");
        assert!(found.is_none() || found.unwrap().get_name() >= "test.zzzz");
    }
}
