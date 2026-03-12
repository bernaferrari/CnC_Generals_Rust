//! Systems Module - Update Loops and Frame Management
//!
//! This module provides the core update system with:
//! - Update loops for game logic
//! - Sleepy update system using priority queue
//! - Normal update management
//! - Frame management and timing

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crate::common::{ObjectID, Real, UnsignedInt};
use crate::modules::{UpdateModule, UpdateModulePtr};
use crate::object::Object;
use crate::GameLogicResult;

/// Sleepy update entry for priority queue
#[derive(Debug, Clone)]
pub struct SleepyUpdateEntry {
    pub wake_frame: UnsignedInt,
    pub module: UpdateModulePtr,
    pub object_id: ObjectID,
}

#[derive(Clone)]
struct NormalUpdateEntry {
    object_id: ObjectID,
    module: UpdateModulePtr,
}

impl PartialEq for SleepyUpdateEntry {
    fn eq(&self, other: &Self) -> bool {
        self.wake_frame == other.wake_frame
    }
}

impl Eq for SleepyUpdateEntry {}

impl PartialOrd for SleepyUpdateEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SleepyUpdateEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap (earliest wake frame first)
        other.wake_frame.cmp(&self.wake_frame)
    }
}

/// Update system configuration
#[derive(Debug, Clone)]
pub struct UpdateSystemConfig {
    /// Maximum number of updates per frame
    pub max_updates_per_frame: usize,
    /// Whether to enable sleepy updates
    pub enable_sleepy_updates: bool,
    /// Frame rate for timing calculations
    pub frame_rate: Real,
    /// Maximum sleep frames allowed
    pub max_sleep_frames: UnsignedInt,
}

impl Default for UpdateSystemConfig {
    fn default() -> Self {
        Self {
            max_updates_per_frame: 1000,
            enable_sleepy_updates: true,
            frame_rate: 30.0,
            max_sleep_frames: 300, // 10 seconds at 30fps
        }
    }
}

/// Update system statistics
#[derive(Debug, Clone)]
pub struct UpdateSystemStats {
    pub total_updates: u64,
    pub sleepy_updates_processed: u64,
    pub normal_updates_processed: u64,
    pub average_update_time: Duration,
    pub last_update_time: Instant,
    pub frames_processed: u64,
}

/// Main update system manager
#[derive(Debug)]
pub struct UpdateSystem {
    /// Configuration
    config: UpdateSystemConfig,

    /// Sleepy update priority queue (min-heap by wake frame)
    sleepy_updates: BinaryHeap<SleepyUpdateEntry>,

    /// Normal update modules (processed every frame)
    normal_updates: Vec<NormalUpdateEntry>,

    /// Module lookup by object ID for quick removal
    module_lookup: HashMap<ObjectID, Vec<UpdateModulePtr>>,

    /// Current frame number
    current_frame: UnsignedInt,

    /// Frame timing
    frame_start_time: Instant,
    last_frame_duration: Duration,

    /// Statistics
    stats: UpdateSystemStats,
}

impl UpdateSystem {
    /// Create a new update system
    pub fn new(config: UpdateSystemConfig) -> Self {
        Self {
            config,
            sleepy_updates: BinaryHeap::new(),
            normal_updates: Vec::new(),
            module_lookup: HashMap::new(),
            current_frame: 0,
            frame_start_time: Instant::now(),
            last_frame_duration: Duration::from_millis(33), // ~30fps default
            stats: UpdateSystemStats {
                total_updates: 0,
                sleepy_updates_processed: 0,
                normal_updates_processed: 0,
                average_update_time: Duration::ZERO,
                last_update_time: Instant::now(),
                frames_processed: 0,
            },
        }
    }

    /// Add a normal update module (updates every frame)
    pub fn add_normal_update(&mut self, module: UpdateModulePtr, object_id: ObjectID) {
        self.normal_updates.push(NormalUpdateEntry {
            object_id,
            module: module.clone(),
        });
        self.module_lookup
            .entry(object_id)
            .or_insert_with(Vec::new)
            .push(module);

        log::debug!("Added normal update module for object {}", object_id);
    }

    /// Add a sleepy update module
    pub fn add_sleepy_update(
        &mut self,
        module: UpdateModulePtr,
        object_id: ObjectID,
        wake_frame: UnsignedInt,
    ) {
        if !self.config.enable_sleepy_updates {
            // If sleepy updates disabled, add as normal update
            self.add_normal_update(module, object_id);
            return;
        }

        let entry = SleepyUpdateEntry {
            wake_frame: wake_frame.min(self.current_frame + self.config.max_sleep_frames),
            module: module.clone(),
            object_id,
        };

        self.sleepy_updates.push(entry);
        self.module_lookup
            .entry(object_id)
            .or_insert_with(Vec::new)
            .push(module);

        log::debug!(
            "Added sleepy update module for object {} waking at frame {}",
            object_id,
            wake_frame
        );
    }

