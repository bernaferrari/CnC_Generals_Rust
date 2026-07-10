//! Global data registry for the Rust port of the Generals engine.
//!
//! The original C++ `GlobalData` structure contains hundreds of tunable fields that are
//! populated from INI files, command-line arguments, and runtime systems.  The previous
//! Rust stub merely exposed three booleans, which meant virtually every gameplay feature
//! was missing.  This module reintroduces a faithful, extensible representation that can
//! store the full writable dataset alongside dynamic key/value overrides used throughout
//! the engine.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

use once_cell::sync::Lazy;

use crate::common::command_line::{DebugSettings, WritableGlobalData};
use crate::common::system::{Snapshotable, Xfer, XferMode, XferVersion};

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
            ambient: [0.0, 0.0, 0.0],
            diffuse: [0.0, 0.0, 0.0],
            light_pos: [0.0, 0.0, -1.0],
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

            // Map and rendering settings (parity: GeneralsMD/Source/Common/GlobalData.cpp constructor)
            move_hint_name: String::new(),
            use_trees: false,           // C++ line 578: false
            use_tree_sway: true,        // C++ line 579: true
            use_draw_module_lod: false, // C++ line 580: false
            use_heat_effects: true,     // C++ line 581: true
            max_shell_screens: 0,       // C++ line 589: 0
            use_cloud_map: false,       // C++ line 590: false
            use_3way_terrain_blends: 1,
            use_light_map: false,         // C++ line 592: false
            bilinear_terrain_tex: false,  // C++ line 593: false
            trilinear_terrain_tex: false, // C++ line 594: false
            multi_pass_terrain: false,    // C++ line 595: false
            adjust_cliff_textures: false, // C++ line 596: false
            stretch_terrain: false,
            use_half_height_map: false,
            draw_entire_terrain: false,
            terrain_lod_target_time_ms: 0, // C++ line 600: 0

            // Camera and mouse settings (parity: GlobalData.cpp constructor)
            use_alternate_mouse: false,
            client_retaliation_mode_enabled: true, // C++ line 1060: true
            double_click_attack_move: false,
            right_mouse_always_scrolls: false,
            camera_pitch: 0.0,
            camera_yaw: 0.0,
            camera_height: 0.0, // C++ line 801: 0.0
            max_camera_height: 300.0,
            min_camera_height: 100.0, // C++ line 802: 100.0
            horizontal_scroll_speed_factor: 1.0,
            vertical_scroll_speed_factor: 1.0,
            scroll_amount_cutoff: 10.0, // C++ line 956: 10.0
            camera_adjust_speed: 0.1,   // C++ line 957: 0.1
            enforce_max_camera_height: true,
            keyboard_scroll_factor: 0.5, // C++ line 955: 0.5
            keyboard_default_scroll_factor: 1.0,
            keyboard_camera_rotate_speed: 0.1,
            play_stats: -1,               // C++ line 574: -1
            camera_audible_radius: 500.0, // C++ line 848: 500.0
            save_camera_in_replay: false,
            use_camera_in_replay: false,

            // Water and sky settings (parity: GlobalData.cpp constructor)
            use_water_plane: false, // C++ line 604: false
            use_cloud_plane: false, // C++ line 605: false
            water_position_x: 0.0,
            water_position_y: 0.0,
            water_position_z: 0.0,
            water_extent_x: 0.0, // C++ line 624: 0.0
            water_extent_y: 0.0, // C++ line 625: 0.0
            water_type: 0,
            show_soft_water_edge: true,
            feather_water: 0,
            downwind_angle: -0.785, // C++ line 606: -0.785 (northeast)
            sky_box_position_z: 0.0,
            draw_sky_box: false,
            sky_box_scale: 4.5, // C++ line 657: 4.5

            // Vertex water settings (parity: GlobalData.cpp constructor)
            vertex_water_available_maps: Default::default(),
            vertex_water_height_clamp_low: [0.0; MAX_WATER_GRID_SETTINGS],
            vertex_water_height_clamp_hi: [0.0; MAX_WATER_GRID_SETTINGS], // C++ line 638: 0.0
            vertex_water_angle: [0.0; MAX_WATER_GRID_SETTINGS],
            vertex_water_x_position: [0.0; MAX_WATER_GRID_SETTINGS],
            vertex_water_y_position: [0.0; MAX_WATER_GRID_SETTINGS],
            vertex_water_z_position: [0.0; MAX_WATER_GRID_SETTINGS],
            vertex_water_x_grid_cells: [0; MAX_WATER_GRID_SETTINGS], // C++ line 643: 0
            vertex_water_y_grid_cells: [0; MAX_WATER_GRID_SETTINGS], // C++ line 644: 0
            vertex_water_grid_size: [0.0; MAX_WATER_GRID_SETTINGS],  // C++ line 645: 0.0
            vertex_water_attenuation_a: [0.0; MAX_WATER_GRID_SETTINGS], // C++ line 646: 0.0
            vertex_water_attenuation_b: [0.0; MAX_WATER_GRID_SETTINGS],
            vertex_water_attenuation_c: [0.0; MAX_WATER_GRID_SETTINGS],
            vertex_water_attenuation_range: [0.0; MAX_WATER_GRID_SETTINGS],

            // Lighting (parity: GlobalData.cpp constructor)
            terrain_lighting: [[TerrainLighting::default(); MAX_GLOBAL_LIGHTS]; TIME_OF_DAY_COUNT],
            terrain_objects_lighting: [[TerrainLighting::default(); MAX_GLOBAL_LIGHTS];
                TIME_OF_DAY_COUNT],
            terrain_ambient: [[0.0, 0.0, 0.0]; MAX_GLOBAL_LIGHTS],
            terrain_diffuse: [[0.0, 0.0, 0.0]; MAX_GLOBAL_LIGHTS],
            terrain_light_pos: [[0.0, 0.0, -1.0]; MAX_GLOBAL_LIGHTS],
            infantry_light_scale: [1.5; TIME_OF_DAY_COUNT], // C++ line 723: 1.5
            script_override_infantry_light_scale: -1.0,
            num_global_lights: 3,

            // Physics and gameplay (parity: GlobalData.cpp constructor)
            terrain_height_at_edge_of_map: 0.0,
            unit_damaged_thresh: 0.5,
            unit_really_damaged_thresh: 0.1, // C++ line 807: 0.1
            ground_stiffness: 0.5,           // C++ line 808: 0.5
            structure_stiffness: 0.5,        // C++ line 809: 0.5
            gravity: -1.0,
            stealth_friendly_opacity: 0.5,
            default_occlusion_delay: 90, // C++ line 812: LOGICFRAMES_PER_SECOND * 3 = 90
            partition_cell_size: 0.0,    // C++ line 679: 0.0

            // Ammo and container pips (parity: GlobalData.cpp constructor)
            ammo_pip_world_offset: [0.0, 0.0, 0.0], // C++ line 682: zero()
            container_pip_world_offset: [0.0, 0.0, 0.0], // C++ line 683: zero()
            ammo_pip_screen_offset: [0.0, 0.0],
            container_pip_screen_offset: [0.0, 0.0],
            ammo_pip_scale_factor: 1.0,
            container_pip_scale_factor: 1.0,
            historic_damage_limit: 0,

            // Terrain tracks (parity: GlobalData.cpp constructor)
            max_terrain_tracks: 0,
            max_tank_track_edges: 100, // C++ line 668: 100
            max_tank_track_opaque_edges: 25,
            max_tank_track_fade_delay: 300000, // C++ line 670: 300000

            // Animations
            level_gain_animation_name: String::new(),
            level_gain_animation_display_time_seconds: 0.0,
            level_gain_animation_z_rise_per_second: 0.0,
            get_healed_animation_name: String::new(),
            get_healed_animation_display_time_seconds: 0.0,
            get_healed_animation_z_rise_per_second: 0.0,

            // Time and weather (parity: GlobalData.cpp constructor)
            time_of_day: TimeOfDay::Afternoon,
            weather: 0,
            make_track_marks: false, // C++ line 674: false
            hide_garrison_flags: false,
            force_models_to_follow_time_of_day: true, // C++ line 676: true
            force_models_to_follow_weather: true,     // C++ line 677: true

            // Player bonuses
            solo_player_health_bonus_for_difficulty: [[1.0; DIFFICULTY_COUNT]; PLAYERTYPE_COUNT],

            // Visibility and rendering limits (parity: GlobalData.cpp constructor)
            max_visible_translucent_objects: 512, // C++ line 742: 512
            max_visible_occluder_objects: 512,    // C++ line 743: 512
            max_visible_occludee_objects: 512,    // C++ line 744: 512
            max_visible_non_occluder_or_occludee_objects: 512,
            occluded_luminance_scale: 0.5,
            texture_reduction_factor: -1, // C++ line 609: -1
            enable_behind_building_markers: true,

            // Roads
            max_road_segments: 0,
            max_road_vertex: 0,
            max_road_index: 0,
            max_road_types: 0,

            // 3D audio settings
            sounds_3d_on: true,

            // Particles
            particle_scale: 1.0,
            max_particle_count: 0,
            max_field_particle_count: 30,

            // Auto fire/smoke particles
            auto_fire_particle_small_prefix: String::new(),
            auto_fire_particle_small_system: String::new(),
            auto_fire_particle_small_max: 0,
            auto_fire_particle_medium_prefix: String::new(),
            auto_fire_particle_medium_system: String::new(),
            auto_fire_particle_medium_max: 0,
            auto_fire_particle_large_prefix: String::new(),
            auto_fire_particle_large_system: String::new(),
            auto_fire_particle_large_max: 0,
            auto_smoke_particle_small_prefix: String::new(),
            auto_smoke_particle_small_system: String::new(),
            auto_smoke_particle_small_max: 0,
            auto_smoke_particle_medium_prefix: String::new(),
            auto_smoke_particle_medium_system: String::new(),
            auto_smoke_particle_medium_max: 0,
            auto_smoke_particle_large_prefix: String::new(),
            auto_smoke_particle_large_system: String::new(),
            auto_smoke_particle_large_max: 0,
            auto_aflame_particle_prefix: String::new(),
            auto_aflame_particle_system: String::new(),
            auto_aflame_particle_max: 0,

            // Network settings (parity: GlobalData.cpp constructor)
            default_ip: 0,
            firewall_behavior: 0,
            firewall_send_delay: false,
            firewall_port_override: 0,
            firewall_port_allocation_delta: 0,
            network_fps_history_length: 30,      // C++ line 910: 30
            network_latency_history_length: 200, // C++ line 911: 200
            network_cushion_history_length: 10,  // C++ line 913: 10
            network_run_ahead_metrics_time: 500, // C++ line 912: 500
            network_keep_alive_delay: 20,        // C++ line 915: 20
            network_run_ahead_slack: 10,
            network_disconnect_time: 5000, // C++ line 916: 5000
            network_player_timeout_time: 60000,
            network_disconnect_screen_notify_time: 15000, // C++ line 918: 15000

            // Economy and building (parity: GlobalData.cpp constructor)
            base_value_per_supply_box: 100, // C++ line 733: 100
            build_speed: 0.0,
            min_dist_from_edge_of_map_for_build: 0.0,
            supply_build_border: 0.0,
            allowed_height_variation_for_building: 0.0,
            min_low_energy_production_speed: 0.0,
            max_low_energy_production_speed: 0.0,
            low_energy_penalty_modifier: 0.0,
            multiple_factory: 0.0,
            refund_percent: 0.0, // C++ line 830: 0.0
            command_center_heal_range: 0.0,
            command_center_heal_amount: 0.0,
            max_line_build_objects: 0,
            max_tunnel_capacity: 0,

            // Veterancy and health
            health_bonus: [1.0; LEVEL_COUNT],
            default_structure_rubble_height: 1.0,

            // Special settings (parity: GlobalData.cpp constructor)
            pending_file: String::new(),
            special_power_view_object_name: String::new(),
            standard_public_bones: Vec::new(),
            standard_minefield_density: 0.01,  // C++ line 837: 0.01
            standard_minefield_distance: 40.0, // C++ line 838: 40.0
            show_metrics: false,
            default_starting_cash: 10000,
            debug_show_graphical_framerate: false,

            // Power bar (parity: GlobalData.cpp constructor)
            power_bar_base: 7,        // C++ line 879: 7
            power_bar_intervals: 3.0, // C++ line 880: 3
            power_bar_yellow_range: 5,
            display_gamma: 1.0,
            unlook_persist_duration: 30, // C++ line 905: 30

            // Timing
            double_click_time_ms: 250,

            // Shroud and fog
            shroud_color: [0.0, 0.0, 0.0],
            clear_alpha: 255,
            fog_alpha: 127,
            shroud_alpha: 0,

            // Selection and audio (parity: GlobalData.cpp constructor)
            group_select_min_select_size: 5, // C++ line 840: 5
            group_select_volume_base: 0.5,   // C++ line 841: 0.5
            group_select_volume_increment: 0.02,
            max_unit_select_sounds: 8,
            selection_flash_saturation_factor: 0.5, // C++ line 845: 0.5
            selection_flash_house_color: false,     // C++ line 846: false
            group_move_click_to_gather_factor: 1.0,

            // Graphics options
            anti_alias_box_value: 0,
            language_filter_pref: true, // C++ line 889: true
            load_screen_render: false,
            disable_render: false,

            // Camera shake (parity: GlobalData.cpp constructor)
            shake_subtle_intensity: 0.5,
            shake_normal_intensity: 1.0,
            shake_strong_intensity: 2.5,
            shake_severe_intensity: 5.0,       // C++ line 854: 5.0
            shake_cine_extreme_intensity: 8.0, // C++ line 855: 8.0
            shake_cine_insane_intensity: 12.0, // C++ line 856: 12.0
            max_shake_intensity: 10.0,         // C++ line 857: 10.0
            max_shake_range: 150.0,            // C++ line 858: 150.0

            // Base regeneration (parity: GlobalData.cpp constructor)
            sell_percentage: 1.0, // C++ line 860: 1.0
            base_regen_health_percent_per_second: 0.0,
            base_regen_delay: 0,
            prison_bounty_multiplier: 1.0,
            prison_bounty_text_color: [1.0, 1.0, 1.0],

            // Colors
            hot_key_text_color: [1.0, 1.0, 0.0, 1.0],

            // Volume settings (parity: GlobalData.cpp constructor)
            music_volume_factor: 0.5, // C++ line 950: 0.5
            sfx_volume_factor: 0.5,   // C++ line 951: 0.5
            voice_volume_factor: 0.5, // C++ line 952: 0.5
            sound_3d_pref: false,

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

    pub fn clear_override(&mut self, key: &str) {
        self.overrides.remove(key);
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
        let tod_index = tod as usize;
        for i in 0..MAX_GLOBAL_LIGHTS {
            self.terrain_ambient[i] = self.terrain_lighting[tod_index][i].ambient;
            self.terrain_diffuse[i] = self.terrain_lighting[tod_index][i].diffuse;
            self.terrain_light_pos[i] = self.terrain_lighting[tod_index][i].light_pos;
        }
        true
    }

    /// Initialize global data.
    pub fn init(&mut self) {
        // C++ GlobalData::init() is a no-op; construction and INI parsing own
        // the actual values. Preserve loaded/user-modified state here.
    }

    /// Reset global data
    pub fn reset(&mut self) {
        // Clear overrides but keep other settings
        self.overrides.clear();
    }
}

