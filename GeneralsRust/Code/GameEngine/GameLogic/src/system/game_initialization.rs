//! COMPLETE GAME INITIALIZATION FLOW
//!
//! Master orchestration module that ties together all initialization components based on:
//! - /GeneralsMD/Code/GameEngine/Source/GameLogic/GameLogic.cpp (startNewGame)
//! - /GeneralsMD/Code/GameEngine/Source/Common/GameEngine.cpp (init/reset)
//!
//! This module provides the complete game initialization flow from map loading
//! through player setup to game start.

use super::game_start::{GameStartSequence, ScriptResult};
use super::map_loader::{Coord3D, MapCache, MapLoader};
use super::player_init::{make_player_template, Difficulty, PlayerInitializer, PlayerList};
use super::victory_conditions::{ScoreKeeper, VictoryConditions, VictoryType};
use crate::ai::integration::{initialize_ai_integration, with_ai_integration_mut};
use crate::ai::THE_AI;
use crate::common::well_known_keys::{
    key_multiplayer_start_index, key_player_allies, key_player_color, key_player_display_name,
    key_player_enemies, key_player_faction, key_player_is_human, key_player_is_preorder,
    key_player_is_skirmish, key_player_name, key_player_night_color, key_player_start_money,
    key_skirmish_difficulty, key_team_all_clear_script, key_team_enemy_sighted_script,
    key_team_generic_script_hook, key_team_name, key_team_on_create_script,
    key_team_on_destroyed_script, key_team_on_idle_script, key_team_on_unit_destroyed_script,
    key_team_owner, key_team_production_condition,
};
use crate::common::{AsciiString, Color, Relationship};
use crate::helpers::TheGameText;
use crate::player::{
    GameDifficulty as LogicGameDifficulty, Player as LogicPlayer, PlayerList as LogicPlayerList,
    PlayerType as LogicPlayerType, ThePlayerList,
};
use crate::sides_list::get_sides_list;
use crate::team::get_team_factory;
use crate::team::MAX_GENERIC_SCRIPTS;
use game_engine::common::ini::ini::{INILoadType, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::resource_manager::get_resource_manager;
use game_engine::common::rts::player_template::{get_player_template_store, PlayerTemplate};
use game_engine::common::system::file::FileAccess;
use game_engine::common::system::file_system::get_file_system;
use game_engine::System::get_game_state;

use std::fs;
use std::io;
use std::path::Path;

pub(crate) const LOAD_PROGRESS_START: i32 = 0;
const LOAD_PROGRESS_POST_PARTICLE_INI_LOAD: i32 = LOAD_PROGRESS_START + 1;
const LOAD_PROGRESS_POST_LOAD_MAP: i32 = LOAD_PROGRESS_POST_PARTICLE_INI_LOAD + 1;
const LOAD_PROGRESS_SIDE_POPULATION: i32 = LOAD_PROGRESS_POST_LOAD_MAP + 1;
const LOAD_PROGRESS_POST_SIDE_LIST_INIT: i32 =
    LOAD_PROGRESS_SIDE_POPULATION + 1 + super::map_loader::MAX_SLOTS as i32;
const LOAD_PROGRESS_POST_PLAYER_LIST_RESET: i32 = LOAD_PROGRESS_POST_SIDE_LIST_INIT + 1;
const LOAD_PROGRESS_POST_SCRIPT_ENGINE_NEW_MAP: i32 = LOAD_PROGRESS_POST_PLAYER_LIST_RESET + 1;
const LOAD_PROGRESS_POST_STARTING_CAMERA: i32 = 92;
const LOAD_PROGRESS_POST_STARTING_CAMERA_2: i32 = LOAD_PROGRESS_POST_STARTING_CAMERA + 1;
pub(crate) const LOAD_PROGRESS_END: i32 = 100;

#[cfg(test)]
thread_local! {
    static START_NEW_GAME_REQUEST_AT_INIT_ENTRY: std::cell::Cell<bool> =
        std::cell::Cell::new(false);
}

/// Game mode enumeration
/// Matches C++ GameMode from GameLogic
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    SinglePlayer,
    Skirmish,
    Multiplayer,
    Replay,
    ShellMap,
}

/// Difficulty setting for AI
/// Matches C++ difficulty levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameDifficulty {
    Easy,
    Normal,
    Hard,
    Brutal,
}

impl From<GameDifficulty> for Difficulty {
    fn from(diff: GameDifficulty) -> Self {
        match diff {
            GameDifficulty::Easy => Difficulty::Easy,
            GameDifficulty::Normal => Difficulty::Normal,
            GameDifficulty::Hard => Difficulty::Hard,
            GameDifficulty::Brutal => Difficulty::Brutal,
        }
    }
}

fn skirmish_difficulty_from_int(value: i32) -> Option<Difficulty> {
    match value {
        0 => Some(Difficulty::Easy),
        1 => Some(Difficulty::Normal),
        2 => Some(Difficulty::Hard),
        3 => Some(Difficulty::Brutal),
        _ => None,
    }
}

/// Game initialization parameters
/// Encapsulates all configuration needed to start a game
pub struct GameInitParams {
    pub map_path: String,
    pub game_mode: GameMode,
    pub difficulty: GameDifficulty,
    pub num_players: usize,
    pub player_templates: Vec<PlayerTemplate>,
    pub victory_type: VictoryType,
    pub score_limit: Option<u64>,
    pub time_limit: Option<std::time::Duration>,
    pub fog_of_war_enabled: bool,
    pub starting_resources: u32,
    pub ai_script: String,
}

#[derive(Debug, Clone)]
struct PlayerMeta {
    is_skirmish: bool,
    is_preorder: bool,
    mp_start_index: i32,
    skirmish_difficulty: Option<Difficulty>,
}

