use super::{ConfigValue, IniParser, LoadMode};
use anyhow::Result;
use crc32fast::Hasher;
use game_engine::common::global_data as runtime_global_data;
use game_engine::common::system::file::FileAccess;
use game_engine::common::system::file_system::get_file_system;
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Global game data system - matches C++ GlobalData functionality
pub struct GlobalData {
    /// INI parser for loading configuration
    ini_parser: IniParser,
    /// Game data paths
    user_data_path: PathBuf,
    game_data_path: PathBuf,
    /// Frame rate settings
    pub frames_per_second_limit: i32,
    pub use_fps_limit: bool,
    /// Audio settings
    pub audio_on: bool,
    pub music_on: bool,
    pub sounds_on: bool,
    pub sounds_3d_on: bool,
    pub speech_on: bool,
    /// Graphics settings
    pub shell_map_on: bool,
    pub shell_map_name: String,
    pub camera_pitch: f32,
    pub camera_yaw: f32,
    pub camera_height: f32,
    pub max_camera_height: f32,
    /// Game state flags
    pub play_intro: bool,
    pub after_intro: bool,
    pub build_map_cache: bool,
    pub should_update_tga_to_dds: bool,
    /// Initial file to load (command line)
    pub initial_file: String,
    /// Pending file for loading
    pub pending_file: String,
    /// Benchmark settings
    pub benchmark_timer: i32,
    pub tivo_fast_mode: bool,
    /// INI CRC for validation
    pub ini_crc: u32,
    /// Additional game settings
    game_settings: HashMap<String, ConfigValue>,
    /// Active language override
    pub language: String,
    /// Active mod override/path
    pub active_mod: Option<String>,
}

impl GlobalData {
    /// Create new GlobalData instance
    pub fn new() -> Self {
        Self {
            ini_parser: IniParser::new(),
            user_data_path: Self::get_default_user_data_path(),
            game_data_path: Self::get_default_game_data_path(),
            frames_per_second_limit: 30,
            use_fps_limit: true,
            audio_on: true,
            music_on: true,
            sounds_on: true,
            sounds_3d_on: true,
            speech_on: true,
            shell_map_on: true,
            shell_map_name: "Maps\\ShellMap1\\ShellMap1.map".to_string(),
            camera_pitch: 37.5,
            camera_yaw: 0.0,
            camera_height: 232.0,
            max_camera_height: 310.0,
            play_intro: true,
            after_intro: false,
            build_map_cache: false,
            should_update_tga_to_dds: false,
            initial_file: String::new(),
            pending_file: String::new(),
            benchmark_timer: 0,
            tivo_fast_mode: false,
            ini_crc: 0,
            game_settings: HashMap::new(),
            language: "English".to_string(),
            active_mod: None,
        }
    }

    /// Load INI configuration file
    pub fn load_ini<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let requested = path.as_ref();
        info!("Loading INI configuration: {:?}", requested);

        let Some(path) = Self::resolve_ini_path(requested) else {
            warn!("INI file not found: {:?}", requested);
            return Ok(()); // Allow missing files like C++ version
        };

        if path != requested {
            info!("Resolved INI path {:?} -> {:?}", requested, path);
        }

        let contents = Self::read_ini_text(&path)
            .map_err(|err| anyhow::anyhow!("Failed to read INI '{}': {}", path.display(), err))?;

        let result = self
            .ini_parser
            .load_from_string(&contents, LoadMode::MultiFile)?;

        // Load specific settings from the INI
        self.load_settings_from_ini();
        self.sync_runtime_view();

        info!(
            "INI loaded: {} sections, {} keys",
            result.sections_loaded, result.keys_loaded
        );
        if !result.warnings.is_empty() {
            warn!("INI warnings: {:?}", result.warnings);
        }
        if !result.errors.is_empty() {
            error!("INI errors: {:?}", result.errors);
        }

