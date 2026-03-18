#![allow(unused_imports, unused_variables, dead_code)]

use crate::common::language::Language;
use crate::common::rts::player_template::{
    get_player_template_store, get_player_template_store_mut,
};
use log::{info, warn};
use std::sync::Arc;
use std::{
    collections::{BTreeSet, HashSet},
    env, fs,
    path::PathBuf,
};

// INI configuration modules - Batch 1 (Converted)
pub mod ini;
pub mod ini_ai_data;
pub mod ini_animation;
pub mod ini_audio_event_info;
pub mod ini_command_button;
pub mod ini_command_set;
pub mod ini_control_bar_scheme;
pub mod ini_crate;

// Batch 2 (Converted)
pub mod ini_audio_settings;
pub mod ini_damage_fx;
pub mod ini_draw_group_info;
pub mod ini_game_data;
pub mod ini_map_cache;
pub mod ini_map_data;
pub mod ini_mapped_image;
pub mod ini_misc_audio;
pub mod ini_model;

// Batch 3 (New parsers - Completed)
pub mod ini_campaign;
pub mod ini_challenge_generals;
pub mod ini_credits;
pub mod ini_eva_event;
pub mod ini_fx_list;
pub mod ini_game_lod;
pub mod ini_language;
pub mod ini_locomotor;
pub mod ini_player_template;
pub mod ini_rank;
pub mod ini_road;
pub mod ini_science;

// Batch 4 (UI/Shell parsers)
pub mod ini_mouse;
pub mod ini_online_chat_colors;
pub mod ini_shell_menu_scheme;
pub mod ini_window_transition;

// Batch 5 (Critical gameplay parsers - newly implemented)
pub mod ini_command_map;
pub mod ini_header_template;
pub mod ini_in_game_ui;
pub mod ini_script;

// Placeholder modules for future batches
pub mod ini_multiplayer;
pub mod ini_object;
pub mod ini_particle_sys;
pub mod ini_special_power;
pub mod ini_terrain;
pub mod ini_terrain_bridge;
pub mod ini_terrain_road;
pub mod ini_upgrade;
pub mod ini_video;
pub mod ini_water;
pub mod ini_weapon;
pub mod ini_webpage_url;
pub mod ini_weather;

// Re-export main types from the batch 1 modules
pub use ini::{
    register_block_parser, FieldParse, INIError, INIFieldParseProc, INILoadType, INIResult, INI,
    LookupListRec,
};
pub use ini_ai_data::{
    get_ai_data_store, get_ai_data_store_mut, parse_ai_data_definition, AIData, AiSideBuildList,
    AiSideInfo, BuildListEntry, SkillSet,
};
pub use ini_animation::{
    get_anim2d_collection, parse_anim2d_definition, Anim2DCollection, Anim2DMode, Anim2DTemplate,
};
pub use ini_audio_event_info::{
    parse_audio_event_definition, parse_dialog_definition, parse_music_track_definition,
    AudioEventInfo, AudioPriority, AudioType,
};
pub use ini_command_button::{parse_command_button_definition, CommandButton, ControlBar};
pub use ini_command_set::{parse_command_set_definition, CommandSet};
pub use ini_control_bar_scheme::{
    ensure_control_bar_scheme_manager, get_control_bar_scheme_manager,
    parse_control_bar_scheme_definition, set_scheme_draw_func, ControlBarScheme,
    ControlBarSchemeManager, SchemeDrawFunc, SchemeImage,
};
pub use ini_crate::{
    parse_crate_template_definition, ensure_crate_system, get_crate_system,
    initialize_crate_system,
    ParsedCrateCreationEntry, ParsedCrateSystem, ParsedCrateTemplate,
};

