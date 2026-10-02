/// Asset management system for WW3D
///
/// This module implements asset loading, caching, and lifecycle management.
use crate::animation::*;
use crate::errors::{W3DError, W3DResult};
use crate::mesh::Mesh;
use crate::texture::Texture;
use crate::w3d_io::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Asset types that can be managed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetType {
    Mesh,
    Texture,
    Animation,
    Hierarchy,
    HModel,
    Material,
    Shader,
}

/// Asset loading status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetStatus {
    /// Asset is not loaded
    Unloaded,
    /// Asset is currently being loaded
    Loading,
    /// Asset is loaded and ready
    Loaded,
    /// Asset failed to load
    Failed,
}

/// Asset handle for reference-counted assets
#[derive(Debug, Clone)]
pub struct AssetHandle<T> {
    inner: Option<T>,
    status: AssetStatus,
    name: String,
}

impl<T> AssetHandle<T> {
    fn new(name: String) -> Self {
        Self {
            inner: None,
            status: AssetStatus::Unloaded,
            name,
        }
    }

    fn with_asset(name: String, asset: T) -> Self {
        Self {
            inner: Some(asset),
            status: AssetStatus::Loaded,
            name,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn status(&self) -> AssetStatus {
        self.status
    }

    pub fn is_loaded(&self) -> bool {
        self.status() == AssetStatus::Loaded
    }

    pub fn get(&self) -> Option<T>
    where
        T: Clone,
    {
        self.inner.clone()
    }

    pub fn with<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&T) -> R,
    {
        self.inner.as_ref().map(f)
    }

    fn set(&mut self, asset: T) {
        self.inner = Some(asset);
        self.status = AssetStatus::Loaded;
    }

    fn set_status(&mut self, status: AssetStatus) {
        self.status = status;
    }

    fn set_failed(&mut self) {
        self.status = AssetStatus::Failed;
    }
}

/// Asset loader trait for loading different asset types
pub trait AssetLoader: Send + Sync {
    /// The asset type this loader handles
    type Asset;

    /// Load an asset from a file
    fn load_from_file(&self, path: &Path) -> W3DResult<Self::Asset>;

    /// Get supported file extensions
    fn supported_extensions(&self) -> &[&str];
}

/// Mesh asset loader
#[derive(Debug)]
pub struct MeshLoader;

impl AssetLoader for MeshLoader {
    type Asset = Mesh;

    fn load_from_file(&self, path: &Path) -> W3DResult<Self::Asset> {
        let chunks = load_w3d_file(path)?;

        // Find mesh chunk
        for chunk in chunks {
            if let W3DChunk::Mesh(w3d_mesh) = chunk {
                return Mesh::from_w3d(&w3d_mesh);
            }
        }

        Err(W3DError::AssetNotFound(format!(
            "No mesh found in file: {}",
            path.display()
        )))
    }

    fn supported_extensions(&self) -> &[&str] {
        &["w3d"]
    }
}

/// Hierarchy asset loader
#[derive(Debug)]
pub struct HierarchyLoader;

impl AssetLoader for HierarchyLoader {
    type Asset = Hierarchy;

    fn load_from_file(&self, path: &Path) -> W3DResult<Self::Asset> {
        let chunks = load_w3d_file(path)?;

        // Find hierarchy chunk
        for chunk in chunks {
            if let W3DChunk::Hierarchy(w3d_hier) = chunk {
                return Hierarchy::from_w3d(&w3d_hier);
            }
        }

        Err(W3DError::AssetNotFound(format!(
            "No hierarchy found in file: {}",
            path.display()
        )))
    }

    fn supported_extensions(&self) -> &[&str] {
        &["w3d"]
    }
}

/// Animation asset loader
#[derive(Debug)]
pub struct AnimationLoader;

impl AssetLoader for AnimationLoader {
    type Asset = HierarchyAnimation;

    fn load_from_file(&self, path: &Path) -> W3DResult<Self::Asset> {
        let chunks = load_w3d_file(path)?;

        // Find animation chunk
        for chunk in chunks {
            if let W3DChunk::Animation(w3d_anim) = chunk {
                return HierarchyAnimation::from_w3d(&w3d_anim);
            }
        }

        Err(W3DError::AssetNotFound(format!(
            "No animation found in file: {}",
            path.display()
        )))
    }

