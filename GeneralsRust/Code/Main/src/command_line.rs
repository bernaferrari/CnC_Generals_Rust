////////////////////////////////////////////////////////////////////////////////
//
//  (c) 2001-2003 Electronic Arts Inc.
//
////////////////////////////////////////////////////////////////////////////////

// FILE: command_line.rs
//
// Command line argument parsing and processing
// Matches the C++ CommandLineParser functionality
//
// Author: Colin Day, April 2001 (Converted to Rust)
//
///////////////////////////////////////////////////////////////////////////////

use anyhow::{Context, Result};
use log::{info, warn};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use crate::config::global_data::normalize_startup_map_path;
use game_engine::common::global_data as runtime_global_data;

/// C++ WinMain only stored the first 20 argv entries.
pub const MAX_STARTUP_ARGS: usize = 20;

/// Command line arguments parsed from the application startup
#[derive(Debug, Clone)]
pub struct CommandLineArgs {
    /// Raw command line arguments
    pub raw_args: Vec<String>,

    /// Parsed options with their values
    pub options: HashMap<String, Option<String>>,

    /// Positional arguments (not flags)
    pub positional_args: Vec<String>,

    // Game-specific options
    pub windowed: bool,
    pub fullscreen: bool,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub map_name: Option<String>,
    pub mod_name: Option<String>,
    pub mod_dir: Option<String>,
    pub mod_big: Option<String>,
    pub player_name: Option<String>,
    pub language: Option<String>,
    pub replay_file: Option<PathBuf>,
    pub config_file: Option<PathBuf>,
    pub log_level: Option<String>,
    pub no_audio: bool,
    pub no_video: bool,
    pub developer_mode: bool,
    pub quick_start: bool,
    pub auto_replay: bool,
    pub benchmark_mode: bool,
    pub server_mode: bool,
    pub client_mode: bool,
    pub network_port: Option<u16>,
    pub network_host: Option<String>,
    pub display_debug_overlay: bool,
    pub integration_diagnostics: bool,
    pub dx_stack_dump: bool,
    pub smoke_test: bool,
    /// Last explicit startup window mode flag from command line order.
    /// `Some(true)` => windowed, `Some(false)` => fullscreen.
    window_mode_override: Option<bool>,
}

impl Default for CommandLineArgs {
    fn default() -> Self {
        Self {
            raw_args: Vec::new(),
            options: HashMap::new(),
            positional_args: Vec::new(),
            windowed: false,
            fullscreen: false,
            width: None,
            height: None,
            map_name: None,
            mod_name: None,
            mod_dir: None,
            mod_big: None,
            player_name: None,
            language: None,
            replay_file: None,
            config_file: None,
            log_level: None,
            no_audio: false,
            no_video: false,
            developer_mode: false,
            quick_start: false,
            auto_replay: false,
            benchmark_mode: false,
            server_mode: false,
            client_mode: false,
            network_port: None,
            network_host: None,
            display_debug_overlay: false,
            integration_diagnostics: false,
            dx_stack_dump: false,
            smoke_test: false,
            window_mode_override: None,
        }
    }
}

impl CommandLineArgs {
    /// Parse command line arguments from environment
    pub fn parse() -> Result<Self> {
        let args = Self::startup_args();
        Self::parse_from_args(args)
    }

    /// Collect the startup argv snapshot using the same practical limit as C++ WinMain.
    pub fn startup_args() -> Vec<String> {
        env::args().take(MAX_STARTUP_ARGS).collect()
    }

    /// Apply the C++ WinMain argv cap to an argument vector.
    pub fn limit_startup_args(mut args: Vec<String>) -> Vec<String> {
        if args.len() > MAX_STARTUP_ARGS {
            args.truncate(MAX_STARTUP_ARGS);
        }
        args
    }

    /// C++ WinMain only triggers the DX bootstrap path when `-DX` is the second token.
    pub fn wants_dx_stack_dump_from_args(args: &[String]) -> bool {
        args.len() > 2 && args[1].eq_ignore_ascii_case("-dx")
    }

