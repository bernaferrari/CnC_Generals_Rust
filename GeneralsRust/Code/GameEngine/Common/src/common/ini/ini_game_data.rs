//! INI parsing for GameData definitions
//!
//! This module handles parsing GameData entries from INI files.
//! GameData contains global game configuration and settings.
//!
//! Author: Colin Day, November 2001
//! Rust port: 2025

use crate::common::global_data as runtime_global_data;
use crate::common::ini::ini::{INIError, INIResult, INI};
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Time of day enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeOfDay {
    Invalid,
    Morning,
    Afternoon,
    Evening,
    Night,
}

impl Default for TimeOfDay {
    fn default() -> Self {
        TimeOfDay::Afternoon
    }
}

/// Weather enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weather {
    Normal,
    Snowy,
}

impl Default for Weather {
    fn default() -> Self {
        Weather::Normal
    }
}

/// Player type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlayerType {
    Human,
    Computer,
    Observer,
}

/// Difficulty enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Difficulty {
    Easy,
    Normal,
    Hard,
    Brutal,
}

/// Body damage type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyDamageType {
    Pristine,
    Damaged,
    ReallyDamaged,
    Rubble,
}

/// AI debug options (bitfield)
#[derive(Debug, Clone, Copy)]
pub struct AIDebugOptions {
    pub value: u32,
}

impl AIDebugOptions {
    pub fn new() -> Self {
        Self { value: 0 }
    }

    pub fn has_flag(&self, flag: u32) -> bool {
        (self.value & flag) != 0
    }

    pub fn set_flag(&mut self, flag: u32, enabled: bool) {
        if enabled {
            self.value |= flag;
        } else {
            self.value &= !flag;
        }
    }
}

/// RGB Color representation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RGBColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl RGBColor {
    pub fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }

    pub fn black() -> Self {
        Self {
            r: 0.0,
            g: 0.0,
            b: 0.0,
        }
    }

    pub fn white() -> Self {
        Self {
            r: 1.0,
            g: 1.0,
            b: 1.0,
        }
    }
}

impl Default for RGBColor {
    fn default() -> Self {
        Self::black()
    }
}

/// 3D coordinate representation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Coord3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

impl Default for Coord3D {
    fn default() -> Self {
        Self::zero()
    }
}

/// 2D coordinate representation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord2D {
    pub x: f32,
    pub y: f32,
}

impl Coord2D {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

impl Default for Coord2D {
    fn default() -> Self {
        Self::zero()
    }
}

/// Terrain lighting configuration
#[derive(Debug, Clone)]
pub struct TerrainLighting {
    pub ambient: RGBColor,
    pub diffuse: RGBColor,
    pub light_pos: Coord3D,
}

impl Default for TerrainLighting {
    fn default() -> Self {
        Self {
            ambient: RGBColor::new(0.3, 0.3, 0.3),
            diffuse: RGBColor::new(0.7, 0.7, 0.7),
            light_pos: Coord3D::new(0.0, 0.0, 100.0),
        }
    }
}

/// Constants
pub const TIME_OF_DAY_COUNT: usize = 5;
pub const TIME_OF_DAY_FIRST: usize = 1;
pub const MAX_GLOBAL_LIGHTS: usize = 3;
pub const PLAYERTYPE_COUNT: usize = 3;
pub const DIFFICULTY_COUNT: usize = 4;
pub const LEVEL_COUNT: usize = 3;
pub const MAX_WATER_GRID_SETTINGS: usize = 4;

/// Weapon bonus entry parsed from GameData.ini
#[derive(Debug, Clone)]
pub struct WeaponBonusEntry {
    pub condition: String,
    pub field: String,
    pub value: f32,
}

/// Global data container class
///
/// Defines all global game data used by the system. This contains configuration
/// for rendering, gameplay, audio, networking, and many other systems.
///
/// Note: This is a large structure that grew over time and contains many different
/// types of configuration. In a more modern design, this might be split into
/// multiple specialized configuration structures.
#[derive(Debug, Clone)]
pub struct GlobalData {
    // Basic game settings
    pub map_name: String,
    pub move_hint_name: String,
    pub use_trees: bool,
    pub use_tree_sway: bool,
    pub use_draw_module_lod: bool,
    pub use_heat_effects: bool,
    pub use_fps_limit: bool,
    pub dump_asset_usage: bool,
    pub frames_per_second_limit: i32,
    pub chipset_type: i32,
    pub windowed: bool,
    pub x_resolution: i32,
    pub y_resolution: i32,
    pub max_shell_screens: i32,

    // Terrain and rendering settings
    pub use_cloud_map: bool,
    pub use_3way_terrain_blends: i32,
    pub use_light_map: bool,
    pub bilinear_terrain_tex: bool,
    pub trilinear_terrain_tex: bool,
    pub multipass_terrain: bool,
    pub adjust_cliff_textures: bool,
    pub stretch_terrain: bool,
    pub use_half_height_map: bool,
    pub draw_entire_terrain: bool,
    pub terrain_lod: i32, // TerrainLOD enum
    pub enable_dynamic_lod: bool,
    pub enable_static_lod: bool,
    pub terrain_lod_target_time_ms: i32,

    // Input and UI settings
    pub use_alternate_mouse: bool,
    pub client_retaliation_mode_enabled: bool,
    pub double_click_attack_move: bool,
    pub right_mouse_always_scrolls: bool,

    // Water and effects settings
    pub use_water_plane: bool,
    pub use_cloud_plane: bool,
    pub use_shadow_volumes: bool,
    pub use_shadow_decals: bool,
    pub texture_reduction_factor: i32,
    pub enable_behind_building_markers: bool,
    pub water_position_x: f32,
    pub water_position_y: f32,
    pub water_position_z: f32,
    pub water_extent_x: f32,
    pub water_extent_y: f32,
    pub water_type: i32,
    pub show_soft_water_edge: bool,
    pub using_water_track_editor: bool,
    pub is_world_builder: bool,
    pub feather_water: i32,

    // Vertex water settings (for water type 3)
    pub vertex_water_available_maps: [String; MAX_WATER_GRID_SETTINGS],
    pub vertex_water_height_clamp_low: [f32; MAX_WATER_GRID_SETTINGS],
    pub vertex_water_height_clamp_hi: [f32; MAX_WATER_GRID_SETTINGS],
    pub vertex_water_angle: [f32; MAX_WATER_GRID_SETTINGS],
    pub vertex_water_x_position: [f32; MAX_WATER_GRID_SETTINGS],
    pub vertex_water_y_position: [f32; MAX_WATER_GRID_SETTINGS],
    pub vertex_water_z_position: [f32; MAX_WATER_GRID_SETTINGS],
    pub vertex_water_x_grid_cells: [i32; MAX_WATER_GRID_SETTINGS],
    pub vertex_water_y_grid_cells: [i32; MAX_WATER_GRID_SETTINGS],
    pub vertex_water_grid_size: [f32; MAX_WATER_GRID_SETTINGS],
    pub vertex_water_attenuation_a: [f32; MAX_WATER_GRID_SETTINGS],
    pub vertex_water_attenuation_b: [f32; MAX_WATER_GRID_SETTINGS],
    pub vertex_water_attenuation_c: [f32; MAX_WATER_GRID_SETTINGS],
    pub vertex_water_attenuation_range: [f32; MAX_WATER_GRID_SETTINGS],

    // Environment settings
    pub downwind_angle: f32,
    pub sky_box_position_z: f32,
    pub draw_sky_box: f32,
    pub sky_box_scale: f32,

    // Camera settings
    pub camera_pitch: f32,
    pub camera_yaw: f32,
    pub camera_height: f32,
    pub max_camera_height: f32,
    pub min_camera_height: f32,

    // Physics and gameplay
    pub terrain_height_at_edge_of_map: f32,
    pub unit_damaged_thresh: f32,
    pub unit_really_damaged_thresh: f32,
    pub ground_stiffness: f32,
    pub structure_stiffness: f32,
    pub gravity: f32,
    pub stealth_friendly_opacity: f32,
    pub default_occlusion_delay: u32,

    // Asset loading
    pub preload_assets: bool,
    pub preload_everything: bool,
    pub preload_report: bool,

    // Partitioning
    pub partition_cell_size: f32,

    // UI positioning
    pub ammo_pip_world_offset: Coord3D,
    pub container_pip_world_offset: Coord3D,
    pub ammo_pip_screen_offset: Coord2D,
    pub container_pip_screen_offset: Coord2D,
    pub ammo_pip_scale_factor: f32,
    pub container_pip_scale_factor: f32,

    // Damage tracking
    pub historic_damage_limit: u32,

    // Terrain tracks
    pub max_terrain_tracks: i32,
    pub max_tank_track_edges: i32,
    pub max_tank_track_opaque_edges: i32,
    pub max_tank_track_fade_delay: i32,

    // Animations
    pub level_gain_animation_name: String,
    pub level_gain_animation_display_time_in_seconds: f32,
    pub level_gain_animation_z_rise_per_second: f32,
    pub get_healed_animation_name: String,
    pub get_healed_animation_display_time_in_seconds: f32,
    pub get_healed_animation_z_rise_per_second: f32,

    // Time and weather
    pub time_of_day: TimeOfDay,
    pub weather: Weather,
    pub make_track_marks: bool,
    pub hide_garrison_flags: bool,
    pub force_models_to_follow_time_of_day: bool,
    pub force_models_to_follow_weather: bool,

    // Lighting
    pub terrain_lighting: [[TerrainLighting; MAX_GLOBAL_LIGHTS]; TIME_OF_DAY_COUNT],
    pub terrain_objects_lighting: [[TerrainLighting; MAX_GLOBAL_LIGHTS]; TIME_OF_DAY_COUNT],
    pub terrain_ambient: [RGBColor; MAX_GLOBAL_LIGHTS],
    pub terrain_diffuse: [RGBColor; MAX_GLOBAL_LIGHTS],
    pub terrain_light_pos: [Coord3D; MAX_GLOBAL_LIGHTS],
    pub infantry_light_scale: [f32; TIME_OF_DAY_COUNT],
    pub script_override_infantry_light_scale: f32,

    // Difficulty bonuses
    pub solo_player_health_bonus_for_difficulty: [[f32; DIFFICULTY_COUNT]; PLAYERTYPE_COUNT],

    // Rendering limits
    pub max_visible_translucent_objects: i32,
    pub max_visible_occluder_objects: i32,
    pub max_visible_occludee_objects: i32,
    pub max_visible_non_occluder_or_occludee_objects: i32,
    pub occluded_luminance_scale: f32,

    // Lighting and roads
    pub num_global_lights: i32,
    pub max_road_segments: i32,
    pub max_road_vertex: i32,
    pub max_road_index: i32,
    pub max_road_types: i32,

    // Audio settings
    pub audio_on: bool,
    pub music_on: bool,
    pub sounds_on: bool,
    pub sounds_3d_on: bool,
    pub speech_on: bool,
    pub video_on: bool,
    pub disable_camera_movement: bool,

    // Debug and development settings
    pub use_fx: bool,
    pub show_client_physics: bool,
    pub show_terrain_normals: bool,
    pub no_draw: u32,
    pub debug_ai: AIDebugOptions,
    pub debug_supply_center_placement: bool,
    pub debug_ai_obstacles: bool,
    pub show_object_health: bool,
    pub script_debug: bool,
    pub particle_edit: bool,
    pub display_debug: bool,
    pub win_cursors: bool,
    pub constant_debug_update: bool,
    pub show_team_dot: bool,

    // Performance
    pub dump_performance_statistics: bool,
    pub dump_stats_at_interval: bool,
    pub stats_interval: i32,
    pub force_benchmark: bool,

    // Random seed
    pub fixed_seed: i32,

    // Particles
    pub particle_scale: f32,

