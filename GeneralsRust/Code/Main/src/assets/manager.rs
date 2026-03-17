////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// Asset manager - coordinates all asset loading systems

use crate::assets::{
    archive::{ArchiveFileSystem, ArchiveStatistics},
    audio::AudioManager,
    models::{get_common_cnc_units, W3DLoader, W3DModel},
    textures::{GPUTexture, RawTexture, TextureManager},
    ww3d_asset_manager::WW3DAssetManager,
};
use crate::localization;
use crate::subsystem_manager::{with_subsystem, GlobalDataSubsystem};
use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

/// Complete asset management system for C&C Generals
pub struct AssetManager {
    /// Archive file system for reading BIG files
    archive_system: ArchiveFileSystem,
    /// Audio manager for music and sound effects
    audio_manager: AudioManager,
    /// W3D model loader
    model_loader: W3DLoader,
    /// Texture manager
    texture_manager: TextureManager,
    /// WW3D Asset Manager for object definitions and texture lookup
    ww3d_manager: WW3DAssetManager,
    /// Cache of loaded models
    model_cache: HashMap<String, W3DModel>,
    /// Known-missing model keys to keep repeated lookups O(1) like C++ hash misses.
    missing_model_keys: HashSet<String>,
    /// Initialization status
    initialized: bool,
    /// Active localization language
    language: String,
    /// Active mod root (if any)
    active_mod_path: Option<PathBuf>,
    /// Explicit BIG files to mount after core init
    manual_big_files: Vec<PathBuf>,
}

impl AssetManager {
    fn should_resolve_object_texture_name(name: &str) -> bool {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return false;
        }

