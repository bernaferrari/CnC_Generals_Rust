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
use std::path::PathBuf;

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
        }
    }
}

impl CommandLineArgs {
    /// Parse command line arguments from environment
    pub fn parse() -> Result<Self> {
        let args: Vec<String> = env::args().collect();
        Self::parse_from_args(args)
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
                    "windowed" | "w" => parsed.windowed = true,
                    "fullscreen" | "f" => parsed.fullscreen = true,
                    "width" => {
                        if let Some(v) = value {
                            parsed.width = Some(v.parse().context("Invalid width value")?);
                        }
                    }
                    "height" => {
                        if let Some(v) = value {
                            parsed.height = Some(v.parse().context("Invalid height value")?);
                        }
                    }
                    "map" => parsed.map_name = value,
                    "mod" => parsed.mod_name = value,
                    "player" | "playername" => parsed.player_name = value,
                    "lang" | "language" => parsed.language = value,
                    "replay" => {
                        if let Some(v) = value {
                            parsed.replay_file = Some(PathBuf::from(v));
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
                    "dev" | "developer" => parsed.developer_mode = true,
                    "quickstart" => parsed.quick_start = true,
                    "autoreplay" => parsed.auto_replay = true,
                    "benchmark" => parsed.benchmark_mode = true,
                    "server" => parsed.server_mode = true,
                    "client" => parsed.client_mode = true,
                    "port" => {
                        if let Some(v) = value {
                            parsed.network_port = Some(v.parse().context("Invalid port value")?);
                        }
                    }
                    "host" => parsed.network_host = value,
                    "displaydebug" | "display_debug" | "displayDebug" => {
                        parsed.display_debug_overlay = true;
                    }
                    "integrationdiagnostics"
                    | "integration_diagnostics"
                    | "integrationdiag"
                    | "integrationDiagnostic"
                    | "integrationdiagostics"
                    | "integrationdiagDebug" => {
                        parsed.integration_diagnostics = true;
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
            let name = option_name[..equals_pos].to_string();
            let value = option_name[equals_pos + 1..].to_string();
            Ok((name, Some(value)))
        } else {
            // Check if next argument is the value (not starting with -)
            if *index + 1 < args.len() && !args[*index + 1].starts_with('-') {
                *index += 1;
                let value = args[*index].clone();
                Ok((option_name.to_string(), Some(value)))
            } else {
                // Flag without value
                Ok((option_name.to_string(), None))
            }
        }
    }

    /// Validate command line arguments for consistency
    fn validate(&self) -> Result<()> {
        // Check for conflicting options
        if self.windowed && self.fullscreen {
            return Err(anyhow::anyhow!(
                "Cannot specify both -windowed and -fullscreen"
            ));
        }

        if self.server_mode && self.client_mode {
            return Err(anyhow::anyhow!("Cannot specify both -server and -client"));
        }

        if self.no_video && !self.server_mode {
            warn!("-novideo specified but not in server mode - video will still be initialized");
        }

        // Validate file paths exist if specified
        if let Some(ref replay_file) = self.replay_file {
            if !replay_file.exists() {
                return Err(anyhow::anyhow!(
                    "Replay file does not exist: {:?}",
                    replay_file
                ));
            }
        }

        if let Some(ref config_file) = self.config_file {
            if !config_file.exists() {
                return Err(anyhow::anyhow!(
                    "Config file does not exist: {:?}",
                    config_file
                ));
            }
        }

        // Validate network options
        if let Some(port) = self.network_port {
            if port == 0 {
                return Err(anyhow::anyhow!("Invalid network port: {}", port));
            }
        }

        Ok(())
    }

    /// Check if a specific option was provided
    pub fn has_option(&self, option: &str) -> bool {
        self.options.contains_key(option)
    }

    /// Get the value of a specific option
    pub fn get_option_value(&self, option: &str) -> Option<&String> {
        self.options.get(option).and_then(|v| v.as_ref())
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
        println!("    -windowed, -w          Run in windowed mode");
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
        let mut global_args = COMMAND_LINE_ARGS.lock().unwrap();
        *global_args = Some(args.clone());
    }

    info!("Command line system initialized");
    Ok(args)
}

/// Get the global command line arguments
pub fn get_command_line_args() -> Option<CommandLineArgs> {
    COMMAND_LINE_ARGS.lock().unwrap().clone()
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_resolution_defaults() {
        let args = CommandLineArgs::default();
        let (width, height) = args.get_resolution();
        assert_eq!(width, 1280);
        assert_eq!(height, 800);
    }

    #[test]
    fn test_conflicting_options() {
        let args = vec![
            "generals".to_string(),
            "-windowed".to_string(),
            "-fullscreen".to_string(),
        ];

        let result = CommandLineArgs::parse_from_args(args);
        assert!(result.is_err());
    }
}
