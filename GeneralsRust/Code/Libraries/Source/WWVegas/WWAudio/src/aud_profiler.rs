//! Audio Profiler System
//! 
//! Provides comprehensive performance profiling for the audio system,
//! including cache statistics, CPU usage tracking, and timing analysis.
//! This is a direct conversion of the C++ AUD_Profiler.cpp file to 
//! idiomatic Rust with enhanced safety and performance monitoring.

use std::time::{Instant, Duration};
use std::sync::{Arc, Mutex, atomic::{AtomicU64, AtomicUsize, Ordering}};
use std::collections::VecDeque;
use crate::time::{Timestamp, SECONDS};

/// Maximum length of profiler names
pub const MAX_PROF_NAME: usize = 64;

/// Profiler states for CPU tracking
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProfilerState {
    Idle,
    Profiling,
    Paused,
}

/// Cache profiling data
/// 
/// Tracks cache performance metrics including hit rates,
/// loading times, and memory usage statistics.
#[derive(Debug)]
pub struct ProfileData {
    /// High-resolution timer frequency
    freq: u64,
    
    /// Update interval for statistics
    update_interval: Duration,
    
    /// Cache configuration
    cache_size: usize,
    num_pages: usize,
    page_size: usize,
    
    /// Hit/miss statistics
    hits: AtomicUsize,
    misses: AtomicUsize,
    hit_percent: AtomicUsize,
    
    /// Memory usage
    pages_used: AtomicUsize,
    cache_used: AtomicUsize,
    
    /// Performance counters
    frames: AtomicUsize,
    page_count: AtomicUsize,
    
    /// Loading statistics
    total_time: AtomicU64,
    total_data_bytes: AtomicU64,
    total_frame_bytes: AtomicU64,
    bytes_per_frame: AtomicUsize,
    
    /// Decompression statistics
    total_decomp_time: AtomicU64,
    total_decomp_bytes: AtomicU64,
    total_load_time: AtomicU64,
    total_load_bytes: AtomicU64,
    
    /// Performance metrics
    total_bytes_per_second: AtomicUsize,
    decomp_bytes_per_second: AtomicUsize,
    load_bytes_per_second: AtomicUsize,
    
    /// Frame timing
    longest_frame: AtomicU64,
    longest_frame_pages: AtomicUsize,
    longest_frame_bytes: AtomicUsize,
    next_longest_frame: AtomicU64,
    next_longest_frame_pages: AtomicUsize,
    next_longest_frame_bytes: AtomicUsize,
    
    /// Current loading session
    session: Mutex<LoadingSession>,
}

/// Individual loading session data
#[derive(Debug)]
struct LoadingSession {
    start_time: Option<Instant>,
    decomp_start_time: Option<Instant>,
    data_bytes: usize,
    loaded_bytes: usize,
}

impl Default for LoadingSession {
    fn default() -> Self {
        LoadingSession {
            start_time: None,
            decomp_start_time: None,
            data_bytes: 0,
            loaded_bytes: 0,
        }
    }
}

impl ProfileData {
    /// Create a new profile data instance
    /// 
    /// # Arguments
    /// * `pages` - Number of cache pages
    /// * `page_size` - Size of each page in bytes
    /// 
    /// # Returns
    /// A new ProfileData instance
    pub fn new(pages: usize, page_size: usize) -> Self {
        ProfileData {
            freq: Self::get_performance_frequency(),
            update_interval: Duration::from_millis(10),
            cache_size: pages * page_size,
            num_pages: pages,
            page_size,
            hits: AtomicUsize::new(0),
            misses: AtomicUsize::new(0),
            hit_percent: AtomicUsize::new(0),
            pages_used: AtomicUsize::new(0),
            cache_used: AtomicUsize::new(0),
            frames: AtomicUsize::new(0),
            page_count: AtomicUsize::new(0),
            total_time: AtomicU64::new(0),
            total_data_bytes: AtomicU64::new(0),
            total_frame_bytes: AtomicU64::new(0),
            bytes_per_frame: AtomicUsize::new(0),
            total_decomp_time: AtomicU64::new(0),
            total_decomp_bytes: AtomicU64::new(0),
            total_load_time: AtomicU64::new(0),
            total_load_bytes: AtomicU64::new(0),
            total_bytes_per_second: AtomicUsize::new(0),
            decomp_bytes_per_second: AtomicUsize::new(0),
            load_bytes_per_second: AtomicUsize::new(0),
            longest_frame: AtomicU64::new(0),
            longest_frame_pages: AtomicUsize::new(0),
            longest_frame_bytes: AtomicUsize::new(0),
            next_longest_frame: AtomicU64::new(0),
            next_longest_frame_pages: AtomicUsize::new(0),
            next_longest_frame_bytes: AtomicUsize::new(0),
            session: Mutex::new(LoadingSession::default()),
        }
    }

