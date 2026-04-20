////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// CommandLine.rs
// The command-line interface
// Author: Matthew D. Campbell, September 2001

use once_cell::sync::OnceCell;
use std::env;
use std::path::Path;
use std::sync::Mutex;

use log::debug;

#[cfg(feature = "debug_crc")]
use crate::common::crc_debug;
use crate::common::global_data;

/// Global debug flags and settings
#[derive(Debug, Clone, Copy)]
pub struct DebugSettings {
    pub ignore_sync_errors: bool,
    pub preserve_fpu: i32,
}

impl Default for DebugSettings {
    fn default() -> Self {
        Self {
            ignore_sync_errors: false,
            preserve_fpu: 0,
        }
    }
}

/// CRC Debug settings
#[cfg(feature = "debug_crc")]
#[derive(Debug, Clone)]
pub struct CrcDebugSettings {
    pub first_frame_to_log: i32,
    pub last_frame_to_log: u32,
    pub keep_crc_saves: bool,
    pub crc_module_data_from_logic: bool,
    pub crc_module_data_from_client: bool,
    pub verify_client_crc: bool,
    pub client_deep_crc: bool,
    pub log_object_crcs: bool,
    pub net_crc_interval: i32,
    pub replay_crc_interval: i32,
}

#[cfg(feature = "debug_crc")]
impl Default for CrcDebugSettings {
    fn default() -> Self {
        Self {
            first_frame_to_log: -1,
            last_frame_to_log: 0xffffffff,
            keep_crc_saves: false,
            crc_module_data_from_logic: false,
            crc_module_data_from_client: false,
            verify_client_crc: false,
            client_deep_crc: false,
            log_object_crcs: false,
            net_crc_interval: 1,
            replay_crc_interval: 1,
        }
    }
}

/// Global game data settings that can be modified by command line
#[derive(Debug, Clone)]
pub struct WritableGlobalData {
    pub windowed: bool,
    pub music_on: bool,
    pub video_on: bool,
    pub audio_on: bool,
    pub speech_on: bool,
    pub sounds_on: bool,
    pub disable_scripted_input_disabling: bool,
    pub disable_camera_fade: bool,
    pub disable_military_caption: bool,
    pub no_draw: bool,
    pub x_resolution: i32,
    pub y_resolution: i32,
    pub latency_average: i32,
    pub latency_amplitude: i32,
    pub latency_period: i32,
    pub latency_noise: i32,
    pub packet_loss: i32,
    pub terrain_lod: i32,
    pub enable_dynamic_lod: bool,
    pub enable_static_lod: bool,
    pub using_water_track_editor: bool,
    pub frames_per_second_limit: i32,
    pub use_camera_constraints: bool,
    pub wireframe: bool,
    pub show_collision_extents: bool,
    pub show_client_physics: bool,
    pub show_terrain_normals: bool,
    pub state_machine_debug: bool,
    pub jabber_on: bool,
    pub munkee_on: bool,
    pub display_debug: bool,
    pub preload_assets: bool,
    pub preload_everything: bool,
    pub preload_report: bool,
    pub v_tune: bool,
    pub use_fx: bool,
    pub shroud_on: bool,
    pub force_benchmark: bool,
    pub disable_camera_movement: bool,
    pub script_debug: bool,
    pub particle_edit: bool,
    pub win_cursors: bool,
    pub build_map_cache: bool,
    pub shell_map_on: bool,
    pub shell_map_name: String,
    pub chip_set_type: i32,
    pub play_intro: bool,
    pub after_intro: bool,
    pub play_sizzle: bool,
    pub allow_exit_out_of_movies: bool,
    pub animate_windows: bool,
    pub constant_debug_update: bool,
    pub extra_logging: bool,
    pub show_team_dot: bool,
    pub allow_unselectable_selection: bool,
    pub fixed_seed: i32,
    pub incremental_agp_buf: bool,
    pub net_min_players: i32,
    pub play_stats: i32,
    pub load_screen_demo: bool,
    pub save_stats: bool,
    pub base_stats_dir: String,
    pub save_all_stats: bool,
    pub use_local_motd: bool,
    pub motd_path: String,
    pub debug_camera: bool,
    pub benchmark_timer: i32,
    pub stats_interval: i32,
    pub dump_stats_at_interval: bool,
    pub debug_ignore_asserts: bool,
    pub debug_ignore_stack_trace: bool,
    pub use_fps_limit: bool,
    pub dump_asset_usage: bool,
    pub should_update_tga_to_dds: bool,
    pub map_name: String,
    pub initial_file: String,
    pub mod_dir: String,
    pub mod_big: String,
    pub use_shadow_volumes: bool,
    pub use_shadow_decals: bool,
}