        Ok(())
    }

    /// Load settings from parsed INI data
    fn load_settings_from_ini(&mut self) {
        // Generals stores these startup/runtime defaults under the GameData block.
        self.audio_on = self
            .ini_parser
            .get_bool("GameData", "AudioOn", self.audio_on);
        self.music_on = self
            .ini_parser
            .get_bool("GameData", "MusicOn", self.music_on);
        self.sounds_on = self
            .ini_parser
            .get_bool("GameData", "SoundsOn", self.sounds_on);
        self.speech_on = self
            .ini_parser
            .get_bool("GameData", "SpeechOn", self.speech_on);
        self.frames_per_second_limit = self.ini_parser.get_int(
            "GameData",
            "FramesPerSecondLimit",
            self.frames_per_second_limit,
        );
        self.use_fps_limit =
            self.ini_parser
                .get_bool("GameData", "UseFPSLimit", self.use_fps_limit);
        self.shell_map_on = self
            .ini_parser
            .get_bool("GameData", "ShellMapOn", self.shell_map_on);
        self.shell_map_name =
            self.ini_parser
                .get_string("GameData", "ShellMapName", Some(&self.shell_map_name));
        self.camera_pitch = self
            .ini_parser
            .get_float("GameData", "CameraPitch", self.camera_pitch);
        self.camera_yaw = self
            .ini_parser
            .get_float("GameData", "CameraYaw", self.camera_yaw);
        self.camera_height =
            self.ini_parser
                .get_float("GameData", "CameraHeight", self.camera_height);
        self.max_camera_height =
            self.ini_parser
                .get_float("GameData", "MaxCameraHeight", self.max_camera_height);
        self.play_intro = self
            .ini_parser
            .get_bool("GameData", "PlayIntro", self.play_intro);
        self.build_map_cache =
            self.ini_parser
                .get_bool("GameData", "BuildMapCache", self.build_map_cache);
        self.should_update_tga_to_dds =
            self.ini_parser
                .get_bool("GameData", "UpdateTGAtoDDS", self.should_update_tga_to_dds);

        // Load benchmark settings
        self.benchmark_timer =
            self.ini_parser
                .get_int("Debug", "BenchmarkTimer", self.benchmark_timer);
        self.tivo_fast_mode =
            self.ini_parser
                .get_bool("Debug", "TiVOFastMode", self.tivo_fast_mode);

        // Load file paths
        let user_data_str = self.ini_parser.get_string("Paths", "UserDataPath", None);
        if !user_data_str.is_empty() {
            self.user_data_path = PathBuf::from(user_data_str);
        }

        let game_data_str = self.ini_parser.get_string("Paths", "GameDataPath", None);
        if !game_data_str.is_empty() {
            self.game_data_path = PathBuf::from(game_data_str);
        }

        debug!("Global data settings loaded:");
        debug!(
            "  FPS Limit: {} (enabled: {})",
            self.frames_per_second_limit, self.use_fps_limit
        );
        debug!(
            "  Audio: {} (music: {}, sounds: {}, 3D: {}, speech: {})",
            self.audio_on, self.music_on, self.sounds_on, self.sounds_3d_on, self.speech_on
        );
        debug!(
            "  Shell map: {} ({})",
            self.shell_map_on, self.shell_map_name
        );
    }

    /// Push the parsed startup view into the runtime global-data singleton.
    pub fn sync_runtime_view(&self) {
        let Ok(mut runtime) = runtime_global_data::write_safe() else {
            warn!("Runtime global data unavailable; startup config sync skipped");
            return;
        };

        runtime.writable.frames_per_second_limit = self.frames_per_second_limit;
        runtime.writable.use_fps_limit = self.use_fps_limit;
        runtime.writable.audio_on = self.audio_on;
        runtime.writable.music_on = self.music_on;
        runtime.writable.sounds_on = self.sounds_on;
        runtime.writable.speech_on = self.speech_on;
        runtime.writable.build_map_cache = self.build_map_cache;
        runtime.writable.shell_map_on = self.shell_map_on;
        runtime.writable.shell_map_name = self.shell_map_name.clone();
        runtime.writable.play_intro = self.play_intro;
        runtime.writable.after_intro = self.after_intro;
        runtime.writable.benchmark_timer = self.benchmark_timer;
        runtime.writable.should_update_tga_to_dds = self.should_update_tga_to_dds;
        runtime.writable.initial_file = self.initial_file.clone();
        runtime.pending_file = self.pending_file.clone();
        runtime.tivo_fast_mode = self.tivo_fast_mode;
        runtime.camera_pitch = self.camera_pitch;
        runtime.camera_yaw = self.camera_yaw;
        runtime.camera_height = self.camera_height;
        runtime.max_camera_height = self.max_camera_height;
        runtime.ini_crc = self.ini_crc;
        runtime.set_user_data_dir(self.user_data_path.to_string_lossy().into_owned());
        runtime.set_override(
            "language",
            runtime_global_data::GlobalValue::String(self.language.clone()),
        );
        if let Some(active_mod) = &self.active_mod {
            runtime.set_override(
                "active_mod",
                runtime_global_data::GlobalValue::String(active_mod.clone()),
            );
        } else {
            runtime.clear_override("active_mod");
        }
    }

    /// Calculate CRC of all loaded INI data
    pub fn calculate_crc(&self) -> u32 {
        let mut hasher = Hasher::new();

        // Hash all configuration data in a deterministic order
        let mut sections: Vec<_> = self.ini_parser.get_sections();
        sections.sort();

        for section_name in sections {
            hasher.update(section_name.as_bytes());

            let mut keys = self.ini_parser.get_keys(&section_name);
            keys.sort();

            for key in keys {
                hasher.update(key.as_bytes());

                if let Some(value) = self.ini_parser.get_value(&section_name, &key) {
                    let value_str = format!("{:?}", value);
                    hasher.update(value_str.as_bytes());
                }
            }
        }

        hasher.finalize()
    }

    /// Get user data path
    pub fn get_path_user_data(&self) -> &Path {
        &self.user_data_path
    }

    /// Get game data path  
    pub fn get_path_game_data(&self) -> &Path {
        &self.game_data_path
    }

    /// Set initial file to load (from command line)
    pub fn set_initial_file<S: Into<String>>(&mut self, file: S) {
        self.initial_file = normalize_startup_map_path(file.into());
        info!("Initial file set to: {}", self.initial_file);
    }

    /// Apply quick start behavior (skip intros/shell map).
    pub fn apply_quick_start(&mut self) {
        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            self.apply_intro_disabled();
        }
        self.shell_map_on = false;
        #[cfg(any(debug_assertions, feature = "internal"))]
        info!("QuickStart applied: intros disabled, shell map off");

        #[cfg(not(any(debug_assertions, feature = "internal")))]
        info!("QuickStart applied: shell map off");
    }

    /// Disable the intro sequence while preserving the current shell-map name.
    fn apply_intro_disabled(&mut self) {
        self.play_intro = false;
        self.after_intro = true;
    }

    /// Override the current language (matches -lang).
    pub fn set_language<S: Into<String>>(&mut self, language: S) {
        self.language = language.into();
        if let Ok(mut runtime) = runtime_global_data::write_safe() {
            runtime.set_override(
                "language",
                runtime_global_data::GlobalValue::String(self.language.clone()),
            );
        }
        info!("Language override set to '{}'", self.language);
    }

    /// Retrieve the active language.
    pub fn language(&self) -> &str {
        &self.language
    }

    /// Set the currently active mod (matches -mod).
    pub fn set_active_mod<S: Into<String>>(&mut self, mod_name: S) {
        let name = mod_name.into();
        if name.is_empty() {
            self.active_mod = None;
        } else {
            self.active_mod = Some(name);
        }
        if let Ok(mut runtime) = runtime_global_data::write_safe() {
            if let Some(active_mod) = &self.active_mod {
                runtime.set_override(
                    "active_mod",
                    runtime_global_data::GlobalValue::String(active_mod.clone()),
                );
            } else {
                runtime.clear_override("active_mod");
            }
        }
        match &self.active_mod {
            Some(name) => info!("Mod override enabled: {}", name),
            None => info!("Mod override cleared"),
        }
    }

    /// Get the active mod, if any.
    pub fn active_mod(&self) -> Option<&str> {
        self.active_mod.as_deref()
    }

    /// Get INI parser (read-only)
    pub fn get_ini_parser(&self) -> &IniParser {
        &self.ini_parser
    }

    /// Get INI parser (mutable)
    pub fn get_ini_parser_mut(&mut self) -> &mut IniParser {
        &mut self.ini_parser
    }

    /// Get custom setting value
    pub fn get_setting(&self, section: &str, key: &str) -> Option<&ConfigValue> {
        self.ini_parser.get_value(section, key)
    }

    /// Get string setting with default
    pub fn get_string_setting(&self, section: &str, key: &str, default: &str) -> String {
        self.ini_parser.get_string(section, key, Some(default))
    }

    /// Get integer setting with default
    pub fn get_int_setting(&self, section: &str, key: &str, default: i32) -> i32 {
        self.ini_parser.get_int(section, key, default)
    }

    /// Get float setting with default
    pub fn get_float_setting(&self, section: &str, key: &str, default: f32) -> f32 {
        self.ini_parser.get_float(section, key, default)
    }

    /// Get boolean setting with default
    pub fn get_bool_setting(&self, section: &str, key: &str, default: bool) -> bool {
        self.ini_parser.get_bool(section, key, default)
    }

    /// Process command line arguments (matches C++ parseCommandLine)
    pub fn parse_command_line(&mut self, args: &[String]) -> Result<()> {
        info!("Parsing command line arguments: {:?}", args);

        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            let arg_lower = arg.to_ascii_lowercase();

            match arg_lower.as_str() {
                "-file" | "-replay" => {
                    if i + 1 < args.len() {
                        self.set_initial_file(args[i + 1].clone());
                        info!("Command line file: {}", self.initial_file);
                        i += 1;
                    }
                }
                "-benchmark" => {
                    if i + 1 < args.len() {
                        if let Ok(time) = args[i + 1].parse::<i32>() {
                            self.benchmark_timer = time;
                            info!("Benchmark timer set to: {} seconds", time);
                        }
                        i += 1;
                    }
                }
                "-nologo" | "-nointro" => {
                    self.apply_intro_disabled();
                    info!("Intro disabled");
                }
                "-quickstart" => {
                    self.apply_quick_start();
                }
                "-buildmapcache" | "-buildcache" => {
                    self.build_map_cache = true;
                    info!("Map cache building enabled");
                }
                "-updateimages" | "-updatedds" => {
                    self.should_update_tga_to_dds = true;
                    info!("TGA to DDS update enabled");
                }
                "-noshellmap" => {
                    self.shell_map_on = false;
                    info!("Shell map disabled");
                }
                "-shellmap" => {
                    if i + 1 < args.len() {
                        self.shell_map_name = args[i + 1].clone();
                        info!("Shell map override: {}", self.shell_map_name);
                        i += 1;
                    }
                }
                "-nofpslimit" => {
                    self.use_fps_limit = false;
                    self.frames_per_second_limit = 30000;
                    info!("FPS limit disabled");
                }
                "-fps" => {
                    if i + 1 < args.len() {
                        if let Ok(fps) = args[i + 1].parse::<i32>() {
                            self.frames_per_second_limit = fps;
                            info!("FPS limit set to: {}", fps);
                        }
                        i += 1;
                    }
                }
                "-userdata" => {
                    if i + 1 < args.len() {
                        self.user_data_path = PathBuf::from(&args[i + 1]);
                        info!("User data path set to: {:?}", self.user_data_path);
                        i += 1;
                    }
                }
                _ => {
                    // Unknown argument - just log and continue
                    debug!("Unknown command line argument: {}", arg);
                }
            }

            i += 1;
        }

        self.sync_runtime_view();
        Ok(())
    }

    /// Get default user data path
    fn get_default_user_data_path() -> PathBuf {
        #[cfg(windows)]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                PathBuf::from(appdata).join("Command & Conquer Generals Zero Hour")
            } else {
                PathBuf::from("UserData")
            }
        }
        #[cfg(not(windows))]
        {
            if let Ok(home) = std::env::var("HOME") {
                PathBuf::from(home).join(".cnc_generals")
            } else {
                PathBuf::from("UserData")
            }
        }
    }

    /// Get default game data path
    fn get_default_game_data_path() -> PathBuf {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("Data")
    }

    fn resolve_ini_path(requested: &Path) -> Option<PathBuf> {
        if requested.as_os_str().is_empty() {
            return None;
        }

        let requested_normalized = requested
            .to_string_lossy()
            .replace('\\', "/")
            .trim()
            .to_string();

        let mut candidates = Vec::new();
        candidates.push(requested_normalized.clone());
        if let Some(stripped) = requested_normalized.strip_prefix("./") {
            candidates.push(stripped.to_string());
        }

        for candidate in &candidates {
            if let Some(found) = resolve_via_file_system(Path::new(candidate)) {
                return Some(found);
            }
            if Path::new(candidate).exists() {
                return Some(Path::new(candidate).to_path_buf());
            }
        }

        None
    }

    fn read_ini_text(path: &Path) -> Result<String> {
        if let Some(contents) = read_text_via_file_system(path) {
            return Ok(contents);
        }

        std::fs::read_to_string(path).map_err(anyhow::Error::from)
    }

    /// Validate configuration
    pub fn validate(&self) -> Vec<String> {
        let mut issues = Vec::new();

        // Check FPS limit
        if self.frames_per_second_limit <= 0 {
            issues.push("Invalid FPS limit: must be > 0".to_string());
        }

        // Check paths exist
        if !self.game_data_path.exists() {
            issues.push(format!(
                "Game data path does not exist: {:?}",
                self.game_data_path
            ));
        }

        // Check initial file if specified
        if !self.initial_file.is_empty() && !Path::new(&self.initial_file).exists() {
            issues.push(format!(
                "Initial file does not exist: {}",
                self.initial_file
            ));
        }

        issues
    }

    /// Get statistics
    pub fn get_stats(&self) -> GlobalDataStats {
        let ini_stats = self.ini_parser.get_stats();

        GlobalDataStats {
            ini_sections: ini_stats.sections,
            ini_keys: ini_stats.total_keys,
            ini_crc: self.ini_crc,
            settings_count: self.game_settings.len(),
            user_data_path: self.user_data_path.clone(),
            game_data_path: self.game_data_path.clone(),
        }
    }
}

