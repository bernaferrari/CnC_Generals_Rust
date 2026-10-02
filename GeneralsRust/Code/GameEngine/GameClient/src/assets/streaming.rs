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

use serde::{Deserialize, Serialize};
use std::cmp::Ordering as CmpOrdering;
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{
    Arc, Mutex, RwLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use glam::Vec3;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::{Notify, RwLock as AsyncRwLock, Semaphore, watch};
use tokio::task::JoinHandle;

use crossbeam::channel::{Receiver, Sender, unbounded};

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
    pub access_locations: VecDeque<Vec3>,
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

/// One `record_asset_access` observation, handed to the maintenance task.
struct UsageSample {
    handle: AssetHandle,
    position: Vec3,
    at: Instant,
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
    pub position: Vec3,
    pub forward: Vec3,
    pub view_distance: f32,
    pub fov_degrees: f32,
    pub movement_velocity: Vec3,
}

impl Default for ViewerContext {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            forward: Vec3::new(0.0, 0.0, -1.0),
            view_distance: 1000.0,
            fov_degrees: 90.0,
            movement_velocity: Vec3::ZERO,
        }
    }
}

/// Complete Streaming Management System
pub struct StreamingManager {
    config: AssetConfig,

    // THREAD: producer -> consumer handoff. Producers are `request_asset`
    // callers (main thread / any API user) and the maintenance LOD pass;
    // consumers are the tokio load workers. MPMC because several workers pop
    // concurrently. The previous BinaryHeap ordered Critical requests by
    // deadline-then-submission; every producer posts `deadline: None`, so
    // FIFO service order is equivalent.
    high_priority_tx: Sender<StreamingRequest>,
    high_priority_rx: Receiver<StreamingRequest>,
    normal_priority_tx: Sender<StreamingRequest>,
    normal_priority_rx: Receiver<StreamingRequest>,
    background_tx: Sender<StreamingRequest>,
    background_rx: Receiver<StreamingRequest>,

    // THREAD: shared mutable residency map written by the parallel load
    // workers (load completion), the maintenance task (cleanup + LOD pass)
    // and `request_asset`; an async lock because every accessor awaits.
    streaming_assets: Arc<AsyncRwLock<HashMap<AssetHandle, StreamingAssetInfo>>>,

    // Memory management
    memory_budget: u64,
    memory_used: Arc<AtomicU64>,

    // Worker management
    worker_semaphore: Arc<Semaphore>,
    active_workers: Arc<AtomicU64>,
    max_workers: usize,

    // THREAD: latest-value handoff — the caller thread publishes a context,
    // the maintenance task samples the newest snapshot each tick (watch is
    // the lock-free equivalent of the old `RwLock<ViewerContext>`).
    viewer_tx: watch::Sender<ViewerContext>,
    viewer_rx: watch::Receiver<ViewerContext>,

    // THREAD: parallel load workers read-modify-write the counters and the
    // running average concurrently; `get_stats` snapshots from the owner
    // thread, so the short critical sections stay behind one lock.
    stats: Arc<RwLock<StreamingStats>>,

    // THREAD: lifecycle only — `start` pushes join handles and `shutdown`
    // drains them on the owning thread; the Mutex is interior mutability
    // behind `&self`, not a cross-thread boundary.
    worker_handles: Mutex<Vec<JoinHandle<()>>>,
    shutdown_signal: Arc<AtomicBool>,
    shutdown_notify: Arc<Notify>,

    // THREAD: registered once during asset-manager init from the caller
    // thread, cloned out per request by the load workers / maintenance task.
    load_handler: Arc<RwLock<Option<StreamingLoadHandler>>>,
    evict_handler: Arc<RwLock<Option<StreamingEvictHandler>>>,

    // THREAD: usage samples flow from `record_asset_access` callers to the
    // maintenance task, which owns the pattern map and prediction model
    // outright (single owner, no shared mutable state).
    usage_tx: Sender<UsageSample>,
    usage_rx: Receiver<UsageSample>,
}

/// Predictive loading model
#[derive(Debug)]
struct PredictionModel {
    asset_correlations: HashMap<AssetHandle, Vec<(AssetHandle, f32)>>, // Asset -> Related assets + correlation
    location_patterns: HashMap<glam::IVec3, Vec<AssetHandle>>, // Grid cell -> Assets likely to be needed
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