    fn supported_extensions(&self) -> &[&str] {
        &["w3d"]
    }
}

/// Asset cache entry
struct CacheEntry<T> {
    handle: AssetHandle<T>,
    #[allow(dead_code)] // Kept for potential cache invalidation/debugging
    path: PathBuf,
    reference_count: usize,
}

/// Asset manager for centralized asset management
pub struct AssetManager {
    mesh_cache: HashMap<String, CacheEntry<Mesh>>,
    texture_cache: HashMap<String, CacheEntry<Texture>>,
    hierarchy_cache: HashMap<String, CacheEntry<Hierarchy>>,
    animation_cache: HashMap<String, CacheEntry<HierarchyAnimation>>,
    search_paths: Vec<PathBuf>,
    mesh_loader: MeshLoader,
    hierarchy_loader: HierarchyLoader,
    animation_loader: AnimationLoader,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            mesh_cache: HashMap::new(),
            texture_cache: HashMap::new(),
            hierarchy_cache: HashMap::new(),
            animation_cache: HashMap::new(),
            search_paths: Vec::new(),
            mesh_loader: MeshLoader,
            hierarchy_loader: HierarchyLoader,
            animation_loader: AnimationLoader,
        }
    }

    /// Add a search path for asset loading
    pub fn add_search_path<P: AsRef<Path>>(&mut self, path: P) {
        self.search_paths.push(path.as_ref().to_path_buf());
    }

    /// Find a file in the search paths
    fn find_file(&self, filename: &str) -> Option<PathBuf> {
        // First try as absolute/relative path
        let path = Path::new(filename);
        if path.exists() {
            return Some(path.to_path_buf());
        }

        // Then search in search paths
        for search_path in self.search_paths.iter() {
            let full_path = search_path.join(filename);
            if full_path.exists() {
                return Some(full_path);
            }
        }

        None
    }

    /// Load a mesh asset
    pub fn load_mesh(&mut self, name: &str) -> W3DResult<AssetHandle<Mesh>> {
        // Check if already in cache
        if let Some(entry) = self.mesh_cache.get_mut(name) {
            entry.reference_count += 1;
            return Ok(entry.handle.clone());
        }

        // Find the file
        let path = self
            .find_file(name)
            .ok_or_else(|| W3DError::AssetNotFound(name.to_string()))?;

        // Create handle and mark as loading
        let mut handle = AssetHandle::<Mesh>::new(name.to_string());
        handle.set_status(AssetStatus::Loading);

        // Load the mesh
        match self.mesh_loader.load_from_file(&path) {
            Ok(mesh) => {
                handle.set(mesh);

                // Add to cache
                self.mesh_cache.insert(
                    name.to_string(),
                    CacheEntry {
                        handle: handle.clone(),
                        path,
                        reference_count: 1,
                    },
                );

                Ok(handle)
            }
            Err(e) => {
                handle.set_failed();
                Err(e)
            }
        }
    }

    /// Load a hierarchy asset
    pub fn load_hierarchy(&mut self, name: &str) -> W3DResult<AssetHandle<Hierarchy>> {
        // Check if already in cache
        if let Some(entry) = self.hierarchy_cache.get_mut(name) {
            entry.reference_count += 1;
            return Ok(entry.handle.clone());
        }

        // Find the file
        let path = self
            .find_file(name)
            .ok_or_else(|| W3DError::AssetNotFound(name.to_string()))?;

        // Create handle and mark as loading
        let mut handle = AssetHandle::<Hierarchy>::new(name.to_string());
        handle.set_status(AssetStatus::Loading);

        // Load the hierarchy
        match self.hierarchy_loader.load_from_file(&path) {
            Ok(hierarchy) => {
                handle.set(hierarchy);

                // Add to cache
                self.hierarchy_cache.insert(
                    name.to_string(),
                    CacheEntry {
                        handle: handle.clone(),
                        path,
                        reference_count: 1,
                    },
                );

                Ok(handle)
            }
            Err(e) => {
                handle.set_failed();
                Err(e)
            }
        }
    }

    /// Load an animation asset
    pub fn load_animation(&mut self, name: &str) -> W3DResult<AssetHandle<HierarchyAnimation>> {
        // Check if already in cache
        if let Some(entry) = self.animation_cache.get_mut(name) {
            entry.reference_count += 1;
            return Ok(entry.handle.clone());
        }

        // Find the file
        let path = self
            .find_file(name)
            .ok_or_else(|| W3DError::AssetNotFound(name.to_string()))?;

        // Create handle and mark as loading
        let mut handle = AssetHandle::<HierarchyAnimation>::new(name.to_string());
        handle.set_status(AssetStatus::Loading);

        // Load the animation
        match self.animation_loader.load_from_file(&path) {
            Ok(animation) => {
                handle.set(animation);

                // Add to cache
                self.animation_cache.insert(
                    name.to_string(),
                    CacheEntry {
                        handle: handle.clone(),
                        path,
                        reference_count: 1,
                    },
                );

                Ok(handle)
            }
            Err(e) => {
                handle.set_failed();
                Err(e)
            }
        }
    }

    /// Register a pre-loaded mesh
    pub fn register_mesh(&mut self, name: String, mesh: Mesh) -> AssetHandle<Mesh> {
        let handle = AssetHandle::with_asset(name.clone(), mesh);

        self.mesh_cache.insert(
            name.clone(),
            CacheEntry {
                handle: handle.clone(),
                path: PathBuf::from(&name),
                reference_count: 1,
            },
        );

        handle
    }

    /// Register a pre-loaded texture
    pub fn register_texture(&mut self, name: String, texture: Texture) -> AssetHandle<Texture> {
        let handle = AssetHandle::with_asset(name.clone(), texture);

        self.texture_cache.insert(
            name.clone(),
            CacheEntry {
                handle: handle.clone(),
                path: PathBuf::from(&name),
                reference_count: 1,
            },
        );

        handle
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> CacheStats {
        CacheStats {
            mesh_count: self.mesh_cache.len(),
            texture_count: self.texture_cache.len(),
            hierarchy_count: self.hierarchy_cache.len(),
            animation_count: self.animation_cache.len(),
        }
    }

    /// Clear all caches
    pub fn clear_all_caches(&mut self) {
        self.mesh_cache.clear();
        self.texture_cache.clear();
        self.hierarchy_cache.clear();
        self.animation_cache.clear();
    }

    /// Clear unused assets (reference count == 0)
    pub fn clear_unused(&mut self) {
        self.mesh_cache.retain(|_, entry| entry.reference_count > 0);
        self.texture_cache.retain(|_, entry| entry.reference_count > 0);
        self.hierarchy_cache
            .retain(|_, entry| entry.reference_count > 0);
        self.animation_cache
            .retain(|_, entry| entry.reference_count > 0);
    }
}

