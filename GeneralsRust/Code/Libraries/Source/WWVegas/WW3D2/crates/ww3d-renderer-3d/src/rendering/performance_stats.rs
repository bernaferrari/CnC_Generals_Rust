//! Performance Statistics - Comprehensive performance monitoring
//!
//! This module implements the Debug_Statistics system from the original C++ code,
//! providing detailed performance monitoring, texture tracking, memory usage,
//! and rendering statistics.
//!
//! Converted from:
//! - statistics.cpp/h (performance statistics)
//! - Texture memory tracking system
//! - Rendering performance metrics

use crate::core::error::Result;
use crate::core::wwstring::StringClass;
use crate::rendering::texture_system::texture_base::TextureClass;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

fn texture_key(texture: &Arc<TextureClass>) -> usize {
    Arc::as_ptr(texture) as usize
}

fn textures_eq(a: Option<&Arc<TextureClass>>, b: Option<&Arc<TextureClass>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// Texture recording mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RecordTextureMode {
    /// No texture recording
    NoRecording = 0,
    /// Record texture summary
    RecordSummary = 1,
    /// Record texture details
    RecordDetails = 2,
}

/// Texture statistics entry
#[derive(Debug, Clone)]
pub struct TextureStatisticsEntry {
    /// Texture reference
    pub texture: Option<Arc<TextureClass>>,
    /// Usage count
    pub usage_count: i32,
    /// Change count
    pub change_count: i32,
}

impl TextureStatisticsEntry {
    /// Create new texture statistics entry
    pub fn new(texture: Option<Arc<TextureClass>>) -> Self {
        Self {
            texture,
            usage_count: 0,
            change_count: 0,
        }
    }

    /// Record texture usage
    pub fn record_usage(&mut self) {
        self.usage_count += 1;
    }

    /// Record texture change
    pub fn record_change(&mut self) {
        self.change_count += 1;
    }

    /// Get texture name
    pub fn get_texture_name(&self) -> String {
        self.texture
            .as_ref()
            .map(|t| t.get_name().to_string())
            .unwrap_or_else(|| "Unknown".to_string())
    }
}

/// Performance statistics system
#[derive(Debug)]
pub struct DebugStatistics {
    /// Texture statistics entries
    pub texture_statistics: Vec<TextureStatisticsEntry>,
    /// Texture statistics map for quick lookup
    pub texture_stats_map: HashMap<usize, usize>,
    /// Current frame texture memory usage
    pub current_texture_memory: usize,
    /// Current frame texture count
    pub current_texture_count: usize,
    /// Current frame lightmap texture memory
    pub current_lightmap_texture_memory: usize,
    /// Current frame lightmap texture count
    pub current_lightmap_texture_count: usize,
    /// Current frame procedural texture memory
    pub current_procedural_texture_memory: usize,
    /// Current frame procedural texture count
    pub current_procedural_texture_count: usize,
    /// Current frame record count
    pub current_record_count: usize,
    /// Current frame texture change count
    pub current_texture_change_count: usize,
    /// Last frame statistics (for comparison)
    pub last_frame_stats: FrameStatistics,
    /// Recording mode
    pub record_texture_mode: RecordTextureMode,
    /// Whether recording is active
    pub is_recording: bool,
    /// Frame start time
    pub frame_start_time: Instant,
    /// Total frames recorded
    pub total_frames: u64,
    /// Statistics string
    pub statistics_string: StringClass,
    /// Last texture recorded (for change tracking)
    latest_texture: Option<Arc<TextureClass>>,

    // DX8 statistics
    dx8_skin_renders: i32,
    dx8_skin_polygons: i32,
    dx8_skin_vertices: i32,
    dx8_polygons: i32,
    dx8_vertices: i32,
    sorting_polygons: i32,
    sorting_vertices: i32,
    draw_calls: i32,
    last_frame_dx8_skin_renders: i32,
    last_frame_dx8_skin_polygons: i32,
    last_frame_dx8_skin_vertices: i32,
    last_frame_dx8_polygons: i32,
    last_frame_dx8_vertices: i32,
    last_frame_sorting_polygons: i32,
    last_frame_sorting_vertices: i32,
    last_frame_draw_calls: i32,
}