impl Default for GameInitParams {
    fn default() -> Self {
        Self {
            map_path: String::new(),
            game_mode: GameMode::Skirmish,
            difficulty: GameDifficulty::Normal,
            num_players: 2,
            player_templates: Vec::new(),
            victory_type: VictoryType::Annihilation,
            score_limit: None,
            time_limit: None,
            fog_of_war_enabled: true,
            starting_resources: 10000,
            ai_script: "DefaultAI".to_string(),
        }
    }
}

/// Complete game state after initialization
pub struct GameState {
    pub map_loader: MapLoader,
    pub player_list: PlayerList,
    pub start_sequence: GameStartSequence,
    pub victory_conditions: VictoryConditions,
    pub score_keeper: ScoreKeeper,
    pub game_mode: GameMode,
    pub is_initialized: bool,
}

impl GameState {
    pub fn new() -> Self {
        Self {
            map_loader: MapLoader::new(),
            player_list: PlayerList::new(),
            start_sequence: GameStartSequence::new(0, 0),
            victory_conditions: VictoryConditions::new(VictoryType::Annihilation),
            score_keeper: ScoreKeeper::new(),
            game_mode: GameMode::Skirmish,
            is_initialized: false,
        }
    }

    /// Reset game state
    pub fn reset(&mut self) {
        self.map_loader.reset();
        self.player_list.clear();
        self.victory_conditions.reset();
        self.score_keeper.clear();
        self.is_initialized = false;
    }
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete game initialization orchestrator
/// Matches C++ GameLogic::startNewGame() flow
pub struct GameInitializer;

impl GameInitializer {
    /// Initialize a complete game from parameters
    /// This is the main entry point that orchestrates the entire initialization flow
    ///
    /// Matches C++ flow from GameLogic.cpp startNewGame()
    pub fn initialize_game(params: GameInitParams) -> io::Result<GameState> {
        #[cfg(test)]
        START_NEW_GAME_REQUEST_AT_INIT_ENTRY.with(|slot| {
            slot.set(crate::helpers::TheGameLogic::is_start_new_game_requested());
        });

        let mut game_state = GameState::new();

        // PHASE 1: MAP LOADING
        // =====================================================================
        // Load the map file, parse heightmap, objects, waypoints
        // Matches C++ map loading from MapUtil.cpp
        crate::helpers::TheGameLogic::update_load_progress(LOAD_PROGRESS_POST_PARTICLE_INI_LOAD);
        Self::load_map(&mut game_state, &params.map_path)?;
        crate::helpers::TheGameLogic::update_load_progress(LOAD_PROGRESS_POST_LOAD_MAP);

        // PHASE 2: PLAYER INITIALIZATION
        // =====================================================================
        // Create players from map data, assign colors, resources, alliances
        // Matches C++ player initialization from GameLogic.cpp
        Self::initialize_players(&mut game_state, &params)?;
        crate::helpers::TheGameLogic::update_load_progress(
            LOAD_PROGRESS_POST_SCRIPT_ENGINE_NEW_MAP,
        );

        // PHASE 3: GAME START SEQUENCE
        // =====================================================================
        // Run scripts, position camera, init fog of war, generate minimap, start AI
        // Matches C++ startup sequence from GameLogic.cpp and GameClient.cpp
        Self::execute_start_sequence(&mut game_state, &params)?;
        crate::helpers::TheGameLogic::update_load_progress(LOAD_PROGRESS_POST_STARTING_CAMERA_2);

        // PHASE 4: VICTORY CONDITIONS
        // =====================================================================
        // Setup victory condition checking and score tracking
        // Matches C++ VictoryConditions initialization
        Self::setup_victory_conditions(&mut game_state, &params);

        game_state.game_mode = params.game_mode;
        game_state.is_initialized = true;

        Ok(game_state)
    }

    /// Phase 1: Load map from file
    fn load_map(game_state: &mut GameState, map_path: &str) -> io::Result<()> {
        println!("[INIT] Phase 1: Loading map from {}", map_path);

        // C++ parity: load map.ini/solo.ini and map.str before terrain map load.
        Self::load_map_sidecar_resources(map_path);

        // Load the .map file
        game_state.map_loader.load_map(map_path)?;
        let map_data = game_state.map_loader.to_map_data();

        if let Ok(mut ai) = THE_AI.write() {
            ai.init();
        }
        if let Ok(mut terrain) = crate::terrain::get_terrain_logic().write() {
            terrain.set_source_filename(AsciiString::from(map_path));
            terrain.load_map_data(map_data);
            terrain.new_map(false);
        }

        let heightmap = game_state.map_loader.get_heightmap();
        let (width, height) = heightmap.get_playable_dimensions();

        println!("[INIT] Map loaded: {}x{} grid", width, height);
        println!(
            "[INIT] Found {} start positions",
            game_state.map_loader.count_start_spots()
        );
        println!(
            "[INIT] Found {} tech buildings",
            game_state.map_loader.get_tech_positions().len()
        );
        println!(
            "[INIT] Found {} supply sources",
            game_state.map_loader.get_supply_positions().len()
        );

        // Initialize game start sequence with map dimensions
        game_state.start_sequence = GameStartSequence::new(width as usize, height as usize);

        Ok(())
    }

    fn load_map_sidecar_resources(map_path: &str) {
        let map_for_sidecars = Self::resolve_sidecar_map_path(map_path);
        let Some(map_dir) = Self::map_directory_for_sidecars(&map_for_sidecars) else {
            return;
        };

        Self::load_map_ini_override(&map_dir, "map.ini");
        Self::load_map_ini_override(&map_dir, "solo.ini");

        if let Some(map_str_path) = Self::find_existing_case_variants(&[
            format!("{map_dir}/map.str"),
            format!("{map_dir}/Map.str"),
        ]) {
            if let Err(err) = TheGameText::init_map_string_file(&map_str_path) {
                log::warn!(
                    "Failed to initialize map string file '{}': {}",
                    map_str_path,
                    err
                );
            }
        }

        // C++ parity note: Display::doSmartAssetPurgeAndPreload consumes this file.
        // Missing files still trigger a purge with an empty exclusion list.
        Self::smart_asset_purge_from_usage_manifest(&format!("{map_dir}/AssetUsage.txt"));
    }

