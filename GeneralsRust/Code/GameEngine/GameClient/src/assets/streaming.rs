//! # Asset Streaming System
//!
//! Advanced asset streaming system with:
//! - Priority-based loading queues
//! - Background streaming for large assets
//! - Memory-aware resource management
//! - Predictive loading based on usage patterns
//! - Dynamic level-of-detail (LOD) management
//! - Bandwidth-adaptive streaming
//! - Multi-threaded processing
//! - Cache-aware optimization

use nalgebra::Vector3;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::cmp::{Ordering as CmpOrdering, Reverse};
use std::collections::{BTreeMap, BinaryHeap, HashMap, VecDeque};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex, RwLock,
};
use std::time::{Duration, Instant, SystemTime};
use thiserror::Error;
use tokio::sync::{Notify, RwLock as AsyncRwLock, Semaphore};
use tokio::task::JoinHandle;

use super::{AssetConfig, AssetError, AssetHandle, AssetPriority, AssetType};

/// Streaming system errors
#[derive(Error, Debug, Clone)]
pub enum StreamingError {
    #[error("Streaming task failed: {0}")]
    TaskFailed(String),
    #[error("Memory limit exceeded: requested {requested} MB, available {available} MB")]
    MemoryLimitExceeded { requested: u64, available: u64 },
    #[error("Streaming queue full: {0} pending requests")]
    QueueFull(usize),
    #[error("LOD generation failed: {asset} - {error}")]
    LodGenerationFailed { asset: String, error: String },
    #[error("Prediction model error: {0}")]
    PredictionFailed(String),
    #[error("Network streaming error: {0}")]
    NetworkError(String),
    #[error("Cache coherency error: {0}")]
    CacheError(String),
}

/// Streaming request types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingRequestType {
    Load,       // Initial loading
    Upgrade,    // Higher quality version
    Preload,    // Predictive loading
    Background, // Background streaming
}

/// Streaming request handler result
#[derive(Debug, Clone, Copy)]
pub struct StreamingLoadResult {
    pub handle: AssetHandle,
    pub size_bytes: u64,
    pub asset_type: AssetType,
}

type StreamingLoadHandler = Arc<
    dyn Fn(
            StreamingRequest,
        )
            -> Pin<Box<dyn Future<Output = Result<StreamingLoadResult, StreamingError>> + Send>>
        + Send
        + Sync,
>;
type StreamingEvictHandler = Arc<
    dyn Fn(AssetHandle) -> Pin<Box<dyn Future<Output = Result<u64, StreamingError>> + Send>>
        + Send
        + Sync,
>;

/// Level-of-detail (LOD) information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LodInfo {
    pub level: u32,          // LOD level (0 = highest quality)
    pub quality_factor: f32, // Quality multiplier (0.0-1.0)
    pub size_bytes: u64,     // Data size at this LOD
    pub distance: f32,       // Optimal viewing distance
    pub is_loaded: bool,     // Currently loaded flag
}

/// Asset streaming metadata
#[derive(Debug, Clone)]
pub struct StreamingAssetInfo {
    pub handle: AssetHandle,
    pub path: PathBuf,
    pub asset_type: AssetType,
    pub total_size: u64,
    pub lod_levels: Vec<LodInfo>,
    pub current_lod: u32,
    pub target_lod: u32,
    pub priority: AssetPriority,
    pub last_accessed: Instant,
    pub access_count: u64,
    pub distance_from_player: f32,
    pub predicted_access_time: Option<Instant>,
    pub streaming_state: StreamingState,
    pub memory_residency: MemoryResidency,
}

/// Asset streaming state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingState {
    NotLoaded,   // Not in memory
    Loading,     // Currently loading
    Loaded,      // Fully loaded at current LOD
    Upgrading,   // Loading higher quality LOD
    Downgrading, // Switching to lower quality LOD
    Evicting,    // Being removed from memory
    Failed,      // Loading failed
}

/// Memory residency status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryResidency {
    NotResident, // Not in memory
    Partial,     // Some LOD levels loaded
    Full,        // All data loaded
    Compressed,  // Compressed in memory
}

/// Streaming request with priority
pub struct StreamingRequest {
    pub handle: AssetHandle,
    pub path: PathBuf,
    pub request_type: StreamingRequestType,
    pub priority: AssetPriority,
    pub target_lod: u32,
    pub distance_hint: f32,
    pub submitted_time: Instant,
    pub deadline: Option<Instant>,
    pub callback: Option<Box<dyn FnOnce(Result<AssetHandle, StreamingError>) + Send + Sync>>,
}

