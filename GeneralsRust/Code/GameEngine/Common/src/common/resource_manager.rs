/// Basic resource management system for the game engine
///
/// This provides a foundation for asset loading and management that other
/// engine systems can build upon.
use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};
use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

/// Resource types that the engine can handle
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResourceType {
    Texture,
    Model,
    Audio,
    Map,
    Script,
    Config,
    Data,
    Archive,
}

/// Resource metadata and loading state
#[derive(Debug, Clone)]
pub struct ResourceInfo {
    pub resource_type: ResourceType,
    pub file_path: PathBuf,
    pub size: u64,
    pub last_modified: std::time::SystemTime,
    pub loaded: bool,
    pub load_time: Option<std::time::Duration>,
}

/// Basic resource data container
pub struct ResourceData {
    pub data: Vec<u8>,
    pub info: ResourceInfo,
}

/// Resource loading result
pub type ResourceResult<T> = Result<T>;

/// Basic resource manager implementation
///
/// This provides core functionality for loading and managing game resources.
/// More specialized managers can extend this for specific resource types.
pub struct ResourceManager {
    /// Loaded resources cache
    resources: Arc<RwLock<HashMap<NameKeyType, Arc<ResourceData>>>>,
    /// Canonical resource names keyed by name-key
    resource_names: Arc<RwLock<HashMap<NameKeyType, String>>>,
    /// Search paths for resources
    search_paths: Vec<PathBuf>,
    /// Resource type mappings by file extension
    type_mappings: HashMap<String, ResourceType>,
    /// Load statistics
    stats: Arc<Mutex<LoadStats>>,
}

#[derive(Debug, Default)]
pub struct LoadStats {
    pub total_loaded: u64,
    pub total_size: u64,
    pub load_failures: u64,
    pub cache_hits: u64,
    pub load_time: std::time::Duration,
}

impl ResourceManager {
    /// Create a new resource manager with default configuration
    pub fn new() -> Self {
        let mut manager = Self {
            resources: Arc::new(RwLock::new(HashMap::new())),
            resource_names: Arc::new(RwLock::new(HashMap::new())),
            search_paths: vec![
                PathBuf::from("Data"),
                PathBuf::from("Mods"),
                PathBuf::from("Assets"),
                PathBuf::from("."), // Current directory as fallback
            ],
            type_mappings: HashMap::new(),
            stats: Arc::new(Mutex::new(LoadStats::default())),
        };

        manager.setup_default_type_mappings();
        manager
    }

    /// Set up default file extension to resource type mappings
    fn setup_default_type_mappings(&mut self) {
        // Texture formats
        self.type_mappings
            .insert("tga".to_string(), ResourceType::Texture);
        self.type_mappings
            .insert("dds".to_string(), ResourceType::Texture);
        self.type_mappings
            .insert("png".to_string(), ResourceType::Texture);
        self.type_mappings
            .insert("jpg".to_string(), ResourceType::Texture);

        // Model formats
        self.type_mappings
            .insert("w3d".to_string(), ResourceType::Model);
        self.type_mappings
            .insert("mesh".to_string(), ResourceType::Model);

        // Audio formats
        self.type_mappings
            .insert("wav".to_string(), ResourceType::Audio);
        self.type_mappings
            .insert("mp3".to_string(), ResourceType::Audio);
        self.type_mappings
            .insert("ogg".to_string(), ResourceType::Audio);

        // Map formats
        self.type_mappings
            .insert("map".to_string(), ResourceType::Map);
        self.type_mappings
            .insert("wld".to_string(), ResourceType::Map);

        // Script formats
        self.type_mappings
            .insert("scb".to_string(), ResourceType::Script);
        self.type_mappings
            .insert("lua".to_string(), ResourceType::Script);

        // Config formats
        self.type_mappings
            .insert("ini".to_string(), ResourceType::Config);
        self.type_mappings
            .insert("cfg".to_string(), ResourceType::Config);

        // Archive formats
        self.type_mappings
            .insert("big".to_string(), ResourceType::Archive);
        self.type_mappings
            .insert("zip".to_string(), ResourceType::Archive);
    }

    /// Add a search path for resources
    pub fn add_search_path<P: AsRef<Path>>(&mut self, path: P) {
        let path_buf = path.as_ref().to_path_buf();
        if !self.search_paths.contains(&path_buf) {
            info!("Adding resource search path: {}", path_buf.display());
            self.search_paths.push(path_buf);
        }
    }