    /// Set the update interval for statistics
    /// 
    /// # Arguments
    /// * `milliseconds` - Update interval in milliseconds
    pub fn set_update_interval(&mut self, milliseconds: u64) {
        self.update_interval = Duration::from_millis(milliseconds);
    }

    /// Start a new frame for profiling
    pub fn new_frame(&self) {
        let current_frames = self.frames.fetch_add(1, Ordering::SeqCst);
        let current_time = self.total_time.load(Ordering::SeqCst);
        let current_pages = self.page_count.load(Ordering::SeqCst);
        let current_bytes = self.total_data_bytes.load(Ordering::SeqCst) as usize;
        
        // Update longest frame statistics
        let longest = self.longest_frame.load(Ordering::SeqCst);
        if current_time > longest {
            // Store previous longest as next longest
            self.next_longest_frame.store(longest, Ordering::SeqCst);
            self.next_longest_frame_pages.store(
                self.longest_frame_pages.load(Ordering::SeqCst), 
                Ordering::SeqCst
            );
            self.next_longest_frame_bytes.store(
                self.longest_frame_bytes.load(Ordering::SeqCst), 
                Ordering::SeqCst
            );
            
            // Update longest frame
            self.longest_frame.store(current_time, Ordering::SeqCst);
            self.longest_frame_pages.store(current_pages, Ordering::SeqCst);
            self.longest_frame_bytes.store(current_bytes, Ordering::SeqCst);
        } else if current_time > self.next_longest_frame.load(Ordering::SeqCst) {
            self.next_longest_frame.store(current_time, Ordering::SeqCst);
            self.next_longest_frame_pages.store(current_pages, Ordering::SeqCst);
            self.next_longest_frame_bytes.store(current_bytes, Ordering::SeqCst);
        }
        
        // Reset page count for new frame
        self.page_count.store(0, Ordering::SeqCst);
        
        // Update hit percentage
        let total_requests = self.hits.load(Ordering::SeqCst) + self.misses.load(Ordering::SeqCst);
        if total_requests > 0 {
            let hit_rate = (self.hits.load(Ordering::SeqCst) * 100) / total_requests;
            self.hit_percent.store(hit_rate, Ordering::SeqCst);
        }
        
        // Update bytes per frame
        let frame_count = current_frames + 1;
        if frame_count > 0 {
            let total_frame_bytes = self.total_frame_bytes.load(Ordering::SeqCst) as usize;
            self.bytes_per_frame.store(total_frame_bytes / frame_count, Ordering::SeqCst);
        }
        
        // Reset statistics every 90 frames (3 seconds at 30fps)
        if frame_count > 90 {
            self.total_frame_bytes.store(0, Ordering::SeqCst);
            self.frames.store(0, Ordering::SeqCst);
        }
        
        // Reset hit/miss counters every 30 frames (1 second at 30fps)
        if frame_count % 30 == 0 {
            self.hits.store(0, Ordering::SeqCst);
            self.misses.store(0, Ordering::SeqCst);
        }
        
        // Update throughput statistics
        self.update_throughput_stats();
    }