impl Default for AssetManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics
#[derive(Debug, Clone, Copy)]
pub struct CacheStats {
    pub mesh_count: usize,
    pub texture_count: usize,
    pub hierarchy_count: usize,
    pub animation_count: usize,
}

impl CacheStats {
    pub fn total_count(&self) -> usize {
        self.mesh_count + self.texture_count + self.hierarchy_count + self.animation_count
    }
}

thread_local! {
    /// C++ kept the `WW3DAssetManager` as a plain global; the game thread is its
    /// only writer, so the port carries it in thread-local storage instead of a
    /// shared mutex.
    static GLOBAL_ASSET_MANAGER: RefCell<AssetManager> = RefCell::new(AssetManager::new());
}

/// Run `f` with mutable access to the global asset manager.
pub fn with_global_asset_manager<R>(f: impl FnOnce(&mut AssetManager) -> R) -> R {
    GLOBAL_ASSET_MANAGER.with(|mgr| f(&mut mgr.borrow_mut()))
}

/// Run `f` with shared access to the global asset manager.
pub fn with_global_asset_manager_ref<R>(f: impl FnOnce(&AssetManager) -> R) -> R {
    GLOBAL_ASSET_MANAGER.with(|mgr| f(&mgr.borrow()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::create_cube_mesh;
    use crate::render_object::RenderObject;
    use crate::texture::create_solid_color_texture;

    #[test]
    fn test_asset_handle() {
        let handle = AssetHandle::<Mesh>::new("test".to_string());
        assert_eq!(handle.name(), "test");
        assert_eq!(handle.status(), AssetStatus::Unloaded);
        assert!(!handle.is_loaded());
    }

    #[test]
    fn test_asset_manager_register_mesh() {
        let mut manager = AssetManager::new();

        let mesh = create_cube_mesh("cube".to_string(), 1.0);
        let handle = manager.register_mesh("cube".to_string(), mesh);

        assert!(handle.is_loaded());
        assert_eq!(handle.name(), "cube");

        let stats = manager.cache_stats();
        assert_eq!(stats.mesh_count, 1);
    }

    #[test]
    fn test_asset_manager_register_texture() {
        let mut manager = AssetManager::new();

        let texture = create_solid_color_texture("red".to_string(), [255, 0, 0, 255], 8);
        let handle = manager.register_texture("red".to_string(), texture);

        assert!(handle.is_loaded());
        assert_eq!(handle.name(), "red");

        let stats = manager.cache_stats();
        assert_eq!(stats.texture_count, 1);
    }

    #[test]
    fn test_cache_stats() {
        let mut manager = AssetManager::new();

        let mesh = create_cube_mesh("cube".to_string(), 1.0);
        manager.register_mesh("cube".to_string(), mesh);

        let texture = create_solid_color_texture("red".to_string(), [255, 0, 0, 255], 8);
        manager.register_texture("red".to_string(), texture);

        let stats = manager.cache_stats();
        assert_eq!(stats.total_count(), 2);
    }

    #[test]
    fn test_clear_caches() {
        let mut manager = AssetManager::new();

        let mesh = create_cube_mesh("cube".to_string(), 1.0);
        manager.register_mesh("cube".to_string(), mesh);

        let stats_before = manager.cache_stats();
        assert_eq!(stats_before.mesh_count, 1);

        manager.clear_all_caches();

        let stats_after = manager.cache_stats();
        assert_eq!(stats_after.mesh_count, 0);
    }

    #[test]
    fn test_global_asset_manager() {
        with_global_asset_manager(|manager| {
            let mesh = create_cube_mesh("global_cube".to_string(), 1.0);
            manager.register_mesh("global_cube".to_string(), mesh);
        });

        // Access again to verify it's the same instance
        let stats = with_global_asset_manager_ref(|manager| manager.cache_stats());

        assert!(stats.mesh_count >= 1);
    }

    #[test]
    fn test_asset_handle_with_closure() {
        let mesh = create_cube_mesh("test".to_string(), 1.0);
        let handle = AssetHandle::with_asset("test".to_string(), mesh);

        let poly_count = handle.with(|m| m.get_num_polys());
        assert!(poly_count.is_some());
        assert_eq!(poly_count.unwrap(), 12); // Cube has 12 triangles
    }
}