    /// Find a resource file in the search paths
    pub fn find_resource_path(&self, resource_name: &str) -> Option<PathBuf> {
        for search_path in &self.search_paths {
            let full_path = search_path.join(resource_name);
            if full_path.exists() && full_path.is_file() {
                debug!(
                    "Found resource {} at {}",
                    resource_name,
                    full_path.display()
                );
                return Some(full_path);
            }
        }

        warn!("Resource not found in search paths: {}", resource_name);
        None
    }

    /// Get resource type from file extension
    pub fn get_resource_type(&self, path: &Path) -> ResourceType {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| self.type_mappings.get(&ext.to_lowercase()))
            .cloned()
            .unwrap_or(ResourceType::Data)
    }

    /// Load a resource by name
    pub fn load_resource(&self, resource_name: &str) -> ResourceResult<Arc<ResourceData>> {
        let start_time = std::time::Instant::now();
        let key = NameKeyGenerator::name_to_key_lowercase(resource_name);

        // Check cache first
        {
            let resources = self.resources.read().unwrap();
            if let Some(resource) = resources.get(&key) {
                let mut stats = self.stats.lock().unwrap();
                stats.cache_hits += 1;
                debug!("Resource cache hit: {}", resource_name);
                return Ok(resource.clone());
            }
        }

        // Find the resource file
        let file_path = self
            .find_resource_path(resource_name)
            .ok_or_else(|| anyhow::anyhow!("Resource not found: {}", resource_name))?;

        // Get file metadata
        let metadata = fs::metadata(&file_path)
            .with_context(|| format!("Failed to get metadata for: {}", file_path.display()))?;

        // Load file data
        let data = fs::read(&file_path)
            .with_context(|| format!("Failed to read resource: {}", file_path.display()))?;

        let load_time = start_time.elapsed();

        // Create resource info
        let info = ResourceInfo {
            resource_type: self.get_resource_type(&file_path),
            file_path: file_path.clone(),
            size: metadata.len(),
            last_modified: metadata
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            loaded: true,
            load_time: Some(load_time),
        };

        // Create resource data
        let resource_data = Arc::new(ResourceData {
            data,
            info: info.clone(),
        });

        // Cache the resource
        {
            let mut resources = self.resources.write().unwrap();
            resources.insert(key, resource_data.clone());
        }
        {
            let mut names = self.resource_names.write().unwrap();
            names
                .entry(key)
                .or_insert_with(|| resource_name.to_string());
        }

        // Update statistics
        {
            let mut stats = self.stats.lock().unwrap();
            stats.total_loaded += 1;
            stats.total_size += metadata.len();
            stats.load_time += load_time;
        }

        info!(
            "Loaded resource: {} ({} bytes, {:.2}ms)",
            resource_name,
            metadata.len(),
            load_time.as_secs_f64() * 1000.0
        );

        Ok(resource_data)
    }

    /// Preload a list of resources
    pub fn preload_resources(&self, resource_names: &[&str]) -> ResourceResult<()> {
        info!("Preloading {} resources", resource_names.len());
        let start_time = std::time::Instant::now();

        let mut failed = Vec::new();

        for &resource_name in resource_names {
            if let Err(err) = self.load_resource(resource_name) {
                error!("Failed to preload resource {}: {}", resource_name, err);
                failed.push(resource_name);

                let mut stats = self.stats.lock().unwrap();
                stats.load_failures += 1;
            }
        }

        let total_time = start_time.elapsed();
        info!(
            "Preloading completed in {:.2}ms ({} failed)",
            total_time.as_secs_f64() * 1000.0,
            failed.len()
        );

        if !failed.is_empty() {
            warn!("Some resources failed to preload: {:?}", failed);
        }

        Ok(())
    }

    /// Check if a resource is already loaded
    pub fn is_loaded(&self, resource_name: &str) -> bool {
        let key = NameKeyGenerator::name_to_key_lowercase(resource_name);
        let resources = self.resources.read().unwrap();
        resources.contains_key(&key)
    }

    /// Unload a specific resource from cache
    pub fn unload_resource(&self, resource_name: &str) -> bool {
        let key = NameKeyGenerator::name_to_key_lowercase(resource_name);
        let mut resources = self.resources.write().unwrap();
        let removed = resources.remove(&key).is_some();
        if removed {
            let mut names = self.resource_names.write().unwrap();
            names.remove(&key);
        }
        removed
    }

    /// Clear all loaded resources
    pub fn clear_cache(&self) {
        let mut resources = self.resources.write().unwrap();
        let count = resources.len();
        resources.clear();
        {
            let mut names = self.resource_names.write().unwrap();
            names.clear();
        }

        info!("Cleared resource cache ({} resources)", count);
    }

    /// Get resource loading statistics
    pub fn get_stats(&self) -> LoadStats {
        let stats = self.stats.lock().unwrap();
        LoadStats {
            total_loaded: stats.total_loaded,
            total_size: stats.total_size,
            load_failures: stats.load_failures,
            cache_hits: stats.cache_hits,
            load_time: stats.load_time,
        }
    }

    /// Get list of currently loaded resources
    pub fn get_loaded_resources(&self) -> Vec<String> {
        let names = self.resource_names.read().unwrap();
        names.values().cloned().collect()
    }

    /// Get total memory usage of loaded resources
    pub fn get_memory_usage(&self) -> u64 {
        let resources = self.resources.read().unwrap();
        resources.values().map(|r| r.info.size).sum()
    }
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global resource manager instance
static RESOURCE_MANAGER: std::sync::OnceLock<Arc<Mutex<ResourceManager>>> =
    std::sync::OnceLock::new();