fn normalize_virtual_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim()
        .trim_matches('"')
        .to_string()
}

fn resolve_via_file_system(path: &Path) -> Option<PathBuf> {
    let candidate = normalize_virtual_path(path);
    if candidate.is_empty() {
        return None;
    }

    if let Ok(file_system) = get_file_system().lock() {
        if file_system.does_file_exist(&candidate) {
            return Some(PathBuf::from(candidate));
        }
    }

    None
}

fn read_text_via_file_system(path: &Path) -> Option<String> {
    let candidate = normalize_virtual_path(path);
    if candidate.is_empty() {
        return None;
    }

    let access = FileAccess::READ.combine(FileAccess::BINARY);
    if let Ok(mut file_system) = get_file_system().lock() {
        if let Some(mut file) = file_system.open_file(&candidate, access) {
            if let Ok(bytes) = file.read_entire_and_close() {
                return String::from_utf8(bytes).ok();
            }
        }
    }

    None
}

pub(crate) fn normalize_startup_map_path<S: Into<String>>(path: S) -> String {
    let path = path.into();
    let trimmed = path.trim().trim_matches('"');
    if trimmed.is_empty() {
        return String::new();
    }

    if !trimmed.contains('\\') && !trimmed.contains('/') {
        return trimmed.to_string();
    }

    let mut prefix_parts: Vec<String> = Vec::new();
    let mut map_stem: Option<String> = None;

    for part in trimmed.split(['\\', '/']) {
        if part.is_empty() {
            continue;
        }

        if part.to_ascii_lowercase().ends_with(".map") {
            let stem = &part[..part.len().saturating_sub(4)];
            if stem.is_empty() {
                return trimmed.to_string();
            }
            map_stem = Some(stem.to_string());
            break;
        }

        prefix_parts.push(part.to_string());
    }

    let Some(map_stem) = map_stem else {
        return trimmed.to_string();
    };

    let mut normalized = prefix_parts.join("\\");
    if !normalized.is_empty() {
        normalized.push('\\');
    }
    normalized.push_str(&map_stem);
    normalized.push('\\');
    normalized.push_str(&map_stem);
    normalized.push_str(".map");
    normalized
}