    /// Parse command line arguments from a vector of strings
    pub fn parse_from_args(args: Vec<String>) -> Result<Self> {
        let mut parsed = Self {
            raw_args: args.clone(),
            ..Default::default()
        };

        let mut i = 1; // Skip program name
        while i < args.len() {
            let arg = &args[i];

            if arg.starts_with('-') {
                // Parse option
                let (option, value) = Self::parse_option(arg, &args, &mut i)?;
                parsed.options.insert(option.clone(), value.clone());

                // Handle specific options
                match option.as_str() {
                    "win" | "windowed" | "w" => {
                        parsed.windowed = true;
                        parsed.window_mode_override = Some(true);
                        Self::store_option_aliases(
                            &mut parsed.options,
                            &["win", "windowed", "w"],
                            &value,
                        );
                    }
                    "fullscreen" | "f" | "nowin" => {
                        parsed.fullscreen = true;
                        parsed.window_mode_override = Some(false);
                        Self::store_option_aliases(
                            &mut parsed.options,
                            &["fullscreen", "f", "nowin"],
                            &value,
                        );
                    }
                    "width" => {
                        if let Some(v) = value {
                            parsed.width = Some(Self::parse_startup_dimension(&v));
                        }
                    }
                    "height" => {
                        if let Some(v) = value {
                            parsed.height = Some(Self::parse_startup_dimension(&v));
                        }
                    }
                    "xres" => {
                        if let Some(v) = value {
                            parsed.width = Some(Self::parse_startup_dimension(&v));
                        }
                    }
                    "yres" => {
                        if let Some(v) = value {
                            parsed.height = Some(Self::parse_startup_dimension(&v));
                        }
                    }
                    "file" => {
                        if let Some(v) = value {
                            let normalized = normalize_startup_map_path(v);
                            if normalized.to_ascii_lowercase().ends_with(".map") {
                                parsed.map_name = Some(normalized.clone());
                            }
                            parsed.options.insert("file".to_string(), Some(normalized));
                        }
                    }
                    "map" => {
                        parsed.map_name = value.map(normalize_startup_map_path);
                        parsed
                            .options
                            .insert("map".to_string(), parsed.map_name.clone());
                    }
                    "mod" => {
                        if let Some(v) = value {
                            match Self::resolve_mod_path(&v) {
                                Some(resolved) => {
                                    parsed.mod_name = Some(resolved.active_mod);
                                    parsed.mod_dir = resolved.mod_dir;
                                    parsed.mod_big = resolved.mod_big;
                                }
                                None => warn!(
                                    "Mod path does not exist or could not be inspected: {}",
                                    v
                                ),
                            }
                        }
                    }
                    "player" | "playername" => {
                        Self::store_option_aliases(
                            &mut parsed.options,
                            &["player", "playername"],
                            &value,
                        );
                        parsed.player_name = value;
                    }
                    "lang" | "language" => {
                        Self::store_option_aliases(
                            &mut parsed.options,
                            &["lang", "language"],
                            &value,
                        );
                        parsed.language = value;
                    }
                    "replay" => {
                        if let Some(v) = value {
                            parsed.replay_file = Some(PathBuf::from(v.clone()));
                            parsed.options.insert("replay".to_string(), Some(v));
                        }
                    }
                    "config" => {
                        if let Some(v) = value {
                            parsed.config_file = Some(PathBuf::from(v));
                        }
                    }
                    "loglevel" => parsed.log_level = value,
                    "noaudio" => parsed.no_audio = true,
                    "novideo" => parsed.no_video = true,
                    "dev" | "developer" => {
                        parsed.developer_mode = true;
                        Self::store_option_aliases(
                            &mut parsed.options,
                            &["dev", "developer"],
                            &value,
                        );
                    }
                    "quickstart" => parsed.quick_start = true,
                    "autoreplay" => parsed.auto_replay = true,
                    "benchmark" => parsed.benchmark_mode = true,
                    "seed" | "netminplayers" | "playstats" | "forcebenchmark" | "nomusic"
                    | "nosizzle" | "noshaders" | "particleedit" | "scriptdebug" | "noshellanim"
                    | "wincursors" | "constantdebug" | "showteamdot" | "nomovecamera"
                    | "nodraw" | "jumptoframe" => {}
                    "server" => parsed.server_mode = true,
                    "client" => parsed.client_mode = true,
                    "port" => {
                        if let Some(v) = value {
                            let parsed_port = Self::parse_startup_port(&v);
                            if parsed_port == 0 {
                                warn!(
                                    "Ignoring invalid port value '{}'; startup will use default port behavior",
                                    v
                                );
                                parsed.network_port = None;
                            } else {
                                parsed.network_port = Some(parsed_port);
                            }
                        }
                    }
                    "host" => parsed.network_host = value,
                    "displaydebug" | "display_debug" => {
                        parsed.display_debug_overlay = true;
                        Self::store_option_aliases(
                            &mut parsed.options,
                            &["displaydebug", "display_debug"],
                            &value,
                        );
                    }
                    "integrationdiagnostics"
                    | "integration_diagnostics"
                    | "integrationdiag"
                    | "integrationdiagnostic"
                    | "integrationdiagostics"
                    | "integrationdiagdebug" => {
                        parsed.integration_diagnostics = true;
                        Self::store_option_aliases(
                            &mut parsed.options,
                            &[
                                "integrationdiagnostics",
                                "integration_diagnostics",
                                "integrationdiag",
                                "integrationdiagnostic",
                                "integrationdiagostics",
                                "integrationdiagdebug",
                            ],
                            &value,
                        );
                    }
                    "dx" => {
                        parsed.dx_stack_dump = true;
                        Self::store_option_aliases(&mut parsed.options, &["dx"], &value);
                    }
                    "smoke-test" | "smoketest" => {
                        parsed.smoke_test = true;
                        Self::store_option_aliases(
                            &mut parsed.options,
                            &["smoke-test", "smoketest"],
                            &value,
                        );
                    }
                    "buildmapcache" | "buildcache" => {
                        Self::store_option_aliases(
                            &mut parsed.options,
                            &["buildmapcache", "buildcache"],
                            &value,
                        );
                    }
                    "updateimages" | "updatedds" => {
                        Self::store_option_aliases(
                            &mut parsed.options,
                            &["updateimages", "updatedds"],
                            &value,
                        );
                    }
                    "nologo" | "nointro" => {
                        Self::store_option_aliases(
                            &mut parsed.options,
                            &["nologo", "nointro"],
                            &value,
                        );
                    }
                    _ => {
                        // Unknown option, log but don't fail
                        warn!("Unknown command line option: {}", option);
                    }
                }
            } else {
                // Positional argument
                parsed.positional_args.push(arg.clone());
            }

            i += 1;
        }

        // Validate arguments
        parsed.validate()?;

        info!("Command line parsed successfully");
        if !parsed.options.is_empty() {
            info!("Command line options: {:?}", parsed.options);
        }

        Ok(parsed)
    }