    fn resolve_sidecar_map_path(map_path: &str) -> String {
        let normalized_map = Self::normalize_path_for_compare(map_path);
        let state = get_game_state();
        let normalized_save_dir =
            Self::normalize_path_for_compare(state.get_save_directory().to_string_lossy().as_ref());

        if !normalized_save_dir.is_empty() && normalized_map.starts_with(&normalized_save_dir) {
            let pristine = state.get_pristine_map_name().trim();
            if !pristine.is_empty() {
                return pristine.to_string();
            }
        }

        map_path.to_string()
    }

    fn normalize_path_for_compare(path: &str) -> String {
        path.replace('\\', "/").to_ascii_lowercase()
    }

    fn map_directory_for_sidecars(map_path: &str) -> Option<String> {
        let normalized = map_path.replace('\\', "/");
        let trimmed = normalized.trim().trim_end_matches('/');
        if trimmed.is_empty() {
            return None;
        }

        let mut without_ext = trimmed.to_string();
        if without_ext.to_ascii_lowercase().ends_with(".map") {
            without_ext.truncate(without_ext.len() - 4);
        }

        let slash = without_ext.rfind('/')?;
        let directory = without_ext[..slash].trim_end_matches('/');
        if directory.is_empty() {
            None
        } else {
            Some(directory.to_string())
        }
    }

    fn load_map_ini_override(map_dir: &str, filename: &str) {
        let path = format!("{map_dir}/{filename}");
        if !Self::file_exists(&path) {
            return;
        }

        let mut ini = INI::new();
        if let Err(err) = ini.load(&path, INILoadType::CreateOverrides) {
            log::warn!("Failed to load map override INI '{}': {}", path, err);
        }
    }

    fn smart_asset_purge_from_usage_manifest(path: &str) {
        let resources = Self::read_asset_usage_manifest(path);

        let manager_arc = get_resource_manager();
        let manager_lock = manager_arc.lock();
        if let Ok(manager_guard) = manager_lock {
            let refs = resources.iter().map(|s| s.as_str()).collect::<Vec<_>>();
            let _ = manager_guard.free_resources_with_exclusion_list(&refs);
        }
    }

    fn read_asset_usage_manifest(path: &str) -> Vec<String> {
        let Some(contents) = Self::read_text_file(path) else {
            return Vec::new();
        };

        let mut resources = Vec::new();
        for raw_line in contents.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with(';') {
                continue;
            }

            let resource = line.split_whitespace().next().unwrap_or("").trim();
            if !resource.is_empty() {
                resources.push(resource.to_string());
            }
        }

        resources
    }

    fn find_existing_case_variants(candidates: &[String]) -> Option<String> {
        candidates
            .iter()
            .find(|candidate| Self::file_exists(candidate))
            .cloned()
    }

    fn read_text_file(path: &str) -> Option<String> {
        if let Ok(contents) = fs::read_to_string(path) {
            return Some(contents);
        }

        let fs = get_file_system();
        let mut fs_guard = fs.lock().ok()?;
        let mut file = fs_guard.open_file(path, FileAccess::READ.combine(FileAccess::BINARY))?;
        let bytes = file.read_entire_and_close().ok()?;
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }

    fn file_exists(path: &str) -> bool {
        if Path::new(path).exists() {
            return true;
        }
        let alt = path.replace('/', "\\");
        if alt != path && Path::new(&alt).exists() {
            return true;
        }

        let fs = get_file_system();
        if let Ok(fs_guard) = fs.lock() {
            if fs_guard.does_file_exist(path) {
                return true;
            }
            if alt != path && fs_guard.does_file_exist(&alt) {
                return true;
            }
        }
        false
    }