impl DebugStatistics {
    /// Create new debug statistics
    pub fn new() -> Self {
        Self {
            texture_statistics: Vec::new(),
            texture_stats_map: HashMap::new(),
            current_texture_memory: 0,
            current_texture_count: 0,
            current_lightmap_texture_memory: 0,
            current_lightmap_texture_count: 0,
            current_procedural_texture_memory: 0,
            current_procedural_texture_count: 0,
            current_record_count: 0,
            current_texture_change_count: 0,
            last_frame_stats: FrameStatistics::new(),
            record_texture_mode: RecordTextureMode::NoRecording,
            is_recording: false,
            frame_start_time: Instant::now(),
            total_frames: 0,
            statistics_string: StringClass::new(),
            latest_texture: None,
            dx8_skin_renders: 0,
            dx8_skin_polygons: 0,
            dx8_skin_vertices: 0,
            dx8_polygons: 0,
            dx8_vertices: 0,
            sorting_polygons: 0,
            sorting_vertices: 0,
            draw_calls: 0,
            last_frame_dx8_skin_renders: 0,
            last_frame_dx8_skin_polygons: 0,
            last_frame_dx8_skin_vertices: 0,
            last_frame_dx8_polygons: 0,
            last_frame_dx8_vertices: 0,
            last_frame_sorting_polygons: 0,
            last_frame_sorting_vertices: 0,
            last_frame_draw_calls: 0,
        }
    }

    /// Begin recording frame statistics
    pub fn begin_recording(&mut self, mode: RecordTextureMode) {
        self.is_recording = true;
        self.record_texture_mode = mode;
        self.frame_start_time = Instant::now();

        // Reset current frame counters
        self.current_texture_memory = 0;
        self.current_texture_count = 0;
        self.current_lightmap_texture_memory = 0;
        self.current_lightmap_texture_count = 0;
        self.current_procedural_texture_memory = 0;
        self.current_procedural_texture_count = 0;
        self.current_record_count = 0;
        self.current_texture_change_count = 0;

        self.texture_statistics.clear();
        self.texture_stats_map.clear();
        self.latest_texture = None;
    }

    /// End recording frame statistics
    pub fn end_recording(&mut self) {
        if !self.is_recording {
            return;
        }

        // Save current frame stats to last frame
        self.last_frame_stats = FrameStatistics {
            texture_memory: self.current_texture_memory,
            texture_count: self.current_texture_count,
            lightmap_texture_memory: self.current_lightmap_texture_memory,
            lightmap_texture_count: self.current_lightmap_texture_count,
            procedural_texture_memory: self.current_procedural_texture_memory,
            procedural_texture_count: self.current_procedural_texture_count,
            record_count: self.current_record_count,
            texture_change_count: self.current_texture_change_count,
            frame_time: self.frame_start_time.elapsed(),
        };

        self.total_frames += 1;
        self.is_recording = false;

        // Generate statistics string
        self.generate_statistics_string();
    }

    /// Record a texture (C++ Record_Texture)
    pub fn record_texture(&mut self, texture: Option<&Arc<TextureClass>>) {
        self.current_record_count += 1;
        let previous_latest = self.latest_texture.clone();
        if !textures_eq(previous_latest.as_ref(), texture) {
            self.current_texture_change_count += 1;
        }

        if self.record_texture_mode == RecordTextureMode::NoRecording {
            self.latest_texture = texture.cloned();
            return;
        }

        let Some(tex) = texture else {
            self.latest_texture = None;
            return;
        };

        if let Some(index) = self.find_record_texture_index(tex) {
            if self.record_texture_mode == RecordTextureMode::RecordDetails {
                if let Some(entry) = self.texture_statistics.get_mut(index) {
                    entry.record_usage();
                    if !textures_eq(previous_latest.as_ref(), Some(tex)) {
                        entry.record_change();
                    }
                }
            }
        } else {
            self.add_record_texture(tex);
        }

        self.latest_texture = Some(Arc::clone(tex));
    }