    /// Start loading profiling session
    /// 
    /// # Arguments
    /// * `initial_bytes` - Initial byte count for this session
    pub fn load_start(&self, initial_bytes: usize) {
        if let Ok(mut session) = self.session.lock() {
            session.start_time = Some(Instant::now());
            session.data_bytes = initial_bytes;
            session.loaded_bytes = 0;
        }
        self.page_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Add bytes to current loading session
    /// 
    /// # Arguments
    /// * `bytes` - Number of bytes loaded
    pub fn add_load_bytes(&self, bytes: usize) {
        if let Ok(mut session) = self.session.lock() {
            session.data_bytes += bytes;
        }
    }

    /// Start decompression phase
    /// 
    /// # Arguments
    /// * `compressed_bytes` - Number of compressed bytes
    pub fn decompress_start(&self, compressed_bytes: usize) {
        if let Ok(mut session) = self.session.lock() {
            session.decomp_start_time = Some(Instant::now());
            session.loaded_bytes = compressed_bytes;
        }
    }

    /// End loading profiling session
    pub fn load_end(&self) {
        if let Ok(mut session) = self.session.lock() {
            if let Some(start_time) = session.start_time {
                let elapsed = start_time.elapsed();
                let elapsed_nanos = elapsed.as_nanos() as u64;
                
                // Update totals
                self.total_time.fetch_add(elapsed_nanos, Ordering::SeqCst);
                self.total_data_bytes.fetch_add(session.data_bytes as u64, Ordering::SeqCst);
                self.total_frame_bytes.fetch_add(session.data_bytes as u64, Ordering::SeqCst);
                
                // Handle decompression timing
                if let Some(decomp_start) = session.decomp_start_time {
                    let load_time = decomp_start.duration_since(start_time).as_nanos() as u64;
                    let decomp_time = decomp_start.elapsed().as_nanos() as u64;
                    
                    self.total_load_time.fetch_add(load_time, Ordering::SeqCst);
                    self.total_decomp_time.fetch_add(decomp_time, Ordering::SeqCst);
                    self.total_load_bytes.fetch_add(session.loaded_bytes as u64, Ordering::SeqCst);
                    self.total_decomp_bytes.fetch_add(session.data_bytes as u64, Ordering::SeqCst);
                } else {
                    self.total_load_time.fetch_add(elapsed_nanos, Ordering::SeqCst);
                    self.total_load_bytes.fetch_add(session.data_bytes as u64, Ordering::SeqCst);
                }
                
                // Reset session
                *session = LoadingSession::default();
            }
        }
    }

    /// Record cache hit
    pub fn cache_hit(&self) {
        self.hits.fetch_add(1, Ordering::SeqCst);
    }

    /// Record cache miss
    pub fn cache_miss(&self) {
        self.misses.fetch_add(1, Ordering::SeqCst);
    }

    /// Add a page to cache usage
    pub fn add_page(&self) {
        self.pages_used.fetch_add(1, Ordering::SeqCst);
    }

    /// Remove a page from cache usage
    pub fn remove_page(&self) {
        self.pages_used.fetch_sub(1, Ordering::SeqCst);
    }

    /// Fill cache with data
    /// 
    /// # Arguments
    /// * `bytes` - Number of bytes added to cache
    pub fn fill(&self, bytes: usize) {
        self.cache_used.fetch_add(bytes, Ordering::SeqCst);
    }

    /// Remove data from cache
    /// 
    /// # Arguments
    /// * `bytes` - Number of bytes removed from cache
    pub fn remove(&self, bytes: usize) {
        self.cache_used.fetch_sub(bytes, Ordering::SeqCst);
    }

    /// Get cache hit percentage
    pub fn get_hit_percent(&self) -> usize {
        self.hit_percent.load(Ordering::SeqCst)
    }

    /// Get pages used percentage
    pub fn get_pages_used_percent(&self) -> usize {
        if self.num_pages > 0 {
            (self.pages_used.load(Ordering::SeqCst) * 100) / self.num_pages
        } else {
            0
        }
    }

    /// Get cache filled percentage
    pub fn get_cache_filled_percent(&self) -> usize {
        let used_pages = self.pages_used.load(Ordering::SeqCst);
        if used_pages > 0 {
            let used_capacity = used_pages * self.page_size;
            let cache_used = self.cache_used.load(Ordering::SeqCst);
            (cache_used * 100) / used_capacity
        } else {
            0
        }
    }

    /// Get throughput in KB/s
    pub fn get_total_throughput_kbps(&self) -> usize {
        self.total_bytes_per_second.load(Ordering::SeqCst) / 1024
    }

    /// Get loading throughput in KB/s
    pub fn get_load_throughput_kbps(&self) -> usize {
        self.load_bytes_per_second.load(Ordering::SeqCst) / 1024
    }

    /// Get decompression throughput in KB/s
    pub fn get_decomp_throughput_kbps(&self) -> usize {
        self.decomp_bytes_per_second.load(Ordering::SeqCst) / 1024
    }

    /// Get bytes per frame
    pub fn get_bytes_per_frame(&self) -> usize {
        self.bytes_per_frame.load(Ordering::SeqCst)
    }

    /// Generate profiling report
    /// 
    /// # Arguments
    /// * `writer` - Function to call for each line of output
    pub fn generate_report<F>(&self, writer: F)
    where
        F: Fn(&str),
    {
        writer("Audio Cache Stats");
        writer(&format!("Hits: {}%", self.get_hit_percent()));
        
        let pages_used_pct = self.get_pages_used_percent();
        let cache_filled_pct = self.get_cache_filled_percent();
        writer(&format!("Used: {}% ({}%)", pages_used_pct, cache_filled_pct));
        
        let total_kbps = self.get_total_throughput_kbps();
        let load_kbps = self.get_load_throughput_kbps();
        let decomp_kbps = self.get_decomp_throughput_kbps();
        writer(&format!(
            "KbPS: {}.{:02} ({}.{:02},{}.{:02})",
            total_kbps / 100, (total_kbps % 100),
            load_kbps / 100, (load_kbps % 100),
            decomp_kbps / 100, (decomp_kbps % 100)
        ));
        
        let bytes_per_frame = self.get_bytes_per_frame();
        writer(&format!(
            "KPF: {}.{:02}",
            bytes_per_frame / 1024,
            ((bytes_per_frame % 1024) * 100) / 1024
        ));
        
        // Longest frame info
        let longest_time = self.longest_frame.load(Ordering::SeqCst);
        let longest_pages = self.longest_frame_pages.load(Ordering::SeqCst);
        let longest_bytes = self.longest_frame_bytes.load(Ordering::SeqCst);
        
        if self.freq > 0 {
            let seconds = longest_time / self.freq;
            let centiseconds = ((longest_time % self.freq) * 100) / self.freq;
            writer(&format!(
                " LF: {}.{:02}s; {} pages; {} Kb",
                seconds, centiseconds, longest_pages, longest_bytes / 1024
            ));
        }
        
        // Next longest frame info
        let next_longest_time = self.next_longest_frame.load(Ordering::SeqCst);
        let next_longest_pages = self.next_longest_frame_pages.load(Ordering::SeqCst);
        let next_longest_bytes = self.next_longest_frame_bytes.load(Ordering::SeqCst);
        
        if self.freq > 0 {
            let seconds = next_longest_time / self.freq;
            let centiseconds = ((next_longest_time % self.freq) * 100) / self.freq;
            writer(&format!(
                "NLF: {}.{:02}s; {} pages; {} Kb",
                seconds, centiseconds, next_longest_pages, next_longest_bytes / 1024
            ));
        }
    }

    // Private helper methods

    fn get_performance_frequency() -> u64 {
        // On modern systems, we can use nanosecond precision
        1_000_000_000 // 1 billion nanoseconds per second
    }

    fn update_throughput_stats(&self) {
        let current_time = self.total_time.load(Ordering::SeqCst);
        let update_nanos = self.update_interval.as_nanos() as u64 * self.freq / 1_000_000_000;
        
        if current_time > update_nanos {
            let elapsed_ms = (current_time * 1000) / self.freq;
            
            if elapsed_ms > 0 {
                // Update total throughput
                let total_bytes = self.total_data_bytes.load(Ordering::SeqCst);
                let throughput = ((total_bytes * 1000) / elapsed_ms) as usize;
                self.total_bytes_per_second.store(throughput, Ordering::SeqCst);
                
                // Update decompression throughput
                let decomp_time = self.total_decomp_time.load(Ordering::SeqCst);
                if decomp_time > 0 {
                    let decomp_ms = (decomp_time * 1000) / self.freq;
                    if decomp_ms > 0 {
                        let decomp_bytes = self.total_decomp_bytes.load(Ordering::SeqCst);
                        let decomp_throughput = ((decomp_bytes * 1000) / decomp_ms) as usize;
                        self.decomp_bytes_per_second.store(decomp_throughput, Ordering::SeqCst);
                    }
                }
                
                // Update loading throughput
                let load_time = self.total_load_time.load(Ordering::SeqCst);
                if load_time > 0 {
                    let load_ms = (load_time * 1000) / self.freq;
                    if load_ms > 0 {
                        let load_bytes = self.total_load_bytes.load(Ordering::SeqCst);
                        let load_throughput = ((load_bytes * 1000) / load_ms) as usize;
                        self.load_bytes_per_second.store(load_throughput, Ordering::SeqCst);
                    }
                }
            }
            
            // Reset timing counters
            self.total_data_bytes.store(0, Ordering::SeqCst);
            self.total_time.store(0, Ordering::SeqCst);
            self.total_decomp_bytes.store(0, Ordering::SeqCst);
            self.total_decomp_time.store(0, Ordering::SeqCst);
            self.total_load_bytes.store(0, Ordering::SeqCst);
            self.total_load_time.store(0, Ordering::SeqCst);
        }
    }
}

/// CPU profiler for tracking processing time
/// 
/// Provides detailed CPU usage statistics and performance analysis
/// for audio processing threads and operations.
#[derive(Debug)]
pub struct ProfileCpu {
    /// Profiler name
    name: String,
    