/// Global data statistics
#[derive(Debug)]
pub struct GlobalDataStats {
    pub ini_sections: usize,
    pub ini_keys: usize,
    pub ini_crc: u32,
    pub settings_count: usize,
    pub user_data_path: PathBuf,
    pub game_data_path: PathBuf,
}

/// Configuration system wrapper
pub struct ConfigurationSystem {
    global_data: GlobalData,
}

impl ConfigurationSystem {
    /// Create new configuration system
    pub fn new() -> Self {
        Self {
            global_data: GlobalData::new(),
        }
    }

    /// Initialize with default INI files
    pub fn initialize(&mut self) -> Result<()> {
        info!("Initializing configuration system");

        // Load default game configuration
        self.global_data
            .load_ini("Data/INI/Default/GameData.ini")
            .ok();
        self.global_data.load_ini("Data/INI/GameData.ini").ok();

        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            // Load debug configuration in debug builds
            self.global_data.load_ini("Data/INI/GameDataDebug.ini").ok();
        }

        // Calculate final CRC
        self.global_data.ini_crc = self.global_data.calculate_crc();
        self.global_data.sync_runtime_view();

        // Validate configuration
        let issues = self.global_data.validate();
        if !issues.is_empty() {
            warn!("Configuration validation issues: {:?}", issues);
        }

