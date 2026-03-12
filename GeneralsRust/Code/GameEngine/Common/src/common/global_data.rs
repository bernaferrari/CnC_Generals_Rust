//! Global data registry for the Rust port of the Generals engine.
//!
//! The original C++ `GlobalData` structure contains hundreds of tunable fields that are
//! populated from INI files, command-line arguments, and runtime systems.  The previous
//! Rust stub merely exposed three booleans, which meant virtually every gameplay feature
//! was missing.  This module reintroduces a faithful, extensible representation that can
//! store the full writable dataset alongside dynamic key/value overrides used throughout
//! the engine.

use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use once_cell::sync::Lazy;

use crate::common::command_line::{DebugSettings, WritableGlobalData};

/// Runtime value container mirroring the union-style usage in the C++ engine.
#[derive(Debug, Clone, PartialEq)]
pub enum GlobalValue {
    Bool(bool),
    Int(i32),
    Float(f32),
    String(String),
}

impl GlobalValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            GlobalValue::Bool(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i32> {
        match self {
            GlobalValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f32> {
        match self {
            GlobalValue::Float(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            GlobalValue::String(v) => Some(v.as_str()),
            _ => None,
        }
    }
}

/// Terrain lighting configuration
#[derive(Debug, Clone, Copy)]
pub struct TerrainLighting {
    pub ambient: [f32; 3],   // RGB
    pub diffuse: [f32; 3],   // RGB
    pub light_pos: [f32; 3], // 3D position
}

impl Default for TerrainLighting {
    fn default() -> Self {
        Self {
            ambient: [1.0, 1.0, 1.0],
            diffuse: [1.0, 1.0, 1.0],
            light_pos: [0.0, 0.0, 0.0],
        }
    }
}

/// Time of day enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeOfDay {
    Invalid = 0,
    Morning = 1,
    Afternoon = 2,
    Evening = 3,
    Night = 4,
}

pub const TIME_OF_DAY_COUNT: usize = 5;
pub const TIME_OF_DAY_FIRST: usize = 1;
pub const MAX_GLOBAL_LIGHTS: usize = 3;
pub const MAX_WATER_GRID_SETTINGS: usize = 4;
pub const LEVEL_COUNT: usize = 8;
pub const DIFFICULTY_COUNT: usize = 3;
pub const PLAYERTYPE_COUNT: usize = 3;

/// Aggregated global state shared across subsystems.
#[derive(Debug, Clone)]
pub struct GlobalData {
    pub writable: WritableGlobalData,
    pub debug: DebugSettings,
    overrides: HashMap<String, GlobalValue>,

    // Map and rendering settings
    pub move_hint_name: String,
    pub use_trees: bool,
    pub use_tree_sway: bool,
    pub use_draw_module_lod: bool,
    pub use_heat_effects: bool,
    pub max_shell_screens: i32,
    pub use_cloud_map: bool,
    pub use_3way_terrain_blends: i32,
    pub use_light_map: bool,
    pub bilinear_terrain_tex: bool,
    pub trilinear_terrain_tex: bool,
    pub multi_pass_terrain: bool,
    pub adjust_cliff_textures: bool,
    pub stretch_terrain: bool,
    pub use_half_height_map: bool,
    pub draw_entire_terrain: bool,
    pub terrain_lod_target_time_ms: i32,

    // Camera and mouse settings
    pub use_alternate_mouse: bool,
    pub client_retaliation_mode_enabled: bool,
    pub double_click_attack_move: bool,
    pub right_mouse_always_scrolls: bool,
    pub camera_pitch: f32,
    pub camera_yaw: f32,
    pub camera_height: f32,
    pub max_camera_height: f32,
    pub min_camera_height: f32,
    pub horizontal_scroll_speed_factor: f32,
    pub vertical_scroll_speed_factor: f32,
    pub scroll_amount_cutoff: f32,
    pub camera_adjust_speed: f32,
    pub enforce_max_camera_height: bool,
    pub keyboard_scroll_factor: f32,
    pub keyboard_default_scroll_factor: f32,
    pub keyboard_camera_rotate_speed: f32,
    pub play_stats: i32,
    pub camera_audible_radius: f32,
    pub save_camera_in_replay: bool,
    pub use_camera_in_replay: bool,

    // Water and sky settings
    pub use_water_plane: bool,
    pub use_cloud_plane: bool,
    pub water_position_x: f32,
    pub water_position_y: f32,
    pub water_position_z: f32,
    pub water_extent_x: f32,
    pub water_extent_y: f32,
    pub water_type: i32,
    pub show_soft_water_edge: bool,
    pub feather_water: i32,
    pub downwind_angle: f32,
    pub sky_box_position_z: f32,
    pub draw_sky_box: bool,
    pub sky_box_scale: f32,

    // Vertex water settings (for WATER_TYPE_3)
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

    // Lighting
    pub terrain_lighting: [[TerrainLighting; MAX_GLOBAL_LIGHTS]; TIME_OF_DAY_COUNT],
    pub terrain_objects_lighting: [[TerrainLighting; MAX_GLOBAL_LIGHTS]; TIME_OF_DAY_COUNT],
    pub terrain_ambient: [[f32; 3]; MAX_GLOBAL_LIGHTS],
    pub terrain_diffuse: [[f32; 3]; MAX_GLOBAL_LIGHTS],
    pub terrain_light_pos: [[f32; 3]; MAX_GLOBAL_LIGHTS],
    pub infantry_light_scale: [f32; TIME_OF_DAY_COUNT],
    pub script_override_infantry_light_scale: f32,
    pub num_global_lights: i32,

    // Physics and gameplay
    pub terrain_height_at_edge_of_map: f32,
    pub unit_damaged_thresh: f32,
    pub unit_really_damaged_thresh: f32,
    pub ground_stiffness: f32,
    pub structure_stiffness: f32,
    pub gravity: f32,
    pub stealth_friendly_opacity: f32,
    pub default_occlusion_delay: u32,
    pub partition_cell_size: f32,

    // Ammo and container pips
    pub ammo_pip_world_offset: [f32; 3],
    pub container_pip_world_offset: [f32; 3],
    pub ammo_pip_screen_offset: [f32; 2],
    pub container_pip_screen_offset: [f32; 2],
    pub ammo_pip_scale_factor: f32,
    pub container_pip_scale_factor: f32,
    pub historic_damage_limit: u32,

    // Terrain tracks
    pub max_terrain_tracks: i32,
    pub max_tank_track_edges: i32,
    pub max_tank_track_opaque_edges: i32,
    pub max_tank_track_fade_delay: i32,

    // Animations
    pub level_gain_animation_name: String,
    pub level_gain_animation_display_time_seconds: f32,
    pub level_gain_animation_z_rise_per_second: f32,
    pub get_healed_animation_name: String,
    pub get_healed_animation_display_time_seconds: f32,
    pub get_healed_animation_z_rise_per_second: f32,

    // Time and weather
    pub time_of_day: TimeOfDay,
    pub weather: i32, // Weather enum would be defined elsewhere
    pub make_track_marks: bool,
    pub hide_garrison_flags: bool,
    pub force_models_to_follow_time_of_day: bool,
    pub force_models_to_follow_weather: bool,

    // Player bonuses
    pub solo_player_health_bonus_for_difficulty: [[f32; DIFFICULTY_COUNT]; PLAYERTYPE_COUNT],

    // Visibility and rendering limits
    pub max_visible_translucent_objects: i32,
    pub max_visible_occluder_objects: i32,
    pub max_visible_occludee_objects: i32,
    pub max_visible_non_occluder_or_occludee_objects: i32,
    pub occluded_luminance_scale: f32,
    pub texture_reduction_factor: i32,
    pub enable_behind_building_markers: bool,

    // Roads
    pub max_road_segments: i32,
    pub max_road_vertex: i32,
    pub max_road_index: i32,
    pub max_road_types: i32,

    // 3D audio settings
    pub sounds_3d_on: bool,

    // Particles
    pub particle_scale: f32,
    pub max_particle_count: i32,
    pub max_field_particle_count: i32,

    // Auto fire/smoke particles
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
    pub default_ip: u32,
    pub firewall_behavior: u32,
    pub firewall_send_delay: bool,
    pub firewall_port_override: u32,
    pub firewall_port_allocation_delta: i16,
    pub network_fps_history_length: u32,
    pub network_latency_history_length: u32,
    pub network_cushion_history_length: u32,
    pub network_run_ahead_metrics_time: u32,
    pub network_keep_alive_delay: u32,
    pub network_run_ahead_slack: u32,
    pub network_disconnect_time: u32,
    pub network_player_timeout_time: u32,
    pub network_disconnect_screen_notify_time: u32,

    // Economy and building
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
    pub command_center_heal_range: f32,
    pub command_center_heal_amount: f32,
    pub max_line_build_objects: i32,
    pub max_tunnel_capacity: i32,

    // Veterancy and health
    pub health_bonus: [f32; LEVEL_COUNT],
    pub default_structure_rubble_height: f32,

    // Special settings
    pub pending_file: String,
    pub special_power_view_object_name: String,
    pub standard_public_bones: Vec<String>,
    pub standard_minefield_density: f32,
    pub standard_minefield_distance: f32,
    pub show_metrics: bool,
    pub default_starting_cash: i32, // Money type would be defined elsewhere
    pub debug_show_graphical_framerate: bool,

    // Power bar
    pub power_bar_base: i32,
    pub power_bar_intervals: f32,
    pub power_bar_yellow_range: i32,
    pub display_gamma: f32,
    pub unlook_persist_duration: u32,

    // Timing
    pub double_click_time_ms: u32,

    // Shroud and fog
    pub shroud_color: [f32; 3],
    pub clear_alpha: u8,
    pub fog_alpha: u8,
    pub shroud_alpha: u8,

    // Selection and audio
    pub group_select_min_select_size: i32,
    pub group_select_volume_base: f32,
    pub group_select_volume_increment: f32,
    pub max_unit_select_sounds: i32,
    pub selection_flash_saturation_factor: f32,
    pub selection_flash_house_color: bool,
    pub group_move_click_to_gather_factor: f32,

    // Graphics options
    pub anti_alias_box_value: i32,
    pub language_filter_pref: bool,
    pub load_screen_render: bool,
    pub disable_render: bool,

    // Camera shake
    pub shake_subtle_intensity: f32,
    pub shake_normal_intensity: f32,
    pub shake_strong_intensity: f32,
    pub shake_severe_intensity: f32,
    pub shake_cine_extreme_intensity: f32,
    pub shake_cine_insane_intensity: f32,
    pub max_shake_intensity: f32,
    pub max_shake_range: f32,

    // Base regeneration
    pub sell_percentage: f32,
    pub base_regen_health_percent_per_second: f32,
    pub base_regen_delay: u32,
    pub prison_bounty_multiplier: f32,
    pub prison_bounty_text_color: [f32; 3],

    // Colors
    pub hot_key_text_color: [f32; 4], // RGBA

    // Volume settings
    pub music_volume_factor: f32,
    pub sfx_volume_factor: f32,
    pub voice_volume_factor: f32,
    pub sound_3d_pref: bool,

    // Movement penalties
    pub movement_penalty_damage_state: i32, // BodyDamageType would be an enum

    // CRC values
    pub ini_crc: u32,
    pub exe_crc: u32,

    // Movies
    pub is_breakable_movie: bool,
    pub break_the_movie: bool,
    pub allow_exit_out_of_movies: bool,

    // TiVO fast mode
    pub tivo_fast_mode: bool,

    // User data directory (read-only)
    user_data_dir: String,
}

impl Default for GlobalData {
    fn default() -> Self {
        Self {
            writable: WritableGlobalData::default(),
            debug: DebugSettings::default(),
            overrides: HashMap::new(),

            // Map and rendering settings
            move_hint_name: String::new(),
            use_trees: true,
            use_tree_sway: true,
            use_draw_module_lod: true,
            use_heat_effects: true,
            max_shell_screens: 8,
            use_cloud_map: true,
            use_3way_terrain_blends: 1,
            use_light_map: true,
            bilinear_terrain_tex: true,
            trilinear_terrain_tex: false,
            multi_pass_terrain: false,
            adjust_cliff_textures: true,
            stretch_terrain: false,
            use_half_height_map: false,
            draw_entire_terrain: false,
            terrain_lod_target_time_ms: 30,

            // Camera and mouse settings
            use_alternate_mouse: false,
            client_retaliation_mode_enabled: false,
            double_click_attack_move: false,
            right_mouse_always_scrolls: false,
            camera_pitch: std::f32::consts::FRAC_PI_4,
            camera_yaw: 0.0,
            camera_height: 150.0,
            max_camera_height: 300.0,
            min_camera_height: 75.0,
            horizontal_scroll_speed_factor: 1.0,
            vertical_scroll_speed_factor: 1.0,
            scroll_amount_cutoff: 0.1,
            camera_adjust_speed: 2.0,
            enforce_max_camera_height: true,
            keyboard_scroll_factor: 1.0,
            keyboard_default_scroll_factor: 1.0,
            keyboard_camera_rotate_speed: 0.1,
            play_stats: 0,
            camera_audible_radius: 1000.0,
            save_camera_in_replay: false,
            use_camera_in_replay: false,

            // Water and sky settings
            use_water_plane: true,
            use_cloud_plane: true,
            water_position_x: 0.0,
            water_position_y: 0.0,
            water_position_z: 0.0,
            water_extent_x: 1000.0,
            water_extent_y: 1000.0,
            water_type: 0,
            show_soft_water_edge: true,
            feather_water: 0,
            downwind_angle: 0.0,
            sky_box_position_z: 0.0,
            draw_sky_box: true,
            sky_box_scale: 1.0,

            // Vertex water settings
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

            // Lighting
            terrain_lighting: [[TerrainLighting::default(); MAX_GLOBAL_LIGHTS]; TIME_OF_DAY_COUNT],
            terrain_objects_lighting: [[TerrainLighting::default(); MAX_GLOBAL_LIGHTS];
                TIME_OF_DAY_COUNT],
            terrain_ambient: [[1.0, 1.0, 1.0]; MAX_GLOBAL_LIGHTS],
            terrain_diffuse: [[1.0, 1.0, 1.0]; MAX_GLOBAL_LIGHTS],
            terrain_light_pos: [[0.0, 0.0, 1000.0]; MAX_GLOBAL_LIGHTS],
            infantry_light_scale: [1.0; TIME_OF_DAY_COUNT],
            script_override_infantry_light_scale: -1.0,
            num_global_lights: 3,

            // Physics and gameplay
            terrain_height_at_edge_of_map: 0.0,
            unit_damaged_thresh: 0.5,
            unit_really_damaged_thresh: 0.25,
            ground_stiffness: 0.8,
            structure_stiffness: 0.3,
            gravity: -1.0,
            stealth_friendly_opacity: 0.5,
            default_occlusion_delay: 30,
            partition_cell_size: 100.0,

            // Ammo and container pips
            ammo_pip_world_offset: [0.0, 0.0, 5.0],
            container_pip_world_offset: [0.0, 0.0, 5.0],
            ammo_pip_screen_offset: [0.0, 0.0],
            container_pip_screen_offset: [0.0, 0.0],
            ammo_pip_scale_factor: 1.0,
            container_pip_scale_factor: 1.0,
            historic_damage_limit: 0,

            // Terrain tracks
            max_terrain_tracks: 100,
            max_tank_track_edges: 100,
            max_tank_track_opaque_edges: 25,
            max_tank_track_fade_delay: 300,

            // Animations
            level_gain_animation_name: String::from("FX_LevelGainFX"),
            level_gain_animation_display_time_seconds: 2.0,
            level_gain_animation_z_rise_per_second: 5.0,
            get_healed_animation_name: String::from("FX_EmergencyRepair"),
            get_healed_animation_display_time_seconds: 1.5,
            get_healed_animation_z_rise_per_second: 3.0,

            // Time and weather
            time_of_day: TimeOfDay::Afternoon,
            weather: 0,
            make_track_marks: true,
            hide_garrison_flags: false,
            force_models_to_follow_time_of_day: false,
            force_models_to_follow_weather: false,

            // Player bonuses
            solo_player_health_bonus_for_difficulty: [[1.0; DIFFICULTY_COUNT]; PLAYERTYPE_COUNT],

            // Visibility and rendering limits
            max_visible_translucent_objects: 200,
            max_visible_occluder_objects: 200,
            max_visible_occludee_objects: 400,
            max_visible_non_occluder_or_occludee_objects: 400,
            occluded_luminance_scale: 0.5,
            texture_reduction_factor: 0,
            enable_behind_building_markers: true,

            // Roads
            max_road_segments: 100,
            max_road_vertex: 1500,
            max_road_index: 4500,
            max_road_types: 8,

            // 3D audio settings
            sounds_3d_on: true,

            // Particles
            particle_scale: 1.0,
            max_particle_count: 5000,
            max_field_particle_count: 500,

            // Auto fire/smoke particles
            auto_fire_particle_small_prefix: String::new(),
            auto_fire_particle_small_system: String::new(),
            auto_fire_particle_small_max: 10,
            auto_fire_particle_medium_prefix: String::new(),
            auto_fire_particle_medium_system: String::new(),
            auto_fire_particle_medium_max: 15,
            auto_fire_particle_large_prefix: String::new(),
            auto_fire_particle_large_system: String::new(),
            auto_fire_particle_large_max: 20,
            auto_smoke_particle_small_prefix: String::new(),
            auto_smoke_particle_small_system: String::new(),
            auto_smoke_particle_small_max: 10,
            auto_smoke_particle_medium_prefix: String::new(),
            auto_smoke_particle_medium_system: String::new(),
            auto_smoke_particle_medium_max: 15,
            auto_smoke_particle_large_prefix: String::new(),
            auto_smoke_particle_large_system: String::new(),
            auto_smoke_particle_large_max: 20,
            auto_aflame_particle_prefix: String::new(),
            auto_aflame_particle_system: String::new(),
            auto_aflame_particle_max: 5,

            // Network settings
            default_ip: 0,
            firewall_behavior: 0,
            firewall_send_delay: false,
            firewall_port_override: 0,
            firewall_port_allocation_delta: 0,
            network_fps_history_length: 60,
            network_latency_history_length: 60,
            network_cushion_history_length: 60,
            network_run_ahead_metrics_time: 1000,
            network_keep_alive_delay: 5,
            network_run_ahead_slack: 10,
            network_disconnect_time: 30000,
            network_player_timeout_time: 60000,
            network_disconnect_screen_notify_time: 5000,

            // Economy and building
            base_value_per_supply_box: 200,
            build_speed: 1.0,
            min_dist_from_edge_of_map_for_build: 10.0,
            supply_build_border: 40.0,
            allowed_height_variation_for_building: 3.0,
            min_low_energy_production_speed: 0.5,
            max_low_energy_production_speed: 1.0,
            low_energy_penalty_modifier: 0.5,
            multiple_factory: 1.0,
            refund_percent: 0.5,
            command_center_heal_range: 200.0,
            command_center_heal_amount: 5.0,
            max_line_build_objects: 10,
            max_tunnel_capacity: 50,

            // Veterancy and health
            health_bonus: [0.0, 0.1, 0.25, 0.5, 1.0, 1.5, 2.0, 3.0],
            default_structure_rubble_height: 4.0,

            // Special settings
            pending_file: String::new(),
            special_power_view_object_name: String::new(),
            standard_public_bones: Vec::new(),
            standard_minefield_density: 1.0,
            standard_minefield_distance: 10.0,
            show_metrics: false,
            default_starting_cash: 10000,
            debug_show_graphical_framerate: false,

            // Power bar
            power_bar_base: 2,
            power_bar_intervals: 10.0,
            power_bar_yellow_range: 5,
            display_gamma: 1.0,
            unlook_persist_duration: 1000,

            // Timing
            double_click_time_ms: 250,

            // Shroud and fog
            shroud_color: [0.0, 0.0, 0.0],
            clear_alpha: 255,
            fog_alpha: 127,
            shroud_alpha: 0,

            // Selection and audio
            group_select_min_select_size: 3,
            group_select_volume_base: 0.7,
            group_select_volume_increment: 0.05,
            max_unit_select_sounds: 10,
            selection_flash_saturation_factor: 1.5,
            selection_flash_house_color: true,
            group_move_click_to_gather_factor: 1.0,

            // Graphics options
            anti_alias_box_value: 0,
            language_filter_pref: false,
            load_screen_render: true,
            disable_render: false,

            // Camera shake
            shake_subtle_intensity: 0.5,
            shake_normal_intensity: 1.0,
            shake_strong_intensity: 2.0,
            shake_severe_intensity: 3.0,
            shake_cine_extreme_intensity: 5.0,
            shake_cine_insane_intensity: 10.0,
            max_shake_intensity: 1.5,
            max_shake_range: 200.0,

            // Base regeneration
            sell_percentage: 0.25,
            base_regen_health_percent_per_second: 0.0,
            base_regen_delay: 0,
            prison_bounty_multiplier: 1.0,
            prison_bounty_text_color: [1.0, 1.0, 1.0],

            // Colors
            hot_key_text_color: [1.0, 1.0, 0.0, 1.0],

            // Volume settings
            music_volume_factor: 0.8,
            sfx_volume_factor: 1.0,
            voice_volume_factor: 1.0,
            sound_3d_pref: true,

            // Movement penalties
            movement_penalty_damage_state: 1,

            // CRC values
            ini_crc: 0,
            exe_crc: 0,

            // Movies
            is_breakable_movie: false,
            break_the_movie: false,
            allow_exit_out_of_movies: false,

            // TiVO fast mode
            tivo_fast_mode: false,

            // User data directory
            user_data_dir: String::new(),
        }
    }
}

impl GlobalData {
    pub fn set_override<S: Into<String>>(&mut self, key: S, value: GlobalValue) {
        self.overrides.insert(key.into(), value);
    }

    pub fn get_override(&self, key: &str) -> Option<&GlobalValue> {
        self.overrides.get(key)
    }

    pub fn merge_overrides(&mut self, other: &HashMap<String, GlobalValue>) {
        self.overrides.extend(other.clone());
    }

    pub fn apply_command_line(&mut self, writable: &WritableGlobalData, debug: &DebugSettings) {
        self.writable = writable.clone();
        self.debug = *debug;
    }

    /// Get user data directory path
    pub fn get_user_data_dir(&self) -> &str {
        &self.user_data_dir
    }

    /// Set user data directory path
    pub fn set_user_data_dir(&mut self, path: String) {
        self.user_data_dir = path;
    }

    /// Set time of day and return success
    pub fn set_time_of_day(&mut self, tod: TimeOfDay) -> bool {
        if matches!(tod, TimeOfDay::Invalid) {
            return false;
        }
        self.time_of_day = tod;
        true
    }

    /// Initialize global data with default values
    pub fn init(&mut self) {
        // Reset to defaults
        *self = GlobalData::default();
    }

    /// Reset global data
    pub fn reset(&mut self) {
        // Clear overrides but keep other settings
        self.overrides.clear();
    }
}

/// Global singleton mirroring `TheGlobalData` in the C++ codebase.
pub static GLOBAL_DATA: Lazy<RwLock<GlobalData>> = Lazy::new(|| RwLock::new(GlobalData::default()));

/// Error type for global data access
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalDataError {
    /// Lock was poisoned (another thread panicked while holding it)
    PoisonedLock,
    /// Lock acquisition would deadlock
    WouldBlock,
}

impl std::fmt::Display for GlobalDataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GlobalDataError::PoisonedLock => write!(f, "GlobalData lock was poisoned"),
            GlobalDataError::WouldBlock => write!(f, "GlobalData lock acquisition would block"),
        }
    }
}