    /// Remove all update modules for an object
    pub fn remove_object_updates(&mut self, object_id: ObjectID) {
        // Remove from normal updates
        self.normal_updates
            .retain(|entry| entry.object_id != object_id);

        // Remove from sleepy updates
        let mut temp_heap = BinaryHeap::new();
        while let Some(entry) = self.sleepy_updates.pop() {
            if entry.object_id != object_id {
                temp_heap.push(entry);
            }
        }
        self.sleepy_updates = temp_heap;

        // Remove from lookup
        self.module_lookup.remove(&object_id);

        log::debug!("Removed all update modules for object {}", object_id);
    }

    /// Process one frame of updates
    pub fn process_frame(
        &mut self,
        objects: &HashMap<ObjectID, Arc<RwLock<Object>>>,
    ) -> GameLogicResult<()> {
        let frame_start = Instant::now();
        self.current_frame += 1;
        self.frame_start_time = frame_start;

        // Process sleepy updates that are ready to wake
        self.process_sleepy_updates(objects)?;

        // Process normal updates
        self.process_normal_updates(objects)?;

        // Update statistics
        self.update_stats(frame_start);

        Ok(())
    }

    /// Process sleepy updates that are ready to wake
    fn process_sleepy_updates(
        &mut self,
        objects: &HashMap<ObjectID, Arc<RwLock<Object>>>,
    ) -> GameLogicResult<()> {
        let mut processed = 0;
        let mut temp_entries = Vec::new();

        // Extract entries that are ready to wake
        while let Some(entry) = self.sleepy_updates.pop() {
            if entry.wake_frame <= self.current_frame {
                // Process this update
                if let Some(object) = objects.get(&entry.object_id) {
                    if let Ok(mut object_guard) = object.write() {
                        if let Ok(mut module_guard) = entry.module.write() {
                            module_guard.update(&mut *object_guard)?;
                            self.stats.sleepy_updates_processed += 1;
                            processed += 1;
                        }
                    }
                }

                // Check if we hit the per-frame limit
                if processed >= self.config.max_updates_per_frame {
                    // Re-queue remaining entries
                    while let Some(remaining) = self.sleepy_updates.pop() {
                        temp_entries.push(remaining);
                    }
                    break;
                }
            } else {
                // Not ready yet, put back
                temp_entries.push(entry);
            }
        }

        // Re-queue entries that weren't processed
        for entry in temp_entries {
            self.sleepy_updates.push(entry);
        }

        Ok(())
    }

    /// Process normal updates (every frame)
    fn process_normal_updates(
        &mut self,
        objects: &HashMap<ObjectID, Arc<RwLock<Object>>>,
    ) -> GameLogicResult<()> {
        for entry in &self.normal_updates {
            let Some(object) = objects.get(&entry.object_id) else {
                continue;
            };

            let Ok(mut module_guard) = entry.module.write() else {
                continue;
            };

            if let Ok(mut object_guard) = object.write() {
                module_guard.update(&mut *object_guard)?;
                self.stats.normal_updates_processed += 1;
            }
        }

        Ok(())
    }

    /// Put a module to sleep until a specific frame
    pub fn sleep_module(&mut self, module: UpdateModulePtr, wake_frame: UnsignedInt) {
        let clamped_wake_frame = wake_frame.min(self.current_frame + self.config.max_sleep_frames);

        let object_id = self.module_lookup.iter().find_map(|(object_id, modules)| {
            modules
                .iter()
                .any(|tracked| Arc::ptr_eq(tracked, &module))
                .then_some(*object_id)
        });

        if let Some(object_id) = object_id {
            self.sleepy_updates.push(SleepyUpdateEntry {
                wake_frame: clamped_wake_frame,
                module,
                object_id,
            });
        } else {
            log::warn!("sleep_module called for untracked module");
        }
    }

    /// Wake up a sleeping module immediately
    pub fn wake_module(&mut self, module: UpdateModulePtr) {
        // Find and update the entry in the priority queue
        let mut temp_heap = BinaryHeap::new();
        while let Some(mut entry) = self.sleepy_updates.pop() {
            if Arc::ptr_eq(&entry.module, &module) {
                entry.wake_frame = self.current_frame;
            }
            temp_heap.push(entry);
        }
        self.sleepy_updates = temp_heap;
    }

    /// Get current frame number
    pub fn current_frame(&self) -> UnsignedInt {
        self.current_frame
    }

    /// Get frame delta time
    pub fn frame_delta_time(&self) -> Real {
        self.last_frame_duration.as_secs_f32()
    }

    /// Get update system statistics
    pub fn stats(&self) -> &UpdateSystemStats {
        &self.stats
    }