/// Get the global resource manager instance
pub fn get_resource_manager() -> Arc<Mutex<ResourceManager>> {
    RESOURCE_MANAGER
        .get_or_init(|| Arc::new(Mutex::new(ResourceManager::new())))
        .clone()
}

/// Convenience function to load a resource using the global manager
pub fn load_resource(resource_name: &str) -> ResourceResult<Arc<ResourceData>> {
    let manager = get_resource_manager();
    let manager = manager.lock().unwrap();
    manager.load_resource(resource_name)
}

/// Convenience function to check if a resource is loaded
pub fn is_resource_loaded(resource_name: &str) -> bool {
    let manager = get_resource_manager();
    let manager = manager.lock().unwrap();
    manager.is_loaded(resource_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn test_resource_manager_creation() {
        let manager = ResourceManager::new();
        assert!(!manager.search_paths.is_empty());
        assert!(!manager.type_mappings.is_empty());
    }

    #[test]
    fn test_resource_type_detection() {
        let manager = ResourceManager::new();

        assert_eq!(
            manager.get_resource_type(Path::new("test.tga")),
            ResourceType::Texture
        );
        assert_eq!(
            manager.get_resource_type(Path::new("model.w3d")),
            ResourceType::Model
        );
        assert_eq!(
            manager.get_resource_type(Path::new("sound.wav")),
            ResourceType::Audio
        );
        assert_eq!(
            manager.get_resource_type(Path::new("config.ini")),
            ResourceType::Config
        );
        assert_eq!(
            manager.get_resource_type(Path::new("unknown.xyz")),
            ResourceType::Data
        );
    }

    #[test]
    fn test_resource_loading() -> Result<()> {
        let temp_dir = tempdir()?;
        let test_file = temp_dir.path().join("test.txt");
        let test_content = b"Hello, World!";

        {
            let mut file = File::create(&test_file)?;
            file.write_all(test_content)?;
        }

        let mut manager = ResourceManager::new();
        manager.add_search_path(temp_dir.path());

        let resource = manager.load_resource("test.txt")?;
        assert_eq!(resource.data, test_content);
        assert_eq!(resource.info.size, test_content.len() as u64);
        assert!(resource.info.loaded);

        // Test cache hit
        let resource2 = manager.load_resource("test.txt")?;
        assert!(Arc::ptr_eq(&resource, &resource2));

        Ok(())
    }

    #[test]
    fn test_resource_cache_case_insensitive() -> Result<()> {
        NameKeyGenerator::reset();
        let temp_dir = tempdir()?;
        let file_path = temp_dir.path().join("Example.TXT");
        {
            let mut file = File::create(&file_path)?;
            file.write_all(b"hello world")?;
        }

        let mut manager = ResourceManager::new();
        manager.add_search_path(temp_dir.path());

        let upper = manager.load_resource("Example.TXT")?;
        assert_eq!(upper.info.size, 11);

        let stats = manager.get_stats();
        assert_eq!(stats.cache_hits, 0);

        let lower = manager.load_resource("example.txt")?;
        assert!(Arc::ptr_eq(&upper, &lower));

        let stats = manager.get_stats();
        assert_eq!(stats.cache_hits, 1);

        assert_eq!(
            manager.get_loaded_resources(),
            vec!["Example.TXT".to_string()]
        );

        assert!(manager.unload_resource("EXAMPLE.TXT"));
        assert!(!manager.is_loaded("Example.TXT"));
        assert!(manager.get_loaded_resources().is_empty());

        Ok(())
    }

    #[test]
    fn test_resource_not_found() {
        let manager = ResourceManager::new();
        let result = manager.load_resource("nonexistent.txt");
        assert!(result.is_err());

        let stats = manager.get_stats();
        assert_eq!(stats.load_failures, 0); // load_resource doesn't increment failures, only preload does
    }
}