    /// Parse a single option and its value
    fn parse_option(
        arg: &str,
        args: &[String],
        index: &mut usize,
    ) -> Result<(String, Option<String>)> {
        let option_name = arg.trim_start_matches('-');

        // Check if this is a combined argument like -width=1024
        if let Some(equals_pos) = option_name.find('=') {
            let name = option_name[..equals_pos].to_ascii_lowercase();
            let value = option_name[equals_pos + 1..].to_string();
            Ok((name, Some(value)))
        } else {
            let name = option_name.to_ascii_lowercase();

            // Value-taking C++ options consume the next token even if it looks like
            // another flag; non-value flags stop here.
            if !Self::option_takes_value(&name) {
                return Ok((name, None));
            }

            // C++ parsers consume the next token for value-taking options even if it
            // looks like another flag; that preserves signed numbers like "-1".
            if *index + 1 < args.len() && !args[*index + 1].starts_with('-') {
                *index += 1;
                let value = args[*index].clone();
                Ok((name, Some(value)))
            } else if *index + 1 < args.len() {
                *index += 1;
                let value = args[*index].clone();
                Ok((name, Some(value)))
            } else {
                // Flag without value
                Ok((name, None))
            }
        }
    }

    fn option_takes_value(option: &str) -> bool {
        matches!(
            option,
            "width"
                | "height"
                | "xres"
                | "yres"
                | "file"
                | "map"
                | "mod"
                | "player"
                | "playername"
                | "lang"
                | "language"
                | "replay"
                | "config"
                | "loglevel"
                | "port"
                | "host"
                | "benchmark"
                | "seed"
                | "netminplayers"
                | "playstats"
                | "fps"
                | "shellmap"
                | "jumptoframe"
        )
    }