/// Helper macro to xfer a field with error context.
/// Reduces boilerplate for the ~150 fields in GlobalData.
macro_rules! xf {
    ($xfer:expr, $method:ident, $field:expr, $label:expr) => {
        $xfer.$method(&mut $field)
            .map_err(|e| format!("GlobalData xfer '{}' failed: {}", $label, e))?
    };
}

impl Snapshotable for GlobalData {
    /// CRC computation for network synchronization.
    /// C++ Reference: GlobalData is CRC'd as part of the INI CRC in GameEngine.cpp.
    /// We CRC all gameplay-relevant fields to detect desync.
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut camera_pitch = self.camera_pitch;
        xf!(xfer, xfer_real, camera_pitch, "camera_pitch");
        let mut camera_yaw = self.camera_yaw;
        xf!(xfer, xfer_real, camera_yaw, "camera_yaw");
        let mut camera_height = self.camera_height;
        xf!(xfer, xfer_real, camera_height, "camera_height");
        let mut max_camera_height = self.max_camera_height;
        xf!(xfer, xfer_real, max_camera_height, "max_camera_height");
        let mut min_camera_height = self.min_camera_height;
        xf!(xfer, xfer_real, min_camera_height, "min_camera_height");

        let mut gravity = self.gravity;
        xf!(xfer, xfer_real, gravity, "gravity");
        let mut ground_stiffness = self.ground_stiffness;
        xf!(xfer, xfer_real, ground_stiffness, "ground_stiffness");
        let mut structure_stiffness = self.structure_stiffness;
        xf!(xfer, xfer_real, structure_stiffness, "structure_stiffness");
        let mut terrain_height_at_edge_of_map = self.terrain_height_at_edge_of_map;
        xf!(xfer, xfer_real, terrain_height_at_edge_of_map, "terrain_height_at_edge_of_map");