    /// Current state
    state: ProfilerState,
    
    /// High-resolution timer frequency
    freq: u64,
    
    /// Timing data
    start_time: Option<Instant>,
    last_start: Option<Instant>,
    total_ticks: AtomicU64,
    total_cpu_ticks: AtomicU64,
    last_ticks: AtomicU64,
    last_cpu: AtomicU64,
    
    /// Performance metrics
    cpu_usage: AtomicUsize, // In tenths of percent (e.g., 155 = 15.5%)
    
    /// Update timing
    update_interval: Duration,
    overflow_interval: Duration,
    last_update: Option<Instant>,
}

impl ProfileCpu {
    /// Create a new CPU profiler
    /// 
    /// # Arguments
    /// * `name` - Name for this profiler
    /// 
    /// # Returns
    /// A new ProfileCpu instance
    pub fn new(name: String) -> Self {
        let freq = ProfileData::get_performance_frequency();
        let update_interval = SECONDS(1);
        let overflow_interval = if freq > 0 {
            Duration::from_secs(u64::MAX / freq)
        } else {
            update_interval
        };
        
        ProfileCpu {
            name: name.into_bytes().into_iter()
                .take(MAX_PROF_NAME - 1)
                .map(|b| b as char)
                .collect(),
            state: ProfilerState::Idle,
            freq,
            start_time: None,
            last_start: None,
            total_ticks: AtomicU64::new(0),
            total_cpu_ticks: AtomicU64::new(0),
            last_ticks: AtomicU64::new(0),
            last_cpu: AtomicU64::new(0),
            cpu_usage: AtomicUsize::new(0),
            update_interval: update_interval.min(overflow_interval),
            overflow_interval,
            last_update: None,
        }
    }

