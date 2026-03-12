//! Hot reload system for development tools

use crate::UIError;
use anyhow::Result;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Hot reload manager for automatically reloading changed assets
pub struct HotReloadManager {
    enabled: bool,
    watcher: Option<RecommendedWatcher>,
    receiver: Option<mpsc::Receiver<notify::Result<notify::Event>>>,
    watched_paths: Vec<PathBuf>,
    changed_files: Vec<PathBuf>,
    last_check: Instant,
    callbacks: HashMap<String, Box<dyn Fn(&Path) + Send + Sync>>,
    debounce_time: Duration,
}

impl HotReloadManager {
    pub fn new(enabled: bool) -> Result<Self> {
        let mut manager = Self {
            enabled,
            watcher: None,
            receiver: None,
            watched_paths: Vec::new(),
            changed_files: Vec::new(),
            last_check: Instant::now(),
            callbacks: HashMap::new(),
            debounce_time: Duration::from_millis(100),
        };

        if enabled {
            manager.initialize_watcher()?;
        }

        Ok(manager)
    }

    /// Add a path to watch for changes
    pub fn watch_path(&mut self, path: PathBuf) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        if let Some(ref mut watcher) = self.watcher {
            watcher
                .watch(&path, RecursiveMode::Recursive)
                .map_err(|e| UIError::HotReloadError(e.to_string()))?;

            self.watched_paths.push(path);
        }

        Ok(())
    }

    /// Remove a path from watching
    pub fn unwatch_path(&mut self, path: &Path) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        if let Some(ref mut watcher) = self.watcher {
            watcher
                .unwatch(path)
                .map_err(|e| UIError::HotReloadError(e.to_string()))?;

            self.watched_paths.retain(|p| p != path);
        }

        Ok(())
    }

    /// Register a callback for file changes
    pub fn register_callback<F>(&mut self, id: String, callback: F)
    where
        F: Fn(&Path) + Send + Sync + 'static,
    {
        self.callbacks.insert(id, Box::new(callback));
    }

    /// Remove a callback
    pub fn unregister_callback(&mut self, id: &str) {
        self.callbacks.remove(id);
    }

    /// Check for file changes and return true if any were found
    pub fn check_for_changes(&mut self) -> bool {
        if !self.enabled {
            return false;
        }

        let now = Instant::now();
        if now.duration_since(self.last_check) < self.debounce_time {
            return false;
        }

        self.last_check = now;

        // Process pending file system events
        // Collect events first to avoid borrow checker issues
        let mut events = Vec::new();
        if let Some(receiver) = self.receiver.as_ref() {
            while let Ok(event_result) = receiver.try_recv() {
                events.push(event_result);
            }
        }

        // Process collected events
        for event_result in events {
            match event_result {
                Ok(event) => {
                    self.process_file_event(event);
                }
                Err(e) => {
                    log::warn!("File watcher error: {}", e);
                }
            }
        }

        // Check if we have any changed files
        let has_changes = !self.changed_files.is_empty();

        if has_changes {
            // Process changed files
            let changed_files = std::mem::take(&mut self.changed_files);

            for file_path in changed_files {
                self.handle_file_change(&file_path);
            }
        }

        has_changes
    }

    /// Enable or disable hot reload
    pub fn set_enabled(&mut self, enabled: bool) -> Result<()> {
        if self.enabled == enabled {
            return Ok(());
        }

        self.enabled = enabled;

        if enabled {
            self.initialize_watcher()?;

            // Re-watch all previously watched paths
            let paths = self.watched_paths.clone();
            for path in paths {
                self.watch_path(path)?;
            }
        } else {
            self.shutdown_watcher();
        }

        Ok(())
    }

    /// Get list of watched paths
    pub fn watched_paths(&self) -> &[PathBuf] {
        &self.watched_paths
    }

    fn initialize_watcher(&mut self) -> Result<()> {
        let (tx, rx) = mpsc::channel();

        let watcher =
            notify::recommended_watcher(tx).map_err(|e| UIError::HotReloadError(e.to_string()))?;

        self.watcher = Some(watcher);
        self.receiver = Some(rx);

        log::info!("Hot reload system initialized");
        Ok(())
    }

    fn shutdown_watcher(&mut self) {
        self.watcher = None;
        self.receiver = None;
        log::info!("Hot reload system shut down");
    }

    fn process_file_event(&mut self, event: notify::Event) {
        use notify::EventKind;

        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                for path in event.paths {
                    // Filter out temporary files and directories
                    if self.should_process_file(&path) {
                        self.changed_files.push(path);
                    }
                }
            }
            _ => {}
        }
    }

    fn should_process_file(&self, path: &Path) -> bool {
        // Skip temporary files
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') || name.ends_with('~') || name.ends_with(".tmp") {
                return false;
            }
        }

        // Skip directories
        if path.is_dir() {
            return false;
        }

        true
    }

    fn handle_file_change(&self, path: &Path) {
        log::info!("File changed: {}", path.display());

        // Call all registered callbacks
        for callback in self.callbacks.values() {
            callback(path);
        }
    }
}