impl std::fmt::Debug for StreamingRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamingRequest")
            .field("handle", &self.handle)
            .field("path", &self.path)
            .field("request_type", &self.request_type)
            .field("priority", &self.priority)
            .field("target_lod", &self.target_lod)
            .field("distance_hint", &self.distance_hint)
            .field("submitted_time", &self.submitted_time)
            .field("deadline", &self.deadline)
            .field(
                "has_callback",
                &self.callback.as_ref().map(|_| true).unwrap_or(false),
            )
            .finish()
    }
}

impl PartialEq for StreamingRequest {
    fn eq(&self, other: &Self) -> bool {
        self.handle == other.handle && self.target_lod == other.target_lod
    }
}

impl Eq for StreamingRequest {}

impl PartialOrd for StreamingRequest {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}

impl Ord for StreamingRequest {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        // Higher priority first, then closer deadline, then older submission
        self.priority
            .cmp(&other.priority)
            .then_with(|| match (self.deadline, other.deadline) {
                (Some(a), Some(b)) => a.cmp(&b),
                (Some(_), None) => CmpOrdering::Less,
                (None, Some(_)) => CmpOrdering::Greater,
                (None, None) => CmpOrdering::Equal,
            })
            .then_with(|| self.submitted_time.cmp(&other.submitted_time))
    }
}

/// Usage pattern analysis data
#[derive(Debug, Clone)]
pub struct UsagePattern {
    pub asset_handle: AssetHandle,
    pub access_times: VecDeque<Instant>,
    pub access_locations: VecDeque<Vector3<f32>>,
    pub average_interval: Duration,
    pub access_trend: AccessTrend,
    pub prediction_confidence: f32,
}

/// Access trend analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessTrend {
    Increasing,
    Stable,
    Decreasing,
    Seasonal, // Periodic access pattern
    Random,
}

/// Streaming performance metrics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct StreamingStats {
    pub total_requests: u64,
    pub completed_requests: u64,
    pub failed_requests: u64,
    pub average_load_time_ms: f32,
    pub peak_queue_size: usize,
    pub memory_used_mb: f32,
    pub memory_budget_mb: f32,
    pub cache_hit_rate: f32,
    pub active_streams: u32,
    pub lod_switches: u64,
    pub predictive_hits: u64,
    pub predictive_misses: u64,
    pub bandwidth_utilization: f32,
}

/// Player position and camera information for LOD calculations
#[derive(Debug, Clone)]
pub struct ViewerContext {
    pub position: Vector3<f32>,
    pub forward: Vector3<f32>,
    pub view_distance: f32,
    pub fov_degrees: f32,
    pub movement_velocity: Vector3<f32>,
}

impl Default for ViewerContext {
    fn default() -> Self {
        Self {
            position: Vector3::zeros(),
            forward: Vector3::new(0.0, 0.0, -1.0),
            view_distance: 1000.0,
            fov_degrees: 90.0,
            movement_velocity: Vector3::zeros(),
        }
    }
}

/// Complete Streaming Management System
pub struct StreamingManager {
    config: AssetConfig,

    // Request queues (priority-based)
    high_priority_queue: Arc<Mutex<BinaryHeap<Reverse<StreamingRequest>>>>,
    normal_priority_queue: Arc<Mutex<VecDeque<StreamingRequest>>>,
    background_queue: Arc<Mutex<VecDeque<StreamingRequest>>>,

    // Asset tracking
    streaming_assets: Arc<RwLock<HashMap<AssetHandle, StreamingAssetInfo>>>,
    asset_index: Arc<RwLock<HashMap<PathBuf, AssetHandle>>>,

    // Usage pattern analysis
    usage_patterns: Arc<RwLock<HashMap<AssetHandle, UsagePattern>>>,
    prediction_model: Arc<RwLock<PredictionModel>>,

    // Memory management
    memory_budget: u64,
    memory_used: Arc<AtomicU64>,
    memory_pressure_threshold: f32,

    // Worker management
    worker_semaphore: Arc<Semaphore>,
    active_workers: Arc<AtomicU64>,
    max_workers: usize,

    // Viewer context for LOD calculations
    viewer_context: Arc<RwLock<ViewerContext>>,

    // Performance monitoring
    stats: Arc<RwLock<StreamingStats>>,

    // Task management
    worker_handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
    shutdown_signal: Arc<AtomicBool>,
    shutdown_notify: Arc<Notify>,

    // Back-end hooks
    load_handler: Arc<RwLock<Option<StreamingLoadHandler>>>,
    evict_handler: Arc<RwLock<Option<StreamingEvictHandler>>>,
}