    /// Phase 2: Initialize players from map data and templates
    fn initialize_players(game_state: &mut GameState, params: &GameInitParams) -> io::Result<()> {
        println!(
            "[INIT] Phase 2: Initializing {} players",
            params.num_players
        );

        // Extract start positions from map waypoints
        let start_positions =
            Self::extract_start_positions(&game_state.map_loader, params.num_players);

        let mut human_flags: Option<Vec<bool>> = None;
        let mut per_player_meta: Option<Vec<PlayerMeta>> = None;
        let mut templates = if params.player_templates.is_empty() {
            if let Ok(sides_guard) = get_sides_list().read() {
                let mut from_sides = Vec::new();
                let mut flags = Vec::new();
                let mut meta = Vec::new();
                let store = get_player_template_store();
                for index in 0..sides_guard.get_num_sides() {
                    let Some(side) = sides_guard.get_side_info(index) else {
                        continue;
                    };
                    let dict = side.get_dict();
                    let player_name = dict.get_ascii_string(key_player_name());
                    if player_name.is_empty() {
                        continue;
                    }
                    let faction = dict.get_ascii_string(key_player_faction());
                    if faction.is_empty() {
                        continue;
                    }
                    let display_name = dict.get_unicode_string(key_player_display_name());
                    let allies = dict.get_ascii_string(key_player_allies());
                    let enemies = dict.get_ascii_string(key_player_enemies());
                    let is_human = dict.get_bool(key_player_is_human());
                    let is_skirmish = dict.get_bool(key_player_is_skirmish());
                    let is_preorder = dict.get_bool(key_player_is_preorder());
                    let mp_start_index = if dict.get_type(key_multiplayer_start_index()).is_some() {
                        dict.get_int(key_multiplayer_start_index())
                    } else {
                        0
                    };
                    let skirmish_difficulty = if dict.get_type(key_skirmish_difficulty()).is_some()
                    {
                        skirmish_difficulty_from_int(dict.get_int(key_skirmish_difficulty()))
                    } else {
                        None
                    };

                    let mut template = store
                        .find_template(&faction)
                        .cloned()
                        .unwrap_or_else(|| make_player_template(&player_name, &faction));

                    template.name = player_name.clone();
                    if !display_name.is_empty() {
                        template.display_name = display_name;
                    } else {
                        template.display_name = player_name.clone();
                    }
                    if template.side.is_empty() {
                        template.side = faction.clone();
                    }
                    if template.base_side.is_empty() {
                        template.base_side = template.side.clone();
                    }
                    template.player_allies = allies;
                    template.player_enemies = enemies;

                    from_sides.push(template);
                    flags.push(is_human);
                    meta.push(PlayerMeta {
                        is_skirmish,
                        is_preorder,
                        mp_start_index,
                        skirmish_difficulty,
                    });
                }
                if !from_sides.is_empty() {
                    human_flags = Some(flags);
                    per_player_meta = Some(meta);
                    from_sides
                } else {
                    store
                        .iter()
                        .filter(|template| template.playable && !template.is_observer)
                        .cloned()
                        .collect::<Vec<PlayerTemplate>>()
                }
            } else {
                let store = get_player_template_store();
                store
                    .iter()
                    .filter(|template| template.playable && !template.is_observer)
                    .cloned()
                    .collect::<Vec<PlayerTemplate>>()
            }
        } else {
            params.player_templates.clone()
        };

        if templates.len() < params.num_players {
            let needed = params.num_players - templates.len();
            for i in 0..needed {
                let name = format!("Player {}", templates.len() + i + 1);
                templates.push(make_player_template(&name, "USA"));
            }
        }

        // Create player list from templates and start positions
        game_state.player_list = PlayerInitializer::init_from_map_with_human_flags(
            params.num_players,
            &templates,
            &start_positions,
            human_flags.as_deref(),
        );

        if let Some(meta) = per_player_meta {
            for (player, meta) in game_state
                .player_list
                .get_all_players_mut()
                .iter_mut()
                .zip(meta)
            {
                player.set_mp_start_index(meta.mp_start_index);
                player.set_preorder(meta.is_preorder);
                player.set_skirmish(meta.is_skirmish);
                player.set_skirmish_difficulty(meta.skirmish_difficulty);
            }
        }

        Self::apply_skirmish_overrides(&mut game_state.player_list);
        Self::apply_player_dict_overrides(&mut game_state.player_list);

        // Apply game difficulty to AI players (allow per-player overrides)
        let difficulty = Difficulty::from(params.difficulty);
        for player in game_state.player_list.get_all_players_mut() {
            if player.is_ai {
                if let Some(player_diff) = player.skirmish_difficulty {
                    player.set_difficulty(player_diff);
                } else {
                    player.set_difficulty(difficulty);
                }
            }
        }

        // Set custom starting resources if specified
        if params.starting_resources > 0 {
            for player in game_state.player_list.get_all_players_mut() {
                player.current_money = params.starting_resources;
            }
        }

        // Initialize score tracking for all players
        for player in game_state.player_list.get_all_players() {
            game_state.score_keeper.init_player(player.index);
        }

        // Apply alliances/enemies from SidesList if provided, otherwise default to enemies.
        game_state
            .player_list
            .init_relationships_from_allies_enemies();

        Self::sync_player_list_to_game_logic(&game_state.player_list);
        Self::sync_teams_from_sides();
        Self::sync_side_scripts_to_script_engine();
        Self::initialize_ai_integration_for_players();

        println!("[INIT] Players initialized:");
        for player in game_state.player_list.get_all_players() {
            println!(
                "[INIT]   Player {}: {} ({}, {}, ${}) at {:?}",
                player.index + 1,
                player.name,
                player.template.side,
                if player.is_human { "Human" } else { "AI" },
                player.current_money,
                player.start_position
            );
        }

        Ok(())
    }