    /// Start profiling
    pub fn start(&mut self) {
        if self.state == ProfilerState::Idle {
            let now = Instant::now();
            self.start_time = Some(now);
            
            if let Some(last_start) = self.last_start {
                let cpu_ticks = Self::calc_ticks(last_start, now);
                self.total_cpu_ticks.fetch_add(cpu_ticks, Ordering::SeqCst);
            }
            
            self.last_start = Some(now);
            self.state = ProfilerState::Profiling;
        }
    }

    /// Pause profiling
    pub fn pause(&mut self) {
        if self.state == ProfilerState::Profiling {
            if let Some(start) = self.start_time {
                let elapsed_ticks = Self::calc_ticks(start, Instant::now());
                self.total_ticks.fetch_add(elapsed_ticks, Ordering::SeqCst);
            }
            self.state = ProfilerState::Paused;
        }
    }

    /// Resume profiling
    pub fn resume(&mut self) {
        if self.state == ProfilerState::Paused {
            self.start_time = Some(Instant::now());
            self.state = ProfilerState::Profiling;
        }
    }

    /// End profiling session
    pub fn end(&mut self) {
        let now = Instant::now();
        
        if self.state != ProfilerState::Idle {
            if let Some(start) = self.start_time {
                if self.total_cpu_ticks.load(Ordering::SeqCst) > 0 {
                    let elapsed_ticks = Self::calc_ticks(start, now);
                    self.total_ticks.fetch_add(elapsed_ticks, Ordering::SeqCst);
                }
            }
            self.state = ProfilerState::Idle;
        }

        // Update statistics if enough time has passed
        let should_update = if let Some(last_update) = self.last_update {
            now.duration_since(last_update) > self.update_interval
        } else {
            true
        };

        if should_update {
            self.last_update = Some(now);
            
            let last_update_duration = self.last_update
                .map(|lu| now.duration_since(lu))
                .unwrap_or(Duration::from_secs(1));
            
            if last_update_duration < self.overflow_interval {
                // Safe to use the data
                let ticks = self.total_ticks.swap(0, Ordering::SeqCst);
                let cpu_ticks = self.total_cpu_ticks.swap(0, Ordering::SeqCst);
                
                self.last_ticks.store(ticks, Ordering::SeqCst);
                self.last_cpu.store(cpu_ticks, Ordering::SeqCst);
                self.calc_stats();
            }
        }
    }