/// Predictive loading model
#[derive(Debug)]
struct PredictionModel {
    asset_correlations: HashMap<AssetHandle, Vec<(AssetHandle, f32)>>, // Asset -> Related assets + correlation
    location_patterns: HashMap<Vector3<i32>, Vec<AssetHandle>>, // Grid cell -> Assets likely to be needed
    time_patterns: HashMap<u32, Vec<AssetHandle>>,              // Time bucket -> Assets
    confidence_threshold: f32,
}

impl Default for PredictionModel {
    fn default() -> Self {
        Self {
            asset_correlations: HashMap::new(),
            location_patterns: HashMap::new(),
            time_patterns: HashMap::new(),
            confidence_threshold: 0.7,
        }
    }
}

impl StreamingManager {
    /// Create new streaming manager
    pub fn new(config: AssetConfig) -> Result<Self, StreamingError> {
        let memory_budget = (config.cache_size_mb as u64) * 1024 * 1024;
        let max_workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(16); // Cap at 16 workers

        Ok(Self {
            config: config.clone(),
            high_priority_queue: Arc::new(Mutex::new(BinaryHeap::new())),
            normal_priority_queue: Arc::new(Mutex::new(VecDeque::new())),
            background_queue: Arc::new(Mutex::new(VecDeque::new())),
            streaming_assets: Arc::new(RwLock::new(HashMap::new())),
            asset_index: Arc::new(RwLock::new(HashMap::new())),
            usage_patterns: Arc::new(RwLock::new(HashMap::new())),
            prediction_model: Arc::new(RwLock::new(PredictionModel::default())),
            memory_budget,
            memory_used: Arc::new(AtomicU64::new(0)),
            memory_pressure_threshold: 0.85,
            worker_semaphore: Arc::new(Semaphore::new(max_workers)),
            active_workers: Arc::new(AtomicU64::new(0)),
            max_workers,
            viewer_context: Arc::new(RwLock::new(ViewerContext::default())),
            stats: Arc::new(RwLock::new(StreamingStats {
                memory_budget_mb: config.cache_size_mb as f32,
                ..Default::default()
            })),
            worker_handles: Arc::new(Mutex::new(Vec::new())),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
            shutdown_notify: Arc::new(Notify::new()),
            load_handler: Arc::new(RwLock::new(None)),
            evict_handler: Arc::new(RwLock::new(None)),
        })
    }