    fn sync_player_list_to_game_logic(system_list: &super::player_init::PlayerList) {
        let mut logic_list = LogicPlayerList::new();
        let mut logic_players = Vec::new();

        for system_player in system_list.get_all_players() {
            let mut logic_player = LogicPlayer::new(system_player.index as i32);
            let display_name = if system_player.template.display_name.is_empty() {
                system_player.name.clone()
            } else {
                system_player.template.display_name.clone()
            };
            logic_player.set_display_name(display_name);
            logic_player.set_side(system_player.template.side.clone());
            logic_player.set_base_side(system_player.template.base_side.clone());
            logic_player.set_observer(system_player.is_observer);
            let name_key = NameKeyGenerator::name_to_key(&system_player.original_name);
            logic_player.set_player_name_key(name_key);

            let player_type = if system_player.is_observer {
                LogicPlayerType::Observer
            } else if system_player.is_human {
                LogicPlayerType::Human
            } else {
                LogicPlayerType::Computer
            };
            logic_player.set_player_type(player_type, system_player.is_skirmish);
            logic_player.set_mp_start_index(system_player.mp_start_index);
            logic_player.set_is_preorder(system_player.is_preorder);
            let base_logic_diff = match system_player.difficulty {
                Difficulty::Easy => LogicGameDifficulty::Easy,
                Difficulty::Normal => LogicGameDifficulty::Normal,
                Difficulty::Hard => LogicGameDifficulty::Hard,
                Difficulty::Brutal => LogicGameDifficulty::Brutal,
            };
            logic_player.set_difficulty(base_logic_diff);
            if let Some(player_diff) = system_player.skirmish_difficulty {
                let logic_diff = match player_diff {
                    Difficulty::Easy => LogicGameDifficulty::Easy,
                    Difficulty::Normal => LogicGameDifficulty::Normal,
                    Difficulty::Hard => LogicGameDifficulty::Hard,
                    Difficulty::Brutal => LogicGameDifficulty::Brutal,
                };
                logic_player.set_difficulty(logic_diff);
            }

            let color = Color {
                a: ((system_player.color >> 24) & 0xFF) as u8,
                r: ((system_player.color >> 16) & 0xFF) as u8,
                g: ((system_player.color >> 8) & 0xFF) as u8,
                b: (system_player.color & 0xFF) as u8,
            };
            let night_color = Color {
                a: ((system_player.night_color >> 24) & 0xFF) as u8,
                r: ((system_player.night_color >> 16) & 0xFF) as u8,
                g: ((system_player.night_color >> 8) & 0xFF) as u8,
                b: (system_player.night_color & 0xFF) as u8,
            };
            logic_player.set_colors(color, night_color);
            logic_player
                .get_money_mut()
                .set_money(system_player.current_money as i32);

            let template = crate::player::PlayerTemplate::from_common(&system_player.template);
            logic_player.init(std::sync::Arc::new(template));
            logic_player.init_from_dict_defaults();

            if let Ok(sides_guard) = get_sides_list().read() {
                if let Some(side_info) = sides_guard.get_side_info(system_player.index as usize) {
                    logic_player.apply_handicap_from_dict(side_info.get_dict());
                }
            }

            if let Ok(mut sides_guard) = get_sides_list().write() {
                if let Some(side_info) = sides_guard.get_side_info_mut(system_player.index as usize)
                {
                    if let Some(build_list) = side_info.take_build_list() {
                        logic_player.set_build_list(Some(*build_list));
                    }
                }
            }

            let arc = std::sync::Arc::new(std::sync::RwLock::new(logic_player));
            logic_list.add_player(std::sync::Arc::clone(&arc));
            logic_players.push(arc);
        }

        if let Some(local_index) = system_list.get_local_player_index() {
            logic_list.set_local_player_index(local_index as i32);
        }

        let system_players = system_list.get_all_players();
        for (i, system_player) in system_players.iter().enumerate() {
            let Some(player_arc) = logic_players.get(i) else {
                continue;
            };
            for (j, _) in system_players.iter().enumerate() {
                if i == j {
                    continue;
                }
                let rel = match system_player.get_relationship(j) {
                    super::player_init::PlayerRelationship::Ally => Relationship::Allies,
                    super::player_init::PlayerRelationship::Enemy => Relationship::Enemies,
                    super::player_init::PlayerRelationship::Neutral => Relationship::Neutral,
                };
                let other_arc = match logic_players.get(j) {
                    Some(arc) => arc,
                    None => continue,
                };
                if let (Ok(mut player_guard), Ok(other_guard)) =
                    (player_arc.write(), other_arc.read())
                {
                    player_guard.set_player_relationship(&other_guard, rel);
                }
            }
        }

        if let Ok(mut guard) = ThePlayerList().write() {
            *guard = logic_list;
        }
    }

