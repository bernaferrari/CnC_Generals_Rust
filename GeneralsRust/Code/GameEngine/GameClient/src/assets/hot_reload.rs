//! # Asset Hot-Reload and Development Tools
//!
//! Complete development tooling system for asset hot-reloading:
//! - Real-time file system watching
//! - Automatic asset reloading on changes
//! - Dependency tracking and cascade reloading
//! - Asset validation and error reporting
//! - Performance profiling and analysis
//! - Memory usage visualization
//! - Debug overlays and asset inspection
//! - Build pipeline integration
//!
//! # Thread & ownership model
//!
//! The C++ original has no locks here because hot reload did not exist; this
//! subsystem is Rust-only. Data flows through exactly three thread domains,
//! and every shared item below is annotated with who touches it:
//!
//! 1. **File-watcher thread** (notify crate callback): runs
//!    [`Self::handle_file_event`]. It owns its debounce map outright (no lock)
//!    and only *sends* [`AssetChangeEvent`]s into a channel — the channel is
//!    the ownership handoff to the worker task.
//! 2. **Worker / profiler tokio tasks**: own their private queues outright
//!    (the reload queue is never shared), read the registration tables, and
//!    push results (attempts, stats, profiler samples) back through shared
//!    locks.
//! 3. **Main thread** (via [`crate::assets::AssetManager`], which holds
//!    `Arc<HotReloadManager>` because the worker's reload callbacks capture
//!    that same `Arc`): writes the registration tables, owns the lifecycle
//!    slot (watcher + task handles), and reads stats/profiler data.
//!
//! Locks that remain are genuine cross-thread shared state that a channel
//! cannot express (main *and* tasks read and write the same table); each is
//! documented at its declaration. Everything single-owner is a plain field.

use notify::event::{CreateKind, ModifyKind, RemoveKind};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{
    Arc, Mutex, RwLock,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant, SystemTime};
use thiserror::Error;

use tokio::sync::{Notify, mpsc};
use tokio::task::JoinHandle;

use super::{AssetError, AssetHandle, AssetType};

/// Hot reload system errors
#[derive(Error, Debug)]
pub enum HotReloadError {
    #[error("File watcher setup failed: {0}")]
    WatcherSetupFailed(String),
    #[error("Hot reload failed for {path}: {error}")]
    ReloadFailed { path: String, error: String },
    #[error("Dependency cycle detected: {assets:?}")]
    DependencyCycle { assets: Vec<String> },
    #[error("Validation failed: {path} - {errors:?}")]
    ValidationFailed { path: String, errors: Vec<String> },
    #[error("Development server error: {0}")]
    DevServerError(String),
    #[error("Profiler error: {0}")]
    ProfilerError(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// File change event types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    Created,
    Modified,
    Deleted,
    Renamed,
}

/// Asset change notification
#[derive(Debug, Clone)]
pub struct AssetChangeEvent {
    pub path: PathBuf,
    pub change_type: ChangeType,
    pub timestamp: SystemTime,
    pub asset_type: AssetType,
    pub affected_handles: Vec<AssetHandle>,
}

/// Hot reload configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotReloadConfig {
    pub enabled: bool,
    pub watch_paths: Vec<PathBuf>,
    pub ignore_patterns: Vec<String>,
    pub debounce_duration_ms: u64,
    pub max_reload_attempts: u32,
    pub enable_dependency_tracking: bool,
    pub enable_validation: bool,
    pub enable_profiling: bool,
    pub auto_reload_shaders: bool,
    pub auto_reload_textures: bool,
    pub auto_reload_models: bool,
    pub auto_reload_audio: bool,
    pub auto_reload_scripts: bool,
}

impl Default for HotReloadConfig {
    fn default() -> Self {
        Self {
            enabled: cfg!(debug_assertions),
            watch_paths: vec![PathBuf::from("assets"), PathBuf::from("data")],
            ignore_patterns: vec![
                "*.tmp".to_string(),
                "*.backup".to_string(),
                "*~".to_string(),
                ".git/**".to_string(),
                ".svn/**".to_string(),
            ],
            debounce_duration_ms: 500,
            max_reload_attempts: 3,
            enable_dependency_tracking: true,
            enable_validation: true,
            enable_profiling: true,
            auto_reload_shaders: true,
            auto_reload_textures: true,
            auto_reload_models: true,
            auto_reload_audio: true,
            auto_reload_scripts: true,
        }
    }
}

/// Asset dependency information
#[derive(Debug, Clone)]
pub struct AssetDependency {
    pub dependent: AssetHandle,         // Asset that depends on others
    pub dependencies: Vec<AssetHandle>, // Assets this depends on
    pub dependents: Vec<AssetHandle>,   // Assets that depend on this
    pub last_check: SystemTime,
}