impl Default for WritableGlobalData {
    fn default() -> Self {
        Self {
            windowed: false,
            music_on: true,
            video_on: true,
            audio_on: true,
            speech_on: true,
            sounds_on: true,
            disable_scripted_input_disabling: false,
            disable_camera_fade: false,
            disable_military_caption: false,
            no_draw: false,
            x_resolution: 1024,
            y_resolution: 768,
            latency_average: 0,
            latency_amplitude: 0,
            latency_period: 0,
            latency_noise: 0,
            packet_loss: 0,
            terrain_lod: 0, // TERRAIN_LOD_MIN would be defined elsewhere
            enable_dynamic_lod: true,
            enable_static_lod: true,
            using_water_track_editor: false,
            frames_per_second_limit: 30,
            use_camera_constraints: true,
            wireframe: false,
            show_collision_extents: false,
            show_client_physics: true,
            show_terrain_normals: false,
            state_machine_debug: false,
            jabber_on: false,
            munkee_on: false,
            display_debug: false,
            preload_assets: false,
            preload_everything: false,
            preload_report: false,
            v_tune: false,
            use_fx: true,
            shroud_on: true,
            force_benchmark: false,
            disable_camera_movement: false,
            script_debug: false,
            particle_edit: false,
            win_cursors: false,
            build_map_cache: false,
            shell_map_on: true,
            shell_map_name: String::new(),
            chip_set_type: 0,
            play_intro: true,
            after_intro: false,
            play_sizzle: true,
            allow_exit_out_of_movies: false,
            animate_windows: true,
            constant_debug_update: false,
            extra_logging: false,
            show_team_dot: false,
            allow_unselectable_selection: false,
            fixed_seed: 0,
            incremental_agp_buf: false,
            net_min_players: 2,
            play_stats: 0,
            load_screen_demo: false,
            save_stats: false,
            base_stats_dir: String::new(),
            save_all_stats: false,
            use_local_motd: false,
            motd_path: String::new(),
            debug_camera: false,
            benchmark_timer: 0,
            stats_interval: 0,
            dump_stats_at_interval: false,
            debug_ignore_asserts: false,
            debug_ignore_stack_trace: false,
            use_fps_limit: true,
            dump_asset_usage: false,
            should_update_tga_to_dds: false,
            map_name: String::new(),
            initial_file: String::new(),
            mod_dir: String::new(),
            mod_big: String::new(),
            use_shadow_volumes: true,
            use_shadow_decals: true,
        }
    }
}

/// Command line parser
pub struct CommandLineParser {
    global_data: WritableGlobalData,
    debug_settings: DebugSettings,
    #[cfg(feature = "debug_crc")]
    crc_debug_settings: CrcDebugSettings,
}

impl CommandLineParser {
    pub fn new() -> Self {
        Self {
            global_data: WritableGlobalData::default(),
            debug_settings: DebugSettings::default(),
            #[cfg(feature = "debug_crc")]
            crc_debug_settings: CrcDebugSettings::default(),
        }
    }

    /// Build a parser seeded from current runtime global-data values.
    ///
    /// This preserves INI/runtime settings and applies only command-line deltas.
    pub fn from_runtime_global_data() -> Self {
        let (writable, debug_settings) = {
            let data = global_data::read();
            (data.writable.clone(), data.debug)
        };

        Self {
            global_data: writable,
            debug_settings,
            #[cfg(feature = "debug_crc")]
            crc_debug_settings: CrcDebugSettings::default(),
        }
    }

