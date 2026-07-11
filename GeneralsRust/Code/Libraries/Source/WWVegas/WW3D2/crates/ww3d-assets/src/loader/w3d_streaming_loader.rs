//! Enhanced W3D File Streaming Loader
//!
//! This module provides streaming W3D file loading with dependency management,
//! memory pooling, and asynchronous loading capabilities matching the original
//! C++ WW3D asset loading system.

use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::mpsc;

use crate::AssetManager;

/// Loading priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LoadingPriority {
    Critical = 0,   // Load immediately (blocking)
    High = 1,       // Load as soon as possible
    Normal = 2,     // Standard loading priority
    Low = 3,        // Load when resources are available
    Background = 4, // Load only when nothing else is loading
}

/// Asset loading request
#[derive(Clone)]
pub struct AssetLoadRequest {
    pub asset_path: PathBuf,
    pub priority: LoadingPriority,
    pub dependencies: Vec<PathBuf>,
    pub callback: Option<Arc<dyn Fn(AssetLoadResult) + Send + Sync>>,
}

impl AssetLoadRequest {
    pub fn new(path: PathBuf, priority: LoadingPriority) -> Self {
        Self {
            asset_path: path,
            priority,
            dependencies: Vec::new(),
            callback: None,
        }
    }

    pub fn with_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(AssetLoadResult) + Send + Sync + 'static,
    {
        self.callback = Some(Arc::new(callback));
        self
    }

    pub fn with_dependencies(mut self, deps: Vec<PathBuf>) -> Self {
        self.dependencies = deps;
        self
    }
}

/// Asset loading result
#[derive(Debug)]
pub enum AssetLoadResult {
    Success {
        path: PathBuf,
        asset_type: AssetType,
        data: Vec<u8>,
    },
    Error {
        path: PathBuf,
        error: AssetLoadError,
    },
    DependencyMissing {
        path: PathBuf,
        missing_deps: Vec<PathBuf>,
    },
}

/// Asset types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetType {
    Mesh,
    Hierarchy,
    Animation,
    Texture,
    Material,
    Sound,
    Other,
}

/// Asset loading error
#[derive(Debug, thiserror::Error)]
pub enum AssetLoadError {
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("Invalid W3D file format")]
    InvalidFormat,
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Dependency error: {0}")]
    DependencyError(String),
    #[error("Memory limit exceeded")]
    MemoryLimitExceeded,
}

/// Memory pool for asset data
pub struct AssetMemoryPool {
    /// Available memory chunks
    chunks: VecDeque<Vec<u8>>,
    /// Total memory allocated
    total_allocated: usize,
    /// Maximum memory limit
    max_memory: usize,
    /// Chunk size
    chunk_size: usize,
}

impl AssetMemoryPool {
    pub fn new(max_memory: usize, chunk_size: usize) -> Self {
        Self {
            chunks: VecDeque::new(),
            total_allocated: 0,
            max_memory,
            chunk_size,
        }
    }

    /// Allocate memory from the pool
    pub fn allocate(&mut self, size: usize) -> Option<Vec<u8>> {
        if self.total_allocated + size > self.max_memory {
            return None; // Memory limit exceeded
        }

        // Try to find a suitable existing chunk
        for chunk in &mut self.chunks {
            if chunk.capacity() >= size {
                let mut data = Vec::new();
                std::mem::swap(chunk, &mut data);
                self.chunks.push_back(Vec::with_capacity(self.chunk_size));
                self.total_allocated += size;
                return Some(data);
            }
        }

        // Allocate new chunk with initialized memory
        let data = vec![0u8; size];
        self.total_allocated += size;
        Some(data)
    }

    /// Deallocate memory back to the pool
    pub fn deallocate(&mut self, mut data: Vec<u8>) {
        let size = data.len();
        data.clear();
        data.reserve(self.chunk_size);

        if self.chunks.len() < 10 {
            // Keep max 10 chunks in pool
            self.chunks.push_back(data);
        }

        self.total_allocated = self.total_allocated.saturating_sub(size);
    }

