//! Host runtime lifecycle, game mode, and new-game setup.
#![allow(unused_imports, non_snake_case)]
use super::*;

impl GameLogic {
    /// Update method - matching C++ GameLogic interface
    pub fn update(&mut self) {
        if gamelogic::terrain::get_terrain_logic()
            .read()
            .ok()
            .is_some_and(|t| t.bridge_damage_states_changed())
        {
            self.stamp_live_bridge_decks_and_zones();
        }
        // C++ AI::update → Pathfinder::processPathfindQueue (AI.cpp:332-339)
        // runs before movement so a unit waits at least one frame (m_waitingForPath).
        self.process_pathfind_queue();
        self.step_simulation(LOGIC_FRAME_TIMESTEP, None);
    }

    /// C++ interface methods
    pub fn isInGame(&self) -> bool {
        self.game_mode != GameMode::None && self.map_loaded
    }

    pub fn isInShellGame(&self) -> bool {
        self.game_mode == GameMode::Shell
    }

    pub fn isInReplayGame(&self) -> bool {
        self.game_mode == GameMode::Replay
    }

    pub fn isInMultiplayerGame(&self) -> bool {
        self.game_mode == GameMode::Multiplayer
    }

    pub fn isInInternetGame(&self) -> bool {
        self.game_mode == GameMode::Internet
    }

    pub fn isInLanGame(&self) -> bool {
        self.game_mode == GameMode::Lan
    }

    pub fn isInNetworkGame(&self) -> bool {
        self.isInMultiplayerGame() || self.isInInternetGame() || self.isInLanGame()
    }

    pub fn isGamePaused(&self) -> bool {
        self.is_paused
    }

    pub fn clearGameData(&mut self) {
        log::debug!("GameLogic::clearGameData() - clearing all game data");
        // C++ routes this through the broader engine reset path, so keep the
        // fallback state scrubbed rather than only clearing the minimum fields.
        self.reset();
        self.game_mode = GameMode::None;
        self.map_name.clear();
        self.last_map_settings = None;
        self.map_loaded = false;
    }

    pub fn getFrame(&self) -> u32 {
        self.frame
    }

    pub fn last_parsed_map_settings(&self) -> Option<super::script_loader::MapMetadata> {
        self.last_map_settings.clone()
    }

    /// Player_N_Start / Player_N_Rally spots already decoded with map settings.
    /// Avoids a second RefPack decode of the same `.map` during spawn.
    pub(crate) fn cached_player_start_waypoints(
        &self,
    ) -> Option<
        Vec<(
            u32,
            gamelogic::scripting::core::Coord3D,
            Option<gamelogic::scripting::core::Coord3D>,
        )>,
    > {
        let meta = self.last_map_settings.as_ref()?;
        if meta.start_waypoints.is_empty() {
            return None;
        }
        let mut starts: std::collections::HashMap<u32, gamelogic::scripting::core::Coord3D> =
            std::collections::HashMap::new();
        let mut rallies: std::collections::HashMap<u32, gamelogic::scripting::core::Coord3D> =
            std::collections::HashMap::new();
        for (name, pos) in &meta.start_waypoints {
            let lower = name.trim().to_ascii_lowercase();
            let Some(rest) = lower.strip_prefix("player_") else {
                continue;
            };
            let Some((num, kind)) = rest.split_once('_') else {
                continue;
            };
            let Ok(idx1) = num.parse::<u32>() else {
                continue;
            };
            if idx1 < 1 {
                continue;
            }
            let idx0 = idx1 - 1;
            if kind.starts_with("start") {
                starts.insert(idx0, *pos);
            } else if kind.starts_with("rally") {
                rallies.insert(idx0, *pos);
            }
        }
        if starts.is_empty() {
            return None;
        }
        let mut keys: Vec<u32> = starts.keys().copied().collect();
        keys.sort_unstable();
        Some(
            keys.into_iter()
                .map(|k| (k, starts[&k], rallies.get(&k).copied()))
                .collect(),
        )
    }