impl Drop for HotReloadManager {
    fn drop(&mut self) {
        self.shutdown_watcher();
    }
}

/// Hot reload configuration
#[derive(Debug, Clone)]
pub struct HotReloadConfig {
    pub enabled: bool,
    pub debounce_ms: u64,
    pub watch_extensions: Vec<String>,
    pub ignore_patterns: Vec<String>,
}

impl Default for HotReloadConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            debounce_ms: 100,
            watch_extensions: vec![
                "rs".to_string(),
                "toml".to_string(),
                "png".to_string(),
                "jpg".to_string(),
                "w3d".to_string(),
                "lua".to_string(),
            ],
            ignore_patterns: vec![
                "target".to_string(),
                ".git".to_string(),
                "node_modules".to_string(),
            ],
        }
    }
}

/// Asset hot reload handler
pub struct AssetHotReload {
    manager: Arc<RwLock<HotReloadManager>>,
    asset_cache: HashMap<PathBuf, AssetCacheEntry>,
}

impl AssetHotReload {
    pub fn new(manager: Arc<RwLock<HotReloadManager>>) -> Self {
        Self {
            manager,
            asset_cache: HashMap::new(),
        }
    }

    /// Register an asset for hot reloading
    pub fn register_asset(&mut self, path: PathBuf) -> Result<()> {
        // try_write() returns Option, not Result
        if let Some(mut manager) = self.manager.try_write() {
            manager.watch_path(path.clone())?;
        }

        let entry = AssetCacheEntry {
            path: path.clone(),
            last_modified: std::fs::metadata(&path)
                .ok()
                .and_then(|m| m.modified().ok()),
            reload_count: 0,
        };

        self.asset_cache.insert(path, entry);
        Ok(())
    }

    /// Check if an asset needs reloading
    pub async fn needs_reload(&mut self, path: &Path) -> bool {
        if let Some(entry) = self.asset_cache.get_mut(path) {
            if let Ok(metadata) = tokio::fs::metadata(path).await {
                if let Ok(modified) = metadata.modified() {
                    if entry.last_modified.map_or(true, |last| modified > last) {
                        entry.last_modified = Some(modified);
                        entry.reload_count += 1;
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Get reload statistics for an asset
    pub fn get_reload_stats(&self, path: &Path) -> Option<&AssetCacheEntry> {
        self.asset_cache.get(path)
    }
}

/// Cache entry for hot reloaded assets
#[derive(Debug, Clone)]
pub struct AssetCacheEntry {
    pub path: PathBuf,
    pub last_modified: Option<std::time::SystemTime>,
    pub reload_count: u32,
}

/// Shader hot reload system
pub struct ShaderHotReload {
    shader_paths: HashMap<String, PathBuf>,
    compiled_shaders: HashMap<String, CompiledShader>,
}

impl ShaderHotReload {
    pub fn new() -> Self {
        Self {
            shader_paths: HashMap::new(),
            compiled_shaders: HashMap::new(),
        }
    }

    /// Register a shader for hot reloading
    pub fn register_shader(&mut self, id: String, path: PathBuf) {
        self.shader_paths.insert(id, path);
    }

    /// Reload a shader if it has changed
    pub fn reload_shader(&mut self, id: &str) -> Result<bool> {
        if let Some(path) = self.shader_paths.get(id) {
            // Check if file has been modified
            let needs_reload = if let Some(shader) = self.compiled_shaders.get(id) {
                if let Ok(metadata) = std::fs::metadata(path) {
                    if let Ok(modified) = metadata.modified() {
                        modified > shader.compiled_time
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                true // First time loading
            };

            if needs_reload {
                // Compile the shader
                let source = std::fs::read_to_string(path).map_err(|e| {
                    UIError::HotReloadError(format!("Failed to read shader: {}", e))
                })?;

                let compiled = CompiledShader {
                    source,
                    compiled_time: std::time::SystemTime::now(),
                    compile_errors: Vec::new(),
                };

                self.compiled_shaders.insert(id.to_string(), compiled);
                log::info!("Reloaded shader: {}", id);
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Get compiled shader source
    pub fn get_shader_source(&self, id: &str) -> Option<&str> {
        self.compiled_shaders.get(id).map(|s| s.source.as_str())
    }
}

/// Compiled shader data
#[derive(Debug, Clone)]
struct CompiledShader {
    source: String,
    compiled_time: std::time::SystemTime,
    compile_errors: Vec<String>,
}