    /// Get CPU usage percentage (in tenths of percent)
    /// 
    /// # Returns
    /// CPU usage (e.g., 155 = 15.5%)
    pub fn get_cpu_usage(&self) -> usize {
        self.cpu_usage.load(Ordering::SeqCst)
    }

    /// Get CPU usage as floating point percentage
    /// 
    /// # Returns
    /// CPU usage as percentage (e.g., 15.5)
    pub fn get_cpu_usage_percent(&self) -> f32 {
        self.get_cpu_usage() as f32 / 10.0
    }

    /// Set profiler name
    /// 
    /// # Arguments
    /// * `name` - New name for the profiler
    pub fn set_name(&mut self, name: String) {
        self.name = name.chars()
            .take(MAX_PROF_NAME - 1)
            .collect();
    }

    /// Generate profiling report
    /// 
    /// # Arguments
    /// * `writer` - Function to call for each line of output
    pub fn generate_report<F>(&self, writer: F)
    where
        F: Fn(&str),
    {
        if self.freq > 0 {
            let usage = self.get_cpu_usage();
            let last_ticks = self.last_ticks.load(Ordering::SeqCst);
            writer(&format!(
                "{}: CPU {}.{} / {}",
                self.name,
                usage / 10,
                usage % 10,
                last_ticks
            ));
        } else {
            writer(&format!("{}: CPU (no timer)", self.name));
        }
    }

    /// Get the current state
    pub fn get_state(&self) -> ProfilerState {
        self.state
    }

    /// Check if currently profiling
    pub fn is_profiling(&self) -> bool {
        self.state == ProfilerState::Profiling
    }

    /// Get name
    pub fn get_name(&self) -> &str {
        &self.name
    }

    // Private helper methods