    // Auto particles
    pub auto_fire_particle_small_prefix: String,
    pub auto_fire_particle_small_system: String,
    pub auto_fire_particle_small_max: i32,
    pub auto_fire_particle_medium_prefix: String,
    pub auto_fire_particle_medium_system: String,
    pub auto_fire_particle_medium_max: i32,
    pub auto_fire_particle_large_prefix: String,
    pub auto_fire_particle_large_system: String,
    pub auto_fire_particle_large_max: i32,
    pub auto_smoke_particle_small_prefix: String,
    pub auto_smoke_particle_small_system: String,
    pub auto_smoke_particle_small_max: i32,
    pub auto_smoke_particle_medium_prefix: String,
    pub auto_smoke_particle_medium_system: String,
    pub auto_smoke_particle_medium_max: i32,
    pub auto_smoke_particle_large_prefix: String,
    pub auto_smoke_particle_large_system: String,
    pub auto_smoke_particle_large_max: i32,
    pub auto_aflame_particle_prefix: String,
    pub auto_aflame_particle_system: String,
    pub auto_aflame_particle_max: i32,

    // Network settings
    pub net_min_players: i32,
    pub default_ip: u32,
    pub firewall_behavior: u32,
    pub firewall_send_delay: bool,
    pub firewall_port_override: u32,
    pub firewall_port_allocation_delta: i16,

    // Game economy
    pub base_value_per_supply_box: i32,
    pub build_speed: f32,
    pub min_dist_from_edge_of_map_for_build: f32,
    pub supply_build_border: f32,
    pub allowed_height_variation_for_building: f32,
    pub min_low_energy_production_speed: f32,
    pub max_low_energy_production_speed: f32,
    pub low_energy_penalty_modifier: f32,
    pub multiple_factory: f32,
    pub refund_percent: f32,

    // Command center
    pub command_center_heal_range: f32,
    pub command_center_heal_amount: f32,
    pub max_line_build_objects: i32,
    pub max_tunnel_capacity: i32,

    // Camera controls
    pub horizontal_scroll_speed_factor: f32,
    pub vertical_scroll_speed_factor: f32,
    pub scroll_amount_cutoff: f32,
    pub camera_adjust_speed: f32,
    pub enforce_max_camera_height: bool,

    // Files
    pub build_map_cache: bool,
    pub initial_file: String,
    pub pending_file: String,

    // Particles
    pub max_particle_count: i32,
    pub max_field_particle_count: i32,

    // Health and veterancy
    pub health_bonus: [f32; LEVEL_COUNT],
    pub default_structure_rubble_height: f32,

    // Shell map
    pub shell_map_name: String,
    pub shell_map_on: bool,
    pub play_intro: bool,
    pub play_sizzle: bool,
    pub after_intro: bool,
    pub allow_exit_out_of_movies: bool,

    // Loading
    pub load_screen_render: bool,

    // Input
    pub keyboard_scroll_factor: f32,
    pub keyboard_default_scroll_factor: f32,

    // Audio volume
    pub music_volume_factor: f32,
    pub sfx_volume_factor: f32,
    pub voice_volume_factor: f32,
    pub sound_3d_pref: bool,

    // UI
    pub animate_windows: bool,
    pub incremental_agp_buf: bool,

    // File integrity
    pub ini_crc: u32,
    pub exe_crc: u32,

    // Damage states
    pub movement_penalty_damage_state: BodyDamageType,

    // Audio feedback
    pub group_select_min_select_size: i32,
    pub group_select_volume_base: f32,
    pub group_select_volume_increment: f32,
    pub max_unit_select_sounds: i32,

    // Selection effects
    pub selection_flash_saturation_factor: f32,
    pub selection_flash_house_color: bool,

    // Audio
    pub camera_audible_radius: f32,
    pub group_move_click_to_gather_factor: f32,

    // Graphics options
    pub anti_alias_box_value: i32,
    pub language_filter_pref: bool,
    pub load_screen_demo: bool,
    pub disable_render: bool,

    // Replay
    pub save_camera_in_replay: bool,
    pub use_camera_in_replay: bool,

    // Screen shake
    pub shake_subtle_intensity: f32,
    pub shake_normal_intensity: f32,
    pub shake_strong_intensity: f32,
    pub shake_severe_intensity: f32,
    pub shake_cine_extreme_intensity: f32,
    pub shake_cine_insane_intensity: f32,
    pub max_shake_intensity: f32,
    pub max_shake_range: f32,

    // Economy
    pub sell_percentage: f32,
    pub base_regen_health_percent_per_second: f32,
    pub base_regen_delay: u32,

    // Prison system (conditional compilation)
    pub prison_bounty_multiplier: f32,
    pub prison_bounty_text_color: RGBColor,

    // UI colors
    pub hot_key_text_color: RGBColor,

    // Special powers
    pub special_power_view_object_name: String,
    pub weapon_bonus_entries: Vec<WeaponBonusEntry>,

    // Bones
    pub standard_public_bones: Vec<String>,

    // Minefields
    pub standard_minefield_density: f32,
    pub standard_minefield_distance: f32,

    // Metrics
    pub show_metrics: bool,
    pub default_starting_cash: u32, // Money type

    // Debug
    pub debug_show_graphical_framerate: bool,

    // Power bar
    pub power_bar_base: i32,
    pub power_bar_intervals: f32,
    pub power_bar_yellow_range: i32,
    pub display_gamma: f32,

    // Unlook
    pub unlook_persist_duration: u32,

    // Asset update
    pub should_update_tga_to_dds: bool,

    // Input timing
    pub double_click_time_ms: u32,

    // Shroud and fog
    pub shroud_color: RGBColor,
    pub clear_alpha: u8,
    pub fog_alpha: u8,
    pub shroud_alpha: u8,

    // Network timing
    pub network_fps_history_length: u32,
    pub network_latency_history_length: u32,
    pub network_cushion_history_length: u32,
    pub network_run_ahead_metrics_time: u32,
    pub network_keep_alive_delay: u32,
    pub network_run_ahead_slack: u32,
    pub network_disconnect_time: u32,
    pub network_player_timeout_time: u32,
    pub network_disconnect_screen_notify_time: u32,

    // Camera
    pub keyboard_camera_rotate_speed: f32,
    pub play_stats: i32,

    // Special powers (debug)
    pub special_power_uses_delay: bool,
    pub tivo_fast_mode: bool,

    // Development flags
    pub wireframe: bool,
    pub state_machine_debug: bool,
    pub use_camera_constraints: bool,
    pub shroud_on: bool,
    pub fog_of_war_on: bool,
    pub jabber_on: bool,
    pub munkee_on: bool,
    pub allow_unselectable_selection: bool,
    pub disable_camera_fade: bool,
    pub disable_scripted_input_disabling: bool,
    pub disable_military_caption: bool,
    pub benchmark_timer: i32,
    pub check_for_leaks: bool,
    pub v_tune: bool,
    pub debug_camera: bool,
    pub debug_visibility: bool,
    pub debug_visibility_tile_count: i32,
    pub debug_visibility_tile_width: f32,
    pub debug_visibility_tile_duration: i32,
    pub debug_threat_map: bool,
    pub max_debug_threat: u32,
    pub debug_threat_map_tile_duration: i32,
    pub debug_cash_value_map: bool,
    pub max_debug_value: u32,
    pub debug_cash_value_map_tile_duration: i32,
    pub debug_visibility_targettable_color: RGBColor,
    pub debug_visibility_deshroud_color: RGBColor,
    pub debug_visibility_gap_color: RGBColor,
    pub debug_projectile_path: bool,
    pub debug_projectile_tile_width: f32,
    pub debug_projectile_tile_duration: i32,
    pub debug_projectile_tile_color: RGBColor,
    pub debug_ignore_asserts: bool,
    pub debug_ignore_stack_trace: bool,
    pub show_collision_extents: bool,
    pub show_audio_locations: bool,
    pub save_stats: bool,
    pub save_all_stats: bool,
    pub use_local_motd: bool,
    pub base_stats_dir: String,
    pub motd_path: String,
    pub latency_average: i32,
    pub latency_amplitude: i32,
    pub latency_period: i32,
    pub latency_noise: i32,
    pub packet_loss: i32,
    pub extra_logging: bool,

    // Movie control
    pub is_breakable_movie: bool,
    pub break_the_movie: bool,
    pub mod_dir: String,
    pub mod_big: String,