    /// Convert short map path to long map path
    ///
    /// Matches C++ CommandLine.cpp map path conversion logic.
    /// Returns the original map_name if conversion fails rather than panicking.
    fn convert_short_map_path_to_long_map_path(&self, map_name: &str) -> String {
        if !map_name.contains('\\') && !map_name.contains('/') {
            eprintln!("WARN: Invalid map name format '{}', using as-is", map_name);
            return map_name.to_string();
        }

        let path_parts: Vec<&str> = map_name.split(['\\', '/']).collect();
        let mut actual_path = String::new();
        let mut token = String::new();

        for part in path_parts {
            if part.ends_with(".map") {
                // Remove .map extension
                token = part[..part.len() - 4].to_string();
                break;
            } else {
                actual_path.push_str(part);
                actual_path.push('\\');
            }
        }

        if token.is_empty() {
            eprintln!(
                "WARN: Could not parse map file from '{}', using as-is",
                map_name
            );
            return map_name.to_string();
        }

        actual_path.push_str(&token);
        actual_path.push('\\');
        actual_path.push_str(&token);
        actual_path.push_str(".map");

        actual_path
    }

    /// Parse -win flag
    fn parse_win(&mut self, _args: &[String]) -> usize {
        self.global_data.windowed = true;
        1
    }

    /// Parse -nomusic flag
    fn parse_no_music(&mut self, _args: &[String]) -> usize {
        self.global_data.music_on = false;
        1
    }

    /// Parse -novideo flag
    fn parse_no_video(&mut self, _args: &[String]) -> usize {
        self.global_data.video_on = false;
        1
    }

    /// Parse -FPUPreserve flag
    fn parse_fpu_preserve(&mut self, args: &[String]) -> usize {
        if args.len() > 1 {
            if let Ok(val) = args[1].parse::<i32>() {
                self.debug_settings.preserve_fpu = val;
            }
            2
        } else {
            1
        }
    }

    /// Parse -noaudio flag
    fn parse_no_audio(&mut self, _args: &[String]) -> usize {
        self.global_data.audio_on = false;
        self.global_data.speech_on = false;
        self.global_data.sounds_on = false;
        self.global_data.music_on = false;
        1
    }

    /// Parse -fullscreen flag
    fn parse_no_win(&mut self, _args: &[String]) -> usize {
        self.global_data.windowed = false;
        1
    }

    /// Parse -fullVersion flag
    fn parse_full_version(&mut self, args: &[String]) -> usize {
        if args.len() > 1 {
            // Would set version show setting here
            2
        } else {
            1
        }
    }

    /// Parse -noshadowvolumes flag
    fn parse_no_shadows(&mut self, _args: &[String]) -> usize {
        self.global_data.use_shadow_volumes = false;
        self.global_data.use_shadow_decals = false;
        1
    }

    /// Parse -map flag
    fn parse_map_name(&mut self, args: &[String]) -> usize {
        if args.len() > 1 {
            self.global_data.map_name = self.convert_short_map_path_to_long_map_path(&args[1]);
            2
        } else {
            1
        }
    }

    /// Parse -file flag
    fn parse_file(&mut self, args: &[String]) -> usize {
        if args.len() > 1 {
            self.global_data.initial_file = self.convert_short_map_path_to_long_map_path(&args[1]);
            2
        } else {
            1
        }
    }

    /// Parse -xres flag
    fn parse_x_res(&mut self, args: &[String]) -> usize {
        if args.len() > 1 {
            if let Ok(val) = args[1].parse::<i32>() {
                self.global_data.x_resolution = val;
            }
            2
        } else {
            1
        }
    }

    /// Parse -yres flag
    fn parse_y_res(&mut self, args: &[String]) -> usize {
        if args.len() > 1 {
            if let Ok(val) = args[1].parse::<i32>() {
                self.global_data.y_resolution = val;
            }
            2
        } else {
            1
        }
    }

    /// Parse -scriptDebug flag
    fn parse_script_debug(&mut self, _args: &[String]) -> usize {
        self.global_data.script_debug = true;
        self.global_data.win_cursors = true;
        1
    }

    /// Parse -particleEdit flag
    fn parse_particle_edit(&mut self, _args: &[String]) -> usize {
        self.global_data.particle_edit = true;
        self.global_data.win_cursors = true;
        self.global_data.windowed = true;
        1
    }