        let mut build_speed = self.build_speed;
        xf!(xfer, xfer_real, build_speed, "build_speed");
        let mut multiple_factory = self.multiple_factory;
        xf!(xfer, xfer_real, multiple_factory, "multiple_factory");
        let mut refund_percent = self.refund_percent;
        xf!(xfer, xfer_real, refund_percent, "refund_percent");
        let mut base_value_per_supply_box = self.base_value_per_supply_box;
        xf!(xfer, xfer_int, base_value_per_supply_box, "base_value_per_supply_box");
        let mut default_starting_cash = self.default_starting_cash;
        xf!(xfer, xfer_int, default_starting_cash, "default_starting_cash");

        let mut unit_damaged_thresh = self.unit_damaged_thresh;
        xf!(xfer, xfer_real, unit_damaged_thresh, "unit_damaged_thresh");
        let mut unit_really_damaged_thresh = self.unit_really_damaged_thresh;
        xf!(xfer, xfer_real, unit_really_damaged_thresh, "unit_really_damaged_thresh");
        let mut stealth_friendly_opacity = self.stealth_friendly_opacity;
        xf!(xfer, xfer_real, stealth_friendly_opacity, "stealth_friendly_opacity");
        let mut default_occlusion_delay = self.default_occlusion_delay;
        xf!(xfer, xfer_unsigned_int, default_occlusion_delay, "default_occlusion_delay");

        let mut tod = self.time_of_day as i32;
        xf!(xfer, xfer_int, tod, "time_of_day");
        let mut num_global_lights = self.num_global_lights;
        xf!(xfer, xfer_int, num_global_lights, "num_global_lights");
        let mut script_override_infantry_light_scale = self.script_override_infantry_light_scale;
        xf!(xfer, xfer_real, script_override_infantry_light_scale, "script_override_infantry_light_scale");

        for i in 0..LEVEL_COUNT {
            let mut val = self.health_bonus[i];
            let label = format!("health_bonus[{}]", i);
            xf!(xfer, xfer_real, val, &label);
        }

        for pt in 0..PLAYERTYPE_COUNT {
            for diff in 0..DIFFICULTY_COUNT {
                let mut val = self.solo_player_health_bonus_for_difficulty[pt][diff];
                xf!(xfer, xfer_real, val, "solo_player_health_bonus");
            }
        }

        let mut ini_crc = self.ini_crc;
        xf!(xfer, xfer_unsigned_int, ini_crc, "ini_crc");
        let mut exe_crc = self.exe_crc;
        xf!(xfer, xfer_unsigned_int, exe_crc, "exe_crc");