    // User data path
    user_data_dir: String,
}

impl Default for GlobalData {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalData {
    /// Create a new GlobalData instance with default values
    pub fn new() -> Self {
        Self {
            // Initialize all fields with sensible defaults
            map_name: String::new(),
            move_hint_name: String::new(),
            use_trees: true,
            use_tree_sway: true,
            use_draw_module_lod: true,
            use_heat_effects: true,
            use_fps_limit: false,
            dump_asset_usage: false,
            frames_per_second_limit: 30,
            chipset_type: 0,
            windowed: false,
            x_resolution: 1024,
            y_resolution: 768,
            max_shell_screens: 8,

            use_cloud_map: true,
            use_3way_terrain_blends: 1,
            use_light_map: true,
            bilinear_terrain_tex: true,
            trilinear_terrain_tex: false,
            multipass_terrain: true,
            adjust_cliff_textures: true,
            stretch_terrain: false,
            use_half_height_map: false,
            draw_entire_terrain: false,
            terrain_lod: 0,
            enable_dynamic_lod: true,
            enable_static_lod: true,
            terrain_lod_target_time_ms: 33,

            use_alternate_mouse: false,
            client_retaliation_mode_enabled: true,
            double_click_attack_move: true,
            right_mouse_always_scrolls: false,

            use_water_plane: true,
            use_cloud_plane: true,
            use_shadow_volumes: true,
            use_shadow_decals: true,
            texture_reduction_factor: 0,
            enable_behind_building_markers: true,
            water_position_x: 0.0,
            water_position_y: 0.0,
            water_position_z: 0.0,
            water_extent_x: 100.0,
            water_extent_y: 100.0,
            water_type: 0,
            show_soft_water_edge: true,
            using_water_track_editor: false,
            is_world_builder: false,
            feather_water: 4,

            // Initialize arrays with default values
            vertex_water_available_maps: Default::default(),
            vertex_water_height_clamp_low: [0.0; MAX_WATER_GRID_SETTINGS],
            vertex_water_height_clamp_hi: [10.0; MAX_WATER_GRID_SETTINGS],
            vertex_water_angle: [0.0; MAX_WATER_GRID_SETTINGS],
            vertex_water_x_position: [0.0; MAX_WATER_GRID_SETTINGS],
            vertex_water_y_position: [0.0; MAX_WATER_GRID_SETTINGS],
            vertex_water_z_position: [0.0; MAX_WATER_GRID_SETTINGS],
            vertex_water_x_grid_cells: [32; MAX_WATER_GRID_SETTINGS],
            vertex_water_y_grid_cells: [32; MAX_WATER_GRID_SETTINGS],
            vertex_water_grid_size: [10.0; MAX_WATER_GRID_SETTINGS],
            vertex_water_attenuation_a: [1.0; MAX_WATER_GRID_SETTINGS],
            vertex_water_attenuation_b: [0.0; MAX_WATER_GRID_SETTINGS],
            vertex_water_attenuation_c: [0.0; MAX_WATER_GRID_SETTINGS],
            vertex_water_attenuation_range: [100.0; MAX_WATER_GRID_SETTINGS],

            downwind_angle: 45.0,
            sky_box_position_z: 0.0,
            draw_sky_box: 1.0,
            sky_box_scale: 1.0,

            camera_pitch: std::f32::consts::FRAC_PI_4,
            camera_yaw: std::f32::consts::FRAC_PI_4,
            camera_height: 200.0,
            max_camera_height: 400.0,
            min_camera_height: 100.0,

            terrain_height_at_edge_of_map: 0.0,
            unit_damaged_thresh: 0.5,
            unit_really_damaged_thresh: 0.25,
            ground_stiffness: 0.8,
            structure_stiffness: 1.0,
            gravity: -9.8,
            stealth_friendly_opacity: 0.3,
            default_occlusion_delay: 5000,

            preload_assets: false,
            preload_everything: false,
            preload_report: false,

            partition_cell_size: 100.0,

            ammo_pip_world_offset: Coord3D::default(),
            container_pip_world_offset: Coord3D::default(),
            ammo_pip_screen_offset: Coord2D::default(),
            container_pip_screen_offset: Coord2D::default(),
            ammo_pip_scale_factor: 1.0,
            container_pip_scale_factor: 1.0,

            historic_damage_limit: 8,

            max_terrain_tracks: 250,
            max_tank_track_edges: 100,
            max_tank_track_opaque_edges: 25,
            max_tank_track_fade_delay: 300000,

            level_gain_animation_name: String::new(),
            level_gain_animation_display_time_in_seconds: 2.0,
            level_gain_animation_z_rise_per_second: 15.0,
            get_healed_animation_name: String::new(),
            get_healed_animation_display_time_in_seconds: 2.0,
            get_healed_animation_z_rise_per_second: 15.0,

            time_of_day: TimeOfDay::default(),
            weather: Weather::default(),
            make_track_marks: true,
            hide_garrison_flags: false,
            force_models_to_follow_time_of_day: false,
            force_models_to_follow_weather: false,

            // Initialize lighting arrays
            terrain_lighting: Default::default(),
            terrain_objects_lighting: Default::default(),
            terrain_ambient: [RGBColor::new(0.3, 0.3, 0.3); MAX_GLOBAL_LIGHTS],
            terrain_diffuse: [RGBColor::new(0.7, 0.7, 0.7); MAX_GLOBAL_LIGHTS],
            terrain_light_pos: [Coord3D::new(0.0, 0.0, 100.0); MAX_GLOBAL_LIGHTS],
            infantry_light_scale: [1.0; TIME_OF_DAY_COUNT],
            script_override_infantry_light_scale: -1.0,

            solo_player_health_bonus_for_difficulty: [[1.0; DIFFICULTY_COUNT]; PLAYERTYPE_COUNT],

            max_visible_translucent_objects: 500,
            max_visible_occluder_objects: 200,
            max_visible_occludee_objects: 800,
            max_visible_non_occluder_or_occludee_objects: 1000,
            occluded_luminance_scale: 0.5,

            num_global_lights: 2,
            max_road_segments: 1000,
            max_road_vertex: 10000,
            max_road_index: 30000,
            max_road_types: 4,

            audio_on: true,
            music_on: true,
            sounds_on: true,
            sounds_3d_on: true,
            speech_on: true,
            video_on: true,
            disable_camera_movement: false,

            use_fx: true,
            show_client_physics: false,
            show_terrain_normals: false,
            no_draw: 0,
            debug_ai: AIDebugOptions::new(),
            debug_supply_center_placement: false,
            debug_ai_obstacles: false,
            show_object_health: false,
            script_debug: false,
            particle_edit: false,
            display_debug: false,
            win_cursors: false,
            constant_debug_update: false,
            show_team_dot: true,

            dump_performance_statistics: false,
            dump_stats_at_interval: false,
            stats_interval: 3000,
            force_benchmark: false,

            fixed_seed: -1,

            particle_scale: 1.0,

            // Initialize particle strings
            auto_fire_particle_small_prefix: String::new(),
            auto_fire_particle_small_system: String::new(),
            auto_fire_particle_small_max: 50,
            auto_fire_particle_medium_prefix: String::new(),
            auto_fire_particle_medium_system: String::new(),
            auto_fire_particle_medium_max: 30,
            auto_fire_particle_large_prefix: String::new(),
            auto_fire_particle_large_system: String::new(),
            auto_fire_particle_large_max: 20,
            auto_smoke_particle_small_prefix: String::new(),
            auto_smoke_particle_small_system: String::new(),
            auto_smoke_particle_small_max: 50,
            auto_smoke_particle_medium_prefix: String::new(),
            auto_smoke_particle_medium_system: String::new(),
            auto_smoke_particle_medium_max: 30,
            auto_smoke_particle_large_prefix: String::new(),
            auto_smoke_particle_large_system: String::new(),
            auto_smoke_particle_large_max: 20,
            auto_aflame_particle_prefix: String::new(),
            auto_aflame_particle_system: String::new(),
            auto_aflame_particle_max: 30,

            net_min_players: 2,
            default_ip: 0,
            firewall_behavior: 0,
            firewall_send_delay: false,
            firewall_port_override: 0,
            firewall_port_allocation_delta: 0,

            base_value_per_supply_box: 200,
            build_speed: 1.0,
            min_dist_from_edge_of_map_for_build: 25.0,
            supply_build_border: 150.0,
            allowed_height_variation_for_building: 5.0,
            min_low_energy_production_speed: 0.25,
            max_low_energy_production_speed: 1.0,
            low_energy_penalty_modifier: 0.5,
            multiple_factory: 1.0,
            refund_percent: 0.8,

            command_center_heal_range: 100.0,
            command_center_heal_amount: 5.0,
            max_line_build_objects: 20,
            max_tunnel_capacity: 8,

            horizontal_scroll_speed_factor: 1.0,
            vertical_scroll_speed_factor: 1.0,
            scroll_amount_cutoff: 0.0,
            camera_adjust_speed: 2.0,
            enforce_max_camera_height: true,

            build_map_cache: true,
            initial_file: String::new(),
            pending_file: String::new(),

            max_particle_count: 5000,
            max_field_particle_count: 1000,

            health_bonus: [0.0, 0.25, 1.0],
            default_structure_rubble_height: 8.0,

            shell_map_name: String::new(),
            shell_map_on: true,
            play_intro: true,
            play_sizzle: true,
            after_intro: false,
            allow_exit_out_of_movies: false,

            load_screen_render: true,

            keyboard_scroll_factor: 1.0,
            keyboard_default_scroll_factor: 1.0,

            music_volume_factor: 0.8,
            sfx_volume_factor: 0.8,
            voice_volume_factor: 0.8,
            sound_3d_pref: true,

            animate_windows: true,
            incremental_agp_buf: false,

            ini_crc: 0,
            exe_crc: 0,

            movement_penalty_damage_state: BodyDamageType::ReallyDamaged,

            group_select_min_select_size: 2,
            group_select_volume_base: 0.7,
            group_select_volume_increment: 0.1,
            max_unit_select_sounds: 4,

            selection_flash_saturation_factor: 0.0,
            selection_flash_house_color: false,

            camera_audible_radius: 200.0,
            group_move_click_to_gather_factor: 0.0,

            anti_alias_box_value: 0,
            language_filter_pref: false,
            load_screen_demo: false,
            disable_render: false,

            save_camera_in_replay: false,
            use_camera_in_replay: false,

            shake_subtle_intensity: 2.0,
            shake_normal_intensity: 5.0,
            shake_strong_intensity: 10.0,
            shake_severe_intensity: 20.0,
            shake_cine_extreme_intensity: 40.0,
            shake_cine_insane_intensity: 80.0,
            max_shake_intensity: 100.0,
            max_shake_range: 200.0,

            sell_percentage: 0.25,
            base_regen_health_percent_per_second: 0.0,
            base_regen_delay: 0,

            prison_bounty_multiplier: 1.0,
            prison_bounty_text_color: RGBColor::new(1.0, 1.0, 1.0),

            hot_key_text_color: RGBColor::white(),

            special_power_view_object_name: String::new(),
            weapon_bonus_entries: Vec::new(),

            standard_public_bones: Vec::new(),

            standard_minefield_density: 1.0,
            standard_minefield_distance: 20.0,

            show_metrics: false,
            default_starting_cash: 5000,

            debug_show_graphical_framerate: false,

            power_bar_base: 2,
            power_bar_intervals: 10.0,
            power_bar_yellow_range: 80,
            display_gamma: 1.0,

            unlook_persist_duration: 15000,

            should_update_tga_to_dds: false,

            double_click_time_ms: 250,

            shroud_color: RGBColor::new(0.0, 0.0, 0.0),
            clear_alpha: 255,
            fog_alpha: 127,
            shroud_alpha: 0,

            network_fps_history_length: 64,
            network_latency_history_length: 64,
            network_cushion_history_length: 4,
            network_run_ahead_metrics_time: 1000,
            network_keep_alive_delay: 60,
            network_run_ahead_slack: 10,
            network_disconnect_time: 30000,
            network_player_timeout_time: 120000,
            network_disconnect_screen_notify_time: 5000,

            keyboard_camera_rotate_speed: 1.0,
            play_stats: 0,

            special_power_uses_delay: true,
            tivo_fast_mode: false,

            // Debug flags (only in debug builds)
            wireframe: false,
            state_machine_debug: false,
            use_camera_constraints: true,
            shroud_on: true,
            fog_of_war_on: true,
            jabber_on: false,
            munkee_on: false,
            allow_unselectable_selection: false,
            disable_camera_fade: false,
            disable_scripted_input_disabling: false,
            disable_military_caption: false,
            benchmark_timer: 0,
            check_for_leaks: false,
            v_tune: false,
            debug_camera: false,
            debug_visibility: false,
            debug_visibility_tile_count: 0,
            debug_visibility_tile_width: 10.0,
            debug_visibility_tile_duration: 60,
            debug_threat_map: false,
            max_debug_threat: 100,
            debug_threat_map_tile_duration: 60,
            debug_cash_value_map: false,
            max_debug_value: 1000,
            debug_cash_value_map_tile_duration: 60,
            debug_visibility_targettable_color: RGBColor::new(1.0, 0.0, 0.0),
            debug_visibility_deshroud_color: RGBColor::new(0.0, 1.0, 0.0),
            debug_visibility_gap_color: RGBColor::new(0.0, 0.0, 1.0),
            debug_projectile_path: false,
            debug_projectile_tile_width: 2.0,
            debug_projectile_tile_duration: 60,
            debug_projectile_tile_color: RGBColor::new(1.0, 1.0, 0.0),
            debug_ignore_asserts: false,
            debug_ignore_stack_trace: false,
            show_collision_extents: false,
            show_audio_locations: false,
            save_stats: false,
            save_all_stats: false,
            use_local_motd: false,
            base_stats_dir: String::new(),
            motd_path: String::new(),
            latency_average: 0,
            latency_amplitude: 0,
            latency_period: 1000,
            latency_noise: 0,
            packet_loss: 0,
            extra_logging: false,

            is_breakable_movie: false,
            break_the_movie: false,
            mod_dir: String::new(),
            mod_big: String::new(),

            user_data_dir: String::new(),
        }
    }

    /// Initialize the global data system
    pub fn init(&mut self) {
        // C++ GlobalData::init is a no-op.
    }

    /// Reset the global data system
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Update the global data system (called per frame)
    pub fn update(&mut self) {
        // Update logic here if needed
    }

    /// Set time of day
    ///
    /// # Arguments
    /// * `tod` - New time of day to set
    ///
    /// # Returns
    /// `true` if successful, `false` otherwise
    pub fn set_time_of_day(&mut self, tod: TimeOfDay) -> bool {
        let index = match tod {
            TimeOfDay::Invalid => return false,
            TimeOfDay::Morning => 1,
            TimeOfDay::Afternoon => 2,
            TimeOfDay::Evening => 3,
            TimeOfDay::Night => 4,
        };

        self.time_of_day = tod;
        for i in 0..MAX_GLOBAL_LIGHTS {
            self.terrain_ambient[i] = self.terrain_lighting[index][i].ambient;
            self.terrain_diffuse[i] = self.terrain_lighting[index][i].diffuse;
            self.terrain_light_pos[i] = self.terrain_lighting[index][i].light_pos;
        }
        true
    }