    /// C++ `findNamedWaypoint` (`GameLogic.cpp:160`) + `getGroundHeight`.
    pub(crate) fn leftover_named_waypoint_host_pos(name: &str) -> Option<Vec3> {
        let wp_name = gamelogic::common::AsciiString::from(name);
        let loc = gamelogic::terrain::get_terrain_logic()
            .read()
            .ok()
            .and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&wp_name)
                    .map(|wp| *wp.get_location())
            })?;
        let mut pos = Vec3::new(loc.x, loc.z, loc.y);
        if let Ok(tl) = gamelogic::terrain::get_terrain_logic().read() {
            pos.y = tl.get_ground_height(pos.x, pos.z, None);
        }
        Some(pos)
    }

    /// C++ `placeNetworkBuildingsForPlayer` `Player_%d_Rally` (1-based start pos).
    /// Leftover TerrainLogic first; parsed map cache if leftover is empty.
    pub(crate) fn player_rally_spawn_pos(&self, start_idx0: u32) -> Option<Vec3> {
        let name = format!("Player_{}_Rally", start_idx0 + 1);
        if let Some(pos) = Self::leftover_named_waypoint_host_pos(&name) {
            return Some(pos);
        }
        let starts = self
            .cached_player_start_waypoints()
            .or_else(|| super::script_loader::parse_player_start_waypoints(&self.map_name).ok())?;
        let wp = starts
            .iter()
            .find(|(idx, _, _)| *idx == start_idx0)
            .and_then(|(_, _, rally)| *rally)?;
        let mut pos = Vec3::new(wp.x, wp.z, wp.y);
        if let Some(h) = self.terrain_height_at(Vec3::new(pos.x, 0.0, pos.z)) {
            pos.y = h;
        }
        Some(pos)
    }

    pub fn is_skybox_enabled(&self) -> bool {
        self.script_skybox_enabled
    }

    /// Convenience accessor for any heightmap path hint parsed from the map.
    pub fn heightmap_hint(&self) -> Option<PathBuf> {
        self.last_map_settings
            .as_ref()
            .and_then(|m| m.heightmap_path.clone())
    }

    /// Return a representative base position for the given team (e.g., command center/structure).
    pub fn team_base_position(&self, team: Team) -> Option<Vec3> {
        // Prefer structures that look like command centers.
        for obj in self.objects.values() {
            if obj.team != team {
                continue;
            }
            if obj.is_kind_of(KindOf::Structure)
                && obj.name.to_ascii_lowercase().contains("commandcenter")
            {
                return Some(obj.get_position());
            }
        }
        // Fallback to any structure.
        for obj in self.objects.values() {
            if obj.team == team && obj.is_kind_of(KindOf::Structure) {
                return Some(obj.get_position());
            }
        }
        // Finally, any object owned by the team.
        self.objects
            .values()
            .find(|o| o.team == team)
            .map(|o| o.get_position())
    }

    /// Resolve a base from the controlling player first. A faction fallback is
    /// valid only if the faction has a single active player; otherwise USA-vs-
    /// USA would incorrectly share whichever base happens to be visited first.
    pub fn player_base_position(&self, player_id: u32) -> Option<Vec3> {
        let player = self.players.get(&player_id)?;
        for obj in self.objects.values() {
            if obj.owner_player_id != Some(player_id) {
                continue;
            }
            if obj.is_kind_of(KindOf::Structure)
                && obj.name.to_ascii_lowercase().contains("commandcenter")
            {
                return Some(obj.get_position());
            }
        }
        for obj in self.objects.values() {
            if obj.owner_player_id == Some(player_id) && obj.is_kind_of(KindOf::Structure) {
                return Some(obj.get_position());
            }
        }
        if let Some(position) = self
            .objects
            .values()
            .find(|obj| obj.owner_player_id == Some(player_id))
            .map(|obj| obj.get_position())
        {
            return Some(position);
        }
        (self.unique_player_id_for_team(player.team) == Some(player_id))
            .then(|| self.team_base_position(player.team))
            .flatten()
    }

    /// Initialize the GameLogic singleton
    pub fn initialize() -> GameLogic {
        // For the engine, return a new instance as requested by the original code
        GameLogic::new()
    }

    /// Get reference to the GameLogic singleton
    pub fn instance() -> Arc<Mutex<GameLogic>> {
        GAME_LOGIC
            .get_or_init(|| Arc::new(Mutex::new(GameLogic::new())))
            .clone()
    }

    /// Initialize the global GameLogic singleton
    pub fn init_global() {
        let _ = GAME_LOGIC.get_or_init(|| Arc::new(Mutex::new(GameLogic::new())));
    }

    /// Start a new game with specified mode
    pub fn start_new_game(&mut self, mode: GameMode) {
        log::info!("Starting new game: {:?}", mode);
        // C++ GameLogic.cpp:1254-1256 TheCampaignManager->SetVictorious(FALSE).
        clear_live_campaign_victorious_for_new_game();
        self.reset();
        // C++ TheAI already holds AIData.ini EnableRepulsors after GameEngine init.
        // Live GameLogic ctor stays false (TAiData default); apply on the player path.
        self.apply_aidata_enable_repulsors();
        self.game_mode = mode;
        // C++ GameLogic.cpp:1606 TheVictoryConditions->setVictoryConditions(VICTORY_NOBUILDINGS)
        if matches!(mode, GameMode::Skirmish) {
            self.victory_conditions
                .set_victory_conditions(VictoryType::NO_BUILDINGS);
        }
        // Host combat/movement: ensure WeaponStore + LocomotorStore before units resolve.
        let seeded = super::weapon_bootstrap::ensure_host_weapon_store();
        if seeded > 0 {
            log::info!("Host WeaponStore bootstrap registered {} templates", seeded);
        }
        let loco_seeded = super::locomotor_bootstrap::ensure_host_locomotor_store();
        if loco_seeded > 0 {
            log::info!(
                "Host LocomotorStore bootstrap registered {} templates",
                loco_seeded
            );
        }
        self.setup_templates();
        let asset_template_count = self.seed_asset_definition_templates();
        if asset_template_count > 0 {
            log::info!(
                "Seeded {asset_template_count} missing templates from resolved retail Object INI data"
            );
        }
        let leftover_object_overrides = self.apply_all_leftover_object_create_overrides();
        if leftover_object_overrides > 0 {
            log::info!(
                "Applied {leftover_object_overrides} leftover map.ini Object CREATE_OVERRIDES to live catalog"
            );
        }
        self.create_default_players();
        if matches!(mode, GameMode::Skirmish | GameMode::Replay) {
            let _ = self.ensure_replay_observer_player();
            self.install_replay_observer_side();
            if matches!(mode, GameMode::Skirmish) {
                self.set_install_multiplayer_scripts(true);
            }
        }
        if matches!(mode, GameMode::Replay) {
            self.apply_replay_observer_as_local_player();
            // C++ GameLogic.cpp:2340-2343 startNewGame replay hint.
            #[cfg(feature = "game_client")]
            {
                game_client::helpers::TheInGameUI::message("GUI:FastForwardInstructions");
            }
        }
        log::info!("New game started successfully");
        crate::command_system::tap_host_new_game_for_recorder(mode);
    }

    pub fn game_mode(&self) -> GameMode {
        self.game_mode
    }

    /// Wave 831: spawn SidesList build-list entries (initiallyBuilt faction bases).

    pub(in super::super) fn spawn_side_build_list(
        &mut self,
        _builds: &[super::script_loader::SideBuildEntry],
        _map_player_to_team: &std::collections::HashMap<u32, Team>,
    ) -> u32 {
        // C++ never instantiates SidesList BuildListInfo at map load.
        // initiallyBuilt entries are already placed via ObjectsList; the list
        // is transferred onto Player for AI rebuild (see sync + take_build_list).
        0
    }

    pub(in super::super) fn team_from_string(name: &str) -> Option<Team> {
        let normalized = name.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "usa" | "us" | "america" => Some(Team::USA),
            "gla" => Some(Team::GLA),
            "china" => Some(Team::China),
            "neutral" => Some(Team::Neutral),
            _ if normalized.contains("usa") || normalized.contains("america") => Some(Team::USA),
            _ if normalized.contains("gla") => Some(Team::GLA),
            _ if normalized.contains("china") => Some(Team::China),
            _ if normalized.contains("neutral") || normalized.contains("civilian") => {
                Some(Team::Neutral)
            }
            _ => None,
        }
    }

    /// Wave 830: faction from ThingTemplate name when map team_name is empty.
    pub(in super::super) fn team_from_template_name(template: &str) -> Option<Team> {
        let n = template.trim().to_ascii_lowercase();
        if n.is_empty() {
            return None;
        }
        // Civilian / tech / natural props stay Neutral.
        if n.contains("civilian")
            || n.contains("tree")
            || n.contains("shrub")
            || n.contains("rock")
            || n.contains("bush")
            || n.contains("fence")
            || n.contains("street")
            || n.contains("sign")
            || n.starts_with("p_")
            || n.contains("prop")
        {
            return Some(Team::Neutral);
        }
        if n.contains("america") || n.starts_with("usa") || n.contains("usa_") {
            return Some(Team::USA);
        }
        if n.contains("gla") || n.starts_with("gl") && n.contains("worker") {
            return Some(Team::GLA);
        }
        // GLA unit names without prefix
        if n.contains("rebel")
            || n.contains("terrorist")
            || n.contains("hijacker")
            || n.contains("rpg")
            || n.contains("scud")
            || n.contains("quadcannon")
            || n.contains("technical")
            || n.contains("marauder")
            || n.contains("scorpion")
            || n.contains("tunnel")
            || n.contains("armsdealer")
            || n.contains("blackmarket")
            || n.contains("palace")
            || n.contains("stinger")
            || n.contains("demotrap")
            || n.contains("angrymob")
            || n.contains("jarmen")
            || n.contains("worker")
        {
            return Some(Team::GLA);
        }
        if n.contains("china") || n.starts_with("ch_") {
            return Some(Team::China);
        }
        if n.contains("redguard")
            || n.contains("battlemaster")
            || n.contains("gatling")
            || n.contains("inferno")
            || n.contains("nuke")
            || n.contains("nuclear")
            || n.contains("mig")
            || n.contains("dragon")
            || n.contains("troopcrawler")
            || n.contains("hacker")
            || n.contains("tankhunter")
        {
            return Some(Team::China);
        }
        None
    }
}