/// Reload attempt information
#[derive(Debug, Clone)]
pub struct ReloadAttempt {
    pub path: PathBuf,
    pub attempt_number: u32,
    pub timestamp: Instant,
    pub success: bool,
    pub error: Option<String>,
    pub reload_time: Duration,
}

/// Hot reload statistics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct HotReloadStats {
    pub total_reloads: u64,
    pub successful_reloads: u64,
    pub failed_reloads: u64,
    pub average_reload_time_ms: f32,
    pub files_watched: u64,
    pub dependency_updates: u64,
    pub cascade_reloads: u64,
    pub validation_failures: u64,
}

/// Development profiler data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilerData {
    pub asset_loads: Vec<LoadProfile>,
    pub memory_usage: Vec<MemorySnapshot>,
    pub performance_metrics: PerformanceMetrics,
    #[serde(skip_serializing, skip_deserializing)]
    pub hot_reload_history: Vec<ReloadAttempt>,
}

/// Individual asset load profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadProfile {
    pub asset_path: PathBuf,
    pub asset_type: AssetType,
    pub load_time: Duration,
    pub memory_used: u64,
    pub timestamp: SystemTime,
    pub success: bool,
    pub error: Option<String>,
}

/// Memory usage snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshot {
    pub timestamp: SystemTime,
    pub total_memory: u64,
    pub asset_memory: u64,
    pub texture_memory: u64,
    pub audio_memory: u64,
    pub model_memory: u64,
    pub cached_assets: u32,
}

/// Memory snapshot data without timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshotData {
    pub total_memory: u64,
    pub asset_memory: u64,
    pub texture_memory: u64,
    pub audio_memory: u64,
    pub model_memory: u64,
    pub cached_assets: u32,
}

/// Performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub fps: f32,
    pub frame_time_ms: f32,
    pub asset_loading_time_ms: f32,
    pub memory_pressure: f32,
    pub io_wait_time_ms: f32,
    pub validation_time_ms: f32,
}

/// Debug visualization data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugVisualization {
    pub asset_dependency_graph: Vec<DependencyEdge>,
    pub memory_breakdown: HashMap<AssetType, u64>,
    pub load_timeline: Vec<LoadTimelineEntry>,
    pub error_log: Vec<ErrorEntry>,
}

/// Dependency graph edge for visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyEdge {
    pub from: String,
    pub to: String,
    pub dependency_type: String,
}

/// Load timeline entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadTimelineEntry {
    pub timestamp: SystemTime,
    pub asset_name: String,
    pub event_type: String,
    pub duration: Option<Duration>,
}

/// Error log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEntry {
    pub timestamp: SystemTime,
    pub asset_path: String,
    pub error_type: String,
    pub message: String,
    pub severity: ErrorSeverity,
}

/// Error severity levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ErrorSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// A user-registered hook invoked when an asset of a given type is reloaded.
type ReloadCallback = Arc<
    dyn Fn(AssetHandle, PathBuf) -> Pin<Box<dyn Future<Output = Result<(), AssetError>> + Send>>
        + Send
        + Sync,
>;

/// Reload hooks, keyed by asset type (`Unknown` acts as the catch-all).
type ReloadCallbacks = HashMap<AssetType, Vec<ReloadCallback>>;

/// Bidirectional path <-> handle registration tables.
///
/// Written by the main thread (`register_asset_path`), read by the worker
/// task when it maps a changed file back to the asset to reload. Kept as one
/// lock so the two tables can never disagree with each other.
#[derive(Debug, Default)]
struct PathMaps {
    path_to_handle: HashMap<PathBuf, AssetHandle>,
    handle_to_path: HashMap<AssetHandle, PathBuf>,
}

/// State shared between the main thread and the worker/profiler tasks.
///
/// Every field documents its thread ownership. These locks are the genuinely
/// cross-thread tables (both sides read *and* write, so a channel cannot
/// replace them without copying the whole table per access); everything that
/// a single thread owns lives outside this struct.
struct SharedState {
    /// Shutdown coordination: main thread sets the flag / notifies; worker and
    /// profiler tasks observe.
    shutdown_signal: AtomicBool,
    shutdown_notify: Notify,

    /// Main thread writes (`register_dependency`), worker task reads (cascade
    /// reloads), main thread reads (`generate_debug_visualization`).
    dependencies: RwLock<HashMap<AssetHandle, AssetDependency>>,

    /// Main thread writes (`register_asset_path`), worker task reads.
    paths: RwLock<PathMaps>,

    /// Worker task writes (attempt records), both sides read (max-attempt
    /// throttle in the worker, error log on the main thread).
    reload_attempts: RwLock<HashMap<PathBuf, Vec<ReloadAttempt>>>,