    /// Generate statistics string
    fn generate_statistics_string(&mut self) {
        if self.record_texture_mode == RecordTextureMode::NoRecording {
            self.statistics_string = StringClass::new();
            return;
        }

        let mut stats = String::new();

        if self.record_texture_mode == RecordTextureMode::RecordDetails {
            stats.push_str(&format!(
                "Set_DX8_Texture count: {}\nactual changes: {}\n\n",
                self.last_frame_stats.record_count, self.last_frame_stats.texture_change_count
            ));
            stats.push_str(
                "id      refs changes  size      name\n--------------------------------------\n",
            );
            for entry in &self.texture_statistics {
                let mut marker = "  ";
                let mut size_kb = 0;
                if let Some(tex) = entry.texture.as_ref() {
                    if !tex.is_initialized() {
                        marker = "*";
                    }
                    size_kb = tex.get_memory_usage() / 1024;
                }
                stats.push_str(&format!(
                    "{:4}  {:3}   {:3}     {}{:4}kb         {}\n",
                    0,
                    entry.usage_count,
                    entry.change_count,
                    marker,
                    size_kb,
                    entry.get_texture_name()
                ));
            }
            stats.push_str("\n");
            stats.push_str("\nid              = id of texture. Use with command 'flash_texture [id]'\n");
            stats.push_str("refs          = # of times texture is used when rendering\n");
            stats.push_str("changes    = # of times texture change needed - BAD IF HIGH!\n");
            stats.push_str("red         = texture reduction factor\n");
            stats.push_str("size          = amount of memory needed for texture\n");
            stats.push_str("(w/o red)     = size of reduction not used\n");
            stats.push_str("percent    = savings of reduction system, in percents\n");
            stats.push_str("\n* = thumbnail used\n\n");
        }

        self.statistics_string = StringClass::from(stats.as_str());
    }

    /// Get current frame texture memory usage
    pub fn get_current_texture_memory(&self) -> usize {
        self.current_texture_memory
    }

    /// Get current frame texture count
    pub fn get_current_texture_count(&self) -> usize {
        self.current_texture_count
    }

    /// Get last frame statistics
    pub fn get_last_frame_stats(&self) -> &FrameStatistics {
        &self.last_frame_stats
    }

    /// Get statistics string
    pub fn get_statistics_string(&self) -> &str {
        self.statistics_string.as_str()
    }

    /// Get texture statistics
    pub fn get_texture_statistics(&self) -> &[TextureStatisticsEntry] {
        &self.texture_statistics
    }

    /// Is currently recording
    pub fn is_currently_recording(&self) -> bool {
        self.is_recording
    }

    /// Get recording mode
    pub fn get_recording_mode(&self) -> RecordTextureMode {
        self.record_texture_mode
    }

    pub fn set_record_texture_mode(&mut self, mode: RecordTextureMode) {
        self.record_texture_mode = mode;
    }

    /// Get total frames recorded
    pub fn get_total_frames(&self) -> u64 {
        self.total_frames
    }

    /// Reset all statistics
    pub fn reset(&mut self) {
        self.texture_statistics.clear();
        self.texture_stats_map.clear();
        self.current_texture_memory = 0;
        self.current_texture_count = 0;
        self.current_lightmap_texture_memory = 0;
        self.current_lightmap_texture_count = 0;
        self.current_procedural_texture_memory = 0;
        self.current_procedural_texture_count = 0;
        self.current_record_count = 0;
        self.current_texture_change_count = 0;
        self.last_frame_stats = FrameStatistics::new();
        self.total_frames = 0;
        self.statistics_string = StringClass::new();
        self.latest_texture = None;
    }

    /// Get average frame time
    pub fn get_average_frame_time(&self) -> Duration {
        if self.total_frames == 0 {
            Duration::from_millis(16) // Default 60 FPS
        } else {
            // This would need to track total time across all frames
            Duration::from_millis(16) // Placeholder
        }
    }

    /// Get average FPS
    pub fn get_average_fps(&self) -> f32 {
        let avg_frame_time_ms = self.get_average_frame_time().as_secs_f32() * 1000.0;
        1000.0 / avg_frame_time_ms.max(0.001)
    }