    /// Update performance statistics
    fn update_stats(&mut self, frame_start: Instant) {
        let frame_duration = frame_start.elapsed();
        self.last_frame_duration = frame_duration;
        self.stats.frames_processed += 1;
        self.stats.total_updates =
            self.stats.sleepy_updates_processed + self.stats.normal_updates_processed;

        // Update rolling average
        let alpha = 0.1;
        let current_avg = self.stats.average_update_time.as_secs_f64();
        let new_avg = (1.0 - alpha) * current_avg + alpha * frame_duration.as_secs_f64();
        self.stats.average_update_time = Duration::from_secs_f64(new_avg);

        self.stats.last_update_time = Instant::now();
    }

    /// Get the number of sleeping modules
    pub fn sleeping_module_count(&self) -> usize {
        self.sleepy_updates.len()
    }

    /// Get the number of normal update modules
    pub fn normal_module_count(&self) -> usize {
        self.normal_updates.len()
    }

    /// Clear all updates (for reset/game restart)
    pub fn clear_all_updates(&mut self) {
        self.sleepy_updates.clear();
        self.normal_updates.clear();
        self.module_lookup.clear();
        self.current_frame = 0;
        self.stats = UpdateSystemStats {
            total_updates: 0,
            sleepy_updates_processed: 0,
            normal_updates_processed: 0,
            average_update_time: Duration::ZERO,
            last_update_time: Instant::now(),
            frames_processed: 0,
        };

        log::debug!("Cleared all update modules");
    }
}

/// Frame manager for coordinating frame-based updates
#[derive(Debug)]
pub struct FrameManager {
    /// Current frame number
    current_frame: UnsignedInt,

    /// Frame rate
    frame_rate: Real,

    /// Frame duration in seconds
    frame_duration: Real,

    /// Time accumulator for fixed timestep
    time_accumulator: Real,

    /// Last frame time
    last_frame_time: Instant,
}

impl FrameManager {
    /// Create a new frame manager
    pub fn new(frame_rate: Real) -> Self {
        Self {
            current_frame: 0,
            frame_rate,
            frame_duration: 1.0 / frame_rate,
            time_accumulator: 0.0,
            last_frame_time: Instant::now(),
        }
    }

    /// Update frame timing and return number of frames to process
    pub fn update(&mut self) -> usize {
        let now = Instant::now();
        let delta_time = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;

        self.time_accumulator += delta_time;

        let mut frames_to_process = 0;
        while self.time_accumulator >= self.frame_duration {
            self.time_accumulator -= self.frame_duration;
            self.current_frame += 1;
            frames_to_process += 1;
        }

        frames_to_process
    }

    /// Get current frame number
    pub fn current_frame(&self) -> UnsignedInt {
        self.current_frame
    }

    /// Get frame rate
    pub fn frame_rate(&self) -> Real {
        self.frame_rate
    }

    /// Get frame duration
    pub fn frame_duration(&self) -> Real {
        self.frame_duration
    }

    /// Get time accumulator (for interpolation)
    pub fn time_accumulator(&self) -> Real {
        self.time_accumulator
    }

    /// Reset frame counter
    pub fn reset(&mut self) {
        self.current_frame = 0;
        self.time_accumulator = 0.0;
        self.last_frame_time = Instant::now();
    }
}

/// Example update module implementation
#[derive(Debug)]
pub struct ExampleUpdateModule {
    name: String,
    enabled: bool,
}

impl ExampleUpdateModule {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            enabled: true,
        }
    }
}

impl UpdateModule for ExampleUpdateModule {
    fn update(&mut self, object_id: ObjectID, _delta_time: Real) {
        log::debug!("Updating object {} with module {}", object_id, self.name);
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::Object;

    #[test]
    fn test_sleepy_update_priority_queue() {
        let mut system = UpdateSystem::new(UpdateSystemConfig::default());

        // Create test modules
        let module1 = Arc::new(RwLock::new(ExampleUpdateModule::new("test1")));
        let module2 = Arc::new(RwLock::new(ExampleUpdateModule::new("test2")));
        let module3 = Arc::new(RwLock::new(ExampleUpdateModule::new("test3")));

        // Add sleepy updates with different wake frames
        system.add_sleepy_update(module1, 1, 10);
        system.add_sleepy_update(module2, 2, 5);
        system.add_sleepy_update(module3, 3, 15);

        // Check that the earliest wake frame is at the top
        if let Some(entry) = system.sleepy_updates.peek() {
            assert_eq!(entry.wake_frame, 5);
        }
    }

    #[test]
    fn test_frame_manager() {
        let mut fm = FrameManager::new(30.0);

        // Simulate some time passing
        let frames = fm.update();
        assert!(frames >= 0);

        assert_eq!(fm.frame_rate(), 30.0);
        assert_eq!(fm.frame_duration(), 1.0 / 30.0);
    }

    #[test]
    fn test_update_system_stats() {
        let system = UpdateSystem::new(UpdateSystemConfig::default());
        let stats = system.stats();

        assert_eq!(stats.total_updates, 0);
        assert_eq!(stats.frames_processed, 0);
    }
}