// Re-export main types from batch 2 modules
pub use ini_audio_settings::{
    get_audio_settings, get_audio_settings_read, get_audio_settings_write, init_global_audio_settings,
    parse_audio_settings_definition, AudioSettings, SpeakerType, MAX_HW_PROVIDERS,
};
pub use ini_damage_fx::{parse_damage_fx_definition, DamageFX, DamageFXStore, DamageType};
pub use ini_draw_group_info::{
    parse_draw_group_number_definition, Color, DrawGroupInfo, FontInfo, PositionOffset,
};
pub use ini_game_data::{
    get_global_data, parse_game_data_definition, Coord2D, Coord3D, GlobalData, RGBColor, TimeOfDay,
    Weather,
};
pub use ini_map_cache::{
    parse_map_cache_definition, MapCache, MapMetaData, MapMetaDataReader, Region3D, WinTimeStamp,
};
pub use ini_map_data::{
    parse_map_data_definition, MapBounds, MapCamera, MapData, MapEnvironment, MapLighting,
};
pub use ini_mapped_image::{
    get_mapped_image_collection, parse_mapped_image_definition, ICoord2D, Image, ImageCollection,
    ImageStatus, Region2D,
};
pub use ini_misc_audio::{parse_misc_audio, AudioEventRTS, MiscAudio};
pub use ini_model::{
    parse_model_definition, Model, ModelAnimation, ModelLOD, ModelManager, ModelMaterial, Vector3D,
};
pub use ini_player_template::parse_player_template_definition;

// Re-export main types from batch 3 modules (new parsers)
pub use ini_campaign::{
    get_campaign_store, get_campaign_store_mut, init_campaign_store, parse_campaign_definition,
    Campaign, CampaignStore, Mission, MAX_DISPLAYED_UNITS, MAX_OBJECTIVE_LINES,
};
pub use ini_challenge_generals::{
    get_challenge_generals, get_challenge_generals_mut, init_challenge_generals,
    parse_challenge_generals_definition, ChallengeGenerals, GeneralPersona, NUM_GENERALS,
};
pub use ini_credits::{
    get_credits_manager, get_credits_manager_mut, init_credits_manager, parse_credits_definition,
    CreditStyle, CreditsLine, CreditsManager, CREDIT_SPACE_OFFSET,
};
pub use ini_eva_event::{
    get_eva_event_store, get_eva_event_store_mut, init_eva_event_store, parse_eva_event_definition,
    EvaCheckInfo, EvaEventStore, EvaMessage, EvaSideSounds,
};
pub use ini_fx_list::{
    get_fx_list_store, get_fx_list_store_mut, parse_fx_list_definition, FXList, FXListError,
    FXListResult, FXListStore, FXNugget,
};
pub use ini_game_lod::{
    get_game_lod_manager, get_game_lod_manager_mut, init_game_lod_manager,
    parse_bench_profile, parse_dynamic_game_lod_definition, parse_lod_preset,
    parse_really_low_mhz, parse_static_game_lod_definition, BenchProfile, ChipsetType,
    CpuType, DynamicGameLODInfo, DynamicGameLODLevel, GameLODManager, LODPresetInfo,
    ParticlePriorityType, StaticGameLODInfo, StaticGameLODLevel, MAX_BENCH_PROFILES,
    MAX_LOD_PRESETS_PER_LEVEL,
};
pub use ini_language::{
    get_global_language, get_global_language_read, get_global_language_write, init_global_language,
    parse_language_definition, FontDesc, GlobalLanguage, LANGUAGE_FIELD_PARSE_TABLE,
};
pub use ini_locomotor::{
    get_locomotor_store, get_locomotor_store_mut, parse_locomotor_template_definition,
    LocomotorAppearance, LocomotorBehaviorZ, LocomotorError, LocomotorPriority, LocomotorResult,
    LocomotorStore, LocomotorSurfaceTypeMask, LocomotorTemplate,
};
pub use ini_rank::{
    get_rank_info_store, get_rank_info_store_mut, init_rank_info_store, parse_rank_definition,
    RankError, RankInfo, RankInfoStore, RankResult,
};
pub use ini_road::{
    get_terrain_roads, get_terrain_roads_mut, parse_terrain_bridge_definition,
    parse_terrain_road_definition, BodyDamageType, BridgeTowerType, TerrainRoadCollection,
    TerrainRoadError, TerrainRoadResult, TerrainRoadType,
};
pub use ini_science::{
    get_science_store, get_science_store_mut, parse_science_definition, ScienceError, ScienceInfo,
    ScienceResult, ScienceStore, ScienceType,
};