    fn parse_startup_dimension(value: &str) -> u32 {
        value
            .trim()
            .parse::<i32>()
            .ok()
            .map(|parsed| parsed.max(0) as u32)
            .unwrap_or(0)
    }

    fn parse_startup_port(value: &str) -> u16 {
        value
            .trim()
            .parse::<i32>()
            .ok()
            .map(|parsed| parsed.clamp(0, u16::MAX as i32) as u16)
            .unwrap_or(0)
    }

    fn store_option_aliases(
        options: &mut HashMap<String, Option<String>>,
        aliases: &[&str],
        value: &Option<String>,
    ) {
        for alias in aliases {
            options.insert((*alias).to_string(), value.clone());
        }
    }

    fn resolve_mod_path(candidate: &str) -> Option<ResolvedModPath> {
        let candidate = candidate.trim().trim_matches('"');
        if candidate.is_empty() {
            return None;
        }

        let resolved_path = if Self::is_path_rooted(candidate) {
            PathBuf::from(candidate)
        } else {
            let user_data_dir = runtime_global_data::read().get_user_data_dir().to_string();
            if user_data_dir.is_empty() {
                PathBuf::from(candidate)
            } else {
                PathBuf::from(user_data_dir).join(candidate)
            }
        };

        let metadata = fs::metadata(&resolved_path).ok()?;
        let resolved_string = resolved_path.to_string_lossy().into_owned();

        if metadata.is_dir() {
            let mod_dir = Self::ensure_directory_trailing_separator(resolved_string);
            Some(ResolvedModPath {
                mod_dir: Some(mod_dir.clone()),
                mod_big: None,
                active_mod: mod_dir,
            })
        } else {
            Some(ResolvedModPath {
                mod_dir: None,
                mod_big: Some(resolved_string.clone()),
                active_mod: resolved_string,
            })
        }
    }

    fn is_path_rooted(candidate: &str) -> bool {
        candidate.contains(':') || candidate.starts_with('/') || candidate.starts_with('\\')
    }

    fn ensure_directory_trailing_separator(mut path: String) -> String {
        if !path.ends_with('/') && !path.ends_with('\\') {
            path.push(std::path::MAIN_SEPARATOR);
        }
        path
    }

    /// Validate command line arguments for consistency
    fn validate(&self) -> Result<()> {
        if self.server_mode && self.client_mode {
            return Err(anyhow::anyhow!("Cannot specify both -server and -client"));
        }

        if self.no_video && !self.server_mode {
            warn!("-novideo specified but not in server mode - video will still be initialized");
        }

        // Validate file paths exist if specified
        if let Some(ref replay_file) = self.replay_file {
            if !replay_file.exists() {
                warn!(
                    "Replay file does not exist: {:?} (continuing; runtime will handle failure)",
                    replay_file
                );
            }
        }

        if let Some(ref config_file) = self.config_file {
            if !config_file.exists() {
                warn!(
                    "Config file does not exist: {:?} (continuing without config override)",
                    config_file
                );
            }
        }

        // Validate network options
        if let Some(port) = self.network_port {
            if port == 0 {
                warn!("Network port resolved to 0; continuing with runtime/default port behavior");
            }
        }

        Ok(())
    }

    /// Check if a specific option was provided
    pub fn has_option(&self, option: &str) -> bool {
        let key = option.to_ascii_lowercase();
        self.options.contains_key(&key)
    }

    /// Returns the final explicit startup window mode based on command line order.
    pub fn last_window_mode_override(&self) -> Option<bool> {
        self.window_mode_override
    }

    /// Get the value of a specific option
    pub fn get_option_value(&self, option: &str) -> Option<&String> {
        let key = option.to_ascii_lowercase();
        self.options.get(&key).and_then(|v| v.as_ref())
    }

    /// Get display resolution from command line or defaults
    pub fn get_resolution(&self) -> (u32, u32) {
        let default_width = if self.fullscreen { 1920 } else { 1280 };
        let default_height = if self.fullscreen { 1080 } else { 800 };

        (
            self.width.unwrap_or(default_width),
            self.height.unwrap_or(default_height),
        )
    }

    /// Check if the game should start in developer mode
    pub fn is_developer_mode(&self) -> bool {
        self.developer_mode
    }