        info!(
            "Configuration system initialized (CRC: {:08X})",
            self.global_data.ini_crc
        );
        Ok(())
    }

    /// Get global data
    pub fn get_global_data(&self) -> &GlobalData {
        &self.global_data
    }

    /// Get mutable global data
    pub fn get_global_data_mut(&mut self) -> &mut GlobalData {
        &mut self.global_data
    }

    /// Process command line arguments
    pub fn parse_command_line(&mut self, args: &[String]) -> Result<()> {
        self.global_data.parse_command_line(args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_global_data_creation() {
        let global_data = GlobalData::new();

        assert_eq!(global_data.frames_per_second_limit, 30);
        assert!(global_data.audio_on);
        assert!(global_data.use_fps_limit);
        assert!(global_data.initial_file.is_empty());
    }

    #[test]
    fn test_command_line_parsing() {
        let mut global_data = GlobalData::new();
        let args = vec![
            "program".to_string(),
            "-file".to_string(),
            "test.map".to_string(),
            "-fps".to_string(),
            "120".to_string(),
            "-nointro".to_string(),
        ];

        global_data.parse_command_line(&args[1..]).unwrap();

        assert_eq!(global_data.initial_file, "test.map");
        assert_eq!(global_data.frames_per_second_limit, 120);
        assert!(!global_data.play_intro);
    }

    #[test]
    fn test_command_line_normalizes_short_map_paths_and_preserves_shell_map_name() {
        let mut global_data = GlobalData::new();
        let args = vec![
            "program".to_string(),
            "-file".to_string(),
            "Maps\\ShellMap1.map".to_string(),
            "-quickstart".to_string(),
        ];

        global_data.parse_command_line(&args[1..]).unwrap();

        assert_eq!(global_data.initial_file, "Maps\\ShellMap1\\ShellMap1.map");
        assert!(!global_data.shell_map_on);
        assert_eq!(global_data.shell_map_name, "Maps\\ShellMap1\\ShellMap1.map");
        #[cfg(any(debug_assertions, feature = "internal"))]
        assert!(!global_data.play_intro);
        #[cfg(not(any(debug_assertions, feature = "internal")))]
        assert!(global_data.play_intro);
    }

    #[test]
    fn test_ini_loading() {
        let temp_dir = tempdir().unwrap();
        let ini_path = temp_dir.path().join("test.ini");

        let ini_content = r#"
[GameData]
AudioOn = true
MusicOn = false
SoundsOn = true
FramesPerSecondLimit = 30
UseFPSLimit = true
"#;

        fs::write(&ini_path, ini_content).unwrap();

        let mut global_data = GlobalData::new();
        global_data.load_ini(&ini_path).unwrap();

        assert!(global_data.audio_on);
        assert!(!global_data.music_on);
        assert!(global_data.sounds_on);
        assert_eq!(global_data.frames_per_second_limit, 30);
        assert!(global_data.use_fps_limit);
    }

    #[test]
    fn test_crc_calculation() {
        let mut global_data = GlobalData::new();

        // Load some test data
        let ini_content = r#"
[Test]
Key1 = Value1
Key2 = 42
"#;
        global_data
            .ini_parser
            .load_from_string(ini_content, LoadMode::Overwrite)
            .unwrap();

        let crc1 = global_data.calculate_crc();

        // Load the same data again - should get same CRC
        global_data
            .ini_parser
            .load_from_string(ini_content, LoadMode::Overwrite)
            .unwrap();
        let crc2 = global_data.calculate_crc();

        assert_eq!(crc1, crc2);
        assert_ne!(crc1, 0);
    }
}