    fn apply_skirmish_overrides(system_list: &mut PlayerList) {
        let sides_list = get_sides_list();
        let Ok(mut sides_guard) = sides_list.write() else {
            return;
        };

        if sides_guard.get_num_skirmish_sides() == 0 {
            return;
        }

        let store = get_player_template_store();

        for player in system_list.get_all_players_mut() {
            let side_index = player.index as usize;
            let player_name = match sides_guard.get_side_info(side_index) {
                Some(side_info) => side_info.get_dict().get_ascii_string(key_player_name()),
                None => continue,
            };
            if player_name.is_empty() {
                continue;
            }

            let has_skirmish_key = sides_guard
                .get_side_info(side_index)
                .and_then(|side_info| side_info.get_dict().get_type(key_player_is_skirmish()))
                .is_some();
            if has_skirmish_key {
                let mut found = false;
                for sp_idx in 0..sides_guard.get_num_skirmish_sides() {
                    let Some(skirmish_side) = sides_guard.get_skirmish_side_info(sp_idx) else {
                        continue;
                    };
                    let template_name = skirmish_side
                        .get_dict()
                        .get_ascii_string(key_player_faction());
                    if let Some(template) = store.find_template(&template_name) {
                        if template.side == player.template.side {
                            found = true;
                            break;
                        }
                    }
                }
                if !found {
                    player.is_skirmish = false;
                    player.is_human = true;
                    player.is_ai = false;
                }
            }

            let qualifier = format!("{}", player.mp_start_index);

            if player.is_human {
                if sides_guard.get_num_skirmish_sides() > 0 {
                    let mut civ_index = None;
                    for sp_idx in 0..sides_guard.get_num_skirmish_sides() {
                        let Some(skirmish_side) = sides_guard.get_skirmish_side_info(sp_idx) else {
                            continue;
                        };
                        let template_name = skirmish_side
                            .get_dict()
                            .get_ascii_string(key_player_faction());
                        if let Some(template) = store.find_template(&template_name) {
                            if template.side == "Civilian" {
                                civ_index = Some(sp_idx);
                                break;
                            }
                        }
                    }

                    if let Some(skirmish_idx) = civ_index {
                        let qual_template_player_name = {
                            let skirmish_side =
                                sides_guard.get_skirmish_side_info(skirmish_idx).unwrap();
                            format!(
                                "{}{}",
                                skirmish_side.get_dict().get_ascii_string(key_player_name()),
                                player.mp_start_index
                            )
                        };

                        let duplicated_scripts = if let Some(skirmish_side) =
                            sides_guard.get_skirmish_side_info_mut(skirmish_idx)
                        {
                            let scripts = skirmish_side.get_script_list().map(|scripts| {
                                scripts.duplicate_and_qualify(
                                    &qualifier,
                                    &qual_template_player_name,
                                    &player_name,
                                )
                            });
                            skirmish_side.set_script_list(None);
                            scripts
                        } else {
                            None
                        };
                        if let Some(scripts) = duplicated_scripts {
                            if let Some(side_info) = sides_guard.get_side_info_mut(side_index) {
                                side_info.set_script_list(Some(scripts));
                            }
                        }
                    }
                }

                player.is_skirmish = false;
                continue;
            }

            if !player.is_skirmish {
                continue;
            }

            let mut skirmish_index = None;
            for sp_idx in 0..sides_guard.get_num_skirmish_sides() {
                let Some(skirmish_side) = sides_guard.get_skirmish_side_info(sp_idx) else {
                    continue;
                };
                let template_name = skirmish_side
                    .get_dict()
                    .get_ascii_string(key_player_faction());
                if let Some(template) = store.find_template(&template_name) {
                    if template.side == player.template.side {
                        skirmish_index = Some(sp_idx);
                        break;
                    }
                }
            }

            let Some(skirmish_index) = skirmish_index else {
                continue;
            };

            let qual_template_player_name = {
                let skirmish_side = sides_guard.get_skirmish_side_info(skirmish_index).unwrap();
                format!(
                    "{}{}",
                    skirmish_side.get_dict().get_ascii_string(key_player_name()),
                    player.mp_start_index
                )
            };

            let duplicated_scripts = if let Some(skirmish_side) =
                sides_guard.get_skirmish_side_info_mut(skirmish_index)
            {
                let scripts = skirmish_side.get_script_list().map(|scripts| {
                    scripts.duplicate_and_qualify(
                        &qualifier,
                        &qual_template_player_name,
                        &player_name,
                    )
                });
                skirmish_side.set_script_list(None);
                scripts
            } else {
                None
            };
            if let Some(scripts) = duplicated_scripts {
                if let Some(side_info) = sides_guard.get_side_info_mut(side_index) {
                    side_info.set_script_list(Some(scripts));
                }
            }

            player.name = qual_template_player_name;

            let mut team_index = 0usize;
            while team_index < sides_guard.get_num_teams() {
                let remove = sides_guard
                    .get_team_info(team_index)
                    .map(|team| team.get_dict().get_ascii_string(key_team_owner()) == player_name)
                    .unwrap_or(false);
                if remove {
                    sides_guard.remove_team(team_index);
                } else {
                    team_index += 1;
                }
            }

            let original_player_name = {
                let skirmish_side = sides_guard.get_skirmish_side_info(skirmish_index).unwrap();
                skirmish_side.get_dict().get_ascii_string(key_player_name())
            };

            for team_idx in 0..sides_guard.get_num_skirmish_teams() {
                let Some(team_info) = sides_guard.get_skirmish_team_info(team_idx) else {
                    continue;
                };
                if team_info.get_dict().get_ascii_string(key_team_owner()) != original_player_name {
                    continue;
                }

                let mut team_dict = team_info.get_dict().clone();
                let team_name = team_dict.get_ascii_string(key_team_name());
                let new_name = format!("{}{}", team_name, player.mp_start_index);
                if sides_guard.find_team_info(new_name.as_str()).is_some() {
                    continue;
                }
                team_dict.set_ascii_string(key_team_owner(), player_name.clone());
                team_dict.set_ascii_string(key_team_name(), new_name);

                let script_keys = [
                    key_team_on_create_script(),
                    key_team_on_idle_script(),
                    key_team_on_unit_destroyed_script(),
                    key_team_on_destroyed_script(),
                    key_team_enemy_sighted_script(),
                    key_team_all_clear_script(),
                    key_team_production_condition(),
                ];
                for key in script_keys {
                    if let Some(text) = match team_dict.get_type(key) {
                        Some(_) => {
                            let value = team_dict.get_ascii_string(key);
                            if value.is_empty() {
                                None
                            } else {
                                Some(value)
                            }
                        }
                        None => None,
                    } {
                        team_dict
                            .set_ascii_string(key, format!("{}{}", text, player.mp_start_index));
                    }
                }

                let generic_base = NameKeyGenerator::key_to_name(key_team_generic_script_hook())
                    .unwrap_or_default();
                for j in 0..MAX_GENERIC_SCRIPTS {
                    let key_name = format!("{}{}", generic_base, j);
                    let key = NameKeyGenerator::name_to_key(&key_name);
                    let value = team_dict.get_ascii_string(key);
                    if !value.is_empty() {
                        team_dict
                            .set_ascii_string(key, format!("{}{}", value, player.mp_start_index));
                    }
                }

                sides_guard.add_team(&team_dict);
            }
        }
    }

    fn apply_player_dict_overrides(system_list: &mut PlayerList) {
        let sides_list = get_sides_list();
        let Ok(sides_guard) = sides_list.read() else {
            return;
        };

        for player in system_list.get_all_players_mut() {
            let side_index = player.index as usize;
            let Some(side_info) = sides_guard.get_side_info(side_index) else {
                continue;
            };
            let dict = side_info.get_dict();

            if dict.get_type(key_player_color()).is_some() {
                let color = dict.get_int(key_player_color());
                player.color = (color as u32) | 0xff00_0000;
                player.night_color = player.color;
            }

            if dict.get_type(key_player_night_color()).is_some() {
                let color = dict.get_int(key_player_night_color());
                player.night_color = (color as u32) | 0xff00_0000;
            }

            if dict.get_type(key_player_start_money()).is_some() {
                let money = dict.get_int(key_player_start_money());
                if money > 0 {
                    player.current_money = money as u32;
                }
            }

            if !player.is_human {
                player.is_preorder = false;
            }
        }
    }