    fn calc_ticks(start: Instant, end: Instant) -> u64 {
        if end >= start {
            end.duration_since(start).as_nanos() as u64
        } else {
            // Handle overflow case
            (Duration::from_nanos(u64::MAX).as_nanos() as u64)
                .saturating_sub(start.elapsed().as_nanos() as u64)
                .saturating_add(end.elapsed().as_nanos() as u64)
        }
    }

    fn calc_stats(&self) {
        let last_cpu = self.last_cpu.load(Ordering::SeqCst);
        if last_cpu > 0 {
            let last_ticks = self.last_ticks.load(Ordering::SeqCst);
            let usage = ((last_ticks * 1000) / last_cpu) as usize;
            self.cpu_usage.store(usage, Ordering::SeqCst);
        } else {
            self.cpu_usage.store(0, Ordering::SeqCst);
        }
    }
}

/// Collection of profilers for different subsystems
#[derive(Debug)]
pub struct ProfilerCollection {
    profilers: Vec<Arc<Mutex<ProfileCpu>>>,
    cache_profiler: Option<Arc<ProfileData>>,
}

impl ProfilerCollection {
    /// Create a new profiler collection
    pub fn new() -> Self {
        ProfilerCollection {
            profilers: Vec::new(),
            cache_profiler: None,
        }
    }

    /// Add a CPU profiler
    /// 
    /// # Arguments
    /// * `name` - Name for the profiler
    /// 
    /// # Returns
    /// Arc reference to the created profiler
    pub fn add_cpu_profiler(&mut self, name: String) -> Arc<Mutex<ProfileCpu>> {
        let profiler = Arc::new(Mutex::new(ProfileCpu::new(name)));
        self.profilers.push(Arc::clone(&profiler));
        profiler
    }

    /// Set cache profiler
    /// 
    /// # Arguments
    /// * `profiler` - Cache profiler to add
    pub fn set_cache_profiler(&mut self, profiler: Arc<ProfileData>) {
        self.cache_profiler = Some(profiler);
    }

    /// Generate comprehensive report
    /// 
    /// # Arguments
    /// * `writer` - Function to call for each line of output
    pub fn generate_report<F>(&self, writer: F)
    where
        F: Fn(&str) + Clone,
    {
        // Cache profiler report
        if let Some(ref cache_prof) = self.cache_profiler {
            cache_prof.generate_report(writer.clone());
        }

        // CPU profiler reports
        for profiler in &self.profilers {
            if let Ok(prof) = profiler.lock() {
                prof.generate_report(writer.clone());
            }
        }
    }

    /// Get number of profilers
    pub fn len(&self) -> usize {
        self.profilers.len() + if self.cache_profiler.is_some() { 1 } else { 0 }
    }

    /// Check if collection is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for ProfilerCollection {
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
    fn test_profile_data_creation() {
        let profile = ProfileData::new(100, 4096);
        assert_eq!(profile.cache_size, 100 * 4096);
        assert_eq!(profile.num_pages, 100);
        assert_eq!(profile.page_size, 4096);
    }