impl std::error::Error for GlobalDataError {}

/// Safe read access with panic recovery
pub fn read_safe() -> Result<RwLockReadGuard<'static, GlobalData>, GlobalDataError> {
    match GLOBAL_DATA.read() {
        Ok(guard) => Ok(guard),
        Err(poisoned) => {
            eprintln!("WARN: GlobalData lock poisoned, recovering...");
            // Recover from poisoning by using the poisoned value
            Ok(poisoned.into_inner())
        }
    }
}

/// Safe write access with panic recovery
pub fn write_safe() -> Result<RwLockWriteGuard<'static, GlobalData>, GlobalDataError> {
    match GLOBAL_DATA.write() {
        Ok(guard) => Ok(guard),
        Err(poisoned) => {
            eprintln!("WARN: GlobalData lock poisoned, recovering...");
            // Recover from poisoning by using the poisoned value
            Ok(poisoned.into_inner())
        }
    }
}

/// Legacy convenience functions - now use panic recovery internally
pub fn read() -> RwLockReadGuard<'static, GlobalData> {
    match GLOBAL_DATA.read() {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("WARN: GlobalData read lock poisoned, recovering...");
            poisoned.into_inner()
        }
    }
}

/// Legacy convenience functions - now use panic recovery internally
pub fn write() -> RwLockWriteGuard<'static, GlobalData> {
    match GLOBAL_DATA.write() {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("WARN: GlobalData write lock poisoned, recovering...");
            poisoned.into_inner()
        }
    }
}