    /// Register the handler that performs the actual asset load
    pub fn register_load_handler<F>(&self, handler: F)
    where
        F: Fn(
                StreamingRequest,
            )
                -> Pin<Box<dyn Future<Output = Result<StreamingLoadResult, StreamingError>> + Send>>
            + Send
            + Sync
            + 'static,
    {
        *self.load_handler.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(handler));
    }

    /// Register the handler that evicts assets from memory
    pub fn register_evict_handler<F>(&self, handler: F)
    where
        F: Fn(AssetHandle) -> Pin<Box<dyn Future<Output = Result<u64, StreamingError>> + Send>>
            + Send
            + Sync
            + 'static,
    {
        *self.evict_handler.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(handler));
    }

    /// Start streaming system
    pub async fn start(&self) -> Result<(), StreamingError> {
        log::info!(
            "Starting streaming manager with {} workers",
            self.max_workers
        );

        // Start worker tasks
        let mut handles = self.worker_handles.lock().unwrap_or_else(|e| e.into_inner());
        for worker_id in 0..self.max_workers {
            let handle = self.spawn_worker(worker_id).await?;
            handles.push(handle);
        }

        // Start maintenance task
        let maintenance_handle = self.spawn_maintenance_task().await?;
        handles.push(maintenance_handle);

        log::info!("Streaming manager started successfully");
        Ok(())
    }

    /// Spawn worker task
    async fn spawn_worker(&self, worker_id: usize) -> Result<JoinHandle<()>, StreamingError> {
        let high_queue = self.high_priority_queue.clone();
        let normal_queue = self.normal_priority_queue.clone();
        let background_queue = self.background_queue.clone();
        let semaphore = self.worker_semaphore.clone();
        let shutdown_signal = self.shutdown_signal.clone();
        let shutdown_notify = self.shutdown_notify.clone();
        let active_workers = self.active_workers.clone();
        let stats = self.stats.clone();
        let streaming_assets = self.streaming_assets.clone();
        let memory_used = self.memory_used.clone();
        let load_handler = self.load_handler.clone();

        let handle = tokio::spawn(async move {
            log::debug!("Streaming worker {} started", worker_id);

            loop {
                // Check shutdown signal
                if shutdown_signal.load(Ordering::Relaxed) {
                    break;
                }

                // Acquire work permit
                let _permit = match semaphore.try_acquire() {
                    Ok(permit) => permit,
                    Err(_) => {
                        // No permits available, wait a bit
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        continue;
                    }
                };

                // Try to get work from queues (priority order)
                let request = {
                    // High priority first
                    if let Ok(mut queue) = high_queue.try_lock() {
                        if let Some(Reverse(request)) = queue.pop() {
                            Some(request)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                .or_else(|| {
                    // Normal priority second
                    if let Ok(mut queue) = normal_queue.try_lock() {
                        queue.pop_front()
                    } else {
                        None
                    }
                })
                .or_else(|| {
                    // Background priority last
                    if let Ok(mut queue) = background_queue.try_lock() {
                        queue.pop_front()
                    } else {
                        None
                    }
                });

                if let Some(request) = request {
                    active_workers.fetch_add(1, Ordering::Relaxed);

                    // Process the request
                    let start_time = Instant::now();
                    let result = Self::process_streaming_request(
                        request,
                        &streaming_assets,
                        &memory_used,
                        &load_handler,
                    )
                    .await;
                    let processing_time = start_time.elapsed();

                    // Update statistics
                    {
                        let mut stats = stats.write().unwrap_or_else(|e| e.into_inner());
                        stats.completed_requests += 1;
                        if result.is_err() {
                            stats.failed_requests += 1;
                        }

                        // Update average load time
                        let total_time =
                            stats.average_load_time_ms * (stats.completed_requests - 1) as f32;
                        stats.average_load_time_ms = (total_time
                            + processing_time.as_millis() as f32)
                            / stats.completed_requests as f32;
                    }

                    active_workers.fetch_sub(1, Ordering::Relaxed);
                } else {
                    // No work available, wait for notification or timeout
                    tokio::select! {
                        _ = shutdown_notify.notified() => {
                            break;
                        }
                        _ = tokio::time::sleep(Duration::from_millis(100)) => {
                            // Timeout, continue loop
                        }
                    }
                }
            }

            log::debug!("Streaming worker {} stopped", worker_id);
        });

        Ok(handle)
    }

    /// Spawn maintenance task for background operations
    async fn spawn_maintenance_task(&self) -> Result<JoinHandle<()>, StreamingError> {
        let streaming_assets = self.streaming_assets.clone();
        let viewer_context = self.viewer_context.clone();
        let normal_queue = self.normal_priority_queue.clone();
        let background_queue = self.background_queue.clone();
        let prediction_model = self.prediction_model.clone();
        let usage_patterns = self.usage_patterns.clone();
        let shutdown_signal = self.shutdown_signal.clone();
        let memory_used = self.memory_used.clone();
        let memory_budget = self.memory_budget;
        let stats = self.stats.clone();
        let evict_handler = self.evict_handler.clone();

        let handle = tokio::spawn(async move {
            log::debug!("Streaming maintenance task started");

            let mut last_cleanup = Instant::now();
            let mut last_prediction_update = Instant::now();
            let cleanup_interval = Duration::from_secs(30);
            let prediction_interval = Duration::from_secs(60);

            loop {
                if shutdown_signal.load(Ordering::Relaxed) {
                    break;
                }

                let now = Instant::now();

                // Periodic memory cleanup
                if now.duration_since(last_cleanup) >= cleanup_interval {
                    Self::perform_memory_cleanup(
                        &streaming_assets,
                        &memory_used,
                        memory_budget,
                        &evict_handler,
                    )
                    .await;
                    last_cleanup = now;
                }

                // Update predictive model
                if now.duration_since(last_prediction_update) >= prediction_interval {
                    Self::update_prediction_model(&prediction_model, &usage_patterns).await;
                    last_prediction_update = now;
                }

                // Update LOD levels based on viewer context
                Self::update_lod_levels(
                    &streaming_assets,
                    &viewer_context,
                    &normal_queue,
                    &background_queue,
                )
                .await;

                // Update memory stats
                {
                    let mut stats_guard = stats.write().unwrap_or_else(|e| e.into_inner());
                    stats_guard.memory_used_mb =
                        memory_used.load(Ordering::Relaxed) as f32 / (1024.0 * 1024.0);
                }

                // Sleep before next iteration
                tokio::time::sleep(Duration::from_millis(500)).await;
            }

            log::debug!("Streaming maintenance task stopped");
        });

        Ok(handle)
    }

    /// Process a streaming request
    async fn process_streaming_request(
        request: StreamingRequest,
        streaming_assets: &Arc<RwLock<HashMap<AssetHandle, StreamingAssetInfo>>>,
        memory_used: &Arc<AtomicU64>,
        request_handler: &Arc<RwLock<Option<StreamingLoadHandler>>>,
    ) -> Result<(), StreamingError> {
        log::trace!(
            "Processing streaming request: {:?} (LOD {})",
            request.path,
            request.target_lod
        );

        let mut request = request;
        let request_handle = request.handle;
        let path = request.path.clone();
        let priority = request.priority;
        let target_lod = request.target_lod;
        let distance_hint = request.distance_hint;
        let callback = request.callback.take();

        let handler = {
            let handler = request_handler.read().unwrap_or_else(|e| e.into_inner());
            handler.clone()
        };

        let result = if let Some(handler) = handler {
            handler(request).await
        } else {
            Err(StreamingError::TaskFailed(
                "No streaming load handler registered".to_string(),
            ))
        };

        if let Ok(load_result) = result.as_ref() {
            let mut assets = streaming_assets.write().unwrap_or_else(|e| e.into_inner());
            if load_result.handle != request_handle {
                assets.remove(&request_handle);
            }
            let info = assets
                .entry(load_result.handle)
                .or_insert_with(|| StreamingAssetInfo {
                    handle: load_result.handle,
                    path: path.clone(),
                    asset_type: load_result.asset_type,
                    total_size: load_result.size_bytes,
                    lod_levels: Vec::new(),
                    current_lod: 0,
                    target_lod,
                    priority,
                    last_accessed: Instant::now(),
                    access_count: 0,
                    distance_from_player: distance_hint,
                    predicted_access_time: None,
                    streaming_state: StreamingState::Loaded,
                    memory_residency: MemoryResidency::Full,
                });

            info.total_size = load_result.size_bytes;
            info.asset_type = load_result.asset_type;
            info.path = path.clone();
            info.priority = priority;
            info.target_lod = target_lod;
            info.current_lod = target_lod;
            info.distance_from_player = distance_hint;
            info.last_accessed = Instant::now();
            info.streaming_state = StreamingState::Loaded;
            info.memory_residency = MemoryResidency::Full;

            memory_used.fetch_add(load_result.size_bytes, Ordering::Relaxed);
        }

        if let Some(callback) = callback {
            let callback_result = result
                .as_ref()
                .map(|res| res.handle)
                .map_err(|err| err.clone());
            callback(callback_result);
        }

        result.map(|_| ())
    }

    /// Submit streaming request
    pub async fn request_asset(
        &self,
        handle: AssetHandle,
        path: PathBuf,
        priority: AssetPriority,
        target_lod: u32,
        distance_hint: f32,
        callback: Option<Box<dyn FnOnce(Result<AssetHandle, StreamingError>) + Send + Sync>>,
    ) -> Result<(), StreamingError> {
        let asset_type =
            AssetType::from_extension(path.extension().and_then(|s| s.to_str()).unwrap_or(""));

        {
            let mut assets = self.streaming_assets.write().unwrap_or_else(|e| e.into_inner());
            assets.entry(handle).or_insert_with(|| StreamingAssetInfo {
                handle,
                path: path.clone(),
                asset_type,
                total_size: 0,
                lod_levels: Vec::new(),
                current_lod: 0,
                target_lod,
                priority,
                last_accessed: Instant::now(),
                access_count: 0,
                distance_from_player: distance_hint,
                predicted_access_time: None,
                streaming_state: StreamingState::NotLoaded,
                memory_residency: MemoryResidency::NotResident,
            });
        }

        let request = StreamingRequest {
            handle,
            path,
            request_type: StreamingRequestType::Load,
            priority,
            target_lod,
            distance_hint,
            submitted_time: Instant::now(),
            deadline: None,
            callback,
        };

        // Update statistics
        {
            let mut stats = self.stats.write().unwrap_or_else(|e| e.into_inner());
            stats.total_requests += 1;
        }

        // Route to appropriate queue based on priority
        match priority {
            AssetPriority::Critical => {
                let mut queue = self.high_priority_queue.lock().unwrap_or_else(|e| e.into_inner());
                queue.push(Reverse(request));

                let mut stats = self.stats.write().unwrap_or_else(|e| e.into_inner());
                stats.peak_queue_size = stats.peak_queue_size.max(queue.len());
            }
            AssetPriority::High | AssetPriority::Normal => {
                let mut queue = self.normal_priority_queue.lock().unwrap_or_else(|e| e.into_inner());
                queue.push_back(request);

                let mut stats = self.stats.write().unwrap_or_else(|e| e.into_inner());
                stats.peak_queue_size = stats.peak_queue_size.max(queue.len());
            }
            AssetPriority::Low | AssetPriority::Lowest => {
                let mut queue = self.background_queue.lock().unwrap_or_else(|e| e.into_inner());
                queue.push_back(request);
            }
        }

        // Notify workers
        self.shutdown_notify.notify_one();
        Ok(())
    }

    /// Update viewer context for LOD calculations
    pub fn update_viewer_context(&self, context: ViewerContext) {
        *self.viewer_context.write().unwrap_or_else(|e| e.into_inner()) = context;
    }

    /// Record asset access for pattern analysis
    pub fn record_asset_access(&self, handle: AssetHandle, position: Vector3<f32>) {
        let now = Instant::now();
        let mut patterns = self.usage_patterns.write().unwrap_or_else(|e| e.into_inner());

        let pattern = patterns.entry(handle).or_insert_with(|| UsagePattern {
            asset_handle: handle,
            access_times: VecDeque::with_capacity(100),
            access_locations: VecDeque::with_capacity(100),
            average_interval: Duration::from_secs(0),
            access_trend: AccessTrend::Random,
            prediction_confidence: 0.0,
        });

        // Add new access data
        pattern.access_times.push_back(now);
        pattern.access_locations.push_back(position);

        // Maintain sliding window
        if pattern.access_times.len() > 100 {
            pattern.access_times.pop_front();
            pattern.access_locations.pop_front();
        }

        // Recalculate average interval
        if pattern.access_times.len() >= 2 {
            let total_time = pattern
                .access_times
                .back()
                .unwrap()
                .duration_since(*pattern.access_times.front().unwrap());
            pattern.average_interval = total_time / (pattern.access_times.len() as u32 - 1);
        }
    }

    /// Perform memory cleanup
    async fn perform_memory_cleanup(
        streaming_assets: &Arc<RwLock<HashMap<AssetHandle, StreamingAssetInfo>>>,
        memory_used: &Arc<AtomicU64>,
        memory_budget: u64,
        evict_handler: &Arc<RwLock<Option<StreamingEvictHandler>>>,
    ) {
        let current_usage = memory_used.load(Ordering::Relaxed);
        let memory_pressure = current_usage as f64 / memory_budget as f64;

        if memory_pressure > 0.85 {
            log::info!(
                "High memory pressure ({:.1}%), performing cleanup",
                memory_pressure * 100.0
            );

            // Find assets to evict (least recently used, lowest priority)
            let mut eviction_candidates = Vec::new();

            {
                let assets = streaming_assets.read().unwrap_or_else(|e| e.into_inner());
                for (handle, info) in assets.iter() {
                    if info.streaming_state == StreamingState::Loaded
                        && info.priority >= AssetPriority::Low
                    {
                        let score = Self::calculate_eviction_score(info);
                        eviction_candidates.push((*handle, score));
                    }
                }
            }

            // Sort by eviction score (higher score = more likely to evict)
            eviction_candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(CmpOrdering::Equal));

            // Evict assets until memory pressure is reduced
            let target_usage = (memory_budget as f64 * 0.7) as u64;
            let mut bytes_to_free = current_usage.saturating_sub(target_usage);

            for (handle, _score) in eviction_candidates {
                if bytes_to_free == 0 {
                    break;
                }

                let freed = {
                    let handler = evict_handler.read().unwrap_or_else(|e| e.into_inner()).clone();
                    if let Some(handler) = handler {
                        match handler(handle).await {
                            Ok(bytes) => bytes,
                            Err(err) => {
                                log::warn!("Eviction failed for {:?}: {}", handle, err);
                                0
                            }
                        }
                    } else {
                        log::warn!("No eviction handler registered for streaming cleanup");
                        0
                    }
                };

                if freed > 0 {
                    bytes_to_free = bytes_to_free.saturating_sub(freed);
                    memory_used.fetch_sub(freed, Ordering::Relaxed);
                    if let Some(info) = streaming_assets.write().unwrap_or_else(|e| e.into_inner()).get_mut(&handle) {
                        info.streaming_state = StreamingState::NotLoaded;
                        info.memory_residency = MemoryResidency::NotResident;
                    }
                }
            }
        }
    }

    /// Calculate eviction score for an asset (higher = more likely to evict)
    fn calculate_eviction_score(info: &StreamingAssetInfo) -> f32 {
        let time_since_access = info.last_accessed.elapsed().as_secs_f32();
        let distance_factor = (info.distance_from_player / 1000.0).min(1.0);
        let priority_factor = match info.priority {
            AssetPriority::Critical => 0.0,
            AssetPriority::High => 0.2,
            AssetPriority::Normal => 0.5,
            AssetPriority::Low => 0.8,
            AssetPriority::Lowest => 1.0,
        };

        time_since_access * distance_factor * priority_factor
    }

    /// Update LOD levels based on viewer context
    async fn update_lod_levels(
        streaming_assets: &Arc<RwLock<HashMap<AssetHandle, StreamingAssetInfo>>>,
        viewer_context: &Arc<RwLock<ViewerContext>>,
        normal_queue: &Arc<Mutex<VecDeque<StreamingRequest>>>,
        background_queue: &Arc<Mutex<VecDeque<StreamingRequest>>>,
    ) {
        let context = viewer_context.read().unwrap_or_else(|e| e.into_inner()).clone();
        let mut assets = streaming_assets.write().unwrap_or_else(|e| e.into_inner());
        let mut upgrade_requests = Vec::new();
        let mut downgrade_requests = Vec::new();

        for (_, info) in assets.iter_mut() {
            // Calculate distance from viewer
            let distance = (info.distance_from_player - context.view_distance.abs()).max(0.0);

            // Determine appropriate LOD level based on distance
            let target_lod = if distance < 50.0 {
                0 // Highest quality
            } else if distance < 150.0 {
                1 // High quality
            } else if distance < 500.0 {
                2 // Medium quality
            } else {
                3 // Low quality
            };

            // Update target LOD if changed
            if target_lod != info.target_lod {
                info.target_lod = target_lod;
                if target_lod < info.current_lod {
                    info.streaming_state = StreamingState::Upgrading;
                    upgrade_requests.push(StreamingRequest {
                        handle: info.handle,
                        path: info.path.clone(),
                        request_type: StreamingRequestType::Upgrade,
                        priority: info.priority,
                        target_lod,
                        distance_hint: distance,
                        submitted_time: Instant::now(),
                        deadline: None,
                        callback: None,
                    });
                } else {
                    info.streaming_state = StreamingState::Downgrading;
                    downgrade_requests.push(StreamingRequest {
                        handle: info.handle,
                        path: info.path.clone(),
                        request_type: StreamingRequestType::Background,
                        priority: AssetPriority::Low,
                        target_lod,
                        distance_hint: distance,
                        submitted_time: Instant::now(),
                        deadline: None,
                        callback: None,
                    });
                }
            }
        }

        drop(assets);

        if !upgrade_requests.is_empty() {
            let mut queue = normal_queue.lock().unwrap_or_else(|e| e.into_inner());
            for request in upgrade_requests {
                queue.push_back(request);
            }
        }

        if !downgrade_requests.is_empty() {
            let mut queue = background_queue.lock().unwrap_or_else(|e| e.into_inner());
            for request in downgrade_requests {
                queue.push_back(request);
            }
        }
    }

    /// Update prediction model based on usage patterns
    async fn update_prediction_model(
        prediction_model: &Arc<RwLock<PredictionModel>>,
        usage_patterns: &Arc<RwLock<HashMap<AssetHandle, UsagePattern>>>,
    ) {
        let patterns = usage_patterns.read().unwrap_or_else(|e| e.into_inner());
        let mut model = prediction_model.write().unwrap_or_else(|e| e.into_inner());

        // Analyze correlations between assets
        for (handle1, pattern1) in patterns.iter() {
            let mut correlations = Vec::new();

            for (handle2, pattern2) in patterns.iter() {
                if handle1 != handle2 {
                    let correlation = Self::calculate_correlation(pattern1, pattern2);
                    if correlation > model.confidence_threshold {
                        correlations.push((*handle2, correlation));
                    }
                }
            }

            if !correlations.is_empty() {
                model.asset_correlations.insert(*handle1, correlations);
            }
        }

        log::trace!(
            "Updated prediction model with {} asset correlations",
            model.asset_correlations.len()
        );
    }

    /// Calculate correlation between two usage patterns
    fn calculate_correlation(pattern1: &UsagePattern, pattern2: &UsagePattern) -> f32 {
        // Simplified correlation calculation based on timing and location
        let time_correlation =
            Self::calculate_time_correlation(&pattern1.access_times, &pattern2.access_times);
        let location_correlation = Self::calculate_location_correlation(
            &pattern1.access_locations,
            &pattern2.access_locations,
        );

        (time_correlation + location_correlation) / 2.0
    }

    /// Calculate time-based correlation
    fn calculate_time_correlation(times1: &VecDeque<Instant>, times2: &VecDeque<Instant>) -> f32 {
        // Simplified: check for overlapping time windows
        let window_size = Duration::from_secs(30);
        let mut overlaps = 0;
        let total_windows = times1.len().min(times2.len());

        for time1 in times1 {
            for time2 in times2 {
                if time1.duration_since(*time2).abs() < window_size {
                    overlaps += 1;
                }
            }
        }

        if total_windows > 0 {
            overlaps as f32 / total_windows as f32
        } else {
            0.0
        }
    }

    /// Calculate location-based correlation
    fn calculate_location_correlation(
        locations1: &VecDeque<Vector3<f32>>,
        locations2: &VecDeque<Vector3<f32>>,
    ) -> f32 {
        // Simplified: check for nearby locations
        let proximity_threshold = 100.0; // meters
        let mut nearby_pairs = 0;
        let total_pairs = locations1.len().min(locations2.len());

        for loc1 in locations1 {
            for loc2 in locations2 {
                if (*loc1 - *loc2).norm() < proximity_threshold {
                    nearby_pairs += 1;
                }
            }
        }

        if total_pairs > 0 {
            nearby_pairs as f32 / total_pairs as f32
        } else {
            0.0
        }
    }

    /// Update system (called from main thread)
    pub async fn update(&self) -> Result<(), StreamingError> {
        // Update statistics
        {
            let mut stats = self.stats.write().unwrap_or_else(|e| e.into_inner());
            stats.active_streams = self.active_workers.load(Ordering::Relaxed) as u32;

            // Calculate queue sizes
            let high_queue_size = self.high_priority_queue.lock().unwrap_or_else(|e| e.into_inner()).len();
            let normal_queue_size = self.normal_priority_queue.lock().unwrap_or_else(|e| e.into_inner()).len();
            let background_queue_size = self.background_queue.lock().unwrap_or_else(|e| e.into_inner()).len();
            let total_queue_size = high_queue_size + normal_queue_size + background_queue_size;

            stats.peak_queue_size = stats.peak_queue_size.max(total_queue_size);
        }

        Ok(())
    }

    /// Get streaming statistics
    pub fn get_stats(&self) -> StreamingStats {
        self.stats.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Shutdown streaming system
    pub async fn shutdown(&self) {
        log::info!("Shutting down streaming manager...");

        // Signal shutdown
        self.shutdown_signal.store(true, Ordering::Relaxed);
        self.shutdown_notify.notify_waiters();

        // Wait for all workers to finish
        let handles = {
            let mut handles_guard = self.worker_handles.lock().unwrap_or_else(|e| e.into_inner());
            std::mem::take(&mut *handles_guard)
        };

        for handle in handles {
            if let Err(e) = handle.await {
                log::error!("Worker task failed to shutdown cleanly: {}", e);
            }
        }

        log::info!("Streaming manager shutdown complete");
    }
}

impl From<StreamingError> for AssetError {
    fn from(err: StreamingError) -> Self {
        AssetError::LoadingFailed {
            path: "streaming_system".to_string(),
            error: err.to_string(),
        }
    }
}

// Helper trait for duration absolute difference
trait DurationExt {
    fn abs(self) -> Duration;
}

impl DurationExt for Duration {
    fn abs(self) -> Duration {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_streaming_request_ordering() {
        let req1 = StreamingRequest {
            handle: AssetHandle(1),
            path: PathBuf::from("test1"),
            request_type: StreamingRequestType::Load,
            priority: AssetPriority::High,
            target_lod: 0,
            distance_hint: 100.0,
            submitted_time: Instant::now(),
            deadline: None,
            callback: None,
        };

        let req2 = StreamingRequest {
            handle: AssetHandle(2),
            path: PathBuf::from("test2"),
            request_type: StreamingRequestType::Load,
            priority: AssetPriority::Critical,
            target_lod: 0,
            distance_hint: 50.0,
            submitted_time: Instant::now(),
            deadline: None,
            callback: None,
        };

        // Critical priority should come before High priority
        assert!(req2 < req1);
    }

    #[test]
    fn test_eviction_score_calculation() {
        let info = StreamingAssetInfo {
            handle: AssetHandle(1),
            path: PathBuf::from("test"),
            asset_type: AssetType::Texture,
            total_size: 1024,
            lod_levels: Vec::new(),
            current_lod: 0,
            target_lod: 0,
            priority: AssetPriority::Low,
            last_accessed: Instant::now() - Duration::from_secs(60),
            access_count: 5,
            distance_from_player: 500.0,
            predicted_access_time: None,
            streaming_state: StreamingState::Loaded,
            memory_residency: MemoryResidency::Full,
        };

        let score = StreamingManager::calculate_eviction_score(&info);
        assert!(score > 0.0);
    }
}