        let has_path = trimmed.contains('/') || trimmed.contains('\\');
        let has_extension = Path::new(trimmed).extension().is_some();
        !has_path && !has_extension
    }

    fn canonical_model_name(model_name: &str) -> String {
        model_name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(model_name)
            .trim()
            .trim_end_matches(".w3d")
            .trim_end_matches(".W3D")
            .to_string()
    }

    fn resolve_available_model_name(&self, model_name: &str) -> String {
        // C++ parity: request the model name as-authored instead of fuzzy suffix/alias remaps.
        Self::canonical_model_name(model_name)
    }

    /// Create new asset manager
    pub fn new() -> Result<Self> {
        debug!("Creating AssetManager");

        let (language, active_mod_path) = Self::runtime_overrides();

        Ok(Self {
            archive_system: ArchiveFileSystem::new(),
            audio_manager: AudioManager::new()?,
            model_loader: W3DLoader::new(),
            texture_manager: TextureManager::new(),
            ww3d_manager: WW3DAssetManager::new(),
            model_cache: HashMap::new(),
            missing_model_keys: HashSet::new(),
            initialized: false,
            language,
            active_mod_path,
            manual_big_files: Vec::new(),
        })
    }

    /// Initialize the asset manager
    pub async fn init(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<()> {
        debug!("Initializing AssetManager");

        // Add asset search paths before initializing
        // Try to find the assets directory relative to the executable
        let mut asset_paths = vec![
            PathBuf::from("assets"),
            PathBuf::from("Code/Main/assets"),
            PathBuf::from("./Code/Main/assets"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"),
        ];

        // Also try relative to current exe directory
        if let Ok(exe_path) = env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                asset_paths.push(exe_dir.join("assets"));
                asset_paths.push(exe_dir.join("../Code/Main/assets"));
                asset_paths.push(exe_dir.join("Data"));
            }
        }

        if let Some(mod_path) = &self.active_mod_path {
            asset_paths.insert(0, mod_path.clone());
            asset_paths.insert(1, mod_path.join("Data"));
        }

        for language_path in self.language_specific_paths() {
            asset_paths.insert(0, language_path);
        }

        self.register_search_paths(asset_paths);

        // Initialize archive system (loads BIG files)
        self.archive_system
            .init()
            .await
            .map_err(|e| anyhow!("Failed to initialize archive system: {}", e))?;

        self.load_manual_archives().await?;

        // Initialize texture manager with MAGENTA fallback for missing textures
        self.texture_manager
            .init(device, queue)
            .map_err(|e| anyhow!("Failed to initialize texture manager: {}", e))?;
        self.warmup_texture_lookup_metadata();

        // Keep startup/menu responsive: defer non-critical animated water caustic uploads.
        // These frames are optional polish and are not required for shell/menu initialization.
        info!("Deferring caustic water texture warmup until post-startup gameplay path");

        // Initialize WW3D Asset Manager - Load object definitions from INIZH.big
        // This matches C++ WW3DAssetManager initialization
        info!("🎮 Initializing WW3D Asset Manager for object definitions and texture lookup");
        let init_start = SystemTime::now();
        self.ww3d_manager
            .initialize(&mut self.archive_system)
            .await
            .map_err(|e| anyhow!("Failed to initialize WW3D asset manager: {}", e))?;
        let init_elapsed = init_start.elapsed().unwrap_or_default();
        info!(
            "✅ WW3D Asset Manager initialized in {:.2}s with {} object definitions",
            init_elapsed.as_secs_f64(),
            self.ww3d_manager.object_count()
        );

        self.initialized = true;

        // Print statistics
        let stats = self.get_statistics();
        info!("AssetManager initialized successfully!");
        info!("  Archives: {}", stats.archive_stats.total_archives);
        info!("  Total files: {}", stats.archive_stats.total_files);
        info!("  Unique files: {}", stats.archive_stats.unique_files);
        info!("  Textures cached: {}", stats.textures_cached);
        info!("  Models cached: {}", stats.models_cached);

        Ok(())
    }

    fn warmup_texture_lookup_metadata(&mut self) {
        let indexed_paths = self
            .texture_manager
            .warmup_texture_path_index(&self.archive_system);
        debug!(
            "Texture lookup metadata warmup indexed {} canonical texture paths",
            indexed_paths
        );
    }

    fn runtime_overrides() -> (String, Option<PathBuf>) {
        let mut language = "English".to_string();
        let mut mod_path = None;

        if let Some(result) = with_subsystem::<GlobalDataSubsystem, _>(|subsystem| {
            subsystem.get_global_data().map(|global| {
                (
                    global.language().to_string(),
                    global.active_mod().map(|m| m.to_string()),
                )
            })
        }) {
            if let Some((lang, mod_string)) = result {
                if !lang.trim().is_empty() {
                    language = lang;
                }
                if let Some(mod_str) = mod_string {
                    let candidate = PathBuf::from(mod_str);
                    mod_path = std::fs::canonicalize(&candidate).ok().or(Some(candidate));
                }
            }
        }

        (language, mod_path)
    }

    fn language_specific_paths(&self) -> Vec<PathBuf> {
        let lang = self.language.trim();
        if lang.is_empty() {
            return Vec::new();
        }

        let mut candidates = Vec::new();
        let lang_normalized = lang.replace('\\', "/");

        candidates.push(PathBuf::from("Data").join(&lang_normalized));
        candidates.push(PathBuf::from("Data").join(lang_normalized.to_lowercase()));

        if let Ok(cwd) = env::current_dir() {
            candidates.push(cwd.join("Data").join(&lang_normalized));
            candidates.push(cwd.join("Data").join(lang_normalized.to_lowercase()));
        }

        if let Some(mod_path) = &self.active_mod_path {
            candidates.push(mod_path.join("Data").join(&lang_normalized));
        }

        candidates
    }

    fn register_search_paths(&mut self, paths: Vec<PathBuf>) {
        let mut seen = HashSet::new();
        let mut localization_dirs = Vec::new();
        for path in paths {
            let key = path.to_string_lossy().to_string();
            if !seen.insert(key.clone()) {
                continue;
            }
            if path.is_file() {
                self.add_manual_big_file(&path);
            } else {
                self.add_search_path_if_exists(&path);
                localization_dirs.extend(Self::discover_localization_dirs(&path));
            }
        }

        if localization_dirs.is_empty() {
            localization_dirs.push(PathBuf::from("Data/Localization"));
            localization_dirs.push(PathBuf::from("Localization"));
        }
        localization::set_search_paths(&localization_dirs);
    }

    fn add_search_path_if_exists<P: AsRef<Path>>(&mut self, path: P) {
        let path = path.as_ref();
        if path.exists() {
            debug!("📂 Adding asset search path: {}", path.display());
            self.archive_system.add_search_path(path);
        } else {
            debug!("Skipping missing asset path: {}", path.display());
        }
    }

    fn add_manual_big_file<P: AsRef<Path>>(&mut self, path: P) {
        let path = path.as_ref();
        let ext_is_big = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map_or(false, |ext| ext.eq_ignore_ascii_case("big"));

        if !ext_is_big {
            warn!(
                "Ignoring manual archive '{}': unsupported extension",
                path.display()
            );
            return;
        }

        if path.exists() {
            debug!("🗃️ Queuing BIG file for manual load: {}", path.display());
            self.manual_big_files.push(path.to_path_buf());
        } else {
            warn!("Manual BIG file not found, skipping: {}", path.display());
        }
    }

    async fn load_manual_archives(&mut self) -> Result<()> {
        for big in std::mem::take(&mut self.manual_big_files) {
            debug!("🗃️ Loading BIG archive {}", big.display());
            self.archive_system
                .load_big_file(&big)
                .await
                .map_err(|e| anyhow!("Failed to load {}: {}", big.display(), e))?;
        }

        // Auto-mount core archives if present (parity with C++ loader).
        let candidates = [
            "INIZH.big",
            "W3DZH.big",
            "TexturesZH.big",
            "TerrainZH.big",
            "WindowZH.big",
            "AudioZH.big",
            "EnglishZH.big",
            "INI.big",
            "W3D.big",
            "Textures.big",
            "Terrain.big",
            "Window.big",
        ];

        for name in candidates {
            if let Some(path) = self.archive_system.find_archive(name) {
                debug!("🔍 Mounting core archive: {}", path.display());
                if let Err(e) = self.archive_system.load_big_file(&path).await {
                    warn!("Failed to mount {}: {}", path.display(), e);
                }
            }
        }
        Ok(())
    }

    fn discover_localization_dirs(base: &Path) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        let primary = base.join("Localization");
        if primary.exists() && primary.is_dir() {
            dirs.push(primary);
        }

        let data_loc = base.join("Data").join("Localization");
        if data_loc.exists() && data_loc.is_dir() {
            dirs.push(data_loc);
        }

        dirs
    }

    /// Start playing random C&C background music
    pub async fn start_background_music(&mut self) -> Result<()> {
        if !self.initialized {
            return Err(anyhow!("AssetManager not initialized"));
        }

        info!("Starting C&C background music");
        self.audio_manager
            .play_random_cnc_music(&mut self.archive_system)
            .await
    }

    /// Load C&C model (with caching)
    pub async fn load_cnc_model(&mut self, unit_name: &str) -> Result<&W3DModel> {
        let unit_key = unit_name.to_lowercase();

        // Return cached model if available
        if self.model_cache.contains_key(&unit_key) {
            return Ok(self.model_cache.get(&unit_key).unwrap());
        }

        info!("Loading C&C model: {}", unit_name);

        // Load model using W3D loader
        let model = self
            .model_loader
            .load_cnc_model(&mut self.archive_system, unit_name)
            .await
            .map_err(|e| anyhow!("Failed to load model {}: {}", unit_name, e))?;

        self.model_cache.insert(unit_key.clone(), model);
        Ok(self.model_cache.get(&unit_key).unwrap())
    }

    /// Load texture from BIG archives
    pub async fn load_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_name: &str,
    ) -> &GPUTexture {
        if !self.initialized {
            error!("AssetManager not initialized, returning default texture");
            return self.texture_manager.get_default_texture();
        }

        let lookup_name = if Self::should_resolve_object_texture_name(texture_name) {
            self.ww3d_manager
                .get_texture_for_object(texture_name)
                .unwrap_or_else(|| texture_name.to_string())
        } else {
            texture_name.to_string()
        };

        self.texture_manager
            .get_texture_or_default(&mut self.archive_system, device, queue, &lookup_name)
            .await
    }

    /// Load texture synchronously - blocks until loaded, returns texture name for cache lookup
    pub fn load_texture_blocking(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_name: &str,
    ) -> String {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let _ = self.load_texture(device, queue, texture_name).await;
                texture_name.to_string()
            })
        })
    }

    /// Prime only raw texture data synchronously (no GPU texture upload).
    pub fn prime_texture_raw_blocking(&mut self, texture_name: &str) -> String {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let _ = self
                    .texture_manager
                    .prime_raw_texture(&mut self.archive_system, texture_name)
                    .await;
                texture_name.to_string()
            })
        })
    }

    /// Prime a batch of raw texture payloads synchronously (no GPU upload).
    pub fn prime_textures_raw_blocking<I, S>(&mut self, texture_names: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let unique: Vec<String> = {
            let mut seen = HashSet::new();
            texture_names
                .into_iter()
                .filter_map(|name| {
                    let trimmed = name.as_ref().trim();
                    if trimmed.is_empty() {
                        return None;
                    }
                    let key = trimmed.to_ascii_lowercase();
                    if seen.insert(key) {
                        Some(trimmed.to_string())
                    } else {
                        None
                    }
                })
                .collect()
        };

        if unique.is_empty() {
            return;
        }

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                for name in unique {
                    let _ = self
                        .texture_manager
                        .prime_raw_texture(&mut self.archive_system, &name)
                        .await;
                }
            })
        });
    }

    /// Get default texture
    pub fn get_default_texture(&self) -> &GPUTexture {
        self.texture_manager.get_default_texture()
    }

    /// Get raw texture data if it's cached.
    pub fn get_raw_texture(&self, texture_name: &str) -> Option<&RawTexture> {
        self.texture_manager.get_raw_texture(texture_name)
    }

    pub fn is_known_missing_texture(&self, texture_name: &str) -> bool {
        self.texture_manager.is_known_missing_texture(texture_name)
    }

    /// Get colored default texture (for indicating different states)
    pub fn get_colored_default_texture(&self, color_name: &str) -> &GPUTexture {
        self.texture_manager.get_colored_default_texture(color_name)
    }

    /// Get texture for an object from WW3D Asset Manager
    /// Returns the texture filename defined for the object in INI files
    pub fn get_texture_for_object(&self, object_name: &str) -> Option<String> {
        self.ww3d_manager.get_texture_for_object(object_name)
    }

    /// Get model for an object from WW3D Asset Manager
    /// Returns the model filename defined for the object in INI files
    pub fn get_model_for_object(&self, object_name: &str) -> Option<String> {
        self.ww3d_manager.get_model_for_object(object_name)
    }

    /// Get full object definition from WW3D Asset Manager
    pub fn get_object_definition(
        &self,
        object_name: &str,
    ) -> Option<&crate::assets::ObjectDefinition> {
        self.ww3d_manager.get_object_definition(object_name)
    }

    /// Resolve object definition using name with optional model hint fallback
    pub fn resolve_object_definition(
        &self,
        object_name: &str,
        model_hint: Option<&str>,
    ) -> Option<&crate::assets::ObjectDefinition> {
        self.ww3d_manager
            .resolve_object_definition(object_name, model_hint)
    }

    /// Check if an object is defined in the WW3D Asset Manager
    pub fn has_object_definition(&self, object_name: &str) -> bool {
        self.ww3d_manager.has_object(object_name)
    }

    /// Get total count of loaded object definitions
    pub fn get_object_definition_count(&self) -> usize {
        self.ww3d_manager.object_count()
    }

    /// Get all texture filenames from WW3D Asset Manager for preloading
    pub fn get_all_texture_filenames(&self) -> Vec<String> {
        self.ww3d_manager.get_all_texture_filenames()
    }

    /// Load a texture (returns a reference to the GPU texture)
    /// This is the actual async method that loads from archives
    pub async fn load_texture_async(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture_name: &str,
    ) -> Result<&GPUTexture> {
        if !self.initialized {
            return Err(anyhow!("AssetManager not initialized"));
        }

        // This calls the low-level texture manager load_texture with archive system access
        self.texture_manager
            .load_texture(&mut self.archive_system, device, queue, texture_name)
            .await
    }

    /// Play sound effect from archives
    pub async fn play_sound_effect(&mut self, sound_name: &str) -> Result<()> {
        if !self.initialized {
            return Err(anyhow!("AssetManager not initialized"));
        }

        self.audio_manager
            .play_sound_effect(&mut self.archive_system, sound_name)
            .await
    }

    /// Toggle background music
    pub fn toggle_background_music(&self) {
        self.audio_manager.toggle_background_music();
    }

    /// Set music volume
    pub fn set_music_volume(&mut self, volume: f32) {
        self.audio_manager.set_music_volume(volume);
    }

    /// Set sound effects volume
    pub fn set_sfx_volume(&mut self, volume: f32) {
        self.audio_manager.set_sfx_volume(volume);
    }

    /// Check if file exists in archives
    pub fn does_file_exist(&self, filename: &str) -> bool {
        if !self.initialized {
            return false;
        }
        self.archive_system.does_file_exist(filename)
    }

    /// Check if a virtual archive path can be opened with the active mount set.
    pub fn can_open_file_sync(&mut self, filename: &str) -> bool {
        if !self.initialized {
            return false;
        }
        self.archive_system.open_reader(filename).is_ok()
    }

    /// Extract raw file data from archives
    pub async fn extract_file(&mut self, filename: &str) -> Result<Vec<u8>> {
        if !self.initialized {
            return Err(anyhow!("AssetManager not initialized"));
        }
        self.archive_system.open_file(filename).await
    }

    /// List all available files in archives
    pub fn list_all_files(&self) -> Vec<String> {
        if !self.initialized {
            return Vec::new();
        }
        self.archive_system.list_all_files()
    }

    /// List available models
    pub fn list_available_models(&self) -> Vec<String> {
        if !self.initialized {
            return Vec::new();
        }
        self.model_loader
            .list_available_models(&self.archive_system)
    }

    /// List available textures
    pub fn list_available_textures(&self) -> Vec<String> {
        if !self.initialized {
            return Vec::new();
        }
        self.texture_manager
            .list_available_textures(&self.archive_system)
    }

    /// Get loaded archives
    pub fn get_loaded_archives(&self) -> Vec<String> {
        if !self.initialized {
            return Vec::new();
        }
        self.archive_system.get_loaded_archives()
    }

    /// Get common C&C unit names
    pub fn get_common_cnc_units(&self) -> Vec<&'static str> {
        get_common_cnc_units()
    }

    /// Load a specific C&C unit model by name
    pub async fn load_unit_model(&mut self, unit_name: &str) -> Result<&W3DModel> {
        self.load_cnc_model(unit_name).await
    }

    /// Get a cached model by name (synchronous)
    pub fn get_cached_model(&self, unit_name: &str) -> Option<W3DModel> {
        let unit_key = unit_name.to_lowercase();
        self.model_cache.get(&unit_key).cloned()
    }

    /// Load a model asynchronously by cloning from cache or loading fresh
    pub async fn load_w3d_model_async(&mut self, model_name: &str) -> Result<W3DModel> {
        let resolved_name = self.resolve_available_model_name(model_name);
        let model_key = model_name.to_lowercase();
        let resolved_key = resolved_name.to_lowercase();

        // Check cache first
        if let Some(model) = self.model_cache.get(&model_key) {
            return Ok(model.clone());
        }
        if resolved_key != model_key {
            if let Some(model) = self.model_cache.get(&resolved_key).cloned() {
                self.model_cache.insert(model_key.clone(), model.clone());
                return Ok(model);
            }
        }

        if self.missing_model_keys.contains(&model_key)
            || self.missing_model_keys.contains(&resolved_key)
        {
            return Err(anyhow!(
                "W3D load skipped for known-missing model '{}'",
                resolved_name
            ));
        }

        info!("Loading W3D model: {}", resolved_name);

        // Use the actual W3D loader to parse the model.
        let w3d_loader = W3DLoader::new();
        let model = match w3d_loader
            .load_model(&mut self.archive_system, &resolved_name)
            .await
        {
            Ok(model) => model,
            Err(e) => {
                self.missing_model_keys.insert(model_key.clone());
                self.missing_model_keys.insert(resolved_key.clone());
                warn!("W3D loader failed for '{}': {}", model_name, e);
                return Err(anyhow!("W3D load failed for '{}': {e}", model_name));
            }
        };

        info!(
            "✅ Successfully loaded W3D model '{}' with {} meshes, {} total vertices",
            resolved_name,
            model.meshes.len(),
            model.meshes.iter().map(|m| m.vertices.len()).sum::<usize>()
        );

        // Cache the model
        self.model_cache.insert(resolved_key, model.clone());
        if resolved_name != model_name {
            self.model_cache.insert(model_key, model.clone());
        }
        self.missing_model_keys.remove(&model_name.to_lowercase());
        self.missing_model_keys
            .remove(&resolved_name.to_lowercase());
        Ok(model)
    }

    pub fn load_w3d_model_blocking(&mut self, model_name: &str) -> Result<W3DModel> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.load_w3d_model_async(model_name))
        })
    }

    /// Load a model synchronously by cloning from cache or loading through the W3D parser path.
    pub fn load_w3d_model(&mut self, model_name: &str) -> Result<W3DModel> {
        let resolved_name = self.resolve_available_model_name(model_name);
        let model_key = model_name.to_lowercase();
        let resolved_key = resolved_name.to_lowercase();

        // Check cache first
        if let Some(model) = self.model_cache.get(&model_key) {
            return Ok(model.clone());
        }
        if resolved_key != model_key {
            if let Some(model) = self.model_cache.get(&resolved_key).cloned() {
                self.model_cache.insert(model_key.clone(), model.clone());
                return Ok(model);
            }
        }

        if self.missing_model_keys.contains(&model_key)
            || self.missing_model_keys.contains(&resolved_key)
        {
            return Err(anyhow!(
                "Synchronous W3D load skipped for known-missing model '{}'",
                resolved_name
            ));
        }

        let model = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread {
                tokio::task::block_in_place(|| {
                    handle.block_on(
                        self.model_loader
                            .load_model(&mut self.archive_system, &resolved_name),
                    )
                })
            } else {
                Err(anyhow!(
                    "Synchronous W3D loading not supported on current-thread runtime"
                ))
            }
        } else {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(
                self.model_loader
                    .load_model(&mut self.archive_system, &resolved_name),
            )
        };

        let model = match model {
            Ok(model) => model,
            Err(err) => {
                self.missing_model_keys.insert(model_key.clone());
                self.missing_model_keys.insert(resolved_key.clone());
                warn!(
                    "Synchronous W3D load failed for '{}': {}",
                    resolved_name, err
                );
                return Err(anyhow!(
                    "Synchronous W3D load failed for '{}': {}",
                    resolved_name,
                    err
                ));
            }
        };

        // Cache the model
        self.model_cache.insert(resolved_key, model.clone());
        if resolved_name != model_name {
            self.model_cache.insert(model_key, model.clone());
        }
        self.missing_model_keys.remove(&model_name.to_lowercase());
        self.missing_model_keys
            .remove(&resolved_name.to_lowercase());
        Ok(model)
    }

    /// Play faction-specific music
    pub async fn play_faction_music(&mut self, faction: &str) -> Result<()> {
        if !self.initialized {
            return Err(anyhow!("AssetManager not initialized"));
        }
        self.audio_manager
            .play_faction_music(&mut self.archive_system, faction)
            .await
    }

    /// Update asset manager (cleanup, etc.)
    pub fn update(&mut self) {
        if self.initialized {
            self.audio_manager.update();
        }
    }

    /// Get asset statistics
    pub fn get_statistics(&self) -> AssetStatistics {
        let archive_stats = if self.initialized {
            self.archive_system.get_statistics()
        } else {
            ArchiveStatistics {
                total_archives: 0,
                total_files: 0,
                unique_files: 0,
            }
        };

        let (textures_raw, textures_gpu) = if self.initialized {
            self.texture_manager.get_cache_stats()
        } else {
            (0, 0)
        };

        AssetStatistics {
            archive_stats,
            models_cached: self.model_cache.len(),
            textures_cached: textures_gpu,
            textures_raw_cached: textures_raw,
            initialized: self.initialized,
        }
    }

    /// Clear caches to free memory
    pub fn clear_caches(&mut self) {
        info!("Clearing asset caches");
        self.model_cache.clear();
        self.missing_model_keys.clear();
        self.texture_manager.clear_cache();
    }

    /// Check if a texture is already loaded in cache
    pub fn get_cached_texture(&self, texture_name: &str) -> Option<&GPUTexture> {
        if !self.initialized {
            return None;
        }
        self.texture_manager.get_cached_texture(texture_name)
    }

    /// Search for specific assets
    pub fn search_assets(&self, pattern: &str) -> AssetSearchResults {
        if !self.initialized {
            return AssetSearchResults::default();
        }

        let pattern_lower = pattern.to_lowercase();
        let all_files = self.archive_system.list_all_files();

        let mut models = Vec::new();
        let mut textures = Vec::new();
        let mut audio = Vec::new();
        let mut other = Vec::new();

        for file in all_files {
            let file_lower = file.to_lowercase();
            if !file_lower.contains(&pattern_lower) {
                continue;
            }

            if file_lower.ends_with(".w3d") {
                models.push(file);
            } else if file_lower.ends_with(".tga")
                || file_lower.ends_with(".dds")
                || file_lower.ends_with(".bmp")
                || file_lower.ends_with(".jpg")
                || file_lower.ends_with(".png")
            {
                textures.push(file);
            } else if file_lower.ends_with(".mp3")
                || file_lower.ends_with(".ogg")
                || file_lower.ends_with(".wav")
            {
                audio.push(file);
            } else {
                other.push(file);
            }
        }

        AssetSearchResults {
            models,
            textures,
            audio,
            other,
        }
    }

    /// Is the asset manager initialized?
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