    /// Get the user data directory path
    pub fn get_path_user_data(&self) -> &str {
        &self.user_data_dir
    }

    /// Set the user data directory path.
    pub fn set_path_user_data(&mut self, path: String) {
        self.user_data_dir = path;
    }

    /// Parse GameData definition from INI
    pub fn parse_game_data_definition(ini: &mut INI) -> Result<(), String> {
        let global_data = ensure_global_data();
        let mut data = global_data.write();
        parse_game_data_block(ini, &mut data).map_err(|err| err.to_string())?;
        sync_runtime_global_data_from_ini(&data);
        Ok(())
    }
}

/// Global GlobalData instance (matches C++ TheWritableGlobalData/TheGlobalData)
static WRITABLE_GLOBAL_DATA: OnceCell<Arc<RwLock<GlobalData>>> = OnceCell::new();

/// Ensure the global data instance exists and return a handle to it
pub fn ensure_global_data() -> Arc<RwLock<GlobalData>> {
    WRITABLE_GLOBAL_DATA
        .get_or_init(|| {
            let mut data = GlobalData::new();
            data.init();
            Arc::new(RwLock::new(data))
        })
        .clone()
}

/// Initialize the global data instance
pub fn init_global_data() {
    let global_data = ensure_global_data();
    let mut data = global_data.write();
    *data = GlobalData::new();
    data.init();
}

/// Get a handle to the global data (read-only or mutable via locks)
pub fn get_global_data() -> Option<Arc<RwLock<GlobalData>>> {
    WRITABLE_GLOBAL_DATA.get().cloned()
}

/// INI parsing function (matches C++ interface)
///
/// This is the main entry point for parsing GameData definitions from INI files
pub fn parse_game_data_definition(ini: &mut INI) -> Result<(), String> {
    GlobalData::parse_game_data_definition(ini)
}

fn parse_game_data_block(ini: &mut INI, data: &mut GlobalData) -> INIResult<()> {
    loop {
        ini.read_line()?;

        if ini.is_eof() {
            return Err(INIError::EndOfFile);
        }

        let tokens = ini.get_line_tokens();
        if tokens.is_empty() {
            continue;
        }

        let key = tokens[0];
        if key.eq_ignore_ascii_case("End") {
            break;
        }

        let mut args: Vec<&str> = tokens[1..].iter().copied().collect();
        args.retain(|token| *token != "=");

        if let Some((target, time_index, light_index, field)) = parse_lighting_token(key) {
            let color = parse_rgb_color(&args)?;
            match (target, field) {
                (LightingTarget::Terrain, LightingField::Ambient) => {
                    data.terrain_lighting[time_index][light_index].ambient = color;
                }
                (LightingTarget::Terrain, LightingField::Diffuse) => {
                    data.terrain_lighting[time_index][light_index].diffuse = color;
                }
                (LightingTarget::Objects, LightingField::Ambient) => {
                    data.terrain_objects_lighting[time_index][light_index].ambient = color;
                }
                (LightingTarget::Objects, LightingField::Diffuse) => {
                    data.terrain_objects_lighting[time_index][light_index].diffuse = color;
                }
                _ => {}
            }
            continue;
        }

        if let Some((target, time_index, light_index)) = parse_lighting_pos_token(key) {
            let pos = parse_coord3d(&args)?;
            match target {
                LightingTarget::Terrain => {
                    data.terrain_lighting[time_index][light_index].light_pos = pos;
                }
                LightingTarget::Objects => {
                    data.terrain_objects_lighting[time_index][light_index].light_pos = pos;
                }
            }
            continue;
        }

        if let Some(time_index) = parse_infantry_light_token(key) {
            data.infantry_light_scale[time_index] = parse_real(&args)?;
            continue;
        }

        if let Some(index) = parse_indexed_suffix(key, "VertexWaterAvailableMaps") {
            data.vertex_water_available_maps[index] = parse_string(&args)?;
            continue;
        }
        if let Some(index) = parse_indexed_suffix(key, "VertexWaterHeightClampLow") {
            data.vertex_water_height_clamp_low[index] = parse_real(&args)?;
            continue;
        }
        if let Some(index) = parse_indexed_suffix(key, "VertexWaterHeightClampHi") {
            data.vertex_water_height_clamp_hi[index] = parse_real(&args)?;
            continue;
        }
        if let Some(index) = parse_indexed_suffix(key, "VertexWaterAngle") {
            data.vertex_water_angle[index] = parse_angle(&args)?;
            continue;
        }
        if let Some(index) = parse_indexed_suffix(key, "VertexWaterXPosition") {
            data.vertex_water_x_position[index] = parse_real(&args)?;
            continue;
        }
        if let Some(index) = parse_indexed_suffix(key, "VertexWaterYPosition") {
            data.vertex_water_y_position[index] = parse_real(&args)?;
            continue;
        }
        if let Some(index) = parse_indexed_suffix(key, "VertexWaterZPosition") {
            data.vertex_water_z_position[index] = parse_real(&args)?;
            continue;
        }
        if let Some(index) = parse_indexed_suffix(key, "VertexWaterXGridCells") {
            data.vertex_water_x_grid_cells[index] = parse_int(&args)?;
            continue;
        }
        if let Some(index) = parse_indexed_suffix(key, "VertexWaterYGridCells") {
            data.vertex_water_y_grid_cells[index] = parse_int(&args)?;
            continue;
        }
        if let Some(index) = parse_indexed_suffix(key, "VertexWaterGridSize") {
            data.vertex_water_grid_size[index] = parse_real(&args)?;
            continue;
        }
        if let Some(index) = parse_indexed_suffix(key, "VertexWaterAttenuationA") {
            data.vertex_water_attenuation_a[index] = parse_real(&args)?;
            continue;
        }
        if let Some(index) = parse_indexed_suffix(key, "VertexWaterAttenuationB") {
            data.vertex_water_attenuation_b[index] = parse_real(&args)?;
            continue;
        }
        if let Some(index) = parse_indexed_suffix(key, "VertexWaterAttenuationC") {
            data.vertex_water_attenuation_c[index] = parse_real(&args)?;
            continue;
        }
        if let Some(index) = parse_indexed_suffix(key, "VertexWaterAttenuationRange") {
            data.vertex_water_attenuation_range[index] = parse_real(&args)?;
            continue;
        }

        match key.to_ascii_uppercase().as_str() {
            "WINDOWED" => data.windowed = parse_bool(&args)?,
            "XRESOLUTION" => data.x_resolution = parse_int(&args)?,
            "YRESOLUTION" => data.y_resolution = parse_int(&args)?,
            "MAPNAME" => data.map_name = parse_string(&args)?,
            "MOVEHINTNAME" => data.move_hint_name = parse_string(&args)?,
            "USETREES" => data.use_trees = parse_bool(&args)?,
            "USEFPSLIMIT" => data.use_fps_limit = parse_bool(&args)?,
            "DUMPASSETUSAGE" => data.dump_asset_usage = parse_bool(&args)?,
            "FRAMESPERSECONDLIMIT" => data.frames_per_second_limit = parse_int(&args)?,
            "CHIPSETTYPE" => data.chipset_type = parse_int(&args)?,
            "MAXSHELLSCREENS" => data.max_shell_screens = parse_int(&args)?,
            "USECLOUDMAP" => data.use_cloud_map = parse_bool(&args)?,
            "USELIGHTMAP" => data.use_light_map = parse_bool(&args)?,
            "BILINEARTERRAINTEX" => data.bilinear_terrain_tex = parse_bool(&args)?,
            "TRILINEARTERRAINTEX" => data.trilinear_terrain_tex = parse_bool(&args)?,
            "MULTIPASSTERRAIN" => data.multipass_terrain = parse_bool(&args)?,
            "ADJUSTCLIFFTEXTURES" => data.adjust_cliff_textures = parse_bool(&args)?,
            "STRETCHTERRAIN" => data.stretch_terrain = parse_bool(&args)?,
            "USEHALFHEIGHTMAP" => data.use_half_height_map = parse_bool(&args)?,
            "USE3WAYTERRAINBLENDS" => data.use_3way_terrain_blends = parse_int(&args)?,
            "DRAWENTIRETERRAIN" => data.draw_entire_terrain = parse_bool(&args)?,
            "TERRAINLOD" => data.terrain_lod = parse_terrain_lod(&args)?,
            "TERRAINLODTARGETTIMEMS" => data.terrain_lod_target_time_ms = parse_int(&args)?,
            "RIGHTMOUSEALWAYSSCROLLS" => data.right_mouse_always_scrolls = parse_bool(&args)?,
            "USEWATERPLANE" => data.use_water_plane = parse_bool(&args)?,
            "USECLOUDPLANE" => data.use_cloud_plane = parse_bool(&args)?,
            "USESHADOWVOLUMES" => data.use_shadow_volumes = parse_bool(&args)?,
            "USESHADOWDECALS" => data.use_shadow_decals = parse_bool(&args)?,
            "TEXTUREREDUCTIONFACTOR" => data.texture_reduction_factor = parse_int(&args)?,
            "USEBEHINDBUILDINGMARKER" => data.enable_behind_building_markers = parse_bool(&args)?,
            "WATERPOSITIONX" => data.water_position_x = parse_real(&args)?,
            "WATERPOSITIONY" => data.water_position_y = parse_real(&args)?,
            "WATERPOSITIONZ" => data.water_position_z = parse_real(&args)?,
            "WATEREXTENTX" => data.water_extent_x = parse_real(&args)?,
            "WATEREXTENTY" => data.water_extent_y = parse_real(&args)?,
            "WATERTYPE" => data.water_type = parse_int(&args)?,
            "FEATHERWATER" => data.feather_water = parse_int(&args)?,
            "SHOWSOFTWATEREDGE" => data.show_soft_water_edge = parse_bool(&args)?,
            "DOWNWINDANGLE" => data.downwind_angle = parse_real(&args)?,
            "SKYBOXPOSITIONZ" => data.sky_box_position_z = parse_real(&args)?,
            "SKYBOXSCALE" => data.sky_box_scale = parse_real(&args)?,
            "DRAWSKYBOX" => data.draw_sky_box = parse_bool_as_f32(&args)?,
            "CAMERAPITCH" => data.camera_pitch = parse_real(&args)?,
            "CAMERAYAW" => data.camera_yaw = parse_real(&args)?,
            "CAMERAHEIGHT" => data.camera_height = parse_real(&args)?,
            "MAXCAMERAHEIGHT" => data.max_camera_height = parse_real(&args)?,
            "MINCAMERAHEIGHT" => data.min_camera_height = parse_real(&args)?,
            "TERRAINHEIGHTATEDGEOFMAP" => data.terrain_height_at_edge_of_map = parse_real(&args)?,
            "UNITDAMAGEDTHRESHOLD" => data.unit_damaged_thresh = parse_real(&args)?,
            "UNITREALLYDAMAGEDTHRESHOLD" => data.unit_really_damaged_thresh = parse_real(&args)?,
            "GROUNDSTIFFNESS" => data.ground_stiffness = parse_real(&args)?,
            "STRUCTURESTIFFNESS" => data.structure_stiffness = parse_real(&args)?,
            "GRAVITY" => data.gravity = parse_acceleration(&args)?,
            "STEALTHFRIENDLYOPACITY" => data.stealth_friendly_opacity = parse_percent(&args)?,
            "DEFAULTOCCLUSIONDELAY" => data.default_occlusion_delay = parse_duration_u32(&args)?,
            "PARTITIONCELLSIZE" => data.partition_cell_size = parse_real(&args)?,
            "AMMOPIPSCALEFACTOR" => data.ammo_pip_scale_factor = parse_real(&args)?,
            "CONTAINERPIPSCALEFACTOR" => data.container_pip_scale_factor = parse_real(&args)?,
            "AMMOPIPWORLDOFFSET" => data.ammo_pip_world_offset = parse_coord3d(&args)?,
            "CONTAINERPIPWORLDOFFSET" => {
                data.container_pip_world_offset = parse_coord3d(&args)?;
            }
            "AMMOPIPSCREENOFFSET" => data.ammo_pip_screen_offset = parse_coord2d(&args)?,
            "CONTAINERPIPSCREENOFFSET" => {
                data.container_pip_screen_offset = parse_coord2d(&args)?;
            }
            "HISTORICDAMAGELIMIT" => data.historic_damage_limit = parse_duration_u32(&args)?,
            "MAXTERRAINTRACKS" => data.max_terrain_tracks = parse_int(&args)?,
            "TIMEOFDAY" => data.time_of_day = parse_time_of_day(&args)?,
            "WEATHER" => data.weather = parse_weather(&args)?,
            "MAKETRACKMARKS" => data.make_track_marks = parse_bool(&args)?,
            "HIDEGARRISONFLAGS" => data.hide_garrison_flags = parse_bool(&args)?,
            "FORCEMODELSTOFOLLOWTIMEOFDAY" => {
                data.force_models_to_follow_time_of_day = parse_bool(&args)?;
            }
            "FORCEMODELSTOFOLLOWWEATHER" => {
                data.force_models_to_follow_weather = parse_bool(&args)?;
            }
            "NUMBERGLOBALLIGHTS" => data.num_global_lights = parse_int(&args)?,
            "MAXTRANSLUCENTOBJECTS" => {
                data.max_visible_translucent_objects = parse_int(&args)?;
            }
            "OCCLUDEDCOLORLUMINANCESCALE" => data.occluded_luminance_scale = parse_real(&args)?,
            "MAXROADSEGMENTS" => data.max_road_segments = parse_int(&args)?,
            "MAXROADVERTEX" => data.max_road_vertex = parse_int(&args)?,
            "MAXROADINDEX" => data.max_road_index = parse_int(&args)?,
            "MAXROADTYPES" => data.max_road_types = parse_int(&args)?,
            "VALUEPERSUPPLYBOX" => data.base_value_per_supply_box = parse_int(&args)?,
            "AUDIOON" => data.audio_on = parse_bool(&args)?,
            "MUSICON" => data.music_on = parse_bool(&args)?,
            "SOUNDSON" => data.sounds_on = parse_bool(&args)?,
            "SOUNDS3DON" => data.sounds_3d_on = parse_bool(&args)?,
            "SPEECHON" => data.speech_on = parse_bool(&args)?,
            "VIDEOON" => data.video_on = parse_bool(&args)?,
            "DISABLECAMERAMOVEMENTS" => data.disable_camera_movement = parse_bool(&args)?,
            "DEBUGAI" => data.debug_ai.set_flag(1, parse_bool(&args)?),
            "DEBUGAIOBSTACLES" => data.debug_ai_obstacles = parse_bool(&args)?,
            "SHOWCLIENTPHYSICS" => data.show_client_physics = parse_bool(&args)?,
            "SHOWTERRAINNORMALS" => data.show_terrain_normals = parse_bool(&args)?,
            "SHOWOBJECTHEALTH" => data.show_object_health = parse_bool(&args)?,
            "PARTICLESCALE" => data.particle_scale = parse_real(&args)?,
            "AUTOFIREPARTICLESMALLPREFIX" => {
                data.auto_fire_particle_small_prefix = parse_string(&args)?;
            }
            "AUTOFIREPARTICLESMALLSYSTEM" => {
                data.auto_fire_particle_small_system = parse_string(&args)?;
            }
            "AUTOFIREPARTICLESMALLMAX" => data.auto_fire_particle_small_max = parse_int(&args)?,
            "AUTOFIREPARTICLEMEDIUMPREFIX" => {
                data.auto_fire_particle_medium_prefix = parse_string(&args)?;
            }
            "AUTOFIREPARTICLEMEDIUMSYSTEM" => {
                data.auto_fire_particle_medium_system = parse_string(&args)?;
            }
            "AUTOFIREPARTICLEMEDIUMMAX" => data.auto_fire_particle_medium_max = parse_int(&args)?,
            "AUTOFIREPARTICLELARGEPREFIX" => {
                data.auto_fire_particle_large_prefix = parse_string(&args)?;
            }
            "AUTOFIREPARTICLELARGESYSTEM" => {
                data.auto_fire_particle_large_system = parse_string(&args)?;
            }
            "AUTOFIREPARTICLELARGEMAX" => data.auto_fire_particle_large_max = parse_int(&args)?,
            "AUTOSMOKEPARTICLESMALLPREFIX" => {
                data.auto_smoke_particle_small_prefix = parse_string(&args)?;
            }
            "AUTOSMOKEPARTICLESMALLSYSTEM" => {
                data.auto_smoke_particle_small_system = parse_string(&args)?;
            }
            "AUTOSMOKEPARTICLESMALLMAX" => data.auto_smoke_particle_small_max = parse_int(&args)?,
            "AUTOSMOKEPARTICLEMEDIUMPREFIX" => {
                data.auto_smoke_particle_medium_prefix = parse_string(&args)?;
            }
            "AUTOSMOKEPARTICLEMEDIUMSYSTEM" => {
                data.auto_smoke_particle_medium_system = parse_string(&args)?;
            }
            "AUTOSMOKEPARTICLEMEDIUMMAX" => data.auto_smoke_particle_medium_max = parse_int(&args)?,
            "AUTOSMOKEPARTICLELARGEPREFIX" => {
                data.auto_smoke_particle_large_prefix = parse_string(&args)?;
            }
            "AUTOSMOKEPARTICLELARGESYSTEM" => {
                data.auto_smoke_particle_large_system = parse_string(&args)?;
            }
            "AUTOSMOKEPARTICLELARGEMAX" => data.auto_smoke_particle_large_max = parse_int(&args)?,
            "AUTOAFLAMEPARTICLEPREFIX" => data.auto_aflame_particle_prefix = parse_string(&args)?,
            "AUTOAFLAMEPARTICLESYSTEM" => {
                data.auto_aflame_particle_system = parse_string(&args)?;
            }
            "AUTOAFLAMEPARTICLEMAX" => data.auto_aflame_particle_max = parse_int(&args)?,
            "BUILDSPEED" => data.build_speed = parse_real(&args)?,
            "MINDISTFROMEDGEOFMAPFORBUILD" => {
                data.min_dist_from_edge_of_map_for_build = parse_real(&args)?;
            }
            "SUPPLYBUILDBORDER" => data.supply_build_border = parse_real(&args)?,
            "ALLOWEDHEIGHTVARIATIONFORBUILDING" => {
                data.allowed_height_variation_for_building = parse_real(&args)?;
            }
            "MINLOWENERGYPRODUCTIONSPEED" => {
                data.min_low_energy_production_speed = parse_real(&args)?
            }
            "MAXLOWENERGYPRODUCTIONSPEED" => {
                data.max_low_energy_production_speed = parse_real(&args)?
            }
            "LOWENERGYPENALTYMODIFIER" => data.low_energy_penalty_modifier = parse_real(&args)?,
            "MULTIPLEFACTORY" => data.multiple_factory = parse_real(&args)?,
            "REFUNDPERCENT" => data.refund_percent = parse_percent(&args)?,
            "COMMANDCENTERHEALRANGE" => data.command_center_heal_range = parse_real(&args)?,
            "COMMANDCENTERHEALAMOUNT" => data.command_center_heal_amount = parse_real(&args)?,
            "STANDARDMINEFIELDDENSITY" => data.standard_minefield_density = parse_real(&args)?,
            "STANDARDMINEFIELDDISTANCE" => data.standard_minefield_distance = parse_real(&args)?,
            "MAXLINEBUILDOBJECTS" => data.max_line_build_objects = parse_int(&args)?,
            "MAXTUNNELCAPACITY" => data.max_tunnel_capacity = parse_int(&args)?,
            "MAXPARTICLECOUNT" => data.max_particle_count = parse_int(&args)?,
            "MAXFIELDPARTICLECOUNT" => data.max_field_particle_count = parse_int(&args)?,
            "HORIZONTALSCROLLSPEEDFACTOR" => {
                data.horizontal_scroll_speed_factor = parse_real(&args)?
            }
            "VERTICALSCROLLSPEEDFACTOR" => data.vertical_scroll_speed_factor = parse_real(&args)?,
            "SCROLLAMOUNTCUTOFF" => data.scroll_amount_cutoff = parse_real(&args)?,
            "CAMERAADJUSTSPEED" => data.camera_adjust_speed = parse_real(&args)?,
            "ENFORCEMAXCAMERAHEIGHT" => data.enforce_max_camera_height = parse_bool(&args)?,
            "KEYBOARDSCROLLSPEEDFACTOR" => data.keyboard_scroll_factor = parse_real(&args)?,
            "KEYBOARDDEFAULTSCROLLSPEEDFACTOR" => {
                data.keyboard_default_scroll_factor = parse_real(&args)?;
            }
            "MOVEMENTPENALTYDAMAGESTATE" => {
                data.movement_penalty_damage_state = parse_body_damage_type(&args)?;
            }
            "HEALTHBONUS_VETERAN" => data.health_bonus[0] = parse_percent(&args)?,
            "HEALTHBONUS_ELITE" => data.health_bonus[1] = parse_percent(&args)?,
            "HEALTHBONUS_HEROIC" => data.health_bonus[2] = parse_percent(&args)?,
            "HUMANSOLOPLAYERHEALTHBONUS_EASY" => {
                data.solo_player_health_bonus_for_difficulty[0][0] = parse_percent(&args)?;
            }
            "HUMANSOLOPLAYERHEALTHBONUS_NORMAL" => {
                data.solo_player_health_bonus_for_difficulty[0][1] = parse_percent(&args)?;
            }
            "HUMANSOLOPLAYERHEALTHBONUS_HARD" => {
                data.solo_player_health_bonus_for_difficulty[0][2] = parse_percent(&args)?;
            }
            "AISOLOPLAYERHEALTHBONUS_EASY" => {
                data.solo_player_health_bonus_for_difficulty[1][0] = parse_percent(&args)?;
            }
            "AISOLOPLAYERHEALTHBONUS_NORMAL" => {
                data.solo_player_health_bonus_for_difficulty[1][1] = parse_percent(&args)?;
            }
            "AISOLOPLAYERHEALTHBONUS_HARD" => {
                data.solo_player_health_bonus_for_difficulty[1][2] = parse_percent(&args)?;
            }
            "DEFAULTSTRUCTURERUBBLEHEIGHT" => {
                data.default_structure_rubble_height = parse_real(&args)?;
            }
            "FIXEDSEED" => data.fixed_seed = parse_int(&args)?,
            "SHELLMAPNAME" => data.shell_map_name = parse_string(&args)?,
            "SHELLMAPON" => data.shell_map_on = parse_bool(&args)?,
            "PLAYINTRO" => data.play_intro = parse_bool(&args)?,
            "FIREWALLBEHAVIOR" => data.firewall_behavior = parse_u32(&args)?,
            "FIREWALLPORTOVERRIDE" => data.firewall_port_override = parse_u32(&args)?,
            "FIREWALLPORTALLOCATIONDELTA" => {
                data.firewall_port_allocation_delta = parse_i16(&args)?;
            }
            "GROUPSELECTMINSELECTSIZE" => data.group_select_min_select_size = parse_int(&args)?,
            "GROUPSELECTVOLUMEBASE" => data.group_select_volume_base = parse_real(&args)?,
            "GROUPSELECTVOLUMEINCREMENT" => data.group_select_volume_increment = parse_real(&args)?,
            "MAXUNITSELECTSOUNDS" => data.max_unit_select_sounds = parse_int(&args)?,
            "SELECTIONFLASHSATURATIONFACTOR" => {
                data.selection_flash_saturation_factor = parse_real(&args)?;
            }
            "SELECTIONFLASHHOUSECOLOR" => data.selection_flash_house_color = parse_bool(&args)?,
            "CAMERAAUDIBLERADIUS" => data.camera_audible_radius = parse_real(&args)?,
            "GROUPMOVECLICKTOGATHERAREAFACTOR" => {
                data.group_move_click_to_gather_factor = parse_real(&args)?;
            }
            "SHAKESUBTLEINTENSITY" => data.shake_subtle_intensity = parse_real(&args)?,
            "SHAKENORMALINTENSITY" => data.shake_normal_intensity = parse_real(&args)?,
            "SHAKESTRONGINTENSITY" => data.shake_strong_intensity = parse_real(&args)?,
            "SHAKESEVEREINTENSITY" => data.shake_severe_intensity = parse_real(&args)?,
            "SHAKECINEEXTREMEINTENSITY" => data.shake_cine_extreme_intensity = parse_real(&args)?,
            "SHAKECINEINSANEINTENSITY" => data.shake_cine_insane_intensity = parse_real(&args)?,
            "MAXSHAKEINTENSITY" => data.max_shake_intensity = parse_real(&args)?,
            "MAXSHAKERANGE" => data.max_shake_range = parse_real(&args)?,
            "SELLPERCENTAGE" => data.sell_percentage = parse_percent(&args)?,
            "BASEREGENHEALTHPERCENTPERSECOND" => {
                data.base_regen_health_percent_per_second = parse_percent(&args)?;
            }
            "BASEREGENDELAY" => data.base_regen_delay = parse_duration_u32(&args)?,
            "PRISONBOUNTYMULTIPLIER" => data.prison_bounty_multiplier = parse_real(&args)?,
            "PRISONBOUNTYTEXTCOLOR" => {
                data.prison_bounty_text_color = parse_color_int(&args)?;
            }
            "SPECIALPOWERVIEWOBJECT" => {
                data.special_power_view_object_name = parse_string(&args)?;
            }
            "STANDARDPUBLICBONE" => data.standard_public_bones.push(parse_string(&args)?),
            "SHOWMETRICS" => data.show_metrics = parse_bool(&args)?,
            "DEFAULTSTARTINGCASH" => data.default_starting_cash = parse_u32(&args)?,
            "SHROUDCOLOR" => data.shroud_color = parse_rgb_color(&args)?,
            "CLEARALPHA" => data.clear_alpha = parse_u8(&args)?,
            "FOGALPHA" => data.fog_alpha = parse_u8(&args)?,
            "SHROUDALPHA" => data.shroud_alpha = parse_u8(&args)?,
            "HOTKEYTEXTCOLOR" => data.hot_key_text_color = parse_color_int(&args)?,
            "POWERBARBASE" => data.power_bar_base = parse_int(&args)?,
            "POWERBARINTERVALS" => data.power_bar_intervals = parse_real(&args)?,
            "POWERBARYELLOWRANGE" => data.power_bar_yellow_range = parse_int(&args)?,
            "UNLOOKPERSISTDURATION" => data.unlook_persist_duration = parse_duration_u32(&args)?,
            "NETWORKFPSHISTORYLENGTH" => data.network_fps_history_length = parse_u32(&args)?,
            "NETWORKLATENCYHISTORYLENGTH" => {
                data.network_latency_history_length = parse_u32(&args)?
            }
            "NETWORKRUNAHEADMETRICSTIME" => {
                data.network_run_ahead_metrics_time = parse_u32(&args)?;
            }
            "NETWORKCUSHIONHISTORYLENGTH" => {
                data.network_cushion_history_length = parse_u32(&args)?;
            }
            "NETWORKRUNAHEADSLACK" => data.network_run_ahead_slack = parse_u32(&args)?,
            "NETWORKKEEPALIVEDELAY" => data.network_keep_alive_delay = parse_u32(&args)?,
            "NETWORKDISCONNECTTIME" => data.network_disconnect_time = parse_u32(&args)?,
            "NETWORKPLAYERTIMEOUTTIME" => data.network_player_timeout_time = parse_u32(&args)?,
            "NETWORKDISCONNECTSCREENNOTIFYTIME" => {
                data.network_disconnect_screen_notify_time = parse_u32(&args)?;
            }
            "KEYBOARDCAMERAROTATESPEED" => data.keyboard_camera_rotate_speed = parse_real(&args)?,
            "PLAYSTATS" => data.play_stats = parse_int(&args)?,
            "WEAPONBONUS" => data.weapon_bonus_entries.push(parse_weapon_bonus(&args)?),
            "LEVELGAINANIMATIONNAME" => data.level_gain_animation_name = parse_string(&args)?,
            "LEVELGAINANIMATIONTIME" => {
                data.level_gain_animation_display_time_in_seconds = parse_real(&args)?;
            }
            "LEVELGAINANIMATIONZRISE" => {
                data.level_gain_animation_z_rise_per_second = parse_real(&args)?;
            }
            "GETHEALEDANIMATIONNAME" => data.get_healed_animation_name = parse_string(&args)?,
            "GETHEALEDANIMATIONTIME" => {
                data.get_healed_animation_display_time_in_seconds = parse_real(&args)?;
            }
            "GETHEALEDANIMATIONZRISE" => {
                data.get_healed_animation_z_rise_per_second = parse_real(&args)?;
            }
            _ => return Err(INIError::UnknownToken),
        }
    }
    let _ = data.set_time_of_day(data.time_of_day);
    sync_runtime_global_data_from_ini(data);
    Ok(())
}

fn sync_runtime_global_data_from_ini(data: &GlobalData) {
    let Ok(mut runtime) = runtime_global_data::write_safe() else {
        return;
    };

    runtime.move_hint_name = data.move_hint_name.clone();
    runtime.use_trees = data.use_trees;
    runtime.use_cloud_map = data.use_cloud_map;
    runtime.use_3way_terrain_blends = data.use_3way_terrain_blends;
    runtime.use_light_map = data.use_light_map;
    runtime.bilinear_terrain_tex = data.bilinear_terrain_tex;
    runtime.trilinear_terrain_tex = data.trilinear_terrain_tex;
    runtime.multi_pass_terrain = data.multipass_terrain;
    runtime.adjust_cliff_textures = data.adjust_cliff_textures;
    runtime.stretch_terrain = data.stretch_terrain;
    runtime.use_half_height_map = data.use_half_height_map;
    runtime.draw_entire_terrain = data.draw_entire_terrain;
    runtime.terrain_lod_target_time_ms = data.terrain_lod_target_time_ms;
    runtime.right_mouse_always_scrolls = data.right_mouse_always_scrolls;
    runtime.use_water_plane = data.use_water_plane;
    runtime.use_cloud_plane = data.use_cloud_plane;
    runtime.writable.use_shadow_volumes = data.use_shadow_volumes;
    runtime.writable.use_shadow_decals = data.use_shadow_decals;
    runtime.texture_reduction_factor = data.texture_reduction_factor;
    runtime.enable_behind_building_markers = data.enable_behind_building_markers;
    runtime.water_position_x = data.water_position_x;
    runtime.water_position_y = data.water_position_y;
    runtime.water_position_z = data.water_position_z;
    runtime.water_extent_x = data.water_extent_x;
    runtime.water_extent_y = data.water_extent_y;
    runtime.water_type = data.water_type;
    runtime.feather_water = data.feather_water;
    runtime.show_soft_water_edge = data.show_soft_water_edge;
    runtime.downwind_angle = data.downwind_angle;
    runtime.sky_box_position_z = data.sky_box_position_z;
    runtime.draw_sky_box = data.draw_sky_box != 0.0;
    runtime.sky_box_scale = data.sky_box_scale;
    runtime.camera_pitch = data.camera_pitch;
    runtime.camera_yaw = data.camera_yaw;
    runtime.camera_height = data.camera_height;
    runtime.max_camera_height = data.max_camera_height;
    runtime.min_camera_height = data.min_camera_height;
    runtime.terrain_height_at_edge_of_map = data.terrain_height_at_edge_of_map;
    runtime.unit_damaged_thresh = data.unit_damaged_thresh;
    runtime.unit_really_damaged_thresh = data.unit_really_damaged_thresh;
    runtime.ground_stiffness = data.ground_stiffness;
    runtime.structure_stiffness = data.structure_stiffness;
    runtime.gravity = data.gravity;
    runtime.stealth_friendly_opacity = data.stealth_friendly_opacity;
    runtime.default_occlusion_delay = data.default_occlusion_delay;
    runtime.partition_cell_size = data.partition_cell_size;
    runtime.ammo_pip_world_offset = [
        data.ammo_pip_world_offset.x,
        data.ammo_pip_world_offset.y,
        data.ammo_pip_world_offset.z,
    ];
    runtime.container_pip_world_offset = [
        data.container_pip_world_offset.x,
        data.container_pip_world_offset.y,
        data.container_pip_world_offset.z,
    ];
    runtime.ammo_pip_screen_offset = [data.ammo_pip_screen_offset.x, data.ammo_pip_screen_offset.y];
    runtime.container_pip_screen_offset = [
        data.container_pip_screen_offset.x,
        data.container_pip_screen_offset.y,
    ];
    runtime.ammo_pip_scale_factor = data.ammo_pip_scale_factor;
    runtime.container_pip_scale_factor = data.container_pip_scale_factor;
    runtime.historic_damage_limit = data.historic_damage_limit;
    runtime.max_terrain_tracks = data.max_terrain_tracks;
    runtime.level_gain_animation_name = data.level_gain_animation_name.clone();
    runtime.level_gain_animation_display_time_seconds =
        data.level_gain_animation_display_time_in_seconds;
    runtime.level_gain_animation_z_rise_per_second = data.level_gain_animation_z_rise_per_second;
    runtime.get_healed_animation_name = data.get_healed_animation_name.clone();
    runtime.get_healed_animation_display_time_seconds =
        data.get_healed_animation_display_time_in_seconds;
    runtime.get_healed_animation_z_rise_per_second = data.get_healed_animation_z_rise_per_second;
    runtime.make_track_marks = data.make_track_marks;
    runtime.hide_garrison_flags = data.hide_garrison_flags;
    runtime.force_models_to_follow_time_of_day = data.force_models_to_follow_time_of_day;
    runtime.force_models_to_follow_weather = data.force_models_to_follow_weather;
    runtime.num_global_lights = data.num_global_lights;
    runtime.max_visible_translucent_objects = data.max_visible_translucent_objects;
    runtime.occluded_luminance_scale = data.occluded_luminance_scale;
    runtime.max_road_segments = data.max_road_segments;
    runtime.max_road_vertex = data.max_road_vertex;
    runtime.max_road_index = data.max_road_index;
    runtime.max_road_types = data.max_road_types;
    runtime.base_value_per_supply_box = data.base_value_per_supply_box;
    runtime.particle_scale = data.particle_scale;
    runtime.auto_fire_particle_small_prefix = data.auto_fire_particle_small_prefix.clone();
    runtime.auto_fire_particle_small_system = data.auto_fire_particle_small_system.clone();
    runtime.auto_fire_particle_small_max = data.auto_fire_particle_small_max;
    runtime.auto_fire_particle_medium_prefix = data.auto_fire_particle_medium_prefix.clone();
    runtime.auto_fire_particle_medium_system = data.auto_fire_particle_medium_system.clone();
    runtime.auto_fire_particle_medium_max = data.auto_fire_particle_medium_max;
    runtime.auto_fire_particle_large_prefix = data.auto_fire_particle_large_prefix.clone();
    runtime.auto_fire_particle_large_system = data.auto_fire_particle_large_system.clone();
    runtime.auto_fire_particle_large_max = data.auto_fire_particle_large_max;
    runtime.auto_smoke_particle_small_prefix = data.auto_smoke_particle_small_prefix.clone();
    runtime.auto_smoke_particle_small_system = data.auto_smoke_particle_small_system.clone();
    runtime.auto_smoke_particle_small_max = data.auto_smoke_particle_small_max;
    runtime.auto_smoke_particle_medium_prefix = data.auto_smoke_particle_medium_prefix.clone();
    runtime.auto_smoke_particle_medium_system = data.auto_smoke_particle_medium_system.clone();
    runtime.auto_smoke_particle_medium_max = data.auto_smoke_particle_medium_max;
    runtime.auto_smoke_particle_large_prefix = data.auto_smoke_particle_large_prefix.clone();
    runtime.auto_smoke_particle_large_system = data.auto_smoke_particle_large_system.clone();
    runtime.auto_smoke_particle_large_max = data.auto_smoke_particle_large_max;
    runtime.auto_aflame_particle_prefix = data.auto_aflame_particle_prefix.clone();
    runtime.auto_aflame_particle_system = data.auto_aflame_particle_system.clone();
    runtime.auto_aflame_particle_max = data.auto_aflame_particle_max;
    runtime.build_speed = data.build_speed;
    runtime.min_dist_from_edge_of_map_for_build = data.min_dist_from_edge_of_map_for_build;
    runtime.supply_build_border = data.supply_build_border;
    runtime.allowed_height_variation_for_building = data.allowed_height_variation_for_building;
    runtime.min_low_energy_production_speed = data.min_low_energy_production_speed;
    runtime.max_low_energy_production_speed = data.max_low_energy_production_speed;
    runtime.low_energy_penalty_modifier = data.low_energy_penalty_modifier;
    runtime.multiple_factory = data.multiple_factory;
    runtime.refund_percent = data.refund_percent;
    runtime.command_center_heal_range = data.command_center_heal_range;
    runtime.command_center_heal_amount = data.command_center_heal_amount;
    runtime.standard_minefield_density = data.standard_minefield_density;
    runtime.standard_minefield_distance = data.standard_minefield_distance;
    runtime.max_line_build_objects = data.max_line_build_objects;
    runtime.max_tunnel_capacity = data.max_tunnel_capacity;
    runtime.max_particle_count = data.max_particle_count;
    runtime.max_field_particle_count = data.max_field_particle_count;
    runtime.horizontal_scroll_speed_factor = data.horizontal_scroll_speed_factor;
    runtime.vertical_scroll_speed_factor = data.vertical_scroll_speed_factor;
    runtime.scroll_amount_cutoff = data.scroll_amount_cutoff;
    runtime.camera_adjust_speed = data.camera_adjust_speed;
    runtime.enforce_max_camera_height = data.enforce_max_camera_height;
    runtime.keyboard_scroll_factor = data.keyboard_scroll_factor;
    runtime.keyboard_default_scroll_factor = data.keyboard_default_scroll_factor;
    runtime.movement_penalty_damage_state = match data.movement_penalty_damage_state {
        BodyDamageType::Pristine => 0,
        BodyDamageType::Damaged => 1,
        BodyDamageType::ReallyDamaged => 2,
        BodyDamageType::Rubble => 3,
    };
    runtime.health_bonus[0] = data.health_bonus[0];
    runtime.health_bonus[1] = data.health_bonus[1];
    runtime.health_bonus[2] = data.health_bonus[2];
    for player in 0..runtime_global_data::PLAYERTYPE_COUNT {
        for difficulty in 0..runtime_global_data::DIFFICULTY_COUNT {
            runtime.solo_player_health_bonus_for_difficulty[player][difficulty] =
                data.solo_player_health_bonus_for_difficulty[player][difficulty];
        }
    }
    runtime.default_structure_rubble_height = data.default_structure_rubble_height;
    runtime.writable.fixed_seed = data.fixed_seed;
    runtime.writable.shell_map_name = data.shell_map_name.clone();
    runtime.writable.shell_map_on = data.shell_map_on;
    runtime.writable.play_intro = data.play_intro;
    runtime.writable.play_sizzle = data.play_sizzle;
    runtime.writable.after_intro = data.after_intro;
    runtime.writable.allow_exit_out_of_movies = data.allow_exit_out_of_movies;
    runtime.group_select_min_select_size = data.group_select_min_select_size;
    runtime.group_select_volume_base = data.group_select_volume_base;
    runtime.group_select_volume_increment = data.group_select_volume_increment;
    runtime.max_unit_select_sounds = data.max_unit_select_sounds;
    runtime.selection_flash_saturation_factor = data.selection_flash_saturation_factor;
    runtime.selection_flash_house_color = data.selection_flash_house_color;
    runtime.camera_audible_radius = data.camera_audible_radius;
    runtime.group_move_click_to_gather_factor = data.group_move_click_to_gather_factor;
    runtime.shake_subtle_intensity = data.shake_subtle_intensity;
    runtime.shake_normal_intensity = data.shake_normal_intensity;
    runtime.shake_strong_intensity = data.shake_strong_intensity;
    runtime.shake_severe_intensity = data.shake_severe_intensity;
    runtime.shake_cine_extreme_intensity = data.shake_cine_extreme_intensity;
    runtime.shake_cine_insane_intensity = data.shake_cine_insane_intensity;
    runtime.max_shake_intensity = data.max_shake_intensity;
    runtime.max_shake_range = data.max_shake_range;
    runtime.sell_percentage = data.sell_percentage;
    runtime.base_regen_health_percent_per_second = data.base_regen_health_percent_per_second;
    runtime.base_regen_delay = data.base_regen_delay;
    runtime.prison_bounty_multiplier = data.prison_bounty_multiplier;
    runtime.prison_bounty_text_color = [
        data.prison_bounty_text_color.r,
        data.prison_bounty_text_color.g,
        data.prison_bounty_text_color.b,
    ];
    runtime.special_power_view_object_name = data.special_power_view_object_name.clone();
    runtime.standard_public_bones = data.standard_public_bones.clone();
    runtime.show_metrics = data.show_metrics;
    runtime.default_starting_cash = data.default_starting_cash as i32;
    runtime.shroud_color = [
        data.shroud_color.r,
        data.shroud_color.g,
        data.shroud_color.b,
    ];
    runtime.clear_alpha = data.clear_alpha;
    runtime.fog_alpha = data.fog_alpha;
    runtime.shroud_alpha = data.shroud_alpha;
    runtime.hot_key_text_color = [
        data.hot_key_text_color.r,
        data.hot_key_text_color.g,
        data.hot_key_text_color.b,
        1.0,
    ];
    runtime.power_bar_base = data.power_bar_base;
    runtime.power_bar_intervals = data.power_bar_intervals;
    runtime.power_bar_yellow_range = data.power_bar_yellow_range;
    runtime.unlook_persist_duration = data.unlook_persist_duration;
    runtime.network_fps_history_length = data.network_fps_history_length;
    runtime.network_latency_history_length = data.network_latency_history_length;
    runtime.network_run_ahead_metrics_time = data.network_run_ahead_metrics_time;
    runtime.network_cushion_history_length = data.network_cushion_history_length;
    runtime.network_run_ahead_slack = data.network_run_ahead_slack;
    runtime.network_keep_alive_delay = data.network_keep_alive_delay;
    runtime.network_disconnect_time = data.network_disconnect_time;
    runtime.network_player_timeout_time = data.network_player_timeout_time;
    runtime.network_disconnect_screen_notify_time = data.network_disconnect_screen_notify_time;
    runtime.keyboard_camera_rotate_speed = data.keyboard_camera_rotate_speed;
    runtime.play_stats = data.play_stats;

    runtime.time_of_day = match data.time_of_day {
        TimeOfDay::Invalid => runtime_global_data::TimeOfDay::Invalid,
        TimeOfDay::Morning => runtime_global_data::TimeOfDay::Morning,
        TimeOfDay::Afternoon => runtime_global_data::TimeOfDay::Afternoon,
        TimeOfDay::Evening => runtime_global_data::TimeOfDay::Evening,
        TimeOfDay::Night => runtime_global_data::TimeOfDay::Night,
    };
    runtime.weather = match data.weather {
        Weather::Normal => 0,
        Weather::Snowy => 1,
    };

    for time_index in 0..TIME_OF_DAY_COUNT.min(runtime_global_data::TIME_OF_DAY_COUNT) {
        for light_index in 0..MAX_GLOBAL_LIGHTS {
            let src = &data.terrain_lighting[time_index][light_index];
            runtime.terrain_lighting[time_index][light_index] =
                runtime_global_data::TerrainLighting {
                    ambient: [src.ambient.r, src.ambient.g, src.ambient.b],
                    diffuse: [src.diffuse.r, src.diffuse.g, src.diffuse.b],
                    light_pos: [src.light_pos.x, src.light_pos.y, src.light_pos.z],
                };
            let src_obj = &data.terrain_objects_lighting[time_index][light_index];
            runtime.terrain_objects_lighting[time_index][light_index] =
                runtime_global_data::TerrainLighting {
                    ambient: [src_obj.ambient.r, src_obj.ambient.g, src_obj.ambient.b],
                    diffuse: [src_obj.diffuse.r, src_obj.diffuse.g, src_obj.diffuse.b],
                    light_pos: [
                        src_obj.light_pos.x,
                        src_obj.light_pos.y,
                        src_obj.light_pos.z,
                    ],
                };
        }
    }

    for i in 0..MAX_GLOBAL_LIGHTS {
        runtime.terrain_ambient[i] = [
            data.terrain_ambient[i].r,
            data.terrain_ambient[i].g,
            data.terrain_ambient[i].b,
        ];
        runtime.terrain_diffuse[i] = [
            data.terrain_diffuse[i].r,
            data.terrain_diffuse[i].g,
            data.terrain_diffuse[i].b,
        ];
        runtime.terrain_light_pos[i] = [
            data.terrain_light_pos[i].x,
            data.terrain_light_pos[i].y,
            data.terrain_light_pos[i].z,
        ];
    }

    for i in 0..TIME_OF_DAY_COUNT.min(runtime_global_data::TIME_OF_DAY_COUNT) {
        runtime.infantry_light_scale[i] = data.infantry_light_scale[i];
    }
    runtime.script_override_infantry_light_scale = data.script_override_infantry_light_scale;
    runtime.num_global_lights = data.num_global_lights;
}

#[derive(Debug, Clone, Copy)]
enum LightingTarget {
    Terrain,
    Objects,
}

#[derive(Debug, Clone, Copy)]
enum LightingField {
    Ambient,
    Diffuse,
    LightPos,
}

fn parse_lighting_token(key: &str) -> Option<(LightingTarget, usize, usize, LightingField)> {
    let (target, rest) = if let Some(remainder) = key.strip_prefix("TerrainLighting") {
        (LightingTarget::Terrain, remainder)
    } else if let Some(remainder) = key.strip_prefix("TerrainObjectsLighting") {
        (LightingTarget::Objects, remainder)
    } else {
        return None;
    };

    let (rest, light_index) = parse_light_index(rest)?;
    let (time_index, field) = parse_time_field(rest)?;
    if matches!(field, LightingField::LightPos) {
        return None;
    }

    Some((target, time_index, light_index, field))
}

fn parse_lighting_pos_token(key: &str) -> Option<(LightingTarget, usize, usize)> {
    let (target, rest) = if let Some(remainder) = key.strip_prefix("TerrainLighting") {
        (LightingTarget::Terrain, remainder)
    } else if let Some(remainder) = key.strip_prefix("TerrainObjectsLighting") {
        (LightingTarget::Objects, remainder)
    } else {
        return None;
    };

    let (rest, light_index) = parse_light_index(rest)?;
    let (time_index, field) = parse_time_field(rest)?;
    if !matches!(field, LightingField::LightPos) {
        return None;
    }

    Some((target, time_index, light_index))
}

fn parse_infantry_light_token(key: &str) -> Option<usize> {
    key.strip_prefix("InfantryLight")
        .and_then(|rest| rest.strip_suffix("Scale"))
        .and_then(time_index_from_name)
}

fn parse_light_index(rest: &str) -> Option<(&str, usize)> {
    if let Some(stripped) = rest.strip_suffix('2') {
        return Some((stripped, 1));
    }
    if let Some(stripped) = rest.strip_suffix('3') {
        return Some((stripped, 2));
    }
    Some((rest, 0))
}

fn parse_time_field(rest: &str) -> Option<(usize, LightingField)> {
    let (time_name, field_name) = if let Some(remainder) = rest.strip_prefix("Morning") {
        ("MORNING", remainder)
    } else if let Some(remainder) = rest.strip_prefix("Afternoon") {
        ("AFTERNOON", remainder)
    } else if let Some(remainder) = rest.strip_prefix("Evening") {
        ("EVENING", remainder)
    } else if let Some(remainder) = rest.strip_prefix("Night") {
        ("NIGHT", remainder)
    } else {
        return None;
    };

    let time_index = time_index_from_name(time_name)?;
    let field = match field_name {
        "Ambient" => LightingField::Ambient,
        "Diffuse" => LightingField::Diffuse,
        "LightPos" => LightingField::LightPos,
        _ => return None,
    };

    Some((time_index, field))
}

fn time_index_from_name(name: &str) -> Option<usize> {
    match name.to_ascii_uppercase().as_str() {
        "MORNING" => Some(1),
        "AFTERNOON" => Some(2),
        "EVENING" => Some(3),
        "NIGHT" => Some(4),
        _ => None,
    }
}

fn parse_indexed_suffix(key: &str, prefix: &str) -> Option<usize> {
    let suffix = key.strip_prefix(prefix)?;
    let index = suffix.parse::<usize>().ok()?;
    if index == 0 || index > MAX_WATER_GRID_SETTINGS {
        return None;
    }
    Some(index - 1)
}

fn parse_bool(args: &[&str]) -> INIResult<bool> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    INI::parse_bool(value)
}

fn parse_bool_as_f32(args: &[&str]) -> INIResult<f32> {
    Ok(if parse_bool(args)? { 1.0 } else { 0.0 })
}

fn parse_int(args: &[&str]) -> INIResult<i32> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    INI::parse_int(value)
}