    /// Worker task writes reload counters, main thread writes
    /// `dependency_updates` and reads snapshots (`get_stats`).
    stats: Mutex<HotReloadStats>,

    /// Main thread writes (`record_asset_load`), profiler task writes memory
    /// samples, main thread reads (`get_profiler_data`, debug visualization).
    profiler_data: RwLock<ProfilerData>,

    /// Main thread writes (`register_reload_callback`), worker task reads.
    reload_callbacks: RwLock<ReloadCallbacks>,

    /// Main thread writes (`register_memory_snapshot_provider`), profiler task
    /// reads (calls the provider every snapshot interval).
    memory_snapshot_provider: RwLock<Option<Arc<dyn Fn() -> MemorySnapshotData + Send + Sync>>>,
}

/// Main-thread-owned lifecycle resources, installed by `start` and torn down
/// by `shutdown`. Only the main thread touches this slot; it sits in a mutex
/// solely because the manager is `Arc`-shared and these resources are
/// installed after construction (there is no second contender in practice).
struct HotReloadRuntime {
    watcher: Option<Box<dyn Watcher + Send>>,
    worker_handle: Option<JoinHandle<()>>,
    profiler_handle: Option<JoinHandle<()>>,
    /// Receiving end of the watcher-event channel, parked here until `start`
    /// hands it to the worker task (the worker then owns it exclusively).
    events_rx: Option<mpsc::UnboundedReceiver<AssetChangeEvent>>,
}

/// Complete Hot Reload Management System
pub struct HotReloadManager {
    config: HotReloadConfig,
    watch_paths: Vec<PathBuf>,

    /// Watcher-thread -> worker-task event channel. The manager keeps the
    /// sending end (cloned into the watcher callback on the main thread);
    /// the receiving end is parked in `runtime` until the worker takes it.
    events_tx: mpsc::UnboundedSender<AssetChangeEvent>,

    /// Registration tables + counters shared with the worker/profiler tasks.
    shared: Arc<SharedState>,

    /// Main-thread lifecycle slot (see [`HotReloadRuntime`]).
    runtime: Mutex<Option<HotReloadRuntime>>,
}

impl HotReloadManager {
    /// Create new hot reload manager
    pub fn new(base_path: PathBuf) -> Result<Self, HotReloadError> {
        let config = HotReloadConfig::default();
        let watch_paths = config
            .watch_paths
            .iter()
            .map(|p| base_path.join(p))
            .collect();

        // Watcher thread --[AssetChangeEvent]--> worker task. The sender stays
        // on the manager; the receiver is handed to the worker task in `start`.
        let (events_tx, events_rx) = mpsc::unbounded_channel();

        Ok(Self {
            events_tx,
            shared: Arc::new(SharedState {
                shutdown_signal: AtomicBool::new(false),
                shutdown_notify: Notify::new(),
                dependencies: RwLock::new(HashMap::new()),
                paths: RwLock::new(PathMaps::default()),
                reload_attempts: RwLock::new(HashMap::new()),
                stats: Mutex::new(HotReloadStats::default()),
                profiler_data: RwLock::new(ProfilerData {
                    asset_loads: Vec::new(),
                    memory_usage: Vec::new(),
                    performance_metrics: PerformanceMetrics {
                        fps: 0.0,
                        frame_time_ms: 0.0,
                        asset_loading_time_ms: 0.0,
                        memory_pressure: 0.0,
                        io_wait_time_ms: 0.0,
                        validation_time_ms: 0.0,
                    },
                    hot_reload_history: Vec::new(),
                }),
                reload_callbacks: RwLock::new(HashMap::new()),
                memory_snapshot_provider: RwLock::new(None),
            }),
            config,
            watch_paths,
            runtime: Mutex::new(Some(HotReloadRuntime {
                watcher: None,
                worker_handle: None,
                profiler_handle: None,
                events_rx: Some(events_rx),
            })),
        })
    }

    /// Start hot reload system
    pub async fn start(&self) -> Result<(), HotReloadError> {
        if !self.config.enabled {
            log::info!("Hot reload is disabled");
            return Ok(());
        }

        log::info!("Starting hot reload manager...");

        // Setup file watcher
        let watcher = self.setup_file_watcher()?;

        // Hand the receiving end of the event channel to the worker task.
        let events_rx = self
            .runtime
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_mut()
            .and_then(|runtime| runtime.events_rx.take())
            .unwrap_or_else(|| {
                // Only reachable if `start` is called twice; a disconnected
                // channel just idles the extra worker.
                let (_, rx) = mpsc::unbounded_channel();
                rx
            });

        // Start worker task
        let worker_handle = self.spawn_worker_task(events_rx);

        // Start profiler if enabled
        let profiler_handle = if self.config.enable_profiling {
            Some(self.spawn_profiler_task().await)
        } else {
            None
        };

        if let Some(runtime) = self
            .runtime
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_mut()
        {
            runtime.watcher = Some(Box::new(watcher));
            runtime.worker_handle = Some(worker_handle);
            runtime.profiler_handle = profiler_handle;
        }

        log::info!("Hot reload manager started");
        Ok(())
    }