    fn sync_teams_from_sides() {
        let sides_list = get_sides_list();
        let Ok(sides_guard) = sides_list.read() else {
            return;
        };

        let mut team_factory = get_team_factory().lock().unwrap();
        team_factory.reset();

        for index in 0..sides_guard.get_num_teams() {
            let Some(team_info) = sides_guard.get_team_info(index) else {
                continue;
            };
            let dict = team_info.get_dict();
            let team_name = dict.get_ascii_string(crate::common::well_known_keys::key_team_name());
            if team_name.is_empty() {
                continue;
            }
            let owner = dict.get_ascii_string(crate::common::well_known_keys::key_team_owner());
            let singleton = dict.get_bool(crate::common::well_known_keys::key_team_is_singleton());

            let _ = team_factory.init_team(
                team_name.clone().into(),
                owner.clone().into(),
                singleton,
                Some(dict),
            );
            let team = team_factory
                .find_team(&team_name)
                .or_else(|| team_factory.create_team(&team_name));

            let Some(team_arc) = team else { continue };

            if let Ok(mut team_guard) = team_arc.write() {
                if !owner.is_empty() {
                    if let Ok(player_list) = ThePlayerList().read() {
                        if let Some(player_arc) = player_list.find_player_by_name(&owner) {
                            if let Ok(player_guard) = player_arc.read() {
                                team_guard.set_controlling_player_id(Some(
                                    player_guard.get_player_index() as u32,
                                ));
                            }
                        }
                    }
                }
            };
        }

        if let Ok(player_list) = ThePlayerList().read() {
            for player_arc in player_list.iter() {
                let Ok(player_guard) = player_arc.read() else {
                    continue;
                };
                let name_key = player_guard.get_player_name_key();
                let player_name = NameKeyGenerator::key_to_name(name_key).unwrap_or_default();
                drop(player_guard);

                let default_team_name = format!("team{}", player_name);
                if let Some(team_arc) = team_factory.find_team(&default_team_name) {
                    if let Ok(mut player_guard) = player_arc.write() {
                        player_guard.set_default_team(Some(team_arc.clone()));
                    }
                    if let Ok(mut team_guard) = team_arc.write() {
                        team_guard.set_active();
                    }
                }
            }
        }
    }

    fn sync_side_scripts_to_script_engine() {
        let side_scripts = {
            let sides_list = get_sides_list();
            let Ok(sides_guard) = sides_list.read() else {
                return;
            };

            let mut side_scripts = Vec::with_capacity(sides_guard.get_num_sides());
            for index in 0..sides_guard.get_num_sides() {
                let script_list = sides_guard
                    .get_side_info(index)
                    .and_then(|side| side.get_script_list().cloned())
                    .map(Box::new);
                side_scripts.push(script_list);
            }
            side_scripts
        };

        if let Err(err) = crate::scripting::engine::initialize_script_engine() {
            log::warn!(
                "Failed to initialize script engine while syncing side scripts: {}",
                err
            );
            return;
        }

        let engine_lock = crate::scripting::engine::get_script_engine();
        let Ok(mut engine_guard) = engine_lock.write() else {
            return;
        };
        let Some(engine) = engine_guard.as_mut() else {
            return;
        };

        engine.clear_script_lists();
        for (index, script_list) in side_scripts.into_iter().enumerate() {
            if let Err(err) = engine.set_script_list_for_player(index, script_list) {
                log::warn!(
                    "Failed to register side script list {} into script engine: {}",
                    index,
                    err
                );
            }
        }
    }

    fn initialize_ai_integration_for_players() {
        if initialize_ai_integration().is_err() {
            return;
        }

        let Ok(player_list) = ThePlayerList().read() else {
            return;
        };

        for player_arc in player_list.iter() {
            let Ok(player_guard) = player_arc.read() else {
                continue;
            };
            if player_guard.get_player_type() != LogicPlayerType::Computer {
                continue;
            }
            let player_id = player_guard.get_player_index() as u32;
            let difficulty = player_guard.get_player_difficulty();

            let _ = with_ai_integration_mut(|manager| manager.create_ai_player(player_id));
            let _ = with_ai_integration_mut(|manager| {
                manager.set_ai_player_difficulty(player_id, difficulty)
            });
        }
    }

