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
use glam::{Mat4, Vec3};
use log::{debug, error, info, warn};
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
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
    fn pt_vegetation_alias_mode() -> &'static str {
        static MODE: OnceLock<String> = OnceLock::new();
        MODE.get_or_init(|| {
            env::var("GENERALS_PT_VEGETATION_ALIAS_MODE")
                .unwrap_or_else(|_| "all_fir".to_string())
                .to_ascii_lowercase()
        })
        .as_str()
    }

    fn remap_pt_vegetation_alias(model_name_lower: &str) -> Option<&'static str> {
        let tree_target = match Self::pt_vegetation_alias_mode() {
            "trees_birch" | "all_birch" => Some("PTXBirch06"),
            "trees_oak" | "all_oak" => Some("PTXOak06"),
            "trees_palm" | "all_palm" => Some("PTPalm01"),
            "trees_maple" | "all_maple" => Some("PTMaple02"),
            "trees" | "trees_fir" | "all" | "all_fir" | "tree_pine1" | "tree_pine2"
            | "tree_spruce2" | "tree_spruce05" | "trees_pines" | "trees_spruces"
            | "trees_three" | "bushes_pines" | "bushes_spruces" => Some("PTXFir07"),
            _ => None,
        };

        match Self::pt_vegetation_alias_mode() {
            "bushes" => match model_name_lower {
                "ptbush02" => Some("PTBush17"),
                "ptbush03" => Some("PTBush18"),
                "ptbush08" => Some("PTBush20"),
                "ptbush11" => Some("PTBush21"),
                _ => None,
            },
            "trees" | "trees_fir" | "trees_birch" | "trees_oak" | "trees_palm" | "trees_maple" => {
                match model_name_lower {
                    "ptpine01" | "ptpine02" | "ptspruce01_hi" | "ptxpine05" => tree_target,
                    _ => None,
                }
            }
            "tree_pine1" => match model_name_lower {
                "ptpine01" => tree_target,
                _ => None,
            },
            "tree_pine2" => match model_name_lower {
                "ptpine02" => tree_target,
                _ => None,
            },
            "tree_spruce2" => match model_name_lower {
                "ptspruce01_hi" => tree_target,
                _ => None,
            },
            "tree_spruce05" => match model_name_lower {
                "ptxpine05" => tree_target,
                _ => None,
            },
            "trees_pines" => match model_name_lower {
                "ptpine01" | "ptpine02" => tree_target,
                _ => None,
            },
            "trees_spruces" => match model_name_lower {
                "ptspruce01_hi" | "ptxpine05" => tree_target,
                _ => None,
            },
            "trees_three" => match model_name_lower {
                "ptpine01" | "ptpine02" | "ptspruce01_hi" => tree_target,
                _ => None,
            },
            "bushes_pines" => match model_name_lower {
                "ptbush02" => Some("PTBush17"),
                "ptbush03" => Some("PTBush18"),
                "ptbush08" => Some("PTBush20"),
                "ptbush11" => Some("PTBush21"),
                "ptpine01" | "ptpine02" => tree_target,
                _ => None,
            },
            "bushes_spruces" => match model_name_lower {
                "ptbush02" => Some("PTBush17"),
                "ptbush03" => Some("PTBush18"),
                "ptbush08" => Some("PTBush20"),
                "ptbush11" => Some("PTBush21"),
                "ptspruce01_hi" | "ptxpine05" => tree_target,
                _ => None,
            },
            "all" | "all_fir" | "all_birch" | "all_oak" | "all_palm" | "all_maple" => {
                match model_name_lower {
                    "ptbush02" => Some("PTBush17"),
                    "ptbush03" => Some("PTBush18"),
                    "ptbush08" => Some("PTBush20"),
                    "ptbush11" => Some("PTBush21"),
                    "ptpine01" | "ptpine02" | "ptspruce01_hi" | "ptxpine05" => tree_target,
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn remap_known_model_alias(model_name: &str) -> &str {
        let model_name_lower = model_name.to_ascii_lowercase();
        if let Some(alias) = Self::remap_pt_vegetation_alias(&model_name_lower) {
            return alias;
        }

        match model_name_lower.as_str() {
            // Model ids that appear in challenge/skirmish map objects but do not ship as direct W3D ids.
            "100dollarcrate" | "200dollarcrate" | "1000dollarcrate" | "1500dollarcrate"
            | "2500dollarcrate" => "PMWldCrate",
            "salvagecrate" | "smalllevelupcrate" | "mediumlevelupcrate" | "2freecrusaderscrate" => {
                "PMWldCrate"
            }
            "ubcmdhq" => "UBCmdHQ_FA",
            "ubsupply" => "UBSupplyF",
            "nbconyard" => "NBConYard_FA",
            "uvtechjeep" => "UVTechJeep_d4",
            "uvtechvan" => "UVTechVan_d1",
            "uvtechtrck" => "UVTechTrck_D4",
            "nvssupplytk" => "NVSSupplyTk_B",
            "cbtaltower" => "CBTalTower_N",
            "cbtaltower_tr" => "CBTalTower_N",
            "cbtower01_tr" => "CBTower02_TR",
            "cbtower05_tr" => "CBTower05_N",
            "cbtower04_tr" => "CBTower03_SN",
            "pmoilsttk" => "CBOilRefny",
            "zzsupplydock" => "PMWldCrate",
            // Decorative map-object aliases observed in challenge/skirmish maps.
            "pmboulders" => "PMBoulders_D",
            "pmlclusters" => "PMLClusters_D",
            "pmmcluster" => "PMMCluster_D",
            "pmcluster" => "PMCluster_D",
            "pmrocks02" | "pmrocks03" | "pmrocks05" | "pmrocks06" | "pmrocks07" => "PMBoulders_D",
            "pmtrshpp03" | "pmtrshpl02" => "PMBrnTrshPl_D",
            "pmpump" => "PMWldCrate",
            "pmcrates" => "PMWldCrate",
            // Shell-map aliases already remapped in GameLogic; keep the render-side resolver in sync
            // so startup visuals do not fall back to placeholder meshes.
            "pmrocks01b" | "pmrocks02b" => "PMBoulders_D",
            "ptcypress01" => "PTXARBVT01",
            "ptxpine03" => "PTXFIR07",
            "pmswing" => "PMBikeRack",
            "pmplygdst" => "PMPavilion",
            "avamphib" => "AVChinook_A2",
            "avpaladin" => "AVCrusader_A",
            "cbsandbw2" => "CBSandBWY1",
            "cbsandbw4c" => "CBSandBWX",
            "cvtruck" => "CVTruck_D1",
            "cbnshack" => "CBNShack_S",
            // gc_tankgeneral renderer stability: these models can trip WW3D frame-state.
            "zbartplat" | "zbsmalpile_s" => "PMWldCrate",
            _ => model_name,
        }
    }

    fn normalize_model_lookup_key(model_name: &str) -> String {
        model_name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(model_name)
            .trim()
            .trim_end_matches(".w3d")
            .trim_end_matches(".W3D")
            .to_ascii_lowercase()
    }

    fn trim_model_variant_suffixes(model_key: &str) -> String {
        let mut trimmed = model_key
            .trim_end_matches(|ch: char| ch.is_ascii_digit())
            .to_string();
        for suffix in [
            "_dsng", "_esn", "_rsn", "_dsn", "_sng", "_dsg", "_sg", "_sn", "_dn", "_en", "_rn",
            "_ds", "_es", "_rs", "_ng", "_dg", "_ns", "_s", "_n", "_d", "_e", "_r", "_g", "_a",
            "_b", "_c",
        ] {
            if let Some(stripped) = trimmed.strip_suffix(suffix) {
                trimmed = stripped.to_string();
                break;
            }
        }
        trimmed
    }

    fn compact_model_signature(model_key: &str) -> String {
        model_key
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase()
    }

    fn levenshtein_distance(left: &str, right: &str) -> usize {
        if left == right {
            return 0;
        }
        if left.is_empty() {
            return right.len();
        }
        if right.is_empty() {
            return left.len();
        }

        let left_chars: Vec<char> = left.chars().collect();
        let right_chars: Vec<char> = right.chars().collect();
        let mut previous: Vec<usize> = (0..=right_chars.len()).collect();
        let mut current = vec![0usize; right_chars.len() + 1];

        for (i, left_char) in left_chars.iter().enumerate() {
            current[0] = i + 1;
            for (j, right_char) in right_chars.iter().enumerate() {
                let substitution_cost = usize::from(left_char != right_char);
                current[j + 1] = (previous[j + 1] + 1)
                    .min(current[j] + 1)
                    .min(previous[j] + substitution_cost);
            }
            previous.clone_from_slice(&current);
        }

        previous[right_chars.len()]
    }

    fn best_available_model_match<I>(requested_key: &str, available_models: I) -> Option<String>
    where
        I: Iterator<Item = String>,
    {
        let requested_trimmed = Self::trim_model_variant_suffixes(requested_key);
        let requested_signature = Self::compact_model_signature(&requested_trimmed);
        let mut best_match: Option<(i32, String)> = None;

        for available_model in available_models {
            let candidate_key = Self::normalize_model_lookup_key(&available_model);
            let candidate_trimmed = Self::trim_model_variant_suffixes(&candidate_key);
            let candidate_signature = Self::compact_model_signature(&candidate_trimmed);
            let score = if candidate_key == requested_key {
                10_000
            } else if candidate_key.starts_with(requested_key) {
                9_000 - (candidate_key.len() as i32 - requested_key.len() as i32).abs()
            } else if requested_key.starts_with(&candidate_key) {
                8_800 - (requested_key.len() as i32 - candidate_key.len() as i32).abs()
            } else if candidate_trimmed == requested_trimmed {
                8_400 - (candidate_key.len() as i32 - requested_key.len() as i32).abs()
            } else if candidate_trimmed.starts_with(&requested_trimmed)
                || requested_trimmed.starts_with(&candidate_trimmed)
            {
                8_000 - (candidate_trimmed.len() as i32 - requested_trimmed.len() as i32).abs()
            } else if !requested_signature.is_empty() && candidate_signature == requested_signature
            {
                7_600 - (candidate_key.len() as i32 - requested_key.len() as i32).abs()
            } else if !requested_signature.is_empty()
                && candidate_signature.contains(&requested_signature)
            {
                7_200 - (candidate_signature.len() as i32 - requested_signature.len() as i32).abs()
            } else {
                let distance =
                    Self::levenshtein_distance(&requested_signature, &candidate_signature);
                if distance <= 2 {
                    6_000 - distance as i32 * 100
                } else {
                    continue;
                }
            };

            match &best_match {
                Some((best_score, _)) if *best_score >= score => {}
                _ => {
                    let canonical = available_model
                        .rsplit(['/', '\\'])
                        .next()
                        .unwrap_or(&available_model)
                        .trim_end_matches(".w3d")
                        .trim_end_matches(".W3D")
                        .to_string();
                    best_match = Some((score, canonical));
                }
            }
        }

        best_match.map(|(_, model)| model)
    }

    fn model_variant_candidates(model_name: &str) -> Vec<String> {
        let base = model_name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(model_name)
            .trim()
            .trim_end_matches(".w3d")
            .trim_end_matches(".W3D");
        let mut candidates = vec![base.to_string()];
        for suffix in [
            "_d4", "_d3", "_d2", "_d1", "_d", "_dsn", "_dsng", "_ds", "_dsg", "_esn", "_es", "_en",
            "_rsn", "_rs", "_rn", "_sng", "_sn", "_sg", "_s", "_ng", "_n", "_g", "_a", "_b", "_c",
        ] {
            candidates.push(format!("{base}{suffix}"));
        }
        candidates
    }

    fn resolve_available_model_name(&mut self, model_name: &str) -> String {
        static MODEL_RESOLUTION_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> =
            OnceLock::new();

        let remapped_name = Self::remap_known_model_alias(model_name);
        let requested_key = Self::normalize_model_lookup_key(remapped_name);
        let cache = MODEL_RESOLUTION_CACHE.get_or_init(|| Mutex::new(HashMap::new()));

        if let Ok(cache) = cache.lock() {
            if let Some(cached) = cache.get(&requested_key) {
                return cached.clone().unwrap_or_else(|| remapped_name.to_string());
            }
        }

        for candidate in Self::model_variant_candidates(remapped_name) {
            for path in [
                format!("art/w3d/{candidate}.w3d"),
                format!("Art/W3D/{candidate}.W3D"),
                format!("{candidate}.w3d"),
                format!("{candidate}.W3D"),
            ] {
                if self.can_open_file_sync(&path) {
                    if let Ok(mut cache) = cache.lock() {
                        cache.insert(requested_key.clone(), Some(candidate.clone()));
                    }
                    if !candidate.eq_ignore_ascii_case(remapped_name) {
                        info!(
                            "Resolved W3D model '{}' -> fast variant '{}'",
                            model_name, candidate
                        );
                    }
                    return candidate;
                }
            }
        }

        let resolved = Self::best_available_model_match(
            &requested_key,
            self.list_available_models().into_iter(),
        );

        if let Ok(mut cache) = cache.lock() {
            cache.insert(requested_key, resolved.clone());
        }

        if let Some(resolved) = resolved {
            if !resolved.eq_ignore_ascii_case(remapped_name) {
                info!(
                    "Resolved W3D model '{}' -> closest shipped asset '{}'",
                    model_name, resolved
                );
            }
            resolved
        } else {
            remapped_name.to_string()
        }
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

        self.texture_manager
            .get_texture_or_default(&mut self.archive_system, device, queue, texture_name)
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

    /// Get default texture
    pub fn get_default_texture(&self) -> &GPUTexture {
        self.texture_manager.get_default_texture()
    }

    /// Get raw texture data if it's cached.
    pub fn get_raw_texture(&self, texture_name: &str) -> Option<&RawTexture> {
        self.texture_manager.get_raw_texture(texture_name)
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
        let remapped_name = self.resolve_available_model_name(model_name);
        let model_key = model_name.to_lowercase();
        let remapped_key = remapped_name.to_lowercase();

        // Check cache first
        if let Some(model) = self.model_cache.get(&model_key) {
            return Ok(model.clone());
        }
        if remapped_key != model_key {
            if let Some(model) = self.model_cache.get(&remapped_key).cloned() {
                self.model_cache.insert(model_key.clone(), model.clone());
                return Ok(model);
            }
        }

        if remapped_name != model_name {
            info!(
                "Loading W3D model: {} (alias -> {})",
                model_name, remapped_name
            );
        } else {
            info!("Loading W3D model: {}", model_name);
        }

        // Use the actual W3D loader to parse the model with global timeout protection
        let w3d_loader = W3DLoader::new();

        // CRITICAL FIX: Add global timeout to prevent infinite hangs in W3D parsing
        let global_timeout = tokio::time::Duration::from_secs(15);
        let load_result = tokio::time::timeout(
            global_timeout,
            w3d_loader.load_model(&mut self.archive_system, &remapped_name),
        )
        .await;

        let model = match load_result {
            Ok(Ok(model)) => model,
            Ok(Err(e)) => {
                warn!(
                    "W3D loader failed for '{}': {}. Using fallback mesh.",
                    model_name, e
                );
                let fallback = Self::build_fallback_model(model_name);
                self.model_cache.insert(model_key.clone(), fallback.clone());
                if remapped_key != model_key {
                    self.model_cache.insert(remapped_key, fallback.clone());
                }
                return Ok(fallback);
            }
            Err(_) => {
                warn!(
                    "W3D loading timed out after 15s for '{}'. Using fallback mesh.",
                    model_name
                );
                let fallback = Self::build_fallback_model(model_name);
                self.model_cache.insert(model_key.clone(), fallback.clone());
                if remapped_key != model_key {
                    self.model_cache.insert(remapped_key, fallback.clone());
                }
                return Ok(fallback);
            }
        };

        info!(
            "✅ Successfully loaded W3D model '{}' with {} meshes, {} total vertices",
            remapped_name,
            model.meshes.len(),
            model.meshes.iter().map(|m| m.vertices.len()).sum::<usize>()
        );

        // Cache the model
        self.model_cache.insert(remapped_key, model.clone());
        if remapped_name != model_name {
            self.model_cache.insert(model_key, model.clone());
        }
        Ok(model)
    }

    pub fn load_w3d_model_blocking(&mut self, model_name: &str) -> Result<W3DModel> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.load_w3d_model_async(model_name))
        })
    }

    /// Load a model synchronously by cloning from cache or loading through the W3D parser path.
    pub fn load_w3d_model(&mut self, model_name: &str) -> Result<W3DModel> {
        let remapped_name = self.resolve_available_model_name(model_name);
        let model_key = model_name.to_lowercase();
        let remapped_key = remapped_name.to_lowercase();

        // Check cache first
        if let Some(model) = self.model_cache.get(&model_key) {
            return Ok(model.clone());
        }
        if remapped_key != model_key {
            if let Some(model) = self.model_cache.get(&remapped_key).cloned() {
                self.model_cache.insert(model_key.clone(), model.clone());
                return Ok(model);
            }
        }

        let model = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread {
                tokio::task::block_in_place(|| {
                    handle.block_on(async {
                        let global_timeout = tokio::time::Duration::from_secs(15);
                        match tokio::time::timeout(
                            global_timeout,
                            self.model_loader
                                .load_model(&mut self.archive_system, &remapped_name),
                        )
                        .await
                        {
                            Ok(result) => result,
                            Err(_) => Err(anyhow!(
                                "Synchronous W3D loading timed out after 15s for '{}'",
                                remapped_name
                            )),
                        }
                    })
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
            runtime.block_on(async {
                let global_timeout = tokio::time::Duration::from_secs(15);
                match tokio::time::timeout(
                    global_timeout,
                    self.model_loader
                        .load_model(&mut self.archive_system, &remapped_name),
                )
                .await
                {
                    Ok(result) => result,
                    Err(_) => Err(anyhow!(
                        "Synchronous W3D loading timed out after 15s for '{}'",
                        remapped_name
                    )),
                }
            })
        };

        let model = match model {
            Ok(model) => model,
            Err(err) => {
                warn!(
                    "Synchronous W3D load failed for '{}': {}. Using fallback mesh.",
                    remapped_name, err
                );
                Self::build_fallback_model(&remapped_name)
            }
        };

        // Cache the model
        self.model_cache.insert(remapped_key, model.clone());
        if remapped_name != model_name {
            self.model_cache.insert(model_key, model.clone());
        }
        Ok(model)
    }

    fn build_fallback_model(model_name: &str) -> W3DModel {
        let mut fallback_mesh = crate::assets::models::W3DMesh::new(format!("{}_mesh", model_name));
        fallback_mesh.vertices = vec![
            crate::assets::models::W3DVertex {
                position: [-1.0, -1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
            },
            crate::assets::models::W3DVertex {
                position: [1.0, -1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
            },
            crate::assets::models::W3DVertex {
                position: [0.0, 1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.5, 1.0],
                color: [1.0, 1.0, 1.0, 1.0],
            },
        ];
        fallback_mesh.indices = vec![0, 1, 2];
        fallback_mesh.material = crate::assets::models::W3DMaterial::default();
        fallback_mesh.transform = Mat4::IDENTITY;
        fallback_mesh.stage_uv_channels = vec![0];

        W3DModel {
            name: model_name.to_string(),
            meshes: vec![fallback_mesh],
            materials: HashMap::new(),
            texture_names: Vec::new(),
            ww3d_mesh_models: HashMap::new(),
            bounding_box_min: Vec3::new(-1.0, -1.0, 0.0),
            bounding_box_max: Vec3::new(1.0, 1.0, 0.0),
        }
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

    tokio::task::spawn_blocking(move || {
        let result = {
            let mut manager = manager_arc.lock().expect("asset manager mutex poisoned");
            manager
                .texture_manager
                .load_caustic_textures(device.as_ref(), queue.as_ref())
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

    #[test]
    fn remap_known_model_alias_covers_shell_map_aliases() {
        assert_eq!(
            AssetManager::remap_known_model_alias("PMRocks01b"),
            "PMBoulders_D"
        );
        assert_eq!(
            AssetManager::remap_known_model_alias("PMRocks02b"),
            "PMBoulders_D"
        );
        assert_eq!(
            AssetManager::remap_known_model_alias("PTCypress01"),
            "PTXARBVT01"
        );
        assert_eq!(
            AssetManager::remap_known_model_alias("PTXPine03"),
            "PTXFIR07"
        );
        assert_eq!(
            AssetManager::remap_known_model_alias("PMSwing"),
            "PMBikeRack"
        );
        assert_eq!(
            AssetManager::remap_known_model_alias("PMPlygdSt"),
            "PMPavilion"
        );
        assert_eq!(
            AssetManager::remap_known_model_alias("AVAMPHIB"),
            "AVChinook_A2"
        );
        assert_eq!(
            AssetManager::remap_known_model_alias("AVPaladin"),
            "AVCrusader_A"
        );
    }
}