fn parse_u32(args: &[&str]) -> INIResult<u32> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    INI::parse_unsigned_int(value)
}

fn parse_u8(args: &[&str]) -> INIResult<u8> {
    let value = parse_u32(args)?;
    u8::try_from(value).map_err(|_| INIError::InvalidData)
}

fn parse_i16(args: &[&str]) -> INIResult<i16> {
    let value = parse_int(args)?;
    i16::try_from(value).map_err(|_| INIError::InvalidData)
}

fn parse_real(args: &[&str]) -> INIResult<f32> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    INI::parse_real(value)
}

fn parse_string(args: &[&str]) -> INIResult<String> {
    let joined = args.join(" ");
    INI::parse_ascii_string(&joined)
}

fn parse_percent(args: &[&str]) -> INIResult<f32> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    INI::parse_percent_to_real(value)
}

fn parse_angle(args: &[&str]) -> INIResult<f32> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    INI::parse_angle_real(value)
}

fn parse_acceleration(args: &[&str]) -> INIResult<f32> {
    let value = parse_real(args)?;
    Ok(INI::convert_acceleration_secs_to_frames(value))
}

fn parse_duration_u32(args: &[&str]) -> INIResult<u32> {
    let value = parse_u32(args)? as f32;
    Ok(INI::convert_duration_msecs_to_frames(value).ceil() as u32)
}