    /// Parse -buildmapcache flag
    fn parse_build_map_cache(&mut self, _args: &[String]) -> usize {
        self.global_data.build_map_cache = true;
        1
    }

    /// Parse -preload flag
    #[allow(dead_code)]
    fn parse_preload(&mut self, _args: &[String]) -> usize {
        self.global_data.preload_assets = true;
        1
    }

    /// Parse -nofx flag
    fn parse_no_fx(&mut self, _args: &[String]) -> usize {
        self.global_data.use_fx = false;
        1
    }

    /// Parse -fps / -maxfps flag
    fn parse_fps(&mut self, args: &[String]) -> usize {
        if args.len() > 1 {
            if let Ok(val) = args[1].parse::<i32>() {
                self.global_data.frames_per_second_limit = val.max(0);
                self.global_data.use_fps_limit = val > 0;
            }
            2
        } else {
            1
        }
    }

    /// Parse -resolution flag accepting width/height pair
    fn parse_resolution(&mut self, args: &[String]) -> usize {
        if args.len() > 2 {
            if let (Ok(width), Ok(height)) = (args[1].parse::<i32>(), args[2].parse::<i32>()) {
                self.global_data.x_resolution = width.max(0);
                self.global_data.y_resolution = height.max(0);
            }
            3
        } else {
            args.len()
        }
    }

    /// Parse -quickstart flag
    fn parse_quick_start(&mut self, args: &[String]) -> usize {
        #[cfg(any(feature = "debug", feature = "internal"))]
        {
            self.parse_no_logo(args);
        }
        #[cfg(not(any(feature = "debug", feature = "internal")))]
        {
            self.parse_no_sizzle(args);
        }
        self.parse_no_shell_map(args);
        self.parse_no_window_animation(args);
        1
    }

    /// Parse -nologo flag
    #[cfg(any(feature = "debug", feature = "internal"))]
    fn parse_no_logo(&mut self, _args: &[String]) -> usize {
        self.global_data.play_intro = false;
        self.global_data.after_intro = true;
        self.global_data.play_sizzle = false;
        1
    }

    /// Parse -nosizzle flag
    fn parse_no_sizzle(&mut self, _args: &[String]) -> usize {
        self.global_data.play_sizzle = false;
        1
    }

    /// Parse -noshellmap flag
    fn parse_no_shell_map(&mut self, _args: &[String]) -> usize {
        self.global_data.shell_map_on = false;
        1
    }

    /// Parse -noShellAnim flag
    fn parse_no_window_animation(&mut self, _args: &[String]) -> usize {
        self.global_data.animate_windows = false;
        1
    }

    /// Parse -mod flag
    fn parse_mod(&mut self, args: &[String]) -> usize {
        if args.len() > 1 {
            let mut mod_path = args[1].clone();

            // Check if it's a full path
            if !mod_path.contains(':') && !mod_path.starts_with('/') && !mod_path.starts_with('\\')
            {
                // Would append user data path here
                // mod_path = format!("{}{}", get_user_data_path(), args[1]);
            }

            if Path::new(&mod_path).exists() {
                if Path::new(&mod_path).is_dir() {
                    if !mod_path.ends_with('\\') && !mod_path.ends_with('/') {
                        mod_path.push('\\');
                    }
                    self.global_data.mod_dir = mod_path;
                } else {
                    self.global_data.mod_big = mod_path;
                }
            }
            2
        } else {
            1
        }
    }