        let (high_priority_tx, high_priority_rx) = unbounded();
        let (normal_priority_tx, normal_priority_rx) = unbounded();
        let (background_tx, background_rx) = unbounded();
        let (usage_tx, usage_rx) = unbounded();
        let (viewer_tx, viewer_rx) = watch::channel(ViewerContext::default());

        Ok(Self {
            config: config.clone(),
            high_priority_tx,
            high_priority_rx,
            normal_priority_tx,
            normal_priority_rx,
            background_tx,
            background_rx,
            streaming_assets: Arc::new(AsyncRwLock::new(HashMap::new())),
            memory_budget,
            memory_used: Arc::new(AtomicU64::new(0)),
            worker_semaphore: Arc::new(Semaphore::new(max_workers)),
            active_workers: Arc::new(AtomicU64::new(0)),
            max_workers,
            viewer_tx,
            viewer_rx,
            stats: Arc::new(RwLock::new(StreamingStats {
                memory_budget_mb: config.cache_size_mb as f32,
                ..Default::default()
            })),
            worker_handles: Mutex::new(Vec::new()),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
            shutdown_notify: Arc::new(Notify::new()),
            load_handler: Arc::new(RwLock::new(None)),
            evict_handler: Arc::new(RwLock::new(None)),
            usage_tx,
            usage_rx,
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
        *self
            .evict_handler
            .write()
            .unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(handler));
    }