fn parse_rgb_color(args: &[&str]) -> INIResult<RGBColor> {
    let (r, g, b) = INI::parse_rgb_color(args)?;
    Ok(RGBColor::new(r, g, b))
}

fn parse_color_int(args: &[&str]) -> INIResult<RGBColor> {
    let mut r = None;
    let mut g = None;
    let mut b = None;
    let mut i = 0;

    while i < args.len() {
        let token = args[i];
        let (key, value) = if let Some((left, right)) = token.split_once(':') {
            if right.is_empty() {
                i += 1;
                if i >= args.len() {
                    return Err(INIError::InvalidData);
                }
                (left, args[i])
            } else {
                (left, right)
            }
        } else {
            if i + 1 >= args.len() {
                return Err(INIError::InvalidData);
            }
            (token.trim_end_matches(':'), args[i + 1])
        };

        let value: i32 = value.parse().map_err(|_| INIError::InvalidData)?;
        if value < 0 || value > 255 {
            return Err(INIError::InvalidData);
        }

        match key.to_ascii_uppercase().as_str() {
            "R" => r = Some(value),
            "G" => g = Some(value),
            "B" => b = Some(value),
            _ => {}
        }

        i += 1;
    }

    let r = r.ok_or(INIError::InvalidData)?;
    let g = g.ok_or(INIError::InvalidData)?;
    let b = b.ok_or(INIError::InvalidData)?;

    Ok(RGBColor::new(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
    ))
}