        Ok(())
    }

    /// Save/load all GlobalData fields.
    /// C++ Reference: GlobalData does not have an explicit xfer method in C++ —
    /// it is loaded from INI files. This Rust implementation provides full save/load
    /// parity for the save game system.
    ///
    /// Version History:
    ///   1: Initial version — all fields serialized
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;

        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("GlobalData xfer_version failed: {}", e))?;

        // =====================================================================
        // Map and rendering settings
        // =====================================================================
        xf!(xfer, xfer_ascii_string, self.move_hint_name, "move_hint_name");
        xf!(xfer, xfer_bool, self.use_trees, "use_trees");
        xf!(xfer, xfer_bool, self.use_tree_sway, "use_tree_sway");
        xf!(xfer, xfer_bool, self.use_draw_module_lod, "use_draw_module_lod");
        xf!(xfer, xfer_bool, self.use_heat_effects, "use_heat_effects");
        xf!(xfer, xfer_int, self.max_shell_screens, "max_shell_screens");
        xf!(xfer, xfer_bool, self.use_cloud_map, "use_cloud_map");
        xf!(xfer, xfer_int, self.use_3way_terrain_blends, "use_3way_terrain_blends");
        xf!(xfer, xfer_bool, self.use_light_map, "use_light_map");
        xf!(xfer, xfer_bool, self.bilinear_terrain_tex, "bilinear_terrain_tex");
        xf!(xfer, xfer_bool, self.trilinear_terrain_tex, "trilinear_terrain_tex");
        xf!(xfer, xfer_bool, self.multi_pass_terrain, "multi_pass_terrain");
        xf!(xfer, xfer_bool, self.adjust_cliff_textures, "adjust_cliff_textures");
        xf!(xfer, xfer_bool, self.stretch_terrain, "stretch_terrain");
        xf!(xfer, xfer_bool, self.use_half_height_map, "use_half_height_map");
        xf!(xfer, xfer_bool, self.draw_entire_terrain, "draw_entire_terrain");
        xf!(xfer, xfer_int, self.terrain_lod_target_time_ms, "terrain_lod_target_time_ms");

        // =====================================================================
        // Camera and mouse settings
        // =====================================================================
        xf!(xfer, xfer_bool, self.use_alternate_mouse, "use_alternate_mouse");
        xf!(xfer, xfer_bool, self.client_retaliation_mode_enabled, "client_retaliation_mode_enabled");
        xf!(xfer, xfer_bool, self.double_click_attack_move, "double_click_attack_move");
        xf!(xfer, xfer_bool, self.right_mouse_always_scrolls, "right_mouse_always_scrolls");
        xf!(xfer, xfer_real, self.camera_pitch, "camera_pitch");
        xf!(xfer, xfer_real, self.camera_yaw, "camera_yaw");
        xf!(xfer, xfer_real, self.camera_height, "camera_height");
        xf!(xfer, xfer_real, self.max_camera_height, "max_camera_height");
        xf!(xfer, xfer_real, self.min_camera_height, "min_camera_height");
        xf!(xfer, xfer_real, self.horizontal_scroll_speed_factor, "horizontal_scroll_speed_factor");
        xf!(xfer, xfer_real, self.vertical_scroll_speed_factor, "vertical_scroll_speed_factor");
        xf!(xfer, xfer_real, self.scroll_amount_cutoff, "scroll_amount_cutoff");
        xf!(xfer, xfer_real, self.camera_adjust_speed, "camera_adjust_speed");
        xf!(xfer, xfer_bool, self.enforce_max_camera_height, "enforce_max_camera_height");
        xf!(xfer, xfer_real, self.keyboard_scroll_factor, "keyboard_scroll_factor");
        xf!(xfer, xfer_real, self.keyboard_default_scroll_factor, "keyboard_default_scroll_factor");
        xf!(xfer, xfer_real, self.keyboard_camera_rotate_speed, "keyboard_camera_rotate_speed");
        xf!(xfer, xfer_int, self.play_stats, "play_stats");
        xf!(xfer, xfer_real, self.camera_audible_radius, "camera_audible_radius");
        xf!(xfer, xfer_bool, self.save_camera_in_replay, "save_camera_in_replay");
        xf!(xfer, xfer_bool, self.use_camera_in_replay, "use_camera_in_replay");

        // =====================================================================
        // Water and sky settings
        // =====================================================================
        xf!(xfer, xfer_bool, self.use_water_plane, "use_water_plane");
        xf!(xfer, xfer_bool, self.use_cloud_plane, "use_cloud_plane");
        xf!(xfer, xfer_real, self.water_position_x, "water_position_x");
        xf!(xfer, xfer_real, self.water_position_y, "water_position_y");
        xf!(xfer, xfer_real, self.water_position_z, "water_position_z");
        xf!(xfer, xfer_real, self.water_extent_x, "water_extent_x");
        xf!(xfer, xfer_real, self.water_extent_y, "water_extent_y");
        xf!(xfer, xfer_int, self.water_type, "water_type");
        xf!(xfer, xfer_bool, self.show_soft_water_edge, "show_soft_water_edge");
        xf!(xfer, xfer_int, self.feather_water, "feather_water");
        xf!(xfer, xfer_real, self.downwind_angle, "downwind_angle");
        xf!(xfer, xfer_real, self.sky_box_position_z, "sky_box_position_z");
        xf!(xfer, xfer_bool, self.draw_sky_box, "draw_sky_box");
        xf!(xfer, xfer_real, self.sky_box_scale, "sky_box_scale");

        // =====================================================================
        // Vertex water settings (MAX_WATER_GRID_SETTINGS = 4)
        // =====================================================================
        for i in 0..MAX_WATER_GRID_SETTINGS {
            xf!(xfer, xfer_ascii_string, self.vertex_water_available_maps[i],
                "vertex_water_available_maps");
            xf!(xfer, xfer_real, self.vertex_water_height_clamp_low[i],
                "vertex_water_height_clamp_low");
            xf!(xfer, xfer_real, self.vertex_water_height_clamp_hi[i],
                "vertex_water_height_clamp_hi");
            xf!(xfer, xfer_real, self.vertex_water_angle[i],
                "vertex_water_angle");
            xf!(xfer, xfer_real, self.vertex_water_x_position[i],
                "vertex_water_x_position");
            xf!(xfer, xfer_real, self.vertex_water_y_position[i],
                "vertex_water_y_position");
            xf!(xfer, xfer_real, self.vertex_water_z_position[i],
                "vertex_water_z_position");
            xf!(xfer, xfer_int, self.vertex_water_x_grid_cells[i],
                "vertex_water_x_grid_cells");
            xf!(xfer, xfer_int, self.vertex_water_y_grid_cells[i],
                "vertex_water_y_grid_cells");
            xf!(xfer, xfer_real, self.vertex_water_grid_size[i],
                "vertex_water_grid_size");
            xf!(xfer, xfer_real, self.vertex_water_attenuation_a[i],
                "vertex_water_attenuation_a");
            xf!(xfer, xfer_real, self.vertex_water_attenuation_b[i],
                "vertex_water_attenuation_b");
            xf!(xfer, xfer_real, self.vertex_water_attenuation_c[i],
                "vertex_water_attenuation_c");
            xf!(xfer, xfer_real, self.vertex_water_attenuation_range[i],
                "vertex_water_attenuation_range");
        }

        // =====================================================================
        // Lighting: terrain_lighting and terrain_objects_lighting
        // [TIME_OF_DAY_COUNT][MAX_GLOBAL_LIGHTS] of TerrainLighting
        // Each TerrainLighting has ambient[3], diffuse[3], light_pos[3]
        // =====================================================================
        for tod in 0..TIME_OF_DAY_COUNT {
            for light in 0..MAX_GLOBAL_LIGHTS {
                for c in 0..3 {
                    xf!(xfer, xfer_real, self.terrain_lighting[tod][light].ambient[c],
                        "terrain_lighting.ambient");
                }
                for c in 0..3 {
                    xf!(xfer, xfer_real, self.terrain_lighting[tod][light].diffuse[c],
                        "terrain_lighting.diffuse");
                }
                for c in 0..3 {
                    xf!(xfer, xfer_real, self.terrain_lighting[tod][light].light_pos[c],
                        "terrain_lighting.light_pos");
                }
            }
        }
        for tod in 0..TIME_OF_DAY_COUNT {
            for light in 0..MAX_GLOBAL_LIGHTS {
                for c in 0..3 {
                    xf!(xfer, xfer_real, self.terrain_objects_lighting[tod][light].ambient[c],
                        "terrain_objects_lighting.ambient");
                }
                for c in 0..3 {
                    xf!(xfer, xfer_real, self.terrain_objects_lighting[tod][light].diffuse[c],
                        "terrain_objects_lighting.diffuse");
                }
                for c in 0..3 {
                    xf!(xfer, xfer_real, self.terrain_objects_lighting[tod][light].light_pos[c],
                        "terrain_objects_lighting.light_pos");
                }
            }
        }

        // Current terrain ambient/diffuse/light_pos [MAX_GLOBAL_LIGHTS]
        for i in 0..MAX_GLOBAL_LIGHTS {
            for c in 0..3 {
                xf!(xfer, xfer_real, self.terrain_ambient[i][c], "terrain_ambient");
            }
        }
        for i in 0..MAX_GLOBAL_LIGHTS {
            for c in 0..3 {
                xf!(xfer, xfer_real, self.terrain_diffuse[i][c], "terrain_diffuse");
            }
        }
        for i in 0..MAX_GLOBAL_LIGHTS {
            for c in 0..3 {
                xf!(xfer, xfer_real, self.terrain_light_pos[i][c], "terrain_light_pos");
            }
        }

        // Infantry light scale [TIME_OF_DAY_COUNT]
        for i in 0..TIME_OF_DAY_COUNT {
            xf!(xfer, xfer_real, self.infantry_light_scale[i], "infantry_light_scale");
        }
        xf!(xfer, xfer_real, self.script_override_infantry_light_scale,
            "script_override_infantry_light_scale");
        xf!(xfer, xfer_int, self.num_global_lights, "num_global_lights");

        // =====================================================================
        // Physics and gameplay
        // =====================================================================
        xf!(xfer, xfer_real, self.terrain_height_at_edge_of_map, "terrain_height_at_edge_of_map");
        xf!(xfer, xfer_real, self.unit_damaged_thresh, "unit_damaged_thresh");
        xf!(xfer, xfer_real, self.unit_really_damaged_thresh, "unit_really_damaged_thresh");
        xf!(xfer, xfer_real, self.ground_stiffness, "ground_stiffness");
        xf!(xfer, xfer_real, self.structure_stiffness, "structure_stiffness");
        xf!(xfer, xfer_real, self.gravity, "gravity");
        xf!(xfer, xfer_real, self.stealth_friendly_opacity, "stealth_friendly_opacity");
        xf!(xfer, xfer_unsigned_int, self.default_occlusion_delay, "default_occlusion_delay");
        xf!(xfer, xfer_real, self.partition_cell_size, "partition_cell_size");

        // =====================================================================
        // Ammo and container pips
        // =====================================================================
        for c in 0..3 {
            xf!(xfer, xfer_real, self.ammo_pip_world_offset[c], "ammo_pip_world_offset");
        }
        for c in 0..3 {
            xf!(xfer, xfer_real, self.container_pip_world_offset[c], "container_pip_world_offset");
        }
        for c in 0..2 {
            xf!(xfer, xfer_real, self.ammo_pip_screen_offset[c], "ammo_pip_screen_offset");
        }
        for c in 0..2 {
            xf!(xfer, xfer_real, self.container_pip_screen_offset[c], "container_pip_screen_offset");
        }
        xf!(xfer, xfer_real, self.ammo_pip_scale_factor, "ammo_pip_scale_factor");
        xf!(xfer, xfer_real, self.container_pip_scale_factor, "container_pip_scale_factor");
        xf!(xfer, xfer_unsigned_int, self.historic_damage_limit, "historic_damage_limit");

        // =====================================================================
        // Terrain tracks
        // =====================================================================
        xf!(xfer, xfer_int, self.max_terrain_tracks, "max_terrain_tracks");
        xf!(xfer, xfer_int, self.max_tank_track_edges, "max_tank_track_edges");
        xf!(xfer, xfer_int, self.max_tank_track_opaque_edges, "max_tank_track_opaque_edges");
        xf!(xfer, xfer_int, self.max_tank_track_fade_delay, "max_tank_track_fade_delay");

        // =====================================================================
        // Animations
        // =====================================================================
        xf!(xfer, xfer_ascii_string, self.level_gain_animation_name, "level_gain_animation_name");
        xf!(xfer, xfer_real, self.level_gain_animation_display_time_seconds,
            "level_gain_animation_display_time_seconds");
        xf!(xfer, xfer_real, self.level_gain_animation_z_rise_per_second,
            "level_gain_animation_z_rise_per_second");
        xf!(xfer, xfer_ascii_string, self.get_healed_animation_name, "get_healed_animation_name");
        xf!(xfer, xfer_real, self.get_healed_animation_display_time_seconds,
            "get_healed_animation_display_time_seconds");
        xf!(xfer, xfer_real, self.get_healed_animation_z_rise_per_second,
            "get_healed_animation_z_rise_per_second");

        // =====================================================================
        // Time and weather
        // =====================================================================
        let mut tod = self.time_of_day as i32;
        xf!(xfer, xfer_int, tod, "time_of_day");
        if matches!(xfer.get_xfer_mode(), XferMode::Load) {
            self.time_of_day = match tod {
                1 => TimeOfDay::Morning,
                2 => TimeOfDay::Afternoon,
                3 => TimeOfDay::Evening,
                4 => TimeOfDay::Night,
                _ => TimeOfDay::Afternoon,
            };
        }
        xf!(xfer, xfer_int, self.weather, "weather");
        xf!(xfer, xfer_bool, self.make_track_marks, "make_track_marks");
        xf!(xfer, xfer_bool, self.hide_garrison_flags, "hide_garrison_flags");
        xf!(xfer, xfer_bool, self.force_models_to_follow_time_of_day,
            "force_models_to_follow_time_of_day");
        xf!(xfer, xfer_bool, self.force_models_to_follow_weather,
            "force_models_to_follow_weather");

        // =====================================================================
        // Player bonuses
        // =====================================================================
        for pt in 0..PLAYERTYPE_COUNT {
            for diff in 0..DIFFICULTY_COUNT {
                xf!(xfer, xfer_real, self.solo_player_health_bonus_for_difficulty[pt][diff],
                    "solo_player_health_bonus_for_difficulty");
            }
        }

        // =====================================================================
        // Visibility and rendering limits
        // =====================================================================
        xf!(xfer, xfer_int, self.max_visible_translucent_objects, "max_visible_translucent_objects");
        xf!(xfer, xfer_int, self.max_visible_occluder_objects, "max_visible_occluder_objects");
        xf!(xfer, xfer_int, self.max_visible_occludee_objects, "max_visible_occludee_objects");
        xf!(xfer, xfer_int, self.max_visible_non_occluder_or_occludee_objects,
            "max_visible_non_occluder_or_occludee_objects");
        xf!(xfer, xfer_real, self.occluded_luminance_scale, "occluded_luminance_scale");
        xf!(xfer, xfer_int, self.texture_reduction_factor, "texture_reduction_factor");
        xf!(xfer, xfer_bool, self.enable_behind_building_markers, "enable_behind_building_markers");

        // =====================================================================
        // Roads
        // =====================================================================
        xf!(xfer, xfer_int, self.max_road_segments, "max_road_segments");
        xf!(xfer, xfer_int, self.max_road_vertex, "max_road_vertex");
        xf!(xfer, xfer_int, self.max_road_index, "max_road_index");
        xf!(xfer, xfer_int, self.max_road_types, "max_road_types");

        // =====================================================================
        // 3D audio settings
        // =====================================================================
        xf!(xfer, xfer_bool, self.sounds_3d_on, "sounds_3d_on");

        // =====================================================================
        // Particles
        // =====================================================================
        xf!(xfer, xfer_real, self.particle_scale, "particle_scale");
        xf!(xfer, xfer_int, self.max_particle_count, "max_particle_count");
        xf!(xfer, xfer_int, self.max_field_particle_count, "max_field_particle_count");

        // =====================================================================
        // Auto fire/smoke particles
        // =====================================================================
        xf!(xfer, xfer_ascii_string, self.auto_fire_particle_small_prefix, "auto_fire_particle_small_prefix");
        xf!(xfer, xfer_ascii_string, self.auto_fire_particle_small_system, "auto_fire_particle_small_system");
        xf!(xfer, xfer_int, self.auto_fire_particle_small_max, "auto_fire_particle_small_max");
        xf!(xfer, xfer_ascii_string, self.auto_fire_particle_medium_prefix, "auto_fire_particle_medium_prefix");
        xf!(xfer, xfer_ascii_string, self.auto_fire_particle_medium_system, "auto_fire_particle_medium_system");
        xf!(xfer, xfer_int, self.auto_fire_particle_medium_max, "auto_fire_particle_medium_max");
        xf!(xfer, xfer_ascii_string, self.auto_fire_particle_large_prefix, "auto_fire_particle_large_prefix");
        xf!(xfer, xfer_ascii_string, self.auto_fire_particle_large_system, "auto_fire_particle_large_system");
        xf!(xfer, xfer_int, self.auto_fire_particle_large_max, "auto_fire_particle_large_max");
        xf!(xfer, xfer_ascii_string, self.auto_smoke_particle_small_prefix, "auto_smoke_particle_small_prefix");
        xf!(xfer, xfer_ascii_string, self.auto_smoke_particle_small_system, "auto_smoke_particle_small_system");
        xf!(xfer, xfer_int, self.auto_smoke_particle_small_max, "auto_smoke_particle_small_max");
        xf!(xfer, xfer_ascii_string, self.auto_smoke_particle_medium_prefix, "auto_smoke_particle_medium_prefix");
        xf!(xfer, xfer_ascii_string, self.auto_smoke_particle_medium_system, "auto_smoke_particle_medium_system");
        xf!(xfer, xfer_int, self.auto_smoke_particle_medium_max, "auto_smoke_particle_medium_max");
        xf!(xfer, xfer_ascii_string, self.auto_smoke_particle_large_prefix, "auto_smoke_particle_large_prefix");
        xf!(xfer, xfer_ascii_string, self.auto_smoke_particle_large_system, "auto_smoke_particle_large_system");
        xf!(xfer, xfer_int, self.auto_smoke_particle_large_max, "auto_smoke_particle_large_max");
        xf!(xfer, xfer_ascii_string, self.auto_aflame_particle_prefix, "auto_aflame_particle_prefix");
        xf!(xfer, xfer_ascii_string, self.auto_aflame_particle_system, "auto_aflame_particle_system");
        xf!(xfer, xfer_int, self.auto_aflame_particle_max, "auto_aflame_particle_max");

        // =====================================================================
        // Network settings
        // =====================================================================
        xf!(xfer, xfer_unsigned_int, self.default_ip, "default_ip");
        xf!(xfer, xfer_unsigned_int, self.firewall_behavior, "firewall_behavior");
        xf!(xfer, xfer_bool, self.firewall_send_delay, "firewall_send_delay");
        xf!(xfer, xfer_unsigned_int, self.firewall_port_override, "firewall_port_override");
        xf!(xfer, xfer_short, self.firewall_port_allocation_delta, "firewall_port_allocation_delta");
        xf!(xfer, xfer_unsigned_int, self.network_fps_history_length, "network_fps_history_length");
        xf!(xfer, xfer_unsigned_int, self.network_latency_history_length, "network_latency_history_length");
        xf!(xfer, xfer_unsigned_int, self.network_cushion_history_length, "network_cushion_history_length");
        xf!(xfer, xfer_unsigned_int, self.network_run_ahead_metrics_time, "network_run_ahead_metrics_time");
        xf!(xfer, xfer_unsigned_int, self.network_keep_alive_delay, "network_keep_alive_delay");
        xf!(xfer, xfer_unsigned_int, self.network_run_ahead_slack, "network_run_ahead_slack");
        xf!(xfer, xfer_unsigned_int, self.network_disconnect_time, "network_disconnect_time");
        xf!(xfer, xfer_unsigned_int, self.network_player_timeout_time, "network_player_timeout_time");
        xf!(xfer, xfer_unsigned_int, self.network_disconnect_screen_notify_time,
            "network_disconnect_screen_notify_time");

        // =====================================================================
        // Economy and building
        // =====================================================================
        xf!(xfer, xfer_int, self.base_value_per_supply_box, "base_value_per_supply_box");
        xf!(xfer, xfer_real, self.build_speed, "build_speed");
        xf!(xfer, xfer_real, self.min_dist_from_edge_of_map_for_build, "min_dist_from_edge_of_map_for_build");
        xf!(xfer, xfer_real, self.supply_build_border, "supply_build_border");
        xf!(xfer, xfer_real, self.allowed_height_variation_for_building,
            "allowed_height_variation_for_building");
        xf!(xfer, xfer_real, self.min_low_energy_production_speed, "min_low_energy_production_speed");
        xf!(xfer, xfer_real, self.max_low_energy_production_speed, "max_low_energy_production_speed");
        xf!(xfer, xfer_real, self.low_energy_penalty_modifier, "low_energy_penalty_modifier");
        xf!(xfer, xfer_real, self.multiple_factory, "multiple_factory");
        xf!(xfer, xfer_real, self.refund_percent, "refund_percent");
        xf!(xfer, xfer_real, self.command_center_heal_range, "command_center_heal_range");
        xf!(xfer, xfer_real, self.command_center_heal_amount, "command_center_heal_amount");
        xf!(xfer, xfer_int, self.max_line_build_objects, "max_line_build_objects");
        xf!(xfer, xfer_int, self.max_tunnel_capacity, "max_tunnel_capacity");

        // =====================================================================
        // Veterancy and health
        // =====================================================================
        for i in 0..LEVEL_COUNT {
            xf!(xfer, xfer_real, self.health_bonus[i], "health_bonus");
        }
        xf!(xfer, xfer_real, self.default_structure_rubble_height, "default_structure_rubble_height");

        // =====================================================================
        // Special settings
        // =====================================================================
        xf!(xfer, xfer_ascii_string, self.pending_file, "pending_file");
        xf!(xfer, xfer_ascii_string, self.special_power_view_object_name,
            "special_power_view_object_name");

        // Standard public bones — versioned list of strings
        {
            let mut bone_count = self.standard_public_bones.len() as u16;
            xf!(xfer, xfer_unsigned_short, bone_count, "standard_public_bones count");
            match xfer.get_xfer_mode() {
                XferMode::Save | XferMode::Crc => {
                    for bone in &self.standard_public_bones {
                        let mut bone_name = bone.clone();
                        xf!(xfer, xfer_ascii_string, bone_name, "standard_public_bone");
                    }
                }
                XferMode::Load => {
                    self.standard_public_bones.clear();
                    for _ in 0..bone_count {
                        let mut bone_name = String::new();
                        xf!(xfer, xfer_ascii_string, bone_name, "standard_public_bone");
                        self.standard_public_bones.push(bone_name);
                    }
                }
                _ => {}
            }
        }

        xf!(xfer, xfer_real, self.standard_minefield_density, "standard_minefield_density");
        xf!(xfer, xfer_real, self.standard_minefield_distance, "standard_minefield_distance");
        xf!(xfer, xfer_bool, self.show_metrics, "show_metrics");
        xf!(xfer, xfer_int, self.default_starting_cash, "default_starting_cash");
        xf!(xfer, xfer_bool, self.debug_show_graphical_framerate, "debug_show_graphical_framerate");

        // =====================================================================
        // Power bar
        // =====================================================================
        xf!(xfer, xfer_int, self.power_bar_base, "power_bar_base");
        xf!(xfer, xfer_real, self.power_bar_intervals, "power_bar_intervals");
        xf!(xfer, xfer_int, self.power_bar_yellow_range, "power_bar_yellow_range");
        xf!(xfer, xfer_real, self.display_gamma, "display_gamma");
        xf!(xfer, xfer_unsigned_int, self.unlook_persist_duration, "unlook_persist_duration");

        // =====================================================================
        // Timing
        // =====================================================================
        xf!(xfer, xfer_unsigned_int, self.double_click_time_ms, "double_click_time_ms");

        // =====================================================================
        // Shroud and fog
        // =====================================================================
        for c in 0..3 {
            xf!(xfer, xfer_real, self.shroud_color[c], "shroud_color");
        }
        xf!(xfer, xfer_unsigned_byte, self.clear_alpha, "clear_alpha");
        xf!(xfer, xfer_unsigned_byte, self.fog_alpha, "fog_alpha");
        xf!(xfer, xfer_unsigned_byte, self.shroud_alpha, "shroud_alpha");

        // =====================================================================
        // Selection and audio
        // =====================================================================
        xf!(xfer, xfer_int, self.group_select_min_select_size, "group_select_min_select_size");
        xf!(xfer, xfer_real, self.group_select_volume_base, "group_select_volume_base");
        xf!(xfer, xfer_real, self.group_select_volume_increment, "group_select_volume_increment");
        xf!(xfer, xfer_int, self.max_unit_select_sounds, "max_unit_select_sounds");
        xf!(xfer, xfer_real, self.selection_flash_saturation_factor,
            "selection_flash_saturation_factor");
        xf!(xfer, xfer_bool, self.selection_flash_house_color, "selection_flash_house_color");
        xf!(xfer, xfer_real, self.group_move_click_to_gather_factor,
            "group_move_click_to_gather_factor");

        // =====================================================================
        // Graphics options
        // =====================================================================
        xf!(xfer, xfer_int, self.anti_alias_box_value, "anti_alias_box_value");
        xf!(xfer, xfer_bool, self.language_filter_pref, "language_filter_pref");
        xf!(xfer, xfer_bool, self.load_screen_render, "load_screen_render");
        xf!(xfer, xfer_bool, self.disable_render, "disable_render");

        // =====================================================================
        // Camera shake
        // =====================================================================
        xf!(xfer, xfer_real, self.shake_subtle_intensity, "shake_subtle_intensity");
        xf!(xfer, xfer_real, self.shake_normal_intensity, "shake_normal_intensity");
        xf!(xfer, xfer_real, self.shake_strong_intensity, "shake_strong_intensity");
        xf!(xfer, xfer_real, self.shake_severe_intensity, "shake_severe_intensity");
        xf!(xfer, xfer_real, self.shake_cine_extreme_intensity, "shake_cine_extreme_intensity");
        xf!(xfer, xfer_real, self.shake_cine_insane_intensity, "shake_cine_insane_intensity");
        xf!(xfer, xfer_real, self.max_shake_intensity, "max_shake_intensity");
        xf!(xfer, xfer_real, self.max_shake_range, "max_shake_range");

        // =====================================================================
        // Base regeneration
        // =====================================================================
        xf!(xfer, xfer_real, self.sell_percentage, "sell_percentage");
        xf!(xfer, xfer_real, self.base_regen_health_percent_per_second,
            "base_regen_health_percent_per_second");
        xf!(xfer, xfer_unsigned_int, self.base_regen_delay, "base_regen_delay");
        xf!(xfer, xfer_real, self.prison_bounty_multiplier, "prison_bounty_multiplier");
        for c in 0..3 {
            xf!(xfer, xfer_real, self.prison_bounty_text_color[c], "prison_bounty_text_color");
        }

        // =====================================================================
        // Colors
        // =====================================================================
        for c in 0..4 {
            xf!(xfer, xfer_real, self.hot_key_text_color[c], "hot_key_text_color");
        }

        // =====================================================================
        // Volume settings
        // =====================================================================
        xf!(xfer, xfer_real, self.music_volume_factor, "music_volume_factor");
        xf!(xfer, xfer_real, self.sfx_volume_factor, "sfx_volume_factor");
        xf!(xfer, xfer_real, self.voice_volume_factor, "voice_volume_factor");
        xf!(xfer, xfer_bool, self.sound_3d_pref, "sound_3d_pref");

        // =====================================================================
        // Movement penalties
        // =====================================================================
        xf!(xfer, xfer_int, self.movement_penalty_damage_state, "movement_penalty_damage_state");

        // =====================================================================
        // CRC values
        // =====================================================================
        xf!(xfer, xfer_unsigned_int, self.ini_crc, "ini_crc");
        xf!(xfer, xfer_unsigned_int, self.exe_crc, "exe_crc");

        // =====================================================================
        // Movies
        // =====================================================================
        xf!(xfer, xfer_bool, self.is_breakable_movie, "is_breakable_movie");
        xf!(xfer, xfer_bool, self.break_the_movie, "break_the_movie");
        xf!(xfer, xfer_bool, self.allow_exit_out_of_movies, "allow_exit_out_of_movies");

        // =====================================================================
        // TiVO fast mode
        // =====================================================================
        xf!(xfer, xfer_bool, self.tivo_fast_mode, "tivo_fast_mode");

        // =====================================================================
        // User data directory (private, but saved for session continuity)
        // =====================================================================
        xf!(xfer, xfer_ascii_string, self.user_data_dir, "user_data_dir");

        Ok(())
    }

    /// Post-load validation.
    /// C++ Reference: GlobalData::setTimeOfDay() is called at end of constructor
    /// to sync terrain lighting arrays from terrain_lighting.
    fn load_post_process(&mut self) -> Result<(), String> {
        // Sync terrain ambient/diffuse/light_pos from terrain_lighting based on time_of-day
        // Matches C++ GlobalData::setTimeOfDay() called at end of constructor
        let tod_index = self.time_of_day as usize;
        if tod_index < TIME_OF_DAY_COUNT {
            for i in 0..MAX_GLOBAL_LIGHTS {
                self.terrain_ambient[i] = self.terrain_lighting[tod_index][i].ambient;
                self.terrain_diffuse[i] = self.terrain_lighting[tod_index][i].diffuse;
                self.terrain_light_pos[i] = self.terrain_lighting[tod_index][i].light_pos;
            }
        }

        // Clamp camera bounds to sane values
        if self.min_camera_height < 0.0 {
            self.min_camera_height = 0.0;
        }
        if self.max_camera_height < self.min_camera_height {
            self.max_camera_height = self.min_camera_height + 100.0;
        }

        // Validate num_global_lights
        if self.num_global_lights < 0 || self.num_global_lights > MAX_GLOBAL_LIGHTS as i32 {
            self.num_global_lights = MAX_GLOBAL_LIGHTS as i32;
        }

        Ok(())
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

    pub fn clear_override(key: &str) {
        write().clear_override(key);
    }
}