    /// Whether the legacy `-displayDebug` overlay should be shown.
    pub fn wants_debug_overlay(&self) -> bool {
        self.display_debug_overlay || self.is_developer_mode()
    }

    /// Whether the integration diagnostics bridge should be enabled.
    pub fn wants_integration_diagnostics(&self) -> bool {
        self.integration_diagnostics
    }

    /// Whether the early DX bootstrap path should run.
    pub fn wants_dx_stack_dump(&self) -> bool {
        self.dx_stack_dump
    }

    pub fn wants_smoke_test(&self) -> bool {
        self.smoke_test
    }

    /// Emit the C++-style DX stack dump and return immediately.
    pub fn emit_dx_stack_dump(&self) {
        Self::emit_dx_stack_dump_from_args(&self.raw_args);
    }

    /// Emit the C++-style DX stack dump from a raw argv slice.
    pub fn emit_dx_stack_dump_from_args(args: &[String]) {
        eprintln!("\n--- DX STACK DUMP");
        for token in args.iter().skip(2) {
            let trimmed = token.trim();
            let trimmed = trimmed
                .strip_prefix("0x")
                .or_else(|| trimmed.strip_prefix("0X"))
                .unwrap_or(trimmed);

            match u64::from_str_radix(trimmed, 16) {
                Ok(pc) => {
                    eprintln!("0x{pc:x} - {token}");
                }
                Err(_) => {
                    eprintln!("{token}");
                }
            }
        }
        eprintln!("\n--- END OF DX STACK DUMP");
    }

    /// Get the effective log level
    pub fn get_log_level(&self) -> String {
        self.log_level.clone().unwrap_or_else(|| "info".to_string())
    }

    /// Print help information
    pub fn print_help() {
        println!("Command & Conquer Generals Zero Hour - Rust Edition");
        println!();
        println!("USAGE:");
        println!("    generals [OPTIONS]");
        println!();
        println!("OPTIONS:");
        println!("    -win, -windowed, -w    Run in windowed mode");
        println!("    -fullscreen, -f        Run in fullscreen mode");
        println!("    -width <WIDTH>         Set window/screen width");
        println!("    -height <HEIGHT>       Set window/screen height");
        println!("    -map <MAP>             Load specific map");
        println!("    -mod <MOD>             Load specific mod");
        println!("    -player <NAME>         Set player name");
        println!("    -lang <LANGUAGE>       Set language");
        println!("    -replay <FILE>         Play replay file");
        println!("    -config <FILE>         Use specific config file");
        println!("    -loglevel <LEVEL>      Set log level (error, warn, info, debug, trace)");
        println!("    -noaudio               Disable audio system");
        println!("    -novideo               Disable video system (server mode only)");
        println!("    -dev, -developer       Enable developer mode");
        println!("    -quickstart            Skip intro videos and menus");
        println!("    -autoreplay            Automatically replay last game");
        println!("    -benchmark             Run in benchmark mode");
        println!("    -displayDebug          Show the legacy debug/diagnostics overlay");
        println!("    -integrationDiagnostics Enable WW3D integration telemetry bridge (requires feature)");
        println!("    -smoke-test           Boot to the main menu and exit successfully");
        println!("    -server                Run as dedicated server");
        println!("    -client                Run as client");
        println!("    -port <PORT>           Network port (default: 8086)");
        println!("    -host <HOST>           Network host to connect to");
        println!("    -help, -h              Show this help message");
        println!();
        println!("EXAMPLES:");
        println!("    generals -windowed -width 1024 -height 768");
        println!("    generals -fullscreen -map \"GLA02\"");
        println!("    generals -server -port 8087");
        println!("    generals -client -host 192.168.1.100");
    }

    /// Check if help was requested
    pub fn wants_help(&self) -> bool {
        self.has_option("help") || self.has_option("h")
    }
}

/// Global command line arguments
static COMMAND_LINE_ARGS: std::sync::LazyLock<std::sync::Mutex<Option<CommandLineArgs>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

/// Initialize command line parsing system
pub fn initialize_command_line() -> Result<CommandLineArgs> {
    let args = CommandLineArgs::parse()?;

    // Store globally for access by other systems
    {
        let mut global_args = COMMAND_LINE_ARGS.lock().unwrap_or_else(|e| e.into_inner());
        *global_args = Some(args.clone());
    }

    info!("Command line system initialized");
    Ok(args)
}