// Re-export main types from batch 4 modules (UI/Shell parsers)
pub use ini_window_transition::{
    get_window_transition_store, get_window_transition_store_mut, init_window_transition_store,
    parse_window_transition_block, parse_window_transition_definition, TransitionGroup,
    TransitionStyle, TransitionWindow, WindowTransitionStore,
};
pub use ini_shell_menu_scheme::{
    get_shell_menu_scheme_manager, init_shell_menu_scheme_manager, parse_shell_menu_scheme_definition,
    ShellMenuScheme, ShellMenuSchemeImage, ShellMenuSchemeLine, ShellMenuSchemeManager,
};
pub use ini_mouse::{
    add_cursor_info, get_cursor_info, get_mouse_settings, get_mouse_settings_mut,
    init_global_mouse_settings, parse_mouse_cursor_definition, parse_mouse_definition,
    CursorInfo, MouseSettings, RedrawMode, RGBAColorInt,
    CURSOR_INFO_FIELD_PARSE_TABLE, MOUSE_SETTINGS_FIELD_PARSE_TABLE,
};
pub use ini_online_chat_colors::{
    get_online_chat_colors, get_online_chat_colors_mut, init_online_chat_colors,
    parse_online_chat_color_definition, register_online_chat_colors_parser,
    GSColorIndex, OnlineChatColors, GSCOLOR_MAX,
};

pub use crate::common::system::Matrix3D;

fn push_player_template_ini_file(
    files: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
    path: PathBuf,
) {
    if path.is_file() {
        let key = fs::canonicalize(&path).unwrap_or(path.clone());
        if seen.insert(key) {
            files.push(path);
        }
    }
}

fn discover_player_template_ini_files() -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();
    if let Ok(cwd) = env::current_dir() {
        for ancestor in cwd.ancestors() {
            roots.insert(ancestor.to_path_buf());
        }
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            for ancestor in parent.ancestors() {
                roots.insert(ancestor.to_path_buf());
            }
        }
    }

    let mod_dir = {
        let guard = crate::common::global_data::read();
        guard.writable.mod_dir.clone()
    };
    if !mod_dir.trim().is_empty() {
        let mod_root = PathBuf::from(mod_dir.trim());
        roots.insert(mod_root.clone());
        if let Ok(canonical) = fs::canonicalize(&mod_root) {
            roots.insert(canonical);
        }
    }

    let mut seen = HashSet::new();
    let mut files = Vec::new();
    for root in roots {
        push_player_template_ini_file(
            &mut files,
            &mut seen,
            root.join("Data/INI/Default/PlayerTemplate.ini"),
        );
        push_player_template_ini_file(
            &mut files,
            &mut seen,
            root.join("Data/INI/PlayerTemplate.ini"),
        );
        for extracted in [
            root.join("windows_game/extracted_big_files/INIZH"),
            root.join("windows_game/extracted_big_files_v2/INIZH"),
        ] {
            push_player_template_ini_file(
                &mut files,
                &mut seen,
                extracted.join("Data/INI/Default/PlayerTemplate.ini"),
            );
            push_player_template_ini_file(
                &mut files,
                &mut seen,
                extracted.join("Data/INI/PlayerTemplate.ini"),
            );
        }
    }
    files
}