    /// Get memory usage breakdown
    pub fn get_memory_breakdown(&self) -> MemoryBreakdown {
        MemoryBreakdown {
            total_texture_memory: self.current_texture_memory,
            lightmap_texture_memory: self.current_lightmap_texture_memory,
            procedural_texture_memory: self.current_procedural_texture_memory,
            other_memory: 0, // Would track other memory usage
        }
    }

    /// Export statistics to file
    pub fn export_to_file(&self, filename: &str) -> Result<()> {
        // In a full implementation, this would write statistics to a file
        let _ = filename; // Use parameter to avoid warning
        Ok(())
    }

    /// Get performance summary
    pub fn get_performance_summary(&self) -> PerformanceSummary {
        PerformanceSummary {
            average_fps: self.get_average_fps(),
            average_frame_time: self.get_average_frame_time(),
            total_frames: self.total_frames,
            texture_change_rate: if self.total_frames > 0 {
                self.last_frame_stats.texture_change_count as f32 / self.total_frames as f32
            } else {
                0.0
            },
            memory_efficiency: if self.current_texture_memory > 0 {
                self.current_record_count as f32 / self.current_texture_memory as f32
            } else {
                0.0
            },
        }
    }

    pub fn record_dx8_skin_polys_and_vertices(&mut self, pcount: i32, vcount: i32) {
        self.dx8_skin_polygons += pcount;
        self.dx8_skin_vertices += vcount;
        self.dx8_skin_renders += 1;
        self.draw_calls += 1;
    }

    pub fn record_dx8_polys_and_vertices(&mut self, pcount: i32, vcount: i32) {
        self.dx8_polygons += pcount;
        self.dx8_vertices += vcount;
        self.draw_calls += 1;
    }

    pub fn record_sorting_polys_and_vertices(&mut self, pcount: i32, vcount: i32) {
        self.sorting_polygons += pcount;
        self.sorting_vertices += vcount;
        self.draw_calls += 1;
    }

    pub fn begin_statistics(&mut self) {
        self.dx8_polygons = 0;
        self.dx8_vertices = 0;
        self.dx8_skin_polygons = 0;
        self.dx8_skin_vertices = 0;
        self.dx8_skin_renders = 0;
        self.sorting_polygons = 0;
        self.sorting_vertices = 0;
        self.draw_calls = 0;
        self.begin_recording(self.record_texture_mode);
    }

    pub fn end_statistics(&mut self) {
        self.end_recording();
        self.last_frame_dx8_skin_polygons = self.dx8_skin_polygons;
        self.last_frame_dx8_skin_vertices = self.dx8_skin_vertices;
        self.last_frame_dx8_skin_renders = self.dx8_skin_renders;
        self.last_frame_dx8_polygons = self.dx8_polygons;
        self.last_frame_dx8_vertices = self.dx8_vertices;
        self.last_frame_sorting_polygons = self.sorting_polygons;
        self.last_frame_sorting_vertices = self.sorting_vertices;
        self.last_frame_draw_calls = self.draw_calls;
    }

    pub fn shutdown_statistics(&mut self) {
        self.statistics_string = StringClass::new();
    }

    pub fn get_dx8_skin_renders(&self) -> i32 {
        self.last_frame_dx8_skin_renders
    }
    pub fn get_dx8_skin_polygons(&self) -> i32 {
        self.last_frame_dx8_skin_polygons
    }
    pub fn get_dx8_skin_vertices(&self) -> i32 {
        self.last_frame_dx8_skin_vertices
    }
    pub fn get_dx8_polygons(&self) -> i32 {
        self.last_frame_dx8_polygons
    }
    pub fn get_dx8_vertices(&self) -> i32 {
        self.last_frame_dx8_vertices
    }
    pub fn get_sorting_polygons(&self) -> i32 {
        self.last_frame_sorting_polygons
    }
    pub fn get_sorting_vertices(&self) -> i32 {
        self.last_frame_sorting_vertices
    }
    pub fn get_draw_calls(&self) -> i32 {
        self.last_frame_draw_calls
    }