fn parse_coord3d(args: &[&str]) -> INIResult<Coord3D> {
    let (x, y, z) = INI::parse_coord_3d(args)?;
    Ok(Coord3D::new(x, y, z))
}

fn parse_coord2d(args: &[&str]) -> INIResult<Coord2D> {
    let (x, y) = INI::parse_coord_2d(args)?;
    Ok(Coord2D::new(x, y))
}

fn parse_time_of_day(args: &[&str]) -> INIResult<TimeOfDay> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    match value.to_ascii_uppercase().as_str() {
        "NONE" | "INVALID" => Ok(TimeOfDay::Invalid),
        "MORNING" => Ok(TimeOfDay::Morning),
        "AFTERNOON" => Ok(TimeOfDay::Afternoon),
        "EVENING" => Ok(TimeOfDay::Evening),
        "NIGHT" => Ok(TimeOfDay::Night),
        _ => Err(INIError::InvalidData),
    }
}

fn parse_weather(args: &[&str]) -> INIResult<Weather> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    match value.to_ascii_uppercase().as_str() {
        "NORMAL" => Ok(Weather::Normal),
        "SNOWY" => Ok(Weather::Snowy),
        _ => Err(INIError::InvalidData),
    }
}

fn parse_body_damage_type(args: &[&str]) -> INIResult<BodyDamageType> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    match value.to_ascii_uppercase().as_str() {
        "PRISTINE" => Ok(BodyDamageType::Pristine),
        "DAMAGED" => Ok(BodyDamageType::Damaged),
        "REALLYDAMAGED" => Ok(BodyDamageType::ReallyDamaged),
        "RUBBLE" => Ok(BodyDamageType::Rubble),
        _ => Err(INIError::InvalidData),
    }
}