    /// Phase 3: Execute complete startup sequence
    fn execute_start_sequence(
        game_state: &mut GameState,
        params: &GameInitParams,
    ) -> io::Result<()> {
        println!("[INIT] Phase 3: Executing game start sequence");

        // Prepare startup scripts from map
        let startup_scripts = Self::get_startup_scripts(&game_state.map_loader);

        // Execute full startup sequence
        let result = game_state.start_sequence.execute_full_sequence(
            &game_state.map_loader,
            &game_state.player_list,
            &startup_scripts,
            &params.ai_script,
        );

        match result {
            ScriptResult::Success => {
                println!("[INIT] Startup scripts executed successfully");
            }
            ScriptResult::Failed(reason) => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Startup script failed: {}", reason),
                ));
            }
        }

        let _ = with_ai_integration_mut(|manager| manager.new_map());
        // Configure fog of war
        game_state
            .start_sequence
            .get_fog_of_war_mut()
            .set_enabled(params.fog_of_war_enabled);

        println!("[INIT] Camera positioned");
        println!(
            "[INIT] Fog of war: {}",
            if params.fog_of_war_enabled {
                "enabled"
            } else {
                "disabled"
            }
        );
        println!("[INIT] Minimap generated");
        println!(
            "[INIT] {} AI players started",
            game_state.start_sequence.get_ai_players().len()
        );

        Ok(())
    }

    /// Phase 4: Setup victory conditions
    fn setup_victory_conditions(game_state: &mut GameState, params: &GameInitParams) {
        println!("[INIT] Phase 4: Setting up victory conditions");

        game_state.victory_conditions = VictoryConditions::new(params.victory_type);

        if let Some(score_limit) = params.score_limit {
            game_state.victory_conditions.set_score_limit(score_limit);
            println!("[INIT] Victory condition: Score limit ({})", score_limit);
        }

        if let Some(time_limit) = params.time_limit {
            game_state.victory_conditions.set_time_limit(time_limit);
            println!("[INIT] Victory condition: Time limit ({:?})", time_limit);
        }

        if params.victory_type == VictoryType::Annihilation {
            println!("[INIT] Victory condition: Last player/team standing");
        }
    }

    // Helper functions
    // =====================================================================

    /// Extract player start positions from map waypoints
    fn extract_start_positions(map_loader: &MapLoader, num_players: usize) -> Vec<(f32, f32, f32)> {
        let mut positions = Vec::new();

        for i in 0..num_players {
            let waypoint_name = format!("Player_{}_Start", i + 1);

            if let Some(pos) = map_loader.get_waypoints().get(&waypoint_name) {
                positions.push((pos.x, pos.y, pos.z));
            } else {
                // Fallback to default positions if waypoint missing
                positions.push((100.0 * (i as f32 + 1.0), 100.0 * (i as f32 + 1.0), 0.0));
            }
        }

        positions
    }

    /// Get startup script names from map world dictionary
    fn get_startup_scripts(map_loader: &MapLoader) -> Vec<String> {
        let world_dict = map_loader.get_world_dict();

        let mut scripts = Vec::new();
        let keys = [
            "startupScript",
            "startupScripts",
            "StartupScript",
            "StartupScripts",
        ];

        for key in keys {
            let Some(value) = world_dict.get(key) else {
                continue;
            };
            for token in value.split(|c: char| c == ',' || c == ';' || c.is_whitespace()) {
                let name = token.trim();
                if !name.is_empty() {
                    scripts.push(name.to_string());
                }
            }
        }

        scripts
    }

    /// Quick initialization for testing (minimal setup)
    pub fn quick_init_for_test(map_path: &str) -> io::Result<GameState> {
        let params = GameInitParams {
            map_path: map_path.to_string(),
            game_mode: GameMode::Skirmish,
            difficulty: GameDifficulty::Normal,
            num_players: 2,
            player_templates: vec![
                make_player_template("Player 1", "USA"),
                make_player_template("Player 2", "China"),
            ],
            victory_type: VictoryType::Annihilation,
            fog_of_war_enabled: true,
            ..Default::default()
        };

        Self::initialize_game(params)
    }
}

/// Map cache loader for browsing available maps
pub struct MapCacheManager {
    cache: MapCache,
}

impl MapCacheManager {
    pub fn new() -> Self {
        Self {
            cache: MapCache::new(),
        }
    }

    /// Update cache with available maps
    pub fn update_cache(&mut self) -> io::Result<()> {
        self.cache.update_cache()
    }

    /// Get cache
    pub fn get_cache(&self) -> &MapCache {
        &self.cache
    }

    /// Find map by name
    pub fn find_map(&self, name: &str) -> Option<&super::map_loader::MapMetaData> {
        self.cache.find_map(name)
    }

    /// List all available maps
    pub fn list_maps(&self) -> Vec<String> {
        // Would iterate cache and return list
        Vec::new()
    }
}

impl Default for MapCacheManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_init_params_default() {
        let params = GameInitParams::default();

        assert_eq!(params.game_mode, GameMode::Skirmish);
        assert_eq!(params.difficulty, GameDifficulty::Normal);
        assert_eq!(params.num_players, 2);
        assert_eq!(params.victory_type, VictoryType::Annihilation);
        assert!(params.fog_of_war_enabled);
        assert_eq!(params.starting_resources, 10000);
    }

    #[test]
    fn test_game_state_reset() {
        let mut state = GameState::new();
        state.is_initialized = true;

        state.reset();

        assert!(!state.is_initialized);
        assert!(state.player_list.is_empty());
    }

    #[test]
    fn test_start_new_game_request_is_cleared_before_initialize_game_entry() {
        use crate::system::game_logic::{GameLogic, GAME_SKIRMISH};
        use game_engine::common::ini::get_global_data;
        use std::sync::Mutex;

        static TEST_LOCK: Mutex<()> = Mutex::new(());

        let _guard = TEST_LOCK.lock().unwrap();

        let original_map_name = get_global_data()
            .map(|data| data.read().map_name.clone())
            .unwrap_or_default();

        crate::helpers::TheGameLogic::clear_start_new_game_request();
        crate::helpers::TheGameLogic::request_start_new_game();
        START_NEW_GAME_REQUEST_AT_INIT_ENTRY.with(|slot| slot.set(false));

        if let Some(global) = get_global_data() {
            let mut global = global.write();
            global.map_name = "__definitely_missing_startup_map__.map".to_string();
        }

        let mut logic = GameLogic::new();
        logic.set_game_mode(GAME_SKIRMISH);

        let _ = logic.start_new_game_now(false);

        assert!(
            !START_NEW_GAME_REQUEST_AT_INIT_ENTRY.with(|slot| slot.get()),
            "startup request must be cleared before initialize_game() begins"
        );

        if let Some(global) = get_global_data() {
            let mut global = global.write();
            global.map_name = original_map_name;
        }
        crate::helpers::TheGameLogic::clear_start_new_game_request();
    }

    #[test]
    fn test_difficulty_conversion() {
        assert_eq!(Difficulty::from(GameDifficulty::Easy), Difficulty::Easy);
        assert_eq!(Difficulty::from(GameDifficulty::Normal), Difficulty::Normal);
        assert_eq!(Difficulty::from(GameDifficulty::Hard), Difficulty::Hard);
        assert_eq!(Difficulty::from(GameDifficulty::Brutal), Difficulty::Brutal);
    }
}