/// Asset manager statistics
#[derive(Debug)]
pub struct AssetStatistics {
    pub archive_stats: ArchiveStatistics,
    pub models_cached: usize,
    pub textures_cached: usize,
    pub textures_raw_cached: usize,
    pub initialized: bool,
}

/// Asset search results
#[derive(Debug, Default)]
pub struct AssetSearchResults {
    pub models: Vec<String>,
    pub textures: Vec<String>,
    pub audio: Vec<String>,
    pub other: Vec<String>,
}

impl AssetSearchResults {
    pub fn total_results(&self) -> usize {
        self.models.len() + self.textures.len() + self.audio.len() + self.other.len()
    }
}

/// Global asset manager instance
static ASSET_MANAGER: OnceLock<Arc<Mutex<AssetManager>>> = OnceLock::new();
static CAUSTIC_WARMUP_STARTED: AtomicBool = AtomicBool::new(false);
static TEXTURE_PRIME_QUEUE: OnceLock<Sender<String>> = OnceLock::new();

/// Initialize the global asset manager
pub async fn init_asset_manager(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<()> {
    let manager_create_start = SystemTime::now();
    info!("📂 Creating asset manager and loading BIG archives...");

    let mut manager = AssetManager::new().expect("Failed to create asset manager");
    let manager_create_duration = manager_create_start.elapsed().unwrap_or_default();
    info!(
        "📂 BIG archives loaded in {:.2}s",
        manager_create_duration.as_secs_f32()
    );

    let wgpu_init_start = SystemTime::now();
    info!("🖥️ Initializing WGPU asset resources...");
    manager
        .init(device, queue)
        .await
        .expect("Failed to initialize asset manager");
    let wgpu_init_duration = wgpu_init_start.elapsed().unwrap_or_default();
    info!(
        "🖥️ WGPU asset resources initialized in {:.2}s",
        wgpu_init_duration.as_secs_f32()
    );

    ASSET_MANAGER
        .set(Arc::new(Mutex::new(manager)))
        .map_err(|_| anyhow!("Asset manager already initialized"))?;

    begin_background_music_startup();

    info!(
        "Global asset manager initialized (Total: {:.2}s)",
        (manager_create_duration + wgpu_init_duration).as_secs_f32()
    );
    Ok(())
}

fn begin_background_music_startup() {
    let Some(manager_arc) = get_asset_manager() else {
        return;
    };

    tokio::task::spawn_blocking(move || {
        let handle = tokio::runtime::Handle::current();
        let mut manager = manager_arc.lock().expect("asset manager mutex poisoned");
        if let Err(err) = handle.block_on(async { manager.start_background_music().await }) {
            warn!("Failed to start background music: {}", err);
        }
    });
}

/// Get reference to global asset manager
pub fn get_asset_manager() -> Option<Arc<Mutex<AssetManager>>> {
    ASSET_MANAGER.get().cloned()
}

/// Warm up optional caustic animation textures outside startup critical path.
pub fn warmup_caustic_textures_async(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> bool {
    if CAUSTIC_WARMUP_STARTED.swap(true, Ordering::AcqRel) {
        return false;
    }

    let Some(manager_arc) = get_asset_manager() else {
        CAUSTIC_WARMUP_STARTED.store(false, Ordering::Release);
        return false;
    };

    let handle = tokio::runtime::Handle::current();

    tokio::task::spawn_blocking(move || {
        let result = {
            let mut manager = manager_arc.lock().expect("asset manager mutex poisoned");
            let (texture_manager, archive_system) = {
                let manager_ref: &mut AssetManager = &mut manager;
                (
                    &mut manager_ref.texture_manager,
                    &mut manager_ref.archive_system,
                )
            };
            handle.block_on(async {
                texture_manager
                    .load_caustic_textures(archive_system, device.as_ref(), queue.as_ref())
                    .await
            })
        };

        match result {
            Ok(caustic_names) => {
                info!(
                    "Deferred caustic texture warmup complete: {} frames",
                    caustic_names.len()
                );
            }
            Err(err) => {
                warn!("Deferred caustic texture warmup failed: {}", err);
                CAUSTIC_WARMUP_STARTED.store(false, Ordering::Release);
            }
        }
    });

    true
}

/// Queue a background task to prime raw texture data without blocking the caller.
pub fn queue_prime_texture_raw(texture_name: &str) -> bool {
    let Some(manager_arc) = get_asset_manager() else {
        return false;
    };
    let name = texture_name.trim();
    if name.is_empty() || name.eq_ignore_ascii_case("none") {
        return false;
    }

    texture_prime_sender(manager_arc)
        .send(name.to_string())
        .is_ok()
}

fn texture_prime_sender(manager_arc: Arc<Mutex<AssetManager>>) -> &'static Sender<String> {
    TEXTURE_PRIME_QUEUE.get_or_init(move || {
        let (tx, rx) = mpsc::channel::<String>();
        std::thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(err) => {
                    error!("Failed to initialize texture prime runtime: {}", err);
                    return;
                }
            };

            while let Ok(name) = rx.recv() {
                let Ok(mut manager) = manager_arc.lock() else {
                    continue;
                };
                let (texture_manager, archive_system) = {
                    let manager_ref: &mut AssetManager = &mut manager;
                    (
                        &mut manager_ref.texture_manager,
                        &mut manager_ref.archive_system,
                    )
                };

                let _ = runtime.block_on(async {
                    texture_manager
                        .prime_raw_texture(archive_system, &name)
                        .await
                });
            }
        });
        tx
    })
}