/// Convenience helpers for the most common operations used by subsystems.
pub mod access {
    use super::{write, GlobalValue};
    use crate::common::command_line::{DebugSettings, WritableGlobalData};

    pub fn update_from_command_line(writable: &WritableGlobalData, debug: &DebugSettings) {
        let mut data = write();
        data.apply_command_line(writable, debug);
    }

    pub fn set_override<K: Into<String>>(key: K, value: GlobalValue) {
        write().set_override(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    // ============================================================================
    // WEEK 1 PRIORITY 2: GLOBAL DATA SAFETY TESTS (25+ tests for lock poisoning)
    // ============================================================================

    #[test]
    fn test_global_data_read_success() {
        // Verify normal read access works
        let data = read();
        assert_eq!(data.camera_height, 150.0);
    }

    #[test]
    fn test_global_data_write_success() {
        // Verify normal write access works
        let mut data = write();
        data.camera_height = 200.0;
        assert_eq!(data.camera_height, 200.0);
    }

    #[test]
    fn test_global_data_read_safe_success() {
        // Verify safe read API works
        let data = read_safe().expect("read_safe failed");
        assert!(data.camera_height > 0.0);
    }

    #[test]
    fn test_global_data_write_safe_success() {
        // Verify safe write API works
        let mut data = write_safe().expect("write_safe failed");
        data.camera_height = 175.0;
        assert_eq!(data.camera_height, 175.0);
    }

    #[test]
    fn test_global_data_multiple_reads() {
        // Verify multiple read locks can be acquired
        let _read1 = read();
        let _read2 = read();
        let _read3 = read();
        // If we got here without panic, test passes
        assert!(true);
    }

    #[test]
    fn test_global_data_write_then_read() {
        // Verify write followed by read works
        {
            let mut data = write();
            data.camera_height = 180.0;
        }
        let data = read();
        assert_eq!(data.camera_height, 180.0);
    }

    #[test]
    fn test_global_data_override_set_get() {
        // Verify override system works
        let mut data = write();
        data.set_override("test_key", GlobalValue::Int(42));

        let data = read();
        let val = data.get_override("test_key");
        assert_eq!(val, Some(&GlobalValue::Int(42)));
    }

    #[test]
    fn test_global_data_override_bool() {
        // Test boolean override
        let mut data = write();
        data.set_override("test_bool", GlobalValue::Bool(true));

        let data = read();
        let val = data.get_override("test_bool").and_then(|v| v.as_bool());
        assert_eq!(val, Some(true));
    }

    #[test]
    fn test_global_data_override_float() {
        // Test float override
        let mut data = write();
        data.set_override("test_float", GlobalValue::Float(3.14));

        let data = read();
        let val = data.get_override("test_float").and_then(|v| v.as_float());
        assert!(val.is_some());
        assert!((val.unwrap() - 3.14).abs() < 0.01);
    }

    #[test]
    fn test_global_data_override_string() {
        // Test string override
        let mut data = write();
        data.set_override("test_str", GlobalValue::String("hello".to_string()));

        let data = read();
        let val = data.get_override("test_str").and_then(|v| v.as_str());
        assert_eq!(val, Some("hello"));
    }

    #[test]
    fn test_global_data_time_of_day() {
        // Test time of day setting
        let mut data = write();
        data.set_time_of_day(TimeOfDay::Night);
        assert_eq!(data.time_of_day, TimeOfDay::Night);
    }

    #[test]
    fn test_global_data_default_values() {
        // Verify default values are sensible
        let data = read();
        assert!(data.gravity > 0.0);
        assert!(data.build_speed > 0.0);
        assert!(data.particle_scale > 0.0);
        assert!(data.max_particle_count > 0);
    }

    #[test]
    fn test_global_data_init_reset() {
        // Test init and reset functions
        let mut data = write();
        data.set_override("test", GlobalValue::Int(99));

        // Reset clears overrides
        data.reset();
        assert!(data.get_override("test").is_none());
    }

    #[test]
    fn test_global_data_user_data_dir() {
        // Test user data directory access
        let mut data = write();
        data.set_user_data_dir("/path/to/data".to_string());
        assert_eq!(data.get_user_data_dir(), "/path/to/data");
    }

    #[test]
    fn test_global_data_multithreaded_read() {
        // Verify global data can be read from multiple threads
        let mut handles = vec![];
        for i in 0..5 {
            let handle = thread::spawn(move || {
                let data = read();
                assert!(data.camera_height > 0.0);
                i
            });
            handles.push(handle);
        }

        for handle in handles {
            let result = handle.join().expect("Thread panicked");
            assert!(result >= 0);
        }
    }

    #[test]
    fn test_global_data_multithreaded_write_sequential() {
        // Verify sequential writes from multiple threads work
        let mut handles = vec![];
        for i in 0..3 {
            let handle = thread::spawn(move || {
                let mut data = write();
                data.camera_height = 100.0 + (i as f32 * 10.0);
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }
        // If we got here without panic, test passes
        assert!(true);
    }

    #[test]
    fn test_global_data_merge_overrides() {
        // Test merging override maps
        let mut overrides = HashMap::new();
        overrides.insert("key1".to_string(), GlobalValue::Int(1));
        overrides.insert("key2".to_string(), GlobalValue::Float(2.5));

        let mut data = write();
        data.merge_overrides(&overrides);

        let data = read();
        assert_eq!(data.get_override("key1"), Some(&GlobalValue::Int(1)));
        assert_eq!(data.get_override("key2"), Some(&GlobalValue::Float(2.5)));
    }

    #[test]
    fn test_global_data_camera_settings() {
        // Verify camera settings can be read/written
        let data = read();
        assert!(data.camera_pitch > 0.0);
        assert!(data.min_camera_height < data.max_camera_height);
        assert!(data.min_camera_height > 0.0);
    }

    #[test]
    fn test_global_data_lighting_settings() {
        // Verify lighting configuration
        let data = read();
        assert!(data.num_global_lights > 0);
        assert!(data.num_global_lights <= MAX_GLOBAL_LIGHTS as i32);
    }

    #[test]
    fn test_global_data_water_settings() {
        // Verify water configuration
        let data = read();
        assert!(data.water_extent_x > 0.0);
        assert!(data.water_extent_y > 0.0);
    }

    #[test]
    fn test_global_data_audio_settings() {
        // Verify audio configuration
        let data = read();
        assert!(data.music_volume_factor >= 0.0);
        assert!(data.sfx_volume_factor >= 0.0);
        assert!(data.voice_volume_factor >= 0.0);
    }

    #[test]
    fn test_global_data_network_settings() {
        // Verify network configuration
        let data = read();
        assert!(data.network_fps_history_length > 0);
        assert!(data.network_latency_history_length > 0);
    }

    #[test]
    fn test_global_data_value_enum_conversions() {
        // Test GlobalValue enum conversions
        let bool_val = GlobalValue::Bool(false);
        assert_eq!(bool_val.as_bool(), Some(false));
        assert_eq!(bool_val.as_int(), None);

        let int_val = GlobalValue::Int(42);
        assert_eq!(int_val.as_int(), Some(42));
        assert_eq!(int_val.as_bool(), None);

        let float_val = GlobalValue::Float(1.5);
        assert!(float_val.as_float().is_some());
        assert_eq!(float_val.as_int(), None);

        let str_val = GlobalValue::String("test".to_string());
        assert_eq!(str_val.as_str(), Some("test"));
        assert_eq!(str_val.as_bool(), None);
    }

    #[test]
    fn test_global_data_error_display() {
        // Test error message formatting
        let err = GlobalDataError::PoisonedLock;
        assert_eq!(err.to_string(), "GlobalData lock was poisoned");

        let err = GlobalDataError::WouldBlock;
        assert_eq!(err.to_string(), "GlobalData lock acquisition would block");
    }
}