    /// Get memory usage statistics
    pub fn get_stats(&self) -> MemoryPoolStats {
        MemoryPoolStats {
            total_allocated: self.total_allocated,
            max_memory: self.max_memory,
            chunk_count: self.chunks.len(),
            utilization: if self.max_memory > 0 {
                self.total_allocated as f32 / self.max_memory as f32
            } else {
                0.0
            },
        }
    }
}

/// Memory pool statistics
#[derive(Debug, Clone)]
pub struct MemoryPoolStats {
    pub total_allocated: usize,
    pub max_memory: usize,
    pub chunk_count: usize,
    pub utilization: f32,
}

/// Streaming W3D file loader
pub struct StreamingW3dLoader {
    /// Asset manager reference
    _asset_manager: Arc<AssetManager>,
    /// Memory pool for asset data
    memory_pool: Arc<Mutex<AssetMemoryPool>>,
    /// Loading queue
    load_queue: Arc<Mutex<VecDeque<AssetLoadRequest>>>,
    /// Currently loading assets
    loading_assets: Arc<RwLock<HashMap<PathBuf, AssetLoadRequest>>>,
    /// Loaded asset cache
    asset_cache: Arc<RwLock<HashMap<PathBuf, CachedAsset>>>,
    /// Task sender for loading requests
    task_sender: mpsc::UnboundedSender<AssetLoadRequest>,
    /// Task receiver for loading requests
    task_receiver: Arc<Mutex<mpsc::UnboundedReceiver<AssetLoadRequest>>>,
    /// Loading threads
    loading_threads: Vec<std::thread::JoinHandle<()>>,
    /// Maximum concurrent loads
    max_concurrent_loads: usize,
}

impl StreamingW3dLoader {
    /// Create a new streaming W3D loader
    pub fn new(asset_manager: Arc<AssetManager>, max_memory: usize) -> Self {
        let memory_pool = Arc::new(Mutex::new(AssetMemoryPool::new(max_memory, 1024 * 1024))); // 1MB chunks
        let load_queue = Arc::new(Mutex::new(VecDeque::new()));
        let loading_assets = Arc::new(RwLock::new(HashMap::new()));
        let asset_cache = Arc::new(RwLock::new(HashMap::new()));

        let (task_sender, task_receiver) = mpsc::unbounded_channel();

        Self {
            _asset_manager: asset_manager,
            memory_pool,
            load_queue,
            loading_assets,
            asset_cache,
            task_sender,
            task_receiver: Arc::new(Mutex::new(task_receiver)),
            loading_threads: Vec::new(),
            max_concurrent_loads: 4,
        }
    }

    /// Initialize the loader with background threads
    pub fn initialize(&mut self) {
        for i in 0..self.max_concurrent_loads {
            let receiver = Arc::clone(&self.task_receiver);
            let memory_pool = Arc::clone(&self.memory_pool);
            let asset_cache = Arc::clone(&self.asset_cache);
            let loading_assets = Arc::clone(&self.loading_assets);

            let handle = std::thread::spawn(move || {
                Self::loading_worker_thread(i, receiver, memory_pool, asset_cache, loading_assets);
            });

            self.loading_threads.push(handle);
        }
    }

    /// Load an asset asynchronously
    pub fn load_asset_async(&self, request: AssetLoadRequest) {
        let mut queue = self.load_queue.lock().unwrap();

        // Insert based on priority
        let insert_pos = queue
            .iter()
            .position(|req| req.priority > request.priority)
            .unwrap_or(queue.len());

        queue.insert(insert_pos, request);
    }

    /// Load an asset synchronously (blocking)
    pub fn load_asset_sync(&self, path: &Path) -> Result<AssetLoadResult, AssetLoadError> {
        self.load_w3d_file_sync(path)
    }

    /// Process the loading queue
    pub fn process_queue(&self) {
        let mut queue = self.load_queue.lock().unwrap();
        let loading_count = self.loading_assets.read().unwrap().len();

        // Submit requests to loading threads
        while loading_count < self.max_concurrent_loads && !queue.is_empty() {
            if let Some(request) = queue.pop_front() {
                let path = request.asset_path.clone();
                let path_clone = path.clone();
                self.loading_assets
                    .write()
                    .unwrap()
                    .insert(path_clone, request.clone());

                if let Err(_) = self.task_sender.send(request) {
                    // Channel closed, remove from loading assets
                    self.loading_assets.write().unwrap().remove(&path);
                }
            }
        }
    }

    /// Get asset from cache
    pub fn get_cached_asset(&self, path: &Path) -> Option<CachedAsset> {
        self.asset_cache.read().unwrap().get(path).cloned()
    }

    /// Check if asset is currently loading
    pub fn is_asset_loading(&self, path: &Path) -> bool {
        self.loading_assets.read().unwrap().contains_key(path)
    }

    /// Unload an asset
    pub fn unload_asset(&self, path: &Path) {
        if let Some(asset) = self.asset_cache.write().unwrap().remove(path) {
            // Return memory to pool
            if let Some(data) = asset.data {
                self.memory_pool.lock().unwrap().deallocate(data);
            }
        }
    }

    /// Get loading statistics
    pub fn get_loading_stats(&self) -> LoadingStats {
        let queue_len = self.load_queue.lock().unwrap().len();
        let loading_count = self.loading_assets.read().unwrap().len();
        let cached_count = self.asset_cache.read().unwrap().len();
        let memory_stats = self.memory_pool.lock().unwrap().get_stats();

        LoadingStats {
            queue_length: queue_len,
            loading_count,
            cached_count,
            memory_stats,
        }
    }

    /// Shutdown the loader
    pub fn shutdown(&mut self) {
        // Close the channel
        drop(self.task_sender.clone());

        // Wait for loading threads to finish
        for handle in self.loading_threads.drain(..) {
            let _ = handle.join();
        }
    }

    /// Loading worker thread function
    fn loading_worker_thread(
        thread_id: usize,
        receiver: Arc<Mutex<mpsc::UnboundedReceiver<AssetLoadRequest>>>,
        memory_pool: Arc<Mutex<AssetMemoryPool>>,
        asset_cache: Arc<RwLock<HashMap<PathBuf, CachedAsset>>>,
        loading_assets: Arc<RwLock<HashMap<PathBuf, AssetLoadRequest>>>,
    ) {
        println!("Asset loading thread {} started", thread_id);

        loop {
            let request = {
                let mut receiver_lock = receiver.lock().unwrap();
                match receiver_lock.try_recv() {
                    Ok(req) => req,
                    Err(mpsc::error::TryRecvError::Empty) => {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                        continue;
                    }
                    Err(mpsc::error::TryRecvError::Disconnected) => break,
                }
            };

            // Load the asset
            let result = Self::load_asset_from_request(&request, &memory_pool);

            // Cache the result
            if let AssetLoadResult::Success {
                path,
                asset_type,
                data,
            } = &result
            {
                let cached_asset = CachedAsset {
                    asset_type: *asset_type,
                    data: Some(data.clone()),
                    last_access: std::time::SystemTime::now(),
                    reference_count: 1,
                };
                asset_cache
                    .write()
                    .unwrap()
                    .insert(path.clone(), cached_asset);
            }

            // Remove from loading assets
            loading_assets.write().unwrap().remove(&request.asset_path);

            // Call callback if provided
            if let Some(callback) = &request.callback {
                callback(result);
            }
        }

        println!("Asset loading thread {} finished", thread_id);
    }

    /// Load asset from request
    fn load_asset_from_request(
        request: &AssetLoadRequest,
        memory_pool: &Arc<Mutex<AssetMemoryPool>>,
    ) -> AssetLoadResult {
        // Check dependencies first
        for dep in &request.dependencies {
            if !std::path::Path::new(dep).exists() {
                return AssetLoadResult::DependencyMissing {
                    path: request.asset_path.clone(),
                    missing_deps: vec![dep.clone()],
                };
            }
        }

        // Load the asset
        match Self::load_w3d_file(&request.asset_path, memory_pool) {
            Ok((asset_type, data)) => AssetLoadResult::Success {
                path: request.asset_path.clone(),
                asset_type,
                data,
            },
            Err(error) => AssetLoadResult::Error {
                path: request.asset_path.clone(),
                error,
            },
        }
    }

    /// Load W3D file synchronously
    fn load_w3d_file_sync(&self, path: &Path) -> Result<AssetLoadResult, AssetLoadError> {
        let (asset_type, data) = Self::load_w3d_file(path, &self.memory_pool)?;
        Ok(AssetLoadResult::Success {
            path: path.to_path_buf(),
            asset_type,
            data,
        })
    }

    /// Load W3D file from disk
    fn load_w3d_file(
        path: &Path,
        memory_pool: &Arc<Mutex<AssetMemoryPool>>,
    ) -> Result<(AssetType, Vec<u8>), AssetLoadError> {
        if !path.exists() {
            return Err(AssetLoadError::FileNotFound(
                path.to_string_lossy().to_string(),
            ));
        }

        let file_size = std::fs::metadata(path)?.len() as usize;
        let mut memory_pool_lock = memory_pool.lock().unwrap();

        let mut data = memory_pool_lock
            .allocate(file_size)
            .ok_or(AssetLoadError::MemoryLimitExceeded)?;

        data.resize(file_size, 0);

        let mut file = File::open(path)?;
        file.read_exact(&mut data)?;

        // Determine asset type from file content
        let asset_type = Self::determine_asset_type(&data)?;

        Ok((asset_type, data))
    }

    /// Determine asset type from W3D file data
    fn determine_asset_type(data: &[u8]) -> Result<AssetType, AssetLoadError> {
        if data.len() < 8 {
            return Err(AssetLoadError::InvalidFormat);
        }

        // Read first chunk header
        let chunk_type = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

        match chunk_type {
            0x00000000 => Ok(AssetType::Mesh),      // W3D_CHUNK_MESH
            0x00000100 => Ok(AssetType::Hierarchy), // W3D_CHUNK_HIERARCHY
            0x00000200 => Ok(AssetType::Animation), // W3D_CHUNK_ANIMATION
            0x00000030 => Ok(AssetType::Texture),   // W3D_CHUNK_TEXTURES
            0x00000028 => Ok(AssetType::Material),  // W3D_CHUNK_MATERIAL_INFO
            _ => Ok(AssetType::Other),
        }
    }
}

/// Cached asset data
#[derive(Debug, Clone)]
pub struct CachedAsset {
    pub asset_type: AssetType,
    pub data: Option<Vec<u8>>,
    pub last_access: std::time::SystemTime,
    pub reference_count: usize,
}

/// Loading statistics
#[derive(Debug, Clone)]
pub struct LoadingStats {
    pub queue_length: usize,
    pub loading_count: usize,
    pub cached_count: usize,
    pub memory_stats: MemoryPoolStats,
}

/// Dependency resolver for assets
pub struct AssetDependencyResolver {
    /// Dependency graph
    dependency_graph: HashMap<PathBuf, Vec<PathBuf>>,
    /// Reverse dependency graph
    reverse_dependencies: HashMap<PathBuf, Vec<PathBuf>>,
}

impl Default for AssetDependencyResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetDependencyResolver {
    pub fn new() -> Self {
        Self {
            dependency_graph: HashMap::new(),
            reverse_dependencies: HashMap::new(),
        }
    }

    /// Add a dependency relationship
    pub fn add_dependency(&mut self, asset: PathBuf, depends_on: PathBuf) {
        self.dependency_graph
            .entry(asset.clone())
            .or_default()
            .push(depends_on.clone());

        self.reverse_dependencies
            .entry(depends_on)
            .or_default()
            .push(asset);
    }