/// Get the global command line arguments
pub fn get_command_line_args() -> Option<CommandLineArgs> {
    COMMAND_LINE_ARGS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Check if a specific command line option was provided
pub fn has_command_line_option(option: &str) -> bool {
    if let Some(args) = get_command_line_args() {
        args.has_option(option)
    } else {
        false
    }
}

/// Get a command line option value
pub fn get_command_line_option(option: &str) -> Option<String> {
    if let Some(args) = get_command_line_args() {
        args.get_option_value(option).cloned()
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedModPath {
    mod_dir: Option<String>,
    mod_big: Option<String>,
    active_mod: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static GLOBAL_DATA_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn with_runtime_global_data_restored<F: FnOnce()>(f: F) {
        let _guard = GLOBAL_DATA_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let snapshot = game_engine::common::global_data::read().clone();
        f();
        *game_engine::common::global_data::write() = snapshot;
    }

    fn create_temp_test_dir(prefix: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "generals_main_{prefix}_{}_{}",
            std::process::id(),
            nonce
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_basic_parsing() {
        let args = vec![
            "generals".to_string(),
            "-windowed".to_string(),
            "-width".to_string(),
            "1024".to_string(),
        ];

        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        assert!(parsed.windowed);
        assert_eq!(parsed.width, Some(1024));
    }

    #[test]
    fn test_combined_argument_parsing() {
        let args = vec![
            "generals".to_string(),
            "-width=800".to_string(),
            "-height=600".to_string(),
        ];

        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        assert_eq!(parsed.width, Some(800));
        assert_eq!(parsed.height, Some(600));
    }

    #[test]
    fn test_flag_parsing() {
        let args = vec![
            "generals".to_string(),
            "-dev".to_string(),
            "-noaudio".to_string(),
        ];

        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        assert!(parsed.developer_mode);
        assert!(parsed.no_audio);
    }

    #[test]
    fn test_smoke_test_flag_parsing() {
        let args = vec!["generals".to_string(), "--smoke-test".to_string()];

        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        assert!(parsed.wants_smoke_test());
        assert!(parsed.has_option("smoke-test"));
        assert!(parsed.has_option("smoketest"));
    }

    #[test]
    fn test_win_alias_parsing() {
        let args = vec!["generals".to_string(), "-win".to_string()];

        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        assert!(parsed.windowed);
        assert!(parsed.has_option("win"));
        assert!(parsed.has_option("windowed"));
        assert!(parsed.has_option("w"));
    }

    #[test]
    fn test_resolution_defaults() {
        let args = CommandLineArgs::default();
        let (width, height) = args.get_resolution();
        assert_eq!(width, 1280);
        assert_eq!(height, 800);
    }

    #[test]
    fn test_windowed_and_fullscreen_can_coexist() {
        let args = vec![
            "generals".to_string(),
            "-fullscreen".to_string(),
            "-win".to_string(),
        ];

        let result = CommandLineArgs::parse_from_args(args);
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert!(parsed.windowed);
        assert!(parsed.fullscreen);
    }

    #[test]
    fn test_last_window_mode_flag_wins() {
        let first = vec![
            "generals".to_string(),
            "-win".to_string(),
            "-fullscreen".to_string(),
        ];
        let second = vec![
            "generals".to_string(),
            "-fullscreen".to_string(),
            "-win".to_string(),
        ];

        let parsed_first = CommandLineArgs::parse_from_args(first).unwrap();
        let parsed_second = CommandLineArgs::parse_from_args(second).unwrap();

        assert_eq!(parsed_first.last_window_mode_override(), Some(false));
        assert_eq!(parsed_second.last_window_mode_override(), Some(true));
    }

    #[test]
    fn test_xres_yres_flags_map_to_dimensions() {
        let args = vec![
            "generals".to_string(),
            "-xres".to_string(),
            "1024".to_string(),
            "-yres=768".to_string(),
        ];

        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        assert_eq!(parsed.width, Some(1024));
        assert_eq!(parsed.height, Some(768));
    }

    #[test]
    fn test_signed_value_options_consume_following_flag_style_tokens() {
        let args = vec![
            "generals".to_string(),
            "-seed".to_string(),
            "-1".to_string(),
            "-netMinPlayers".to_string(),
            "3".to_string(),
            "-playStats=7".to_string(),
            "-benchmark".to_string(),
            "11".to_string(),
            "-win".to_string(),
        ];

        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        assert_eq!(
            parsed.get_option_value("seed").map(String::as_str),
            Some("-1")
        );
        assert_eq!(
            parsed.get_option_value("netminplayers").map(String::as_str),
            Some("3")
        );
        assert_eq!(
            parsed.get_option_value("playstats").map(String::as_str),
            Some("7")
        );
        assert_eq!(
            parsed.get_option_value("benchmark").map(String::as_str),
            Some("11")
        );
        assert!(parsed.windowed);
    }

    #[test]
    fn test_startup_parity_flags_are_recognized_case_insensitively() {
        let args = vec![
            "generals".to_string(),
            "-NoShellAnim".to_string(),
            "-winCursors".to_string(),
            "-constantDebug".to_string(),
            "-showTeamDot".to_string(),
            "-nomusic".to_string(),
            "-nosizzle".to_string(),
            "-noshaders".to_string(),
            "-particleEdit".to_string(),
            "-scriptDebug".to_string(),
            "-forceBenchmark".to_string(),
            "-nomovecamera".to_string(),
        ];

        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        assert!(parsed.has_option("noshellanim"));
        assert!(parsed.has_option("wincursors"));
        assert!(parsed.has_option("constantdebug"));
        assert!(parsed.has_option("showteamdot"));
        assert!(parsed.has_option("nomusic"));
        assert!(parsed.has_option("nosizzle"));
        assert!(parsed.has_option("noshaders"));
        assert!(parsed.has_option("particleedit"));
        assert!(parsed.has_option("scriptdebug"));
        assert!(parsed.has_option("forcebenchmark"));
        assert!(parsed.has_option("nomovecamera"));
    }

    #[test]
    fn test_dx_flag_enables_stack_dump_mode() {
        let args = vec![
            "generals".to_string(),
            "-DX".to_string(),
            "0x1000".to_string(),
            "2000".to_string(),
        ];

        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        assert!(parsed.wants_dx_stack_dump());
        assert!(parsed.has_option("dx"));
        assert_eq!(
            parsed.positional_args,
            vec!["0x1000".to_string(), "2000".to_string()]
        );
        assert!(!CommandLineArgs::wants_dx_stack_dump_from_args(&[
            "generals".to_string(),
            "-DX".to_string(),
        ]));
        assert!(CommandLineArgs::wants_dx_stack_dump_from_args(&[
            "generals".to_string(),
            "-DX".to_string(),
            "0x1000".to_string(),
        ]));
    }

    #[test]
    fn test_startup_args_are_capped_to_twenty_entries() {
        let mut args = vec!["generals".to_string()];
        for index in 1..25 {
            args.push(format!("arg{index}"));
        }

        let capped = CommandLineArgs::limit_startup_args(args);
        assert_eq!(capped.len(), MAX_STARTUP_ARGS);
        assert_eq!(capped.last().map(String::as_str), Some("arg19"));
    }

    #[test]
    fn test_file_path_is_normalized_to_long_form() {
        let args = vec![
            "generals".to_string(),
            "-FILE".to_string(),
            "Maps\\ShellMap1.map".to_string(),
        ];

        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        assert_eq!(
            parsed.map_name.as_deref(),
            Some("Maps\\ShellMap1\\ShellMap1.map")
        );
        assert_eq!(
            parsed.get_option_value("file").map(String::as_str),
            Some("Maps\\ShellMap1\\ShellMap1.map")
        );
    }

    #[test]
    fn test_file_path_already_long_form_is_preserved() {
        let args = vec![
            "generals".to_string(),
            "-FILE".to_string(),
            "Maps\\Tournament Desert\\Tournament Desert.map".to_string(),
        ];

        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        assert_eq!(
            parsed.map_name.as_deref(),
            Some("Maps\\Tournament Desert\\Tournament Desert.map")
        );
        assert_eq!(
            parsed.get_option_value("file").map(String::as_str),
            Some("Maps\\Tournament Desert\\Tournament Desert.map")
        );
    }

    #[test]
    fn test_intro_aliases_are_stored_case_insensitively() {
        let args = vec!["generals".to_string(), "-NoIntro".to_string()];

        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        assert!(parsed.has_option("nologo"));
        assert!(parsed.has_option("nointro"));
    }

    #[test]
    fn test_missing_replay_file_does_not_fail_parse() {
        let args = vec![
            "generals".to_string(),
            "-replay".to_string(),
            "__missing_replay_file__.rep".to_string(),
        ];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        assert_eq!(
            parsed.replay_file.as_deref(),
            Some(Path::new("__missing_replay_file__.rep"))
        );
    }

    #[test]
    fn test_missing_config_file_does_not_fail_parse() {
        let args = vec![
            "generals".to_string(),
            "-config".to_string(),
            "__missing_override__.cfg".to_string(),
        ];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        assert_eq!(
            parsed.config_file.as_deref(),
            Some(Path::new("__missing_override__.cfg"))
        );
    }

    #[test]
    fn test_relative_mod_directory_resolves_against_user_data_dir() {
        with_runtime_global_data_restored(|| {
            let temp_root = create_temp_test_dir("mod_dir");
            let user_data_dir = temp_root.join("UserData");
            let mod_dir = user_data_dir.join("Mods").join("TestMod");
            fs::create_dir_all(&mod_dir).unwrap();

            {
                let mut global = game_engine::common::global_data::write();
                global.set_user_data_dir(user_data_dir.to_string_lossy().into_owned());
            }

            let args = vec![
                "generals".to_string(),
                "-mod".to_string(),
                Path::new("Mods")
                    .join("TestMod")
                    .to_string_lossy()
                    .into_owned(),
            ];
            let parsed = CommandLineArgs::parse_from_args(args).unwrap();
            let expected = CommandLineArgs::ensure_directory_trailing_separator(
                mod_dir.to_string_lossy().into_owned(),
            );

            assert_eq!(parsed.mod_dir.as_deref(), Some(expected.as_str()));
            assert!(parsed.mod_big.is_none());
            assert_eq!(parsed.mod_name.as_deref(), Some(expected.as_str()));

            let _ = fs::remove_dir_all(temp_root);
        });
    }

    #[test]
    fn test_mod_file_sets_mod_big() {
        with_runtime_global_data_restored(|| {
            let temp_root = create_temp_test_dir("mod_big");
            let user_data_dir = temp_root.join("UserData");
            let mod_file = user_data_dir.join("Mods").join("TestMod.big");
            fs::create_dir_all(mod_file.parent().unwrap()).unwrap();
            fs::write(&mod_file, b"mod archive").unwrap();

            {
                let mut global = game_engine::common::global_data::write();
                global.set_user_data_dir(user_data_dir.to_string_lossy().into_owned());
            }

            let args = vec![
                "generals".to_string(),
                "-mod".to_string(),
                Path::new("Mods")
                    .join("TestMod.big")
                    .to_string_lossy()
                    .into_owned(),
            ];
            let parsed = CommandLineArgs::parse_from_args(args).unwrap();
            let expected = mod_file.to_string_lossy().into_owned();

            assert!(parsed.mod_dir.is_none());
            assert_eq!(parsed.mod_big.as_deref(), Some(expected.as_str()));
            assert_eq!(parsed.mod_name.as_deref(), Some(expected.as_str()));

            let _ = fs::remove_dir_all(temp_root);
        });
    }

    #[test]
    fn test_missing_mod_path_does_not_set_mod_fields() {
        with_runtime_global_data_restored(|| {
            let temp_root = create_temp_test_dir("missing_mod");
            let user_data_dir = temp_root.join("UserData");

            {
                let mut global = game_engine::common::global_data::write();
                global.set_user_data_dir(user_data_dir.to_string_lossy().into_owned());
            }

            let args = vec![
                "generals".to_string(),
                "-mod".to_string(),
                Path::new("Mods")
                    .join("Missing.big")
                    .to_string_lossy()
                    .into_owned(),
            ];
            let parsed = CommandLineArgs::parse_from_args(args).unwrap();

            assert!(parsed.mod_dir.is_none());
            assert!(parsed.mod_big.is_none());
            assert!(parsed.mod_name.is_none());

            let _ = fs::remove_dir_all(temp_root);
        });
    }
}