    fn add_record_texture(&mut self, texture: &Arc<TextureClass>) {
        let mut entry = TextureStatisticsEntry::new(Some(Arc::clone(texture)));
        entry.usage_count = 1;
        entry.change_count = 1;
        let index = self.texture_statistics.len();
        self.texture_statistics.push(entry);
        self.texture_stats_map.insert(texture_key(texture), index);

        let memory_usage = texture.get_memory_usage();
        self.current_texture_count += 1;
        self.current_texture_memory += memory_usage;
        if texture.is_lightmap() {
            self.current_lightmap_texture_count += 1;
            self.current_lightmap_texture_memory += memory_usage;
        }
        if texture.is_procedural() {
            self.current_procedural_texture_count += 1;
            self.current_procedural_texture_memory += memory_usage;
        }
    }

    fn find_record_texture_index(&self, texture: &Arc<TextureClass>) -> Option<usize> {
        self.texture_stats_map.get(&texture_key(texture)).copied()
    }
}

/// Frame statistics structure
#[derive(Debug, Clone)]
pub struct FrameStatistics {
    /// Texture memory usage
    pub texture_memory: usize,
    /// Texture count
    pub texture_count: usize,
    /// Lightmap texture memory
    pub lightmap_texture_memory: usize,
    /// Lightmap texture count
    pub lightmap_texture_count: usize,
    /// Procedural texture memory
    pub procedural_texture_memory: usize,
    /// Procedural texture count
    pub procedural_texture_count: usize,
    /// Record count
    pub record_count: usize,
    /// Texture change count
    pub texture_change_count: usize,
    /// Frame time
    pub frame_time: Duration,
}

impl FrameStatistics {
    /// Create new frame statistics
    pub fn new() -> Self {
        Self {
            texture_memory: 0,
            texture_count: 0,
            lightmap_texture_memory: 0,
            lightmap_texture_count: 0,
            procedural_texture_memory: 0,
            procedural_texture_count: 0,
            record_count: 0,
            texture_change_count: 0,
            frame_time: Duration::from_millis(16),
        }
    }
}

/// Memory breakdown structure
#[derive(Debug, Clone)]
pub struct MemoryBreakdown {
    /// Total texture memory
    pub total_texture_memory: usize,
    /// Lightmap texture memory
    pub lightmap_texture_memory: usize,
    /// Procedural texture memory
    pub procedural_texture_memory: usize,
    /// Other memory usage
    pub other_memory: usize,
}

impl MemoryBreakdown {
    /// Get total memory usage
    pub fn get_total_memory(&self) -> usize {
        self.total_texture_memory
            + self.lightmap_texture_memory
            + self.procedural_texture_memory
            + self.other_memory
    }

    /// Get memory usage as string
    pub fn to_string(&self) -> String {
        format!(
            "Total: {} KB, Textures: {} KB, Lightmaps: {} KB, Procedural: {} KB, Other: {} KB",
            self.get_total_memory() / 1024,
            self.total_texture_memory / 1024,
            self.lightmap_texture_memory / 1024,
            self.procedural_texture_memory / 1024,
            self.other_memory / 1024
        )
    }
}

/// Performance summary structure
#[derive(Debug, Clone)]
pub struct PerformanceSummary {
    /// Average FPS
    pub average_fps: f32,
    /// Average frame time
    pub average_frame_time: Duration,
    /// Total frames recorded
    pub total_frames: u64,
    /// Texture change rate (changes per frame)
    pub texture_change_rate: f32,
    /// Memory efficiency (records per KB)
    pub memory_efficiency: f32,
}

impl PerformanceSummary {
    /// Get summary as string
    pub fn to_string(&self) -> String {
        format!(
            "FPS: {:.1}, Frame Time: {:.2}ms, Frames: {}, Texture Changes/Frame: {:.2}, Memory Efficiency: {:.3}",
            self.average_fps,
            self.average_frame_time.as_secs_f32() * 1000.0,
            self.total_frames,
            self.texture_change_rate,
            self.memory_efficiency
        )
    }
}

fn statistics_slot() -> &'static Mutex<Option<DebugStatistics>> {
    static SLOT: OnceLock<Mutex<Option<DebugStatistics>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn lock_statistics_slot() -> MutexGuard<'static, Option<DebugStatistics>> {
    match statistics_slot().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Handle for interacting with the shared statistics singleton.