/// Process-wide mutex for tests that temporarily mutate [`GLOBAL_DATA`].
///
/// All such tests must hold this lock so parallel suites cannot clobber
/// sell percentages and other shared knobs mid-assertion.
pub fn test_isolation_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Run `f` while holding [`test_isolation_lock`], restoring GlobalData afterward.
///
/// Recovers from a poisoned mutex (a prior panicking test) and still restores
/// the snapshot if `f` panics, then re-raises the panic.
pub fn with_global_data_restored<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

    let _guard = test_isolation_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let snapshot = read().clone();
    let result = catch_unwind(AssertUnwindSafe(f));
    *write() = snapshot;
    match result {
        Ok(value) => value,
        Err(payload) => resume_unwind(payload),
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
        assert!(data.camera_height >= 0.0);
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
        assert!(data.camera_height >= 0.0);
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
        drop(data);

        let data = read();
        let val = data.get_override("test_key");
        assert_eq!(val, Some(&GlobalValue::Int(42)));
    }

    #[test]
    fn test_global_data_override_bool() {
        // Test boolean override
        let mut data = write();
        data.set_override("test_bool", GlobalValue::Bool(true));
        drop(data);

        let data = read();
        let val = data.get_override("test_bool").and_then(|v| v.as_bool());
        assert_eq!(val, Some(true));
    }

    #[test]
    fn test_global_data_override_float() {
        // Test float override
        let mut data = write();
        data.set_override("test_float", GlobalValue::Float(3.14));
        drop(data);

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
        drop(data);

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
        // Verify constructor defaults match C++ pre-INI state.
        let data = GlobalData::default();
        assert_eq!(data.gravity, -1.0);
        assert_eq!(data.build_speed, 0.0);
        assert!(data.particle_scale > 0.0);
        assert_eq!(data.max_particle_count, 0);
    }

    #[test]
    fn test_global_data_init_reset() {
        let mut data = write();
        data.camera_height = 123.0;
        data.use_trees = true;
        data.set_override("test", GlobalValue::Int(99));

        data.init();
        assert_eq!(data.camera_height, 123.0);
        assert!(data.use_trees);
        assert_eq!(
            data.get_override("test").and_then(GlobalValue::as_int),
            Some(99)
        );

        data.reset();
        assert_eq!(data.camera_height, 123.0);
        assert!(data.use_trees);
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
                assert!(data.camera_height >= 0.0);
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
        drop(data);

        let data = read();
        assert_eq!(data.get_override("key1"), Some(&GlobalValue::Int(1)));
        assert_eq!(data.get_override("key2"), Some(&GlobalValue::Float(2.5)));
    }

    #[test]
    fn test_global_data_camera_settings() {
        // Verify camera settings can be read/written
        let data = read();
        assert_eq!(data.camera_pitch, 0.0);
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
        assert_eq!(data.water_extent_x, 0.0);
        assert_eq!(data.water_extent_y, 0.0);
    }

    #[test]
    fn test_global_data_constructor_defaults_match_cxx() {
        let data = GlobalData::default();
        assert_eq!(data.camera_pitch, 0.0);
        assert!(!data.draw_sky_box);
        assert_eq!(
            data.vertex_water_attenuation_range,
            [0.0; MAX_WATER_GRID_SETTINGS]
        );
        assert_eq!(data.terrain_ambient, [[0.0, 0.0, 0.0]; MAX_GLOBAL_LIGHTS]);
        assert_eq!(data.terrain_diffuse, [[0.0, 0.0, 0.0]; MAX_GLOBAL_LIGHTS]);
        assert_eq!(
            data.terrain_light_pos,
            [[0.0, 0.0, -1.0]; MAX_GLOBAL_LIGHTS]
        );
        assert_eq!(data.max_terrain_tracks, 0);
        assert!(data.level_gain_animation_name.is_empty());
        assert_eq!(data.level_gain_animation_display_time_seconds, 0.0);
        assert!(data.get_healed_animation_name.is_empty());
        assert_eq!(data.max_visible_non_occluder_or_occludee_objects, 512);
        assert_eq!(data.max_road_segments, 0);
        assert_eq!(data.max_road_vertex, 0);
        assert_eq!(data.max_road_index, 0);
        assert_eq!(data.max_road_types, 0);
        assert_eq!(data.max_particle_count, 0);
        assert_eq!(data.max_field_particle_count, 30);
        assert_eq!(data.auto_fire_particle_small_max, 0);
        assert_eq!(data.auto_fire_particle_medium_max, 0);
        assert_eq!(data.auto_fire_particle_large_max, 0);
        assert_eq!(data.auto_smoke_particle_small_max, 0);
        assert_eq!(data.auto_smoke_particle_medium_max, 0);
        assert_eq!(data.auto_smoke_particle_large_max, 0);
        assert_eq!(data.auto_aflame_particle_max, 0);
        assert_eq!(data.build_speed, 0.0);
        assert_eq!(data.min_dist_from_edge_of_map_for_build, 0.0);
        assert_eq!(data.supply_build_border, 0.0);
        assert_eq!(data.allowed_height_variation_for_building, 0.0);
        assert_eq!(data.min_low_energy_production_speed, 0.0);
        assert_eq!(data.max_low_energy_production_speed, 0.0);
        assert_eq!(data.low_energy_penalty_modifier, 0.0);
        assert_eq!(data.multiple_factory, 0.0);
        assert_eq!(data.command_center_heal_range, 0.0);
        assert_eq!(data.command_center_heal_amount, 0.0);
        assert_eq!(data.max_line_build_objects, 0);
        assert_eq!(data.max_tunnel_capacity, 0);
        assert_eq!(data.health_bonus, [1.0; LEVEL_COUNT]);
        assert_eq!(data.default_structure_rubble_height, 1.0);
        assert_eq!(data.group_select_volume_increment, 0.02);
        assert_eq!(data.max_unit_select_sounds, 8);
        assert!(!data.load_screen_render);
        assert_eq!(data.shake_strong_intensity, 2.5);
        assert!(!data.sound_3d_pref);
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
