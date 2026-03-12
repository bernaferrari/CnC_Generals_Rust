//! Asset Metadata Management
//!
//! This module provides metadata storage and retrieval for assets:
//! - Persistent metadata storage
//! - Asset versioning
//! - Dependency tracking
//! - Search and query capabilities

use crate::{Asset, AssetError, AssetMetadata, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Metadata store
#[derive(Debug)]
pub struct MetadataStore {
    metadata_dir: PathBuf,
    cache: Arc<RwLock<HashMap<Uuid, AssetMetadata>>>,
}

impl MetadataStore {
    /// Create new metadata store
    pub fn new(metadata_dir: &Path) -> Self {
        Self {
            metadata_dir: metadata_dir.to_path_buf(),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Store asset metadata
    pub fn store(&self, asset: &Asset) -> Result<()> {
        // Create metadata directory if it doesn't exist
        std::fs::create_dir_all(&self.metadata_dir)?;

        // Serialize metadata to JSON
        let json = serde_json::to_string_pretty(&asset.metadata)?;

        // Write to file
        let filename = format!("{}.json", asset.id);
        let path = self.metadata_dir.join(filename);
        std::fs::write(path, json)?;

        log::debug!("Stored metadata for asset: {}", asset.id);
        Ok(())
    }

    /// Load asset metadata
    pub fn load(&self, asset_id: Uuid) -> Result<AssetMetadata> {
        let filename = format!("{}.json", asset_id);
        let path = self.metadata_dir.join(filename);

        if !path.exists() {
            return Err(AssetError::FileNotFound { path });
        }

        let json = std::fs::read_to_string(&path)?;
        let metadata: AssetMetadata = serde_json::from_str(&json)?;

        log::debug!("Loaded metadata for asset: {}", asset_id);
        Ok(metadata)
    }

    /// Update metadata
    pub async fn update(&self, asset_id: Uuid, metadata: AssetMetadata) -> Result<()> {
        // Update cache
        let mut cache = self.cache.write().await;
        cache.insert(asset_id, metadata.clone());

        // Write to disk
        let json = serde_json::to_string_pretty(&metadata)?;
        let filename = format!("{}.json", asset_id);
        let path = self.metadata_dir.join(filename);
        std::fs::write(path, json)?;

        log::debug!("Updated metadata for asset: {}", asset_id);
        Ok(())
    }

    /// Delete metadata
    pub async fn delete(&self, asset_id: Uuid) -> Result<()> {
        // Remove from cache
        let mut cache = self.cache.write().await;
        cache.remove(&asset_id);

        // Delete file
        let filename = format!("{}.json", asset_id);
        let path = self.metadata_dir.join(filename);

        if path.exists() {
            std::fs::remove_file(path)?;
            log::debug!("Deleted metadata for asset: {}", asset_id);
        }

        Ok(())
    }

    /// List all stored metadata
    pub fn list_all(&self) -> Result<Vec<Uuid>> {
        let mut ids = Vec::new();

        if !self.metadata_dir.exists() {
            return Ok(ids);
        }

        for entry in std::fs::read_dir(&self.metadata_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(uuid) = Uuid::parse_str(stem) {
                        ids.push(uuid);
                    }
                }
            }
        }

        Ok(ids)
    }

    /// Search metadata by tag
    pub fn search_by_tag(&self, tag: &str) -> Result<Vec<Uuid>> {
        let mut matches = Vec::new();

        for id in self.list_all()? {
            if let Ok(metadata) = self.load(id) {
                if metadata.tags.contains(&tag.to_string()) {
                    matches.push(id);
                }
            }
        }

        Ok(matches)
    }

    /// Search metadata by property
    pub fn search_by_property(&self, key: &str, value: &str) -> Result<Vec<Uuid>> {
        let mut matches = Vec::new();

        for id in self.list_all()? {
            if let Ok(metadata) = self.load(id) {
                if let Some(prop_value) = metadata.custom_properties.get(key) {
                    if prop_value == value {
                        matches.push(id);
                    }
                }
            }
        }

        Ok(matches)
    }

    /// Get metadata directory
    pub fn metadata_dir(&self) -> &Path {
        &self.metadata_dir
    }

    /// Clear all metadata
    pub async fn clear(&self) -> Result<()> {
        let mut cache = self.cache.write().await;
        cache.clear();

        if self.metadata_dir.exists() {
            std::fs::remove_dir_all(&self.metadata_dir)?;
            std::fs::create_dir_all(&self.metadata_dir)?;
        }

        log::info!("Cleared all metadata");
        Ok(())
    }
}

/// Metadata query builder
#[derive(Debug, Default)]
pub struct MetadataQuery {
    tags: Vec<String>,
    properties: HashMap<String, String>,
    min_version: Option<u32>,
    max_version: Option<u32>,
}

impl MetadataQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }

    pub fn with_min_version(mut self, version: u32) -> Self {
        self.min_version = Some(version);
        self
    }

    pub fn with_max_version(mut self, version: u32) -> Self {
        self.max_version = Some(version);
        self
    }

    /// Execute query against metadata store
    pub fn execute(&self, store: &MetadataStore) -> Result<Vec<Uuid>> {
        let mut results = Vec::new();

        for id in store.list_all()? {
            if let Ok(metadata) = store.load(id) {
                if self.matches(&metadata) {
                    results.push(id);
                }
            }
        }

        Ok(results)
    }

    fn matches(&self, metadata: &AssetMetadata) -> bool {
        // Check tags
        for tag in &self.tags {
            if !metadata.tags.contains(tag) {
                return false;
            }
        }

        // Check properties
        for (key, value) in &self.properties {
            if metadata.custom_properties.get(key) != Some(value) {
                return false;
            }
        }

        // Check version
        if let Some(min_version) = self.min_version {
            if metadata.version < min_version {
                return false;
            }
        }

        if let Some(max_version) = self.max_version {
            if metadata.version > max_version {
                return false;
            }
        }

        true
    }
}