pub struct StatisticsHandle<'a> {
    guard: MutexGuard<'a, Option<DebugStatistics>>,
}

impl<'a> Deref for StatisticsHandle<'a> {
    type Target = DebugStatistics;

    fn deref(&self) -> &Self::Target {
        self.guard
            .as_ref()
            .expect("statistics must be initialized before use")
    }
}

impl<'a> DerefMut for StatisticsHandle<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard
            .as_mut()
            .expect("statistics must be initialized before use")
    }
}

/// Initialize global statistics
pub fn init_statistics() -> Result<()> {
    let mut guard = lock_statistics_slot();
    *guard = Some(DebugStatistics::new());
    Ok(())
}

/// Get global statistics instance
pub fn get_statistics() -> Option<StatisticsHandle<'static>> {
    let guard = lock_statistics_slot();
    if guard.is_none() {
        None
    } else {
        Some(StatisticsHandle { guard })
    }
}

/// Shutdown global statistics
pub fn shutdown_statistics() {
    let mut guard = lock_statistics_slot();
    *guard = None;
}

/// Quick statistics functions
pub fn begin_recording(mode: RecordTextureMode) {
    if let Some(mut stats) = get_statistics() {
        stats.begin_recording(mode);
    }
}

pub fn end_recording() {
    if let Some(mut stats) = get_statistics() {
        stats.end_recording();
    }
}

pub fn record_texture(texture: Option<&Arc<TextureClass>>) {
    if let Some(mut stats) = get_statistics() {
        stats.record_texture(texture);
    }
}

pub fn begin_statistics() {
    if let Some(mut stats) = get_statistics() {
        stats.begin_statistics();
    }
}

pub fn end_statistics() {
    if let Some(mut stats) = get_statistics() {
        stats.end_statistics();
    }
}

pub fn shutdown_statistics() {
    if let Some(mut stats) = get_statistics() {
        stats.shutdown_statistics();
    }
}

pub fn record_dx8_skin_polys_and_vertices(pcount: i32, vcount: i32) {
    if let Some(mut stats) = get_statistics() {
        stats.record_dx8_skin_polys_and_vertices(pcount, vcount);
    }
}

pub fn record_dx8_polys_and_vertices(pcount: i32, vcount: i32) {
    if let Some(mut stats) = get_statistics() {
        stats.record_dx8_polys_and_vertices(pcount, vcount);
    }
}

pub fn record_sorting_polys_and_vertices(pcount: i32, vcount: i32) {
    if let Some(mut stats) = get_statistics() {
        stats.record_sorting_polys_and_vertices(pcount, vcount);
    }
}

pub fn get_dx8_polygons() -> i32 {
    get_statistics().map(|s| s.get_dx8_polygons()).unwrap_or(0)
}
pub fn get_dx8_vertices() -> i32 {
    get_statistics().map(|s| s.get_dx8_vertices()).unwrap_or(0)
}
pub fn get_dx8_skin_polygons() -> i32 {
    get_statistics().map(|s| s.get_dx8_skin_polygons()).unwrap_or(0)
}
pub fn get_dx8_skin_vertices() -> i32 {
    get_statistics().map(|s| s.get_dx8_skin_vertices()).unwrap_or(0)
}
pub fn get_dx8_skin_renders() -> i32 {
    get_statistics().map(|s| s.get_dx8_skin_renders()).unwrap_or(0)
}
pub fn get_sorting_polygons() -> i32 {
    get_statistics().map(|s| s.get_sorting_polygons()).unwrap_or(0)
}
pub fn get_sorting_vertices() -> i32 {
    get_statistics().map(|s| s.get_sorting_vertices()).unwrap_or(0)
}
pub fn get_draw_calls() -> i32 {
    get_statistics().map(|s| s.get_draw_calls()).unwrap_or(0)
}

pub fn get_statistics_string() -> String {
    if let Some(stats) = get_statistics() {
        stats.get_statistics_string().to_string()
    } else {
        String::new()
    }
}