    /// Setup file system watcher, returning the live watcher for the runtime slot.
    fn setup_file_watcher(&self) -> Result<notify::RecommendedWatcher, HotReloadError> {
        use notify::RecommendedWatcher;

        let events_tx = self.events_tx.clone();
        let config = self.config.clone();
        // Owned outright by the watcher callback: notify invokes this closure
        // only from its own single watcher thread, so the debounce state needs
        // no lock (it used to be a shared `Arc<Mutex<HashMap>>`).
        let mut debounce_map: HashMap<PathBuf, Instant> = HashMap::new();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                match res {
                    Ok(event) => {
                        Self::handle_file_event(event, &mut debounce_map, &events_tx, &config)
                    }
                    Err(e) => log::error!("File watcher error: {}", e),
                }
            },
            notify::Config::default(),
        )
        .map_err(|e| HotReloadError::WatcherSetupFailed(e.to_string()))?;

        // Watch configured paths
        for path in &self.watch_paths {
            if path.exists() {
                log::info!("Watching path for changes: {}", path.display());
                watcher
                    .watch(path, RecursiveMode::Recursive)
                    .map_err(|e| HotReloadError::WatcherSetupFailed(e.to_string()))?;
            } else {
                log::warn!("Watch path does not exist: {}", path.display());
            }
        }

        // Update stats
        {
            let mut stats = self.shared.stats.lock().unwrap_or_else(|e| e.into_inner());
            stats.files_watched = self.count_watched_files();
        }

        Ok(watcher)
    }

    /// Handle file system event.
    ///
    /// Runs on the notify watcher thread. Debounces against the closure-owned
    /// map and hands the event to the worker task through the channel.
    fn handle_file_event(
        event: Event,
        debounce_map: &mut HashMap<PathBuf, Instant>,
        events_tx: &mpsc::UnboundedSender<AssetChangeEvent>,
        config: &HotReloadConfig,
    ) {
        let change_type = match event.kind {
            EventKind::Create(CreateKind::File) => ChangeType::Created,
            EventKind::Modify(ModifyKind::Data(_)) => ChangeType::Modified,
            EventKind::Remove(RemoveKind::File) => ChangeType::Deleted,
            EventKind::Modify(ModifyKind::Name(_)) => ChangeType::Renamed,
            _ => return, // Ignore other events
        };

        for path in event.paths {
            // Check ignore patterns
            if Self::should_ignore_path(&path, &config.ignore_patterns) {
                continue;
            }

            // Debounce rapid changes
            let now = Instant::now();
            let should_process = if let Some(last_change) = debounce_map.get(&path) {
                if now.duration_since(*last_change)
                    < Duration::from_millis(config.debounce_duration_ms)
                {
                    false
                } else {
                    debounce_map.insert(path.clone(), now);
                    true
                }
            } else {
                debounce_map.insert(path.clone(), now);
                true
            };

            if should_process {
                let asset_type = AssetType::from_extension(
                    path.extension().and_then(|e| e.to_str()).unwrap_or(""),
                );

                let change_event = AssetChangeEvent {
                    path: path.clone(),
                    change_type,
                    timestamp: SystemTime::now(),
                    asset_type,
                    affected_handles: Vec::new(), // Will be populated later
                };

                // Ownership handoff: the watcher thread only produces events;
                // the worker task is their sole consumer.
                if let Err(e) = events_tx.send(change_event) {
                    log::error!("File change dropped (worker gone): {}", e);
                }
                log::debug!(
                    "File change detected: {} ({:?})",
                    path.display(),
                    change_type
                );
            }
        }
    }

    /// Check if path should be ignored
    fn should_ignore_path(path: &Path, ignore_patterns: &[String]) -> bool {
        let path_str = path.to_string_lossy();

        for pattern in ignore_patterns {
            if glob_match::glob_match(pattern, &path_str) {
                return true;
            }
        }

        false
    }

    /// Spawn worker task for processing changes.
    ///
    /// The task exclusively owns the reload queue: change events arrive on the
    /// channel, and any cascade reloads they generate are appended to the
    /// task-local queue below — the queue never escapes the task, so it needs
    /// no lock.
    fn spawn_worker_task(
        &self,
        mut events_rx: mpsc::UnboundedReceiver<AssetChangeEvent>,
    ) -> JoinHandle<()> {
        let shared = Arc::clone(&self.shared);
        let config = self.config.clone();

        tokio::spawn(async move {
            log::debug!("Hot reload worker started");

            // Task-local; both its producers and its consumer live in this task.
            let mut reload_queue: VecDeque<PathBuf> = VecDeque::new();

            loop {
                if shared.shutdown_signal.load(Ordering::Relaxed) {
                    break;
                }

                // Process pending changes: drain up to 10 events per pass,
                // matching the batching of the previous polling loop.
                let mut changes = Vec::new();
                for _ in 0..10 {
                    match events_rx.try_recv() {
                        Ok(change) => changes.push(change),
                        Err(mpsc::error::TryRecvError::Empty) => break,
                        Err(mpsc::error::TryRecvError::Disconnected) => break,
                    }
                }

                for change in changes {
                    Self::process_asset_change(change, &shared, &mut reload_queue, &config).await;
                }

                // Process reload queue
                let mut reload_paths = Vec::new();
                for _ in 0..5 {
                    match reload_queue.pop_front() {
                        Some(path) => reload_paths.push(path),
                        None => break,
                    }
                }

                for path in reload_paths {
                    Self::perform_asset_reload(&path, &shared, &config).await;
                }

                // Wait for more work or timeout
                tokio::select! {
                    _ = shared.shutdown_notify.notified() => {
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                        // Continue loop
                    }
                }
            }

            log::debug!("Hot reload worker stopped");
        })
    }

    /// Process asset change event
    async fn process_asset_change(
        mut change: AssetChangeEvent,
        shared: &SharedState,
        reload_queue: &mut VecDeque<PathBuf>,
        config: &HotReloadConfig,
    ) {
        // Find affected asset handles
        {
            let handle_map = shared.paths.read().unwrap_or_else(|e| e.into_inner());
            if let Some(handle) = handle_map.path_to_handle.get(&change.path) {
                change.affected_handles.push(*handle);
            }
        }

        // Check if we should auto-reload this asset type
        let should_reload = match change.asset_type {
            AssetType::Shader => config.auto_reload_shaders,
            AssetType::Texture => config.auto_reload_textures,
            AssetType::Model => config.auto_reload_models,
            AssetType::Audio => config.auto_reload_audio,
            _ => true,
        };

        if should_reload && change.change_type != ChangeType::Deleted {
            // Add to reload queue
            reload_queue.push_back(change.path.clone());

            // Handle dependency cascade if enabled
            if config.enable_dependency_tracking {
                Self::queue_dependent_reloads(&change, shared, reload_queue).await;
            }
        }

        log::info!(
            "Processing asset change: {} ({:?})",
            change.path.display(),
            change.change_type
        );
    }

    /// Queue dependent assets for reload
    async fn queue_dependent_reloads(
        change: &AssetChangeEvent,
        shared: &SharedState,
        reload_queue: &mut VecDeque<PathBuf>,
    ) {
        let handle_map = shared.paths.read().unwrap_or_else(|e| e.into_inner());
        let dependencies = shared.dependencies.read().unwrap_or_else(|e| e.into_inner());
        let mut queued = HashSet::new();
        for handle in &change.affected_handles {
            if let Some(dependency) = dependencies.get(handle) {
                // Queue all dependents for reload
                for dependent_handle in &dependency.dependents {
                    if let Some(path) = handle_map.handle_to_path.get(dependent_handle) {
                        if queued.insert(path.clone()) {
                            reload_queue.push_back(path.clone());
                        }
                    }
                }
            }
        }
    }

    /// Perform actual asset reload
    async fn perform_asset_reload(path: &Path, shared: &SharedState, config: &HotReloadConfig) {
        let start_time = Instant::now();
        let mut attempt_number = 1;

        // Check previous attempts
        {
            let attempts = shared.reload_attempts.read().unwrap_or_else(|e| e.into_inner());
            if let Some(previous_attempts) = attempts.get(path) {
                attempt_number = previous_attempts.len() as u32 + 1;

                if attempt_number > config.max_reload_attempts {
                    log::error!("Max reload attempts exceeded for: {}", path.display());
                    return;
                }
            }
        }

        log::info!(
            "Reloading asset: {} (attempt {})",
            path.display(),
            attempt_number
        );

        let asset_type =
            AssetType::from_extension(path.extension().and_then(|e| e.to_str()).unwrap_or(""));

        let handle = {
            let handles = shared.paths.read().unwrap_or_else(|e| e.into_inner());
            handles.path_to_handle.get(path).copied()
        };

        let mut success = false;
        let mut error_message = None;

        if let Some(handle) = handle {
            let handlers = {
                let callbacks = shared
                    .reload_callbacks
                    .read()
                    .unwrap_or_else(|e| e.into_inner());
                callbacks
                    .get(&asset_type)
                    .or_else(|| callbacks.get(&AssetType::Unknown))
                    .cloned()
            };

            if let Some(handlers) = handlers {
                let mut first_error = None;
                for handler in handlers {
                    if let Err(err) = handler(handle, path.to_path_buf()).await {
                        if first_error.is_none() {
                            first_error = Some(err.to_string());
                        }
                    }
                }

                if let Some(err) = first_error {
                    error_message = Some(err);
                } else {
                    success = true;
                }
            } else {
                error_message = Some(format!(
                    "No reload callbacks registered for {:?}",
                    asset_type
                ));
            }
        } else {
            error_message = Some("Asset handle not registered for path".to_string());
        }

        let reload_time = start_time.elapsed();

        // Record attempt
        let attempt = ReloadAttempt {
            path: path.to_path_buf(),
            attempt_number,
            timestamp: start_time,
            success,
            error: error_message.clone(),
            reload_time,
        };

        {
            let mut attempts = shared
                .reload_attempts
                .write()
                .unwrap_or_else(|e| e.into_inner());
            attempts
                .entry(path.to_path_buf())
                .or_default()
                .push(attempt);
        }

        // Update statistics
        {
            let mut stats = shared.stats.lock().unwrap_or_else(|e| e.into_inner());
            stats.total_reloads += 1;

            if success {
                stats.successful_reloads += 1;

                // Update average reload time
                let total_time =
                    stats.average_reload_time_ms * (stats.successful_reloads - 1) as f32;
                stats.average_reload_time_ms =
                    (total_time + reload_time.as_millis() as f32) / stats.successful_reloads as f32;
            } else {
                stats.failed_reloads += 1;
            }
        }

        if success {
            log::info!(
                "Successfully reloaded: {} ({} ms)",
                path.display(),
                reload_time.as_millis()
            );
        } else {
            log::error!(
                "Failed to reload: {} ({})",
                path.display(),
                error_message.unwrap_or_else(|| "unknown error".to_string())
            );
        }
    }

    /// Spawn profiler task
    async fn spawn_profiler_task(&self) -> JoinHandle<()> {
        let shared = Arc::clone(&self.shared);

        tokio::spawn(async move {
            log::debug!("Hot reload profiler started");

            let mut last_memory_snapshot = Instant::now();
            let memory_snapshot_interval = Duration::from_secs(5);

            loop {
                if shared.shutdown_signal.load(Ordering::Relaxed) {
                    break;
                }

                let now = Instant::now();

                // Take memory snapshot
                if now.duration_since(last_memory_snapshot) >= memory_snapshot_interval {
                    let snapshot_data = {
                        let provider = shared
                            .memory_snapshot_provider
                            .read()
                            .unwrap_or_else(|e| e.into_inner());
                        provider.as_ref().map(|provider| provider())
                    };

                    let snapshot = MemorySnapshot {
                        timestamp: SystemTime::now(),
                        total_memory: snapshot_data
                            .as_ref()
                            .map(|data| data.total_memory)
                            .unwrap_or(0),
                        asset_memory: snapshot_data
                            .as_ref()
                            .map(|data| data.asset_memory)
                            .unwrap_or(0),
                        texture_memory: snapshot_data
                            .as_ref()
                            .map(|data| data.texture_memory)
                            .unwrap_or(0),
                        audio_memory: snapshot_data
                            .as_ref()
                            .map(|data| data.audio_memory)
                            .unwrap_or(0),
                        model_memory: snapshot_data
                            .as_ref()
                            .map(|data| data.model_memory)
                            .unwrap_or(0),
                        cached_assets: snapshot_data
                            .as_ref()
                            .map(|data| data.cached_assets)
                            .unwrap_or(0),
                    };

                    shared
                        .profiler_data
                        .write()
                        .unwrap_or_else(|e| e.into_inner())
                        .memory_usage
                        .push(snapshot);
                    last_memory_snapshot = now;
                }

                // Limit memory usage snapshots to last 1000 entries
                {
                    let mut data = shared
                        .profiler_data
                        .write()
                        .unwrap_or_else(|e| e.into_inner());
                    if data.memory_usage.len() > 1000 {
                        data.memory_usage.drain(0..100); // Remove oldest 100 entries
                    }
                }

                tokio::time::sleep(Duration::from_millis(1000)).await;
            }

            log::debug!("Hot reload profiler stopped");
        })
    }

    /// Register asset dependency
    pub fn register_dependency(&self, dependent: AssetHandle, dependencies: Vec<AssetHandle>) {
        let mut deps = self
            .shared
            .dependencies
            .write()
            .unwrap_or_else(|e| e.into_inner());

        // Update dependent's dependencies
        let dependency_info = deps.entry(dependent).or_insert_with(|| AssetDependency {
            dependent,
            dependencies: Vec::new(),
            dependents: Vec::new(),
            last_check: SystemTime::now(),
        });

        dependency_info.dependencies = dependencies.clone();
        dependency_info.last_check = SystemTime::now();

        // Update dependencies' dependents lists
        for dep_handle in dependencies {
            let dep_info = deps.entry(dep_handle).or_insert_with(|| AssetDependency {
                dependent: dep_handle,
                dependencies: Vec::new(),
                dependents: Vec::new(),
                last_check: SystemTime::now(),
            });

            if !dep_info.dependents.contains(&dependent) {
                dep_info.dependents.push(dependent);
            }
        }

        let mut stats = self.shared.stats.lock().unwrap_or_else(|e| e.into_inner());
        stats.dependency_updates += 1;
    }

    /// Register asset path mapping
    pub fn register_asset_path(&self, handle: AssetHandle, path: PathBuf) {
        let mut maps = self.shared.paths.write().unwrap_or_else(|e| e.into_inner());
        maps.path_to_handle.insert(path.clone(), handle);
        maps.handle_to_path.insert(handle, path);
    }

    /// Register reload callback for an asset type
    pub fn register_reload_callback<F>(&self, asset_type: AssetType, callback: F)
    where
        F: Fn(AssetHandle, PathBuf) -> Pin<Box<dyn Future<Output = Result<(), AssetError>> + Send>>
            + Send
            + Sync
            + 'static,
    {
        self.shared
            .reload_callbacks
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .entry(asset_type)
            .or_default()
            .push(Arc::new(callback));
    }

    /// Register memory snapshot provider for profiling
    pub fn register_memory_snapshot_provider<F>(&self, provider: F)
    where
        F: Fn() -> MemorySnapshotData + Send + Sync + 'static,
    {
        *self
            .shared
            .memory_snapshot_provider
            .write()
            .unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(provider));
    }

    /// Record asset load for profiling
    pub fn record_asset_load(&self, profile: LoadProfile) {
        if self.config.enable_profiling {
            self.shared
                .profiler_data
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .asset_loads
                .push(profile);
        }
    }

    /// Get hot reload statistics
    pub fn get_stats(&self) -> HotReloadStats {
        self.shared
            .stats
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Get profiler data
    pub fn get_profiler_data(&self) -> ProfilerData {
        self.shared
            .profiler_data
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Generate debug visualization data
    pub fn generate_debug_visualization(&self) -> DebugVisualization {
        let dependencies = self
            .shared
            .dependencies
            .read()
            .unwrap_or_else(|e| e.into_inner());
        let reload_attempts = self
            .shared
            .reload_attempts
            .read()
            .unwrap_or_else(|e| e.into_inner());
        let profiler = self
            .shared
            .profiler_data
            .read()
            .unwrap_or_else(|e| e.into_inner());
        let mut dependency_graph = Vec::new();
        let mut memory_breakdown = HashMap::new();
        let mut load_timeline = Vec::new();
        let mut error_log = Vec::new();

        // Build dependency graph
        for (handle, dep_info) in dependencies.iter() {
            let from_name = format!("Asset_{}", handle.0);

            for dep_handle in &dep_info.dependencies {
                let to_name = format!("Asset_{}", dep_handle.0);
                dependency_graph.push(DependencyEdge {
                    from: from_name.clone(),
                    to: to_name,
                    dependency_type: "depends_on".to_string(),
                });
            }
        }

        if let Some(snapshot) = profiler.memory_usage.last() {
            memory_breakdown.insert(AssetType::Texture, snapshot.texture_memory);
            memory_breakdown.insert(AssetType::Audio, snapshot.audio_memory);
            memory_breakdown.insert(AssetType::Model, snapshot.model_memory);
        }

        for load in &profiler.asset_loads {
            load_timeline.push(LoadTimelineEntry {
                timestamp: load.timestamp,
                asset_name: load
                    .asset_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("asset")
                    .to_string(),
                event_type: if load.success {
                    "load_success".to_string()
                } else {
                    "load_failure".to_string()
                },
                duration: Some(load.load_time),
            });

            if !load.success {
                error_log.push(ErrorEntry {
                    timestamp: load.timestamp,
                    asset_path: load.asset_path.to_string_lossy().to_string(),
                    error_type: "load".to_string(),
                    message: load
                        .error
                        .clone()
                        .unwrap_or_else(|| "unknown error".to_string()),
                    severity: ErrorSeverity::Error,
                });
            }
        }

        for attempts in reload_attempts.values() {
            for attempt in attempts {
                if !attempt.success {
                    error_log.push(ErrorEntry {
                        timestamp: SystemTime::now(),
                        asset_path: attempt.path.to_string_lossy().to_string(),
                        error_type: "reload".to_string(),
                        message: attempt
                            .error
                            .clone()
                            .unwrap_or_else(|| "unknown error".to_string()),
                        severity: ErrorSeverity::Error,
                    });
                }
            }
        }

        DebugVisualization {
            asset_dependency_graph: dependency_graph,
            memory_breakdown,
            load_timeline,
            error_log,
        }
    }

    /// Count watched files
    fn count_watched_files(&self) -> u64 {
        let mut count = 0;
        for path in &self.watch_paths {
            if path.is_dir() {
                count += Self::count_files_recursive(path);
            } else if path.is_file() {
                count += 1;
            }
        }
        count
    }

    /// Count files recursively using a synchronous helper wrapped in a blocking task.
    fn count_files_recursive(dir: &Path) -> u64 {
        fn walk(path: &Path) -> u64 {
            match std::fs::read_dir(path) {
                Ok(entries) => entries.fold(0, |acc, entry| {
                    acc + entry
                        .ok()
                        .map(|entry| {
                            let path = entry.path();
                            if path.is_dir() {
                                walk(&path)
                            } else if path.is_file() {
                                1
                            } else {
                                0
                            }
                        })
                        .unwrap_or(0)
                }),
                Err(_) => 0,
            }
        }

        walk(dir)
    }

    /// Shutdown hot reload system
    pub async fn shutdown(&self) {
        log::info!("Shutting down hot reload manager...");

        // Signal shutdown
        self.shared.shutdown_signal.store(true, Ordering::Relaxed);
        self.shared.shutdown_notify.notify_waiters();

        // Tear down the main-thread lifecycle slot: dropping the watcher stops
        // new events at the source, then we wait for the tasks to drain out.
        let runtime = self
            .runtime
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();

        if let Some(runtime) = runtime {
            drop(runtime.watcher);

            if let Some(worker_handle) = runtime.worker_handle {
                if let Err(e) = worker_handle.await {
                    log::error!("Worker task failed to shutdown cleanly: {}", e);
                }
            }

            if let Some(profiler_handle) = runtime.profiler_handle {
                if let Err(e) = profiler_handle.await {
                    log::error!("Profiler task failed to shutdown cleanly: {}", e);
                }
            }
        }

        log::info!("Hot reload manager shutdown complete");
    }
}

impl From<HotReloadError> for AssetError {
    fn from(err: HotReloadError) -> Self {
        match err {
            HotReloadError::Io(io_err) => AssetError::Io(io_err),
            _ => AssetError::HotReloadFailed {
                path: "hot_reload".to_string(),
                error: err.to_string(),
            },
        }
    }
}

// Import glob matching from parent module
use super::glob_match;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_ignore_path() {
        let ignore_patterns = vec![
            "*.tmp".to_string(),
            "*.backup".to_string(),
            ".git/**".to_string(),
        ];

        assert!(HotReloadManager::should_ignore_path(
            Path::new("test.tmp"),
            &ignore_patterns
        ));
        assert!(HotReloadManager::should_ignore_path(
            Path::new("file.backup"),
            &ignore_patterns
        ));
        assert!(!HotReloadManager::should_ignore_path(
            Path::new("asset.png"),
            &ignore_patterns
        ));
    }

    #[test]
    fn test_change_type_detection() {
        assert_eq!(ChangeType::Created as u8, 0);
        assert_eq!(ChangeType::Modified as u8, 1);
        assert_eq!(ChangeType::Deleted as u8, 2);
        assert_eq!(ChangeType::Renamed as u8, 3);
    }

    #[test]
    fn test_debounced_events_flow_through_channel() {
        // The watcher-thread -> worker-task handoff is a channel; verify the
        // debounce + send logic in isolation with a local receiver.
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut debounce_map: HashMap<PathBuf, Instant> = HashMap::new();
        let config = HotReloadConfig {
            debounce_duration_ms: 10_000,
            ..HotReloadConfig::default()
        };

        let event = Event::new(EventKind::Modify(ModifyKind::Data(
            notify::event::DataChange::Any,
        )))
        .add_path(PathBuf::from("assets/texture.png"));

        HotReloadManager::handle_file_event(event.clone(), &mut debounce_map, &tx, &config);
        // Second immediate event for the same path is debounced away.
        HotReloadManager::handle_file_event(event, &mut debounce_map, &tx, &config);

        assert_eq!(rx.try_recv().unwrap().path, PathBuf::from("assets/texture.png"));
        assert!(rx.try_recv().is_err());
    }
}