    /// Start streaming system
    pub async fn start(&self) -> Result<(), StreamingError> {
        log::info!(
            "Starting streaming manager with {} workers",
            self.max_workers
        );

        // Start worker tasks
        let mut handles = self
            .worker_handles
            .lock()
            .unwrap_or_else(|e| e.into_inner());
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
        let high_rx = self.high_priority_rx.clone();
        let normal_rx = self.normal_priority_rx.clone();
        let background_rx = self.background_rx.clone();
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
                let request = high_rx
                    .try_recv()
                    .ok()
                    .or_else(|| normal_rx.try_recv().ok())
                    .or_else(|| background_rx.try_recv().ok());

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
        let viewer_rx = self.viewer_rx.clone();
        let normal_tx = self.normal_priority_tx.clone();
        let background_tx = self.background_tx.clone();
        let usage_rx = self.usage_rx.clone();
        let shutdown_signal = self.shutdown_signal.clone();
        let memory_used = self.memory_used.clone();
        let memory_budget = self.memory_budget;
        let stats = self.stats.clone();
        let evict_handler = self.evict_handler.clone();

        let handle = tokio::spawn(async move {
            log::debug!("Streaming maintenance task started");

            // THREAD: this task is the single owner of the usage patterns and
            // the prediction model; samples arrive on `usage_rx`.
            let mut usage_patterns: HashMap<AssetHandle, UsagePattern> = HashMap::new();
            let mut prediction_model = PredictionModel::default();

            let mut last_cleanup = Instant::now();
            let mut last_prediction_update = Instant::now();
            let cleanup_interval = Duration::from_secs(30);
            let prediction_interval = Duration::from_secs(60);

            loop {
                if shutdown_signal.load(Ordering::Relaxed) {
                    break;
                }

                let now = Instant::now();

                // Drain usage samples recorded since the last tick.
                while let Ok(sample) = usage_rx.try_recv() {
                    Self::record_usage_sample(&mut usage_patterns, sample);
                }

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
                    Self::update_prediction_model(&mut prediction_model, &usage_patterns);
                    last_prediction_update = now;
                }

                // Update LOD levels based on the newest viewer snapshot
                let context = viewer_rx.borrow().clone();
                Self::update_lod_levels(
                    &streaming_assets,
                    &context,
                    &normal_tx,
                    &background_tx,
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
        streaming_assets: &AsyncRwLock<HashMap<AssetHandle, StreamingAssetInfo>>,
        memory_used: &AtomicU64,
        request_handler: &RwLock<Option<StreamingLoadHandler>>,
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

        let handler = request_handler
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        let result = if let Some(handler) = handler {
            handler(request).await
        } else {
            Err(StreamingError::TaskFailed(
                "No streaming load handler registered".to_string(),
            ))
        };

        if let Ok(load_result) = result.as_ref() {
            let mut assets = streaming_assets.write().await;
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
            let mut assets = self.streaming_assets.write().await;
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
                let _ = self.high_priority_tx.send(request);
                self.note_queue_depth(&self.high_priority_tx);
            }
            AssetPriority::High | AssetPriority::Normal => {
                let _ = self.normal_priority_tx.send(request);
                self.note_queue_depth(&self.normal_priority_tx);
            }
            AssetPriority::Low | AssetPriority::Lowest => {
                let _ = self.background_tx.send(request);
            }
        }

        // Notify workers
        self.shutdown_notify.notify_one();
        Ok(())
    }

    /// Pending depth of one request channel (still counts buffered messages
    /// after disconnect, matching the old queue `len()` semantics).
    fn channel_depth(tx: &Sender<StreamingRequest>) -> usize {
        tx.len()
    }

    /// Total depth of the three pending-request channels.
    fn pending_requests(&self) -> usize {
        Self::channel_depth(&self.high_priority_tx)
            + Self::channel_depth(&self.normal_priority_tx)
            + Self::channel_depth(&self.background_tx)
    }

    /// Record the new peak depth of one queue, matching the old per-queue
    /// `peak_queue_size` accounting in `request_asset`.
    fn note_queue_depth(&self, tx: &Sender<StreamingRequest>) {
        let depth = Self::channel_depth(tx);
        let mut stats = self.stats.write().unwrap_or_else(|e| e.into_inner());
        stats.peak_queue_size = stats.peak_queue_size.max(depth);
    }

    /// Update viewer context for LOD calculations
    pub fn update_viewer_context(&self, context: ViewerContext) {
        let _ = self.viewer_tx.send(context);
    }

    /// Record asset access for pattern analysis
    pub fn record_asset_access(&self, handle: AssetHandle, position: Vec3) {
        // THREAD: the sample is consumed by the maintenance task, which owns
        // the usage-pattern map — nothing is shared or locked here.
        let _ = self.usage_tx.send(UsageSample {
            handle,
            position,
            at: Instant::now(),
        });
    }

    /// Fold one usage sample into the maintenance task's pattern map.
    fn record_usage_sample(patterns: &mut HashMap<AssetHandle, UsagePattern>, sample: UsageSample) {
        let pattern = patterns.entry(sample.handle).or_insert_with(|| UsagePattern {
            asset_handle: sample.handle,
            access_times: VecDeque::with_capacity(100),
            access_locations: VecDeque::with_capacity(100),
            average_interval: Duration::from_secs(0),
            access_trend: AccessTrend::Random,
            prediction_confidence: 0.0,
        });

        // Add new access data
        pattern.access_times.push_back(sample.at);
        pattern.access_locations.push_back(sample.position);

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
        streaming_assets: &AsyncRwLock<HashMap<AssetHandle, StreamingAssetInfo>>,
        memory_used: &AtomicU64,
        memory_budget: u64,
        evict_handler: &RwLock<Option<StreamingEvictHandler>>,
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
                let assets = streaming_assets.read().await;
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
                    let handler = evict_handler
                        .read()
                        .unwrap_or_else(|e| e.into_inner())
                        .clone();
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
                    if let Some(info) = streaming_assets
                        .write()
                        .await
                        .get_mut(&handle)
                    {
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
        streaming_assets: &AsyncRwLock<HashMap<AssetHandle, StreamingAssetInfo>>,
        context: &ViewerContext,
        normal_tx: &Sender<StreamingRequest>,
        background_tx: &Sender<StreamingRequest>,
    ) {
        let mut assets = streaming_assets.write().await;
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

        for request in upgrade_requests {
            let _ = normal_tx.send(request);
        }

        for request in downgrade_requests {
            let _ = background_tx.send(request);
        }
    }

    /// Update prediction model based on usage patterns
    fn update_prediction_model(
        model: &mut PredictionModel,
        patterns: &HashMap<AssetHandle, UsagePattern>,
    ) {
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
        locations1: &VecDeque<Vec3>,
        locations2: &VecDeque<Vec3>,
    ) -> f32 {
        // Simplified: check for nearby locations
        let proximity_threshold = 100.0; // meters
        let mut nearby_pairs = 0;
        let total_pairs = locations1.len().min(locations2.len());

        for loc1 in locations1 {
            for loc2 in locations2 {
                if (*loc1 - *loc2).length() < proximity_threshold {
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
            let total_queue_size = self.pending_requests();
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
            let mut handles_guard = self
                .worker_handles
                .lock()
                .unwrap_or_else(|e| e.into_inner());
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