fn load_player_templates() {
    let sources = discover_player_template_ini_files();
    if sources.is_empty() {
        warn!("No PlayerTemplate.ini sources discovered");
        return;
    }

    {
        let mut store = get_player_template_store_mut();
        store.clear();
    }

    let mut ini = INI::new();
    for (idx, source) in sources.iter().enumerate() {
        let load_type = if idx == 0 {
            INILoadType::Overwrite
        } else {
            INILoadType::MultiFile
        };
        if let Err(err) = ini.load(source, load_type) {
            warn!(
                "Failed to load PlayerTemplate source '{}': {}",
                source.display(),
                err
            );
        }
    }

    let store = get_player_template_store();
    info!(
        "PlayerTemplate store loaded {} templates from {} source files",
        store.len(),
        sources.len()
    );
}

pub fn ensure_player_templates_loaded() {
    let needs_load = {
        let store = get_player_template_store();
        store.is_empty()
    };
    if needs_load {
        load_player_templates();
    }
}

/// Initialize all INI subsystems
pub fn initialize_ini_systems() {
    // Batch 1 initialization
    ini_animation::initialize_anim2d_collection();
    ini_command_button::initialize_control_bar();
    ini_command_set::initialize_command_set_manager();
    ini_control_bar_scheme::initialize_control_bar_scheme_manager();
    ini_crate::initialize_crate_system();

    // Batch 2 initialization
    ini_audio_settings::init_global_audio_settings();
    ini_damage_fx::init_global_damage_fx_store();
    ini_draw_group_info::init_global_draw_group_info();
    ini_game_data::init_global_data();
    ini_map_cache::init_global_map_cache();
    ini_map_data::init_global_map_data();
    ini_mapped_image::init_global_mapped_image_collection();
    ini_misc_audio::init_global_misc_audio();
    ini_model::init_global_model_manager();
    load_player_templates();

    // Batch 3 initialization (new parsers)
    ini_campaign::init_campaign_store();
    ini_challenge_generals::init_challenge_generals();
    ini_game_lod::init_game_lod_manager();
    let _locomotor_guard = ini_locomotor::get_locomotor_store();
    let _science_guard = ini_science::get_science_store();
    let _road_guard = ini_road::get_terrain_roads();
    let _fx_list_guard = ini_fx_list::get_fx_list_store();

    // Batch 4 initialization (UI/Shell parsers)
    ini_mouse::init_global_mouse_settings();
    ini_online_chat_colors::init_online_chat_colors();
    ini_online_chat_colors::register_online_chat_colors_parser();
    ini_shell_menu_scheme::init_shell_menu_scheme_manager();
    ini_window_transition::init_window_transition_store();

    let _ = crate::game_network::game_info::set_map_players_provider(Arc::new(|map_name: &str| {
        ini_map_cache::get_map_cache()
            .and_then(|cache| cache.get(map_name).map(|meta| meta.num_players))
    }));
    let _ = crate::game_network::game_info::set_multiplayer_settings_provider(Arc::new(|| {
        ini_multiplayer::with_multiplayer_settings(|settings| {
            crate::game_network::game_info::MultiplayerSettingsView {
                show_random_player_template: settings.show_random_player_template,
                show_random_start_pos: settings.show_random_start_pos,
                show_random_color: settings.show_random_color,
                observer_color: settings
                    .get_color_value_by_name("Observer")
                    .map(|value| value as i32),
                random_color: settings
                    .get_color_value_by_name("Random")
                    .map(|value| value as i32),
                color_values: settings
                    .color_definitions
                    .iter()
                    .map(|def| def.get_color() as i32)
                    .collect(),
            }
        })
    }));
    let _ = crate::game_network::game_info::set_game_text_provider(Arc::new(|tag: &str| {
        Language::get_localized_string(tag)
    }));
    let _ = crate::game_network::game_info::set_player_template_display_name_provider(Arc::new(
        |index: i32| {
            if index < 0 {
                return None;
            }
            let store = get_player_template_store();
            store
                .get_nth_player_template(index as usize)
                .map(|template| template.get_display_name().to_string())
        },
    ));
    let _ = ini_map_cache::set_game_text_provider(Arc::new(|tag: &str| {
        Some(Language::get_localized_string(tag))
    }));
}