    /// Main command line parsing function
    pub fn parse_command_line(&mut self, args: Vec<String>) {
        debug!("Command-line args: {}", args.join(" "));

        let mut i = 1; // Skip program name
        while i < args.len() {
            let arg = &args[i];
            let remaining_args = &args[i..];

            let consumed = match arg.as_str() {
                "-noshellmap" => self.parse_no_shell_map(remaining_args),
                "-win" | "-windowed" => self.parse_win(remaining_args),
                "-xres" => self.parse_x_res(remaining_args),
                "-yres" => self.parse_y_res(remaining_args),
                "-fullscreen" => self.parse_no_win(remaining_args),
                "-resolution" => self.parse_resolution(remaining_args),
                "-fps" | "-maxfps" => self.parse_fps(remaining_args),
                "-fullVersion" => self.parse_full_version(remaining_args),
                "-particleEdit" => self.parse_particle_edit(remaining_args),
                "-scriptDebug" => self.parse_script_debug(remaining_args),
                "-mod" => self.parse_mod(remaining_args),
                "-noshaders" => {
                    self.global_data.chip_set_type = 1; // force to a voodoo card
                    1
                }
                "-quickstart" => self.parse_quick_start(remaining_args),
                "-noaudio" => self.parse_no_audio(remaining_args),
                "-map" => self.parse_map_name(remaining_args),
                "-file" => self.parse_file(remaining_args),
                "-nomusic" => self.parse_no_music(remaining_args),
                "-novideo" => self.parse_no_video(remaining_args),
                "-FPUPreserve" => self.parse_fpu_preserve(remaining_args),
                "-buildmapcache" => self.parse_build_map_cache(remaining_args),
                "-noshadowvolumes" => self.parse_no_shadows(remaining_args),
                "-nofx" => self.parse_no_fx(remaining_args),
                #[cfg(any(feature = "debug", feature = "internal"))]
                "-nologo" => self.parse_no_logo(remaining_args),
                #[cfg(any(feature = "debug", feature = "internal"))]
                "-preload" => self.parse_preload(remaining_args),
                _ => 1,
            };

            i += consumed;
        }

        global_data::access::update_from_command_line(&self.global_data, &self.debug_settings);

        #[cfg(feature = "debug_crc")]
        crc_debug::apply_command_line_settings(&self.crc_debug_settings);

        // Load mods after parsing
        // self.load_mods();
    }

    /// Get the parsed global data
    pub fn get_global_data(&self) -> &WritableGlobalData {
        &self.global_data
    }

    /// Get mutable reference to global data
    pub fn get_global_data_mut(&mut self) -> &mut WritableGlobalData {
        &mut self.global_data
    }

    /// Get debug settings
    pub fn get_debug_settings(&self) -> &DebugSettings {
        &self.debug_settings
    }

    /// Get CRC debug settings
    #[cfg(feature = "debug_crc")]
    pub fn get_crc_debug_settings(&self) -> &CrcDebugSettings {
        &self.crc_debug_settings
    }
}

/// Parse command-line parameters
pub fn parse_command_line() -> CommandLineParser {
    let args: Vec<String> = env::args().collect();
    let mut parser = CommandLineParser::from_runtime_global_data();
    parser.parse_command_line(args);
    parser
}

/// Global static parser instance (equivalent to the C++ global variables)
static GLOBAL_PARSER: OnceCell<Mutex<CommandLineParser>> = OnceCell::new();

/// Initialize the global command line parser
pub fn initialize_command_line_parser() {
    let parser = parse_command_line();
    if GLOBAL_PARSER.set(Mutex::new(parser)).is_err() {
        if let Some(existing) = GLOBAL_PARSER.get() {
            let mut guard = existing.lock().expect("Global parser mutex poisoned");
            *guard = parse_command_line();
        }
    }
}

/// Get reference to global parser
pub fn get_global_parser() -> std::sync::MutexGuard<'static, CommandLineParser> {
    GLOBAL_PARSER
        .get()
        .expect("Command line parser not initialized")
        .lock()
        .expect("Global parser mutex poisoned")
}

/// Get mutable reference to global parser
pub fn get_global_parser_mut() -> std::sync::MutexGuard<'static, CommandLineParser> {
    get_global_parser()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_file_converts_short_map_path_into_initial_file() {
        let mut parser = CommandLineParser::new();
        parser.parse_command_line(vec![
            "game.exe".to_string(),
            "-file".to_string(),
            "Maps\\TestMap.map".to_string(),
        ]);

        assert_eq!(
            parser.get_global_data().initial_file,
            "Maps\\TestMap\\TestMap.map"
        );
    }

    #[test]
    fn parse_file_preserves_replay_path() {
        let mut parser = CommandLineParser::new();
        parser.parse_command_line(vec![
            "game.exe".to_string(),
            "-file".to_string(),
            "Replays\\TestReplay.rep".to_string(),
        ]);

        assert_eq!(
            parser.get_global_data().initial_file,
            "Replays\\TestReplay.rep"
        );
    }
}