    #[test]
    fn test_cache_hit_miss_tracking() {
        let profile = ProfileData::new(100, 4096);
        
        profile.cache_hit();
        profile.cache_hit();
        profile.cache_miss();
        
        assert_eq!(profile.hits.load(Ordering::SeqCst), 2);
        assert_eq!(profile.misses.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_page_usage() {
        let profile = ProfileData::new(10, 1024);
        
        profile.add_page();
        profile.add_page();
        profile.add_page();
        
        assert_eq!(profile.get_pages_used_percent(), 30); // 3/10 = 30%
        
        profile.remove_page();
        assert_eq!(profile.get_pages_used_percent(), 20); // 2/10 = 20%
    }

    #[test]
    fn test_loading_session() {
        let profile = ProfileData::new(100, 4096);
        
        profile.load_start(1000);
        profile.add_load_bytes(500);
        thread::sleep(Duration::from_millis(1)); // Ensure some time passes
        profile.load_end();
        
        assert!(profile.total_data_bytes.load(Ordering::SeqCst) > 0);
    }

    #[test]
    fn test_cpu_profiler_creation() {
        let profiler = ProfileCpu::new("test_profiler".to_string());
        assert_eq!(profiler.get_name(), "test_profiler");
        assert_eq!(profiler.get_state(), ProfilerState::Idle);
        assert!(!profiler.is_profiling());
    }

    #[test]
    fn test_cpu_profiler_state_changes() {
        let mut profiler = ProfileCpu::new("test".to_string());
        
        assert_eq!(profiler.get_state(), ProfilerState::Idle);
        
        profiler.start();
        assert_eq!(profiler.get_state(), ProfilerState::Profiling);
        assert!(profiler.is_profiling());
        
        profiler.pause();
        assert_eq!(profiler.get_state(), ProfilerState::Paused);
        assert!(!profiler.is_profiling());
        
        profiler.resume();
        assert_eq!(profiler.get_state(), ProfilerState::Profiling);
        
        profiler.end();
        assert_eq!(profiler.get_state(), ProfilerState::Idle);
    }

    #[test]
    fn test_profiler_collection() {
        let mut collection = ProfilerCollection::new();
        assert!(collection.is_empty());
        
        let cpu_prof = collection.add_cpu_profiler("test_cpu".to_string());
        assert_eq!(collection.len(), 1);
        
        let cache_prof = Arc::new(ProfileData::new(100, 4096));
        collection.set_cache_profiler(cache_prof);
        assert_eq!(collection.len(), 2);
        
        // Test that we can access the CPU profiler
        {
            let prof_guard = cpu_prof.lock().unwrap();
            assert_eq!(prof_guard.get_name(), "test_cpu");
        }
    }

    #[test]
    fn test_report_generation() {
        let profile = ProfileData::new(100, 4096);
        profile.cache_hit();
        profile.cache_hit();
        profile.cache_miss();
        
        let mut output = Vec::new();
        profile.generate_report(|line| {
            output.push(line.to_string());
        });
        
        assert!(!output.is_empty());
        assert!(output[0].contains("Audio Cache Stats"));
    }

    #[test]
    fn test_cpu_usage_calculation() {
        let profiler = ProfileCpu::new("test".to_string());
        
        // Initially should be 0
        assert_eq!(profiler.get_cpu_usage(), 0);
        assert_eq!(profiler.get_cpu_usage_percent(), 0.0);
        
        // After setting some values, should calculate correctly
        profiler.last_ticks.store(1000, Ordering::SeqCst);
        profiler.last_cpu.store(10000, Ordering::SeqCst);
        profiler.calc_stats();
        
        assert_eq!(profiler.get_cpu_usage(), 100); // 1000/10000 * 1000 = 100 (10.0%)
        assert_eq!(profiler.get_cpu_usage_percent(), 10.0);
    }

    #[test]
    fn test_profiler_name_truncation() {
        let long_name = "a".repeat(MAX_PROF_NAME + 10);
        let profiler = ProfileCpu::new(long_name);
        
        assert!(profiler.get_name().len() < MAX_PROF_NAME);
    }

    #[test]
    fn test_cache_fill_percentage() {
        let profile = ProfileData::new(10, 1000); // 10 pages of 1000 bytes each
        
        // Add 3 pages and fill them partially
        profile.add_page();
        profile.add_page();
        profile.add_page();
        profile.fill(1500); // Fill 1.5 pages worth
        
        let filled_pct = profile.get_cache_filled_percent();
        assert_eq!(filled_pct, 50); // 1500 bytes in 3000 bytes capacity = 50%
    }

    #[test]
    fn test_throughput_calculation() {
        let profile = ProfileData::new(100, 4096);
        
        // Simulate some throughput
        profile.total_bytes_per_second.store(1024 * 100, Ordering::SeqCst); // 100 KB/s
        assert_eq!(profile.get_total_throughput_kbps(), 100);
        
        profile.load_bytes_per_second.store(1024 * 50, Ordering::SeqCst); // 50 KB/s
        assert_eq!(profile.get_load_throughput_kbps(), 50);
    }
}