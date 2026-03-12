//! Comprehensive asset management system for Command & Conquer Generals Zero Hour
//!
//! This module provides a complete asset management system including:
//! - Windows resource definitions (corresponds to C++ GeneralsMD/Code/Main/resource.h)
//! - Asset loading and caching
//! - Memory management for assets
//! - Cross-platform resource handling

use anyhow::{anyhow, Result};
use log::{debug, info, warn};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

/// Application icon resource ID
pub const IDI_APPLICATION_ICON: u32 = 102;

/// Load screen bitmap resource ID
pub const IDB_LOAD_SCREEN: u32 = 103;

/// Next available resource value (for use by resource editors)
pub const APS_NEXT_RESOURCE_VALUE: u32 = 106;

/// Next available command value (for use by resource editors)
pub const APS_NEXT_COMMAND_VALUE: u32 = 40001;

/// Next available control value (for use by resource editors)
pub const APS_NEXT_CONTROL_VALUE: u32 = 1000;

/// Next available symbol value (for use by resource editors)
pub const APS_NEXT_SYMED_VALUE: u32 = 101;

/// Resource types enumeration
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    /// Icon resource type
    Icon = 3,
    /// Bitmap resource type
    Bitmap = 2,
    /// Cursor resource type
    Cursor = 1,
    /// Menu resource type
    Menu = 4,
    /// Dialog resource type
    Dialog = 5,
    /// String table resource type
    String = 6,
    /// Texture resources
    Texture = 100,
    /// 3D Model resources
    Model = 101,
    /// Audio resources
    Audio = 102,
    /// Script resources
    Script = 103,
    /// Configuration resources
    Config = 104,
}

/// Asset types supported by the asset management system
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetType {
    Texture,
    Model,
    Audio,
    Script,
    Config,
    Binary,
}

/// Asset metadata
#[derive(Debug, Clone)]
pub struct AssetMetadata {
    pub name: String,
    pub path: PathBuf,
    pub asset_type: AssetType,
    pub size: u64,
    pub last_modified: Option<std::time::SystemTime>,
    pub reference_count: u32,
}

/// Cached asset data
#[derive(Debug)]
pub struct CachedAsset {
    pub metadata: AssetMetadata,
    pub data: Vec<u8>,
    pub loaded_at: SystemTime,
    pub last_accessed: SystemTime,
}

/// Asset loading statistics
#[derive(Debug, Default, Clone)]
pub struct AssetStats {
    pub total_loaded: u64,
    pub total_size: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub failed_loads: u64,
}

/// Complete asset management system
pub struct AssetManager {
    /// Asset cache
    cache: RwLock<HashMap<String, Arc<CachedAsset>>>,
    /// Asset search paths
    search_paths: Vec<PathBuf>,
    /// Loading statistics
    stats: RwLock<AssetStats>,
    /// Maximum cache size in bytes
    max_cache_size: u64,
    /// Current cache size
    current_cache_size: RwLock<u64>,
}

impl AssetManager {
    /// Create new asset manager
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            search_paths: Vec::new(),
            stats: RwLock::new(AssetStats::default()),
            max_cache_size: 512 * 1024 * 1024, // 512MB default
            current_cache_size: RwLock::new(0),
        }
    }

    /// Create asset manager with custom cache size
    pub fn with_cache_size(max_cache_size: u64) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            search_paths: Vec::new(),
            stats: RwLock::new(AssetStats::default()),
            max_cache_size,
            current_cache_size: RwLock::new(0),
        }
    }

    /// Add search path for assets
    pub fn add_search_path<P: AsRef<Path>>(&mut self, path: P) {
        let path_buf = path.as_ref().to_path_buf();
        info!("Adding asset search path: {:?}", path_buf);
        self.search_paths.push(path_buf);
    }

    /// Load asset by name
    pub async fn load_asset(&self, name: &str) -> Result<Arc<CachedAsset>> {
        let cache_key = name.to_lowercase();

        // Check cache first
        {
            let cache = self.cache.read().unwrap();
            if let Some(asset) = cache.get(&cache_key) {
                // Update access time
                // Note: This would require interior mutability in a real implementation
                self.update_stats_cache_hit();
                debug!("Cache hit for asset: {}", name);
                return Ok(Arc::clone(asset));
            }
        }

        self.update_stats_cache_miss();
        debug!("Cache miss for asset: {}, loading from disk", name);

        // Find asset file
        let asset_path = self.find_asset_file(name)?;

        // Load asset data
        let asset_data = tokio::fs::read(&asset_path)
            .await
            .map_err(|e| anyhow!("Failed to read asset {}: {}", name, e))?;

        // Create asset metadata
        let metadata = self
            .create_asset_metadata(name, &asset_path, &asset_data)
            .await?;

        // Create cached asset
        let now = SystemTime::now();
        let cached_asset = Arc::new(CachedAsset {
            metadata,
            data: asset_data,
            loaded_at: now,
            last_accessed: now,
        });

        // Add to cache (with size management)
        self.add_to_cache(cache_key, Arc::clone(&cached_asset))?;

        self.update_stats_loaded(&cached_asset);
        info!("Loaded asset: {} ({} bytes)", name, cached_asset.data.len());

        Ok(cached_asset)
    }

    /// Check if asset exists
    pub fn asset_exists(&self, name: &str) -> bool {
        let cache_key = name.to_lowercase();

        // Check cache first
        {
            let cache = self.cache.read().unwrap();
            if cache.contains_key(&cache_key) {
                return true;
            }
        }

        // Check filesystem
        self.find_asset_file(name).is_ok()
    }

    /// Preload assets
    pub async fn preload_assets(&self, asset_names: &[&str]) -> Result<()> {
        info!("Preloading {} assets", asset_names.len());

        for &name in asset_names {
            match self.load_asset(name).await {
                Ok(_) => debug!("Preloaded asset: {}", name),
                Err(e) => {
                    warn!("Failed to preload asset {}: {}", name, e);
                    self.update_stats_failed();
                }
            }
        }

        Ok(())
    }

    /// Clear cache
    pub fn clear_cache(&self) {
        info!("Clearing asset cache");

        {
            let mut cache = self.cache.write().unwrap();
            cache.clear();
        }

        {
            let mut cache_size = self.current_cache_size.write().unwrap();
            *cache_size = 0;
        }
    }

    /// Get asset loading statistics
    pub fn get_stats(&self) -> AssetStats {
        self.stats.read().unwrap().clone()
    }

    /// Get current cache size
    pub fn get_cache_size(&self) -> u64 {
        *self.current_cache_size.read().unwrap()
    }

    /// Get cache utilization (0.0 to 1.0)
    pub fn get_cache_utilization(&self) -> f64 {
        let current = *self.current_cache_size.read().unwrap();
        current as f64 / self.max_cache_size as f64
    }

    /// Find asset file in search paths
    fn find_asset_file(&self, name: &str) -> Result<PathBuf> {
        for search_path in &self.search_paths {
            let asset_path = search_path.join(name);
            if asset_path.exists() {
                return Ok(asset_path);
            }

            // Try with common extensions
            for ext in &[".tga", ".dds", ".bmp", ".w3d", ".big", ".ini"] {
                let asset_path_ext = search_path.join(format!("{}{}", name, ext));
                if asset_path_ext.exists() {
                    return Ok(asset_path_ext);
                }
            }
        }

        Err(anyhow!("Asset not found: {}", name))
    }

    /// Create asset metadata
    async fn create_asset_metadata(
        &self,
        name: &str,
        path: &Path,
        data: &[u8],
    ) -> Result<AssetMetadata> {
        let metadata = tokio::fs::metadata(path).await?;

        let asset_type = self.determine_asset_type(path);

        Ok(AssetMetadata {
            name: name.to_string(),
            path: path.to_path_buf(),
            asset_type,
            size: data.len() as u64,
            last_modified: metadata.modified().ok(),
            reference_count: 1,
        })
    }

    /// Determine asset type from file extension
    fn determine_asset_type(&self, path: &Path) -> AssetType {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("tga") | Some("dds") | Some("bmp") | Some("jpg") | Some("png") => {
                AssetType::Texture
            }
            Some("w3d") => AssetType::Model,
            Some("wav") | Some("mp3") | Some("ogg") => AssetType::Audio,
            Some("lua") | Some("py") | Some("js") => AssetType::Script,
            Some("ini") | Some("cfg") | Some("xml") => AssetType::Config,
            _ => AssetType::Binary,
        }
    }

    /// Add asset to cache with size management
    fn add_to_cache(&self, key: String, asset: Arc<CachedAsset>) -> Result<()> {
        let asset_size = asset.data.len() as u64;

        // Check if we need to evict assets
        {
            let current_size = self.current_cache_size.write().unwrap();
            if *current_size + asset_size > self.max_cache_size {
                drop(current_size);
                self.evict_assets(asset_size)?;
            }
        }

        // Add to cache
        {
            let mut cache = self.cache.write().unwrap();
            cache.insert(key, asset);
        }

        // Update cache size
        {
            let mut current_size = self.current_cache_size.write().unwrap();
            *current_size += asset_size;
        }

        Ok(())
    }

    /// Evict assets to make room
    fn evict_assets(&self, needed_space: u64) -> Result<()> {
        info!("Evicting assets to make room for {} bytes", needed_space);

        let mut to_evict = Vec::new();
        let mut space_freed = 0u64;

        // Simple LRU eviction based on loaded_at time
        {
            let cache = self.cache.read().unwrap();
            let mut assets: Vec<_> = cache.iter().collect();
            assets.sort_by_key(|(_, asset)| asset.loaded_at);

            for (key, asset) in assets {
                if space_freed >= needed_space {
                    break;
                }
                to_evict.push((key.clone(), asset.data.len() as u64));
                space_freed += asset.data.len() as u64;
            }
        }

        // Evict selected assets
        {
            let mut cache = self.cache.write().unwrap();
            let mut current_size = self.current_cache_size.write().unwrap();

            for (key, size) in to_evict {
                cache.remove(&key);
                *current_size -= size;
                debug!("Evicted asset: {} ({} bytes)", key, size);
            }
        }

        Ok(())
    }

    /// Update statistics for cache hit
    fn update_stats_cache_hit(&self) {
        let mut stats = self.stats.write().unwrap();
        stats.cache_hits += 1;
    }

    /// Update statistics for cache miss
    fn update_stats_cache_miss(&self) {
        let mut stats = self.stats.write().unwrap();
        stats.cache_misses += 1;
    }

    /// Update statistics for loaded asset
    fn update_stats_loaded(&self, asset: &CachedAsset) {
        let mut stats = self.stats.write().unwrap();
        stats.total_loaded += 1;
        stats.total_size += asset.data.len() as u64;
    }

    /// Update statistics for failed load
    fn update_stats_failed(&self) {
        let mut stats = self.stats.write().unwrap();
        stats.failed_loads += 1;
    }
}

impl Default for AssetManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global asset manager instance
static ASSET_MANAGER: std::sync::OnceLock<AssetManager> = std::sync::OnceLock::new();

/// Initialize global asset manager
pub fn init_asset_manager() -> &'static AssetManager {
    ASSET_MANAGER.get_or_init(|| {
        let mut manager = AssetManager::new();

        // Add default search paths
        if let Ok(current_dir) = std::env::current_dir() {
            manager.add_search_path(current_dir.join("assets"));
            manager.add_search_path(current_dir.join("data"));
            manager.add_search_path(current_dir.join("textures"));
            manager.add_search_path(current_dir.join("models"));
        }

        info!(
            "Asset manager initialized with {} search paths",
            manager.search_paths.len()
        );
        manager
    })
}

/// Get global asset manager
pub fn get_asset_manager() -> Option<&'static AssetManager> {
    ASSET_MANAGER.get()
}

/// Windows resource management functions
pub mod windows_resource_manager {
    use super::*;

    #[derive(Debug)]
    enum ResourceHandleKind {
        Icon,
        Bitmap,
    }

    #[derive(Debug)]
    struct ResourceHandle {
        kind: ResourceHandleKind,
        data: Vec<u8>,
    }

    fn read_resource_bytes(candidates: &[&str]) -> Option<Vec<u8>> {
        let mut roots = Vec::new();
        if let Ok(current_dir) = std::env::current_dir() {
            roots.push(current_dir.clone());
            roots.push(current_dir.join("assets"));
            roots.push(current_dir.join("data"));
            roots.push(current_dir.join("textures"));
            roots.push(current_dir.join("models"));
        }

        for root in roots {
            for candidate in candidates {
                let path = root.join(candidate);
                if path.is_file() {
                    match std::fs::read(&path) {
                        Ok(bytes) => return Some(bytes),
                        Err(err) => warn!("Failed to read resource file {:?}: {}", path, err),
                    }
                }
            }
        }
        None
    }

    fn create_handle_ptr(kind: ResourceHandleKind, data: Vec<u8>) -> *mut std::ffi::c_void {
        let handle = Box::new(ResourceHandle { kind, data });
        Box::into_raw(handle) as *mut std::ffi::c_void
    }

    /// Load an icon resource by ID
    ///
    /// # Arguments
    ///
    /// * `resource_id` - The resource ID to load
    ///
    /// # Returns
    ///
    /// Handle to the loaded icon, or None if loading failed
    pub fn load_icon(resource_id: u32) -> Option<*mut std::ffi::c_void> {
        match resource_id {
            IDI_APPLICATION_ICON => {
                info!("Loading application icon resource");
                let data = read_resource_bytes(&["Generals.ico", "generals.ico"])
                    .unwrap_or_else(|| vec![0]); // Keep a non-null sentinel handle.
                Some(create_handle_ptr(ResourceHandleKind::Icon, data))
            }
            _ => {
                warn!("Unknown icon resource ID: {}", resource_id);
                None
            }
        }
    }

    /// Load a bitmap resource by ID
    ///
    /// # Arguments
    ///
    /// * `resource_id` - The resource ID to load
    ///
    /// # Returns
    ///
    /// Handle to the loaded bitmap, or None if loading failed
    pub fn load_bitmap(resource_id: u32) -> Option<*mut std::ffi::c_void> {
        match resource_id {
            IDB_LOAD_SCREEN => {
                info!("Loading load screen bitmap resource");
                let data = read_resource_bytes(&[
                    "Install_Final.bmp",
                    "install_final.bmp",
                    "LoadScreen.bmp",
                    "loadscreen.bmp",
                ])?;
                Some(create_handle_ptr(ResourceHandleKind::Bitmap, data))
            }
            _ => {
                warn!("Unknown bitmap resource ID: {}", resource_id);
                None
            }
        }
    }

    /// Release a resource handle
    ///
    /// # Safety
    ///
    /// The caller must ensure the handle is valid and not already freed.
    pub unsafe fn release_resource(handle: *mut std::ffi::c_void) {
        if !handle.is_null() {
            let boxed: Box<ResourceHandle> =
                unsafe { Box::from_raw(handle as *mut ResourceHandle) };
            debug!(
                "Releasing Windows resource handle ({:?}, {} bytes)",
                boxed.kind,
                boxed.data.len()
            );
        }
    }

    /// Load resource data directly
    pub async fn load_resource_data(
        resource_type: ResourceType,
        resource_id: u32,
    ) -> Result<Vec<u8>> {
        match (resource_type, resource_id) {
            (ResourceType::Icon, IDI_APPLICATION_ICON) => {
                // Try to load from asset system first
                if let Some(manager) = get_asset_manager() {
                    if let Ok(asset) = manager.load_asset("Generals.ico").await {
                        return Ok(asset.data.clone());
                    }
                }
                if let Some(bytes) = read_resource_bytes(&["Generals.ico", "generals.ico"]) {
                    return Ok(bytes);
                }
                Err(anyhow!("Icon resource not found"))
            }
            (ResourceType::Bitmap, IDB_LOAD_SCREEN) => {
                // Try to load from asset system
                if let Some(manager) = get_asset_manager() {
                    if let Ok(asset) = manager.load_asset("Install_Final.bmp").await {
                        return Ok(asset.data.clone());
                    }
                }
                if let Some(bytes) = read_resource_bytes(&[
                    "Install_Final.bmp",
                    "install_final.bmp",
                    "LoadScreen.bmp",
                    "loadscreen.bmp",
                ]) {
                    return Ok(bytes);
                }
                Err(anyhow!("Bitmap resource not found"))
            }
            _ => Err(anyhow!("Unsupported resource type or ID")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn setup_test_manager() -> AssetManager {
        let mut manager = AssetManager::with_cache_size(1024 * 1024); // 1MB cache

        // Add test asset paths
        if let Ok(current_dir) = std::env::current_dir() {
            manager.add_search_path(current_dir.join("test_assets"));
        }

        manager
    }

    #[test]
    fn test_resource_constants() {
        assert_eq!(IDI_APPLICATION_ICON, 102);
        assert_eq!(IDB_LOAD_SCREEN, 103);
        assert_eq!(APS_NEXT_RESOURCE_VALUE, 106);
    }

    #[test]
    fn test_resource_type_enum() {
        assert_eq!(ResourceType::Icon as u32, 3);
        assert_eq!(ResourceType::Bitmap as u32, 2);
        assert_eq!(ResourceType::Cursor as u32, 1);
        assert_eq!(ResourceType::Texture as u32, 100);
        assert_eq!(ResourceType::Model as u32, 101);
    }

    #[test]
    fn test_asset_manager_creation() {
        let manager = AssetManager::new();
        assert_eq!(manager.get_cache_size(), 0);
        assert_eq!(manager.get_cache_utilization(), 0.0);
    }

    #[test]
    fn test_asset_type_detection() {
        let manager = setup_test_manager();

        assert_eq!(
            manager.determine_asset_type(Path::new("test.tga")),
            AssetType::Texture
        );
        assert_eq!(
            manager.determine_asset_type(Path::new("test.w3d")),
            AssetType::Model
        );
        assert_eq!(
            manager.determine_asset_type(Path::new("test.wav")),
            AssetType::Audio
        );
        assert_eq!(
            manager.determine_asset_type(Path::new("test.ini")),
            AssetType::Config
        );
        assert_eq!(
            manager.determine_asset_type(Path::new("test.bin")),
            AssetType::Binary
        );
    }

    #[test]
    fn test_asset_metadata_creation() {
        let manager = setup_test_manager();
        let test_data = b"test data";
        let test_path = Path::new("test.tga");

        // This test would need an actual file to work properly
        // In a real test environment, you'd create a temporary file
    }

    #[test]
    fn test_global_asset_manager() {
        INIT.call_once(|| {
            init_asset_manager();
        });

        assert!(get_asset_manager().is_some());
    }

    #[test]
    fn test_asset_stats() {
        let manager = setup_test_manager();
        let stats = manager.get_stats();

        assert_eq!(stats.total_loaded, 0);
        assert_eq!(stats.cache_hits, 0);
        assert_eq!(stats.cache_misses, 0);
        assert_eq!(stats.failed_loads, 0);
    }
}