    /// Get dependencies for an asset
    pub fn get_dependencies(&self, asset: &Path) -> Vec<PathBuf> {
        self.dependency_graph
            .get(asset)
            .cloned()
            .unwrap_or_default()
    }

    /// Get assets that depend on the given asset
    pub fn get_dependents(&self, asset: &Path) -> Vec<PathBuf> {
        self.reverse_dependencies
            .get(asset)
            .cloned()
            .unwrap_or_default()
    }

    /// Check if an asset can be loaded (all dependencies exist)
    pub fn can_load_asset(&self, asset: &Path) -> bool {
        for dep in self.get_dependencies(asset) {
            if !dep.exists() {
                return false;
            }
        }
        true
    }

    /// Get loading order for a set of assets
    pub fn get_loading_order(&self, assets: &[PathBuf]) -> Vec<PathBuf> {
        // Simple topological sort implementation
        let mut result = Vec::new();
        let mut visited = HashMap::new();
        let mut visiting = HashMap::new();

        fn visit(
            asset: &PathBuf,
            dependency_graph: &HashMap<PathBuf, Vec<PathBuf>>,
            result: &mut Vec<PathBuf>,
            visited: &mut HashMap<PathBuf, bool>,
            visiting: &mut HashMap<PathBuf, bool>,
        ) {
            if *visited.get(asset).unwrap_or(&false) {
                return;
            }

            if *visiting.get(asset).unwrap_or(&false) {
                // Cycle detected - for now, just continue
                return;
            }

            visiting.insert(asset.clone(), true);

            if let Some(deps) = dependency_graph.get(asset) {
                for dep in deps {
                    visit(dep, dependency_graph, result, visited, visiting);
                }
            }

            visiting.insert(asset.clone(), false);
            visited.insert(asset.clone(), true);
            result.push(asset.clone());
        }

        for asset in assets {
            visit(
                asset,
                &self.dependency_graph,
                &mut result,
                &mut visited,
                &mut visiting,
            );
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_loading_priority_ordering() {
        assert!(LoadingPriority::Critical < LoadingPriority::High);
        assert!(LoadingPriority::High < LoadingPriority::Normal);
        assert!(LoadingPriority::Normal < LoadingPriority::Low);
        assert!(LoadingPriority::Low < LoadingPriority::Background);
    }

    #[test]
    fn test_asset_load_request_creation() {
        let path = PathBuf::from("test.w3d");
        let request = AssetLoadRequest::new(path.clone(), LoadingPriority::High);

        assert_eq!(request.asset_path, path);
        assert_eq!(request.priority, LoadingPriority::High);
        assert!(request.dependencies.is_empty());
        assert!(request.callback.is_none());
    }

    #[test]
    fn test_memory_pool_allocation() {
        let mut pool = AssetMemoryPool::new(1024 * 1024, 64 * 1024); // 1MB limit, 64KB chunks

        // Allocate small chunk
        let data = pool.allocate(1024).unwrap();
        assert_eq!(data.capacity(), 1024);

        let stats = pool.get_stats();
        assert_eq!(stats.total_allocated, 1024);
        assert_eq!(stats.chunk_count, 0); // No chunks returned to pool yet

        // Deallocate
        pool.deallocate(data);
        let stats = pool.get_stats();
        assert_eq!(stats.total_allocated, 0);
        assert_eq!(stats.chunk_count, 1); // One chunk in pool
    }

    #[test]
    fn test_dependency_resolver() {
        let mut resolver = AssetDependencyResolver::new();

        let asset1 = PathBuf::from("mesh.w3d");
        let asset2 = PathBuf::from("texture.dds");
        let asset3 = PathBuf::from("animation.w3d");

        resolver.add_dependency(asset1.clone(), asset2.clone());
        resolver.add_dependency(asset3.clone(), asset1.clone());

        let deps = resolver.get_dependencies(&asset1);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0], asset2);

        let dependents = resolver.get_dependents(&asset1);
        assert_eq!(dependents.len(), 1);
        assert_eq!(dependents[0], asset3);
    }
}