fn parse_terrain_lod(args: &[&str]) -> INIResult<i32> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    let lod = match value.to_ascii_uppercase().as_str() {
        "DISABLE" => 0,
        "LOW" => 1,
        "MEDIUM" => 2,
        "HIGH" => 3,
        _ => INI::parse_int(value)?,
    };
    Ok(lod)
}

fn parse_weapon_bonus(args: &[&str]) -> INIResult<WeaponBonusEntry> {
    if args.len() < 3 {
        return Err(INIError::InvalidData);
    }
    let condition = args[0].to_string();
    let field = args[1].to_string();
    let value_token = args[2];
    let value = if value_token.contains('%') {
        INI::parse_percent_to_real(value_token)?
    } else {
        INI::parse_real(value_token)?
    };

    Ok(WeaponBonusEntry {
        condition,
        field,
        value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_data_creation() {
        let global_data = GlobalData::new();
        assert_eq!(global_data.x_resolution, 1024);
        assert_eq!(global_data.y_resolution, 768);
        assert_eq!(global_data.frames_per_second_limit, 30);
        assert!(global_data.use_trees);
        assert!(global_data.audio_on);
        assert!(global_data.play_intro);
        assert!(global_data.play_sizzle);
        assert!(!global_data.after_intro);
        assert!(!global_data.allow_exit_out_of_movies);
    }

    #[test]
    fn test_apply_game_data_copies_startup_movie_runtime_flags() {
        let mut data = GlobalData::new();
        data.play_intro = false;
        data.play_sizzle = true;
        data.after_intro = true;
        data.allow_exit_out_of_movies = true;
        data.shell_map_name = "ShellMap".to_string();
        data.shell_map_on = false;

        let mut runtime = runtime_global_data::RuntimeGlobalData::default();
        apply_to_runtime_global_data(&data, &mut runtime);

        assert!(!runtime.writable.play_intro);
        assert!(runtime.writable.play_sizzle);
        assert!(runtime.writable.after_intro);
        assert!(runtime.writable.allow_exit_out_of_movies);
        assert_eq!(runtime.writable.shell_map_name, "ShellMap");
        assert!(!runtime.writable.shell_map_on);
    }

    #[test]
    fn test_time_of_day() {
        let mut global_data = GlobalData::new();
        assert_eq!(global_data.time_of_day, TimeOfDay::Afternoon);

        assert!(global_data.set_time_of_day(TimeOfDay::Night));
        assert_eq!(global_data.time_of_day, TimeOfDay::Night);
    }

    #[test]
    fn test_coordinates() {
        let coord3d = Coord3D::new(10.0, 20.0, 30.0);
        assert_eq!(coord3d.x, 10.0);
        assert_eq!(coord3d.y, 20.0);
        assert_eq!(coord3d.z, 30.0);

        let coord2d = Coord2D::new(100.0, 200.0);
        assert_eq!(coord2d.x, 100.0);
        assert_eq!(coord2d.y, 200.0);
    }

    #[test]
    fn test_rgb_color() {
        let red = RGBColor::new(1.0, 0.0, 0.0);
        assert_eq!(red.r, 1.0);
        assert_eq!(red.g, 0.0);
        assert_eq!(red.b, 0.0);

        let white = RGBColor::white();
        assert_eq!(white.r, 1.0);
        assert_eq!(white.g, 1.0);
        assert_eq!(white.b, 1.0);
    }

    #[test]
    fn test_terrain_lighting() {
        let lighting = TerrainLighting::default();
        assert!(lighting.ambient.r > 0.0);
        assert!(lighting.diffuse.r > 0.0);
        assert_eq!(lighting.light_pos.z, 100.0);
    }

    #[test]
    fn test_ai_debug_options() {
        let mut debug = AIDebugOptions::new();
        assert_eq!(debug.value, 0);

        debug.set_flag(1, true);
        assert!(debug.has_flag(1));
        assert!(!debug.has_flag(2));

        debug.set_flag(1, false);
        assert!(!debug.has_flag(1));
    }

    #[test]
    fn test_global_instance() {
        init_global_data();
        let handle = ensure_global_data();

        {
            let mut global_data = handle.write();
            global_data.x_resolution = 1920;
        }

        let global_data = handle.read();
        assert_eq!(global_data.x_resolution, 1920);
    }

    #[test]
    fn test_enumerations() {
        assert_eq!(TimeOfDay::default(), TimeOfDay::Afternoon);
        assert_eq!(Weather::default(), Weather::Normal);

        let tod = TimeOfDay::Morning;
        let weather = Weather::Snowy;
        assert_ne!(tod, TimeOfDay::default());
        assert_ne!(weather, Weather::default());
    }
}