/// Asset dependency tracker
#[derive(Debug)]
pub struct DependencyTracker {
    dependencies: HashMap<Uuid, Vec<Uuid>>,
    dependents: HashMap<Uuid, Vec<Uuid>>,
}

impl DependencyTracker {
    pub fn new() -> Self {
        Self {
            dependencies: HashMap::new(),
            dependents: HashMap::new(),
        }
    }

    /// Add dependency relationship
    pub fn add_dependency(&mut self, asset: Uuid, depends_on: Uuid) {
        self.dependencies
            .entry(asset)
            .or_insert_with(Vec::new)
            .push(depends_on);

        self.dependents
            .entry(depends_on)
            .or_insert_with(Vec::new)
            .push(asset);
    }

    /// Get dependencies of an asset
    pub fn get_dependencies(&self, asset: &Uuid) -> Vec<Uuid> {
        self.dependencies.get(asset).cloned().unwrap_or_default()
    }

    /// Get dependents of an asset
    pub fn get_dependents(&self, asset: &Uuid) -> Vec<Uuid> {
        self.dependents.get(asset).cloned().unwrap_or_default()
    }

    /// Get all dependencies recursively
    pub fn get_all_dependencies(&self, asset: &Uuid) -> Vec<Uuid> {
        let mut all_deps = Vec::new();
        let mut visited = std::collections::HashSet::new();
        self.collect_dependencies(asset, &mut all_deps, &mut visited);
        all_deps
    }

    fn collect_dependencies(
        &self,
        asset: &Uuid,
        result: &mut Vec<Uuid>,
        visited: &mut std::collections::HashSet<Uuid>,
    ) {
        if visited.contains(asset) {
            return;
        }

        visited.insert(*asset);

        for dep in self.get_dependencies(asset) {
            result.push(dep);
            self.collect_dependencies(&dep, result, visited);
        }
    }

    /// Remove asset from tracker
    pub fn remove(&mut self, asset: &Uuid) {
        self.dependencies.remove(asset);
        self.dependents.remove(asset);

        // Remove from all dependency lists
        for deps in self.dependencies.values_mut() {
            deps.retain(|id| id != asset);
        }

        for deps in self.dependents.values_mut() {
            deps.retain(|id| id != asset);
        }
    }
}

impl Default for DependencyTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_metadata_store_creation() {
        let temp_dir = TempDir::new().unwrap();
        let store = MetadataStore::new(temp_dir.path());

        assert_eq!(store.metadata_dir(), temp_dir.path());
    }

    #[test]
    fn test_metadata_store_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let store = MetadataStore::new(temp_dir.path());

        let asset = Asset::new("test", crate::AssetType::Mesh);
        let asset_id = asset.id;

        store.store(&asset).unwrap();

        let loaded = store.load(asset_id).unwrap();
        assert_eq!(loaded.version, asset.metadata.version);
    }

    #[test]
    fn test_metadata_list_all() {
        let temp_dir = TempDir::new().unwrap();
        let store = MetadataStore::new(temp_dir.path());

        let asset1 = Asset::new("test1", crate::AssetType::Mesh);
        let asset2 = Asset::new("test2", crate::AssetType::Texture);

        store.store(&asset1).unwrap();
        store.store(&asset2).unwrap();

        let ids = store.list_all().unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&asset1.id));
        assert!(ids.contains(&asset2.id));
    }

    #[test]
    fn test_metadata_query() {
        let query = MetadataQuery::new()
            .with_tag("test")
            .with_property("key", "value")
            .with_min_version(1);

        assert_eq!(query.tags.len(), 1);
        assert_eq!(query.properties.len(), 1);
        assert_eq!(query.min_version, Some(1));
    }

    #[test]
    fn test_dependency_tracker() {
        let mut tracker = DependencyTracker::new();

        let asset1 = Uuid::new_v4();
        let asset2 = Uuid::new_v4();
        let asset3 = Uuid::new_v4();

        tracker.add_dependency(asset2, asset1);
        tracker.add_dependency(asset3, asset2);

        let deps = tracker.get_dependencies(&asset2);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0], asset1);

        let all_deps = tracker.get_all_dependencies(&asset3);
        assert_eq!(all_deps.len(), 2);
        assert!(all_deps.contains(&asset2));
        assert!(all_deps.contains(&asset1));
    }

    #[test]
    fn test_dependency_tracker_remove() {
        let mut tracker = DependencyTracker::new();

        let asset1 = Uuid::new_v4();
        let asset2 = Uuid::new_v4();

        tracker.add_dependency(asset2, asset1);
        tracker.remove(&asset1);

        let deps = tracker.get_dependencies(&asset2);
        assert!(!deps.contains(&asset1));
    }
}