/// Convenience functions for common operations
pub async fn load_cnc_unit_model(unit_name: &str) -> Result<()> {
    let manager_arc =
        get_asset_manager().ok_or_else(|| anyhow!("Asset manager not initialized"))?;
    let handle = tokio::runtime::Handle::current();
    let unit_name = unit_name.to_string();
    let unit_name_for_task = unit_name.clone();

    tokio::task::spawn_blocking(move || -> Result<()> {
        let mut manager = manager_arc.lock().expect("asset manager mutex poisoned");
        handle.block_on(async { manager.load_cnc_model(&unit_name_for_task).await })?;
        Ok(())
    })
    .await
    .map_err(|e| anyhow!("model preload task join failed: {e}"))??;

    info!("Loaded C&C unit model: {}", unit_name);
    Ok(())
}

pub async fn play_cnc_sound_effect(sound_name: &str) -> Result<()> {
    let manager_arc =
        get_asset_manager().ok_or_else(|| anyhow!("Asset manager not initialized"))?;
    let handle = tokio::runtime::Handle::current();
    let sound_name = sound_name.to_string();

    tokio::task::spawn_blocking(move || -> Result<()> {
        let mut manager = manager_arc.lock().expect("asset manager mutex poisoned");
        handle.block_on(async { manager.play_sound_effect(&sound_name).await })?;
        Ok(())
    })
    .await
    .map_err(|e| anyhow!("sound task join failed: {e}"))?
}

pub fn toggle_cnc_music() {
    if let Some(manager_arc) = get_asset_manager() {
        // We need to spawn a task for the async lock
        tokio::spawn(async move {
            let manager = manager_arc.lock().unwrap();
            manager.toggle_background_music();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_statistics() {
        let stats = AssetStatistics {
            archive_stats: ArchiveStatistics {
                total_archives: 10,
                total_files: 5000,
                unique_files: 4500,
            },
            models_cached: 25,
            textures_cached: 100,
            textures_raw_cached: 150,
            initialized: true,
        };

        assert!(stats.initialized);
        assert_eq!(stats.models_cached, 25);
        assert_eq!(stats.archive_stats.total_archives, 10);
    }

    #[test]
    fn test_asset_search_results() {
        let mut results = AssetSearchResults::default();
        results.models.push("tank.w3d".to_string());
        results.textures.push("tank_diffuse.tga".to_string());
        results.audio.push("engine.wav".to_string());

        assert_eq!(results.total_results(), 3);
        assert_eq!(results.models.len(), 1);
        assert_eq!(results.textures.len(), 1);
        assert_eq!(results.audio.len(), 1);
    }
}