pub fn record_texture_mode(mode: RecordTextureMode) {
    if let Some(mut stats) = get_statistics() {
        stats.set_record_texture_mode(mode);
    }
}

pub fn get_record_texture_mode() -> RecordTextureMode {
    get_statistics()
        .map(|stats| stats.get_recording_mode())
        .unwrap_or(RecordTextureMode::NoRecording)
}

pub fn get_performance_summary() -> Option<PerformanceSummary> {
    get_statistics().map(|stats| stats.get_performance_summary())
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_debug_statistics_creation() {
        let stats = DebugStatistics::new();
        assert!(!stats.is_currently_recording());
        assert_eq!(stats.get_total_frames(), 0);
        assert_eq!(stats.get_current_texture_count(), 0);
    }

    #[test]
    fn test_texture_statistics_entry() {
        let texture = Arc::new(crate::texture_system::TextureClass::new(
            "stats_texture",
            1,
            1,
        ));
        let entry = TextureStatisticsEntry::new(Some(Arc::clone(&texture)));

        assert_eq!(entry.usage_count, 0);
        assert_eq!(entry.change_count, 0);

        // Record usage
        let mut entry_clone = entry.clone();
        entry_clone.record_usage();
        entry_clone.record_change();

        assert_eq!(entry_clone.usage_count, 1);
        assert_eq!(entry_clone.change_count, 1);
    }

    #[test]
    fn test_frame_statistics() {
        let frame_stats = FrameStatistics::new();
        assert_eq!(frame_stats.texture_count, 0);
        assert_eq!(frame_stats.texture_memory, 0);
        assert_eq!(frame_stats.texture_change_count, 0);
    }

    #[test]
    fn test_memory_breakdown() {
        let breakdown = MemoryBreakdown {
            total_texture_memory: 1024 * 1024,     // 1MB
            lightmap_texture_memory: 512 * 1024,   // 512KB
            procedural_texture_memory: 256 * 1024, // 256KB
            other_memory: 128 * 1024,              // 128KB
        };

        assert_eq!(
            breakdown.get_total_memory(),
            1024 * 1024 + 512 * 1024 + 256 * 1024 + 128 * 1024
        );
        assert!(breakdown.to_string().contains("1920 KB"));
    }

    #[test]
    fn test_performance_summary() {
        let summary = PerformanceSummary {
            average_fps: 60.0,
            average_frame_time: Duration::from_millis(16),
            total_frames: 1000,
            texture_change_rate: 2.5,
            memory_efficiency: 0.001,
        };

        let summary_str = summary.to_string();
        assert!(summary_str.contains("60.0"));
        assert!(summary_str.contains("16.00ms"));
        assert!(summary_str.contains("1000"));
    }

    #[test]
    fn test_statistics_recording() {
        let mut stats = DebugStatistics::new();

        // Start recording
        stats.begin_recording(RecordTextureMode::RecordSummary);
        assert!(stats.is_currently_recording());
        assert_eq!(stats.get_recording_mode(), RecordTextureMode::RecordSummary);

        // Record some texture usage
        let texture = Arc::new(crate::texture_system::TextureClass::new(
            "stats_texture",
            1,
            1,
        ));
        stats.record_texture(Some(&texture));

        assert_eq!(stats.get_current_texture_count(), 1);
        assert_eq!(stats.get_current_texture_memory(), 0); // Default texture has no memory

        // End recording
        stats.end_recording();
        assert!(!stats.is_currently_recording());
        assert_eq!(stats.get_total_frames(), 1);

        // Check last frame stats
        let last_frame = stats.get_last_frame_stats();
        assert_eq!(last_frame.texture_count, 1);
        assert_eq!(last_frame.texture_change_count, 1);
    }

    #[test]
    fn test_statistics_reset() {
        let mut stats = DebugStatistics::new();

        // Add some data
        stats.total_frames = 100;
        stats.current_texture_count = 50;

        // Reset
        stats.reset();

        assert_eq!(stats.get_total_frames(), 0);
        assert_eq!(stats.get_current_texture_count(), 0);
        assert!(stats.texture_statistics.is_empty());
    }
}
