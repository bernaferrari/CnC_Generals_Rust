//! Host runtime lifecycle, game mode, and new-game setup.
#![allow(unused_imports, non_snake_case)]
use super::*;

#[derive(Clone, Copy)]
enum NewGamePlayerAdmission {
    Bootstrap,
    Resolved,
}

impl GameLogic {
    /// Update method - matching C++ GameLogic interface
    pub fn update(&mut self) {
        self.refresh_owned_bridge_pathfinder_states();
        // C++ AI::update → Pathfinder::processPathfindQueue drains inside
        // update_simulation (every logic frame, live and test paths).
        self.step_simulation(LOGIC_FRAME_TIMESTEP, None);
    }

    /// C++ interface methods
    pub fn isInGame(&self) -> bool {
        self.game_mode != GameMode::None && self.map_loaded
    }

    pub fn isInShellGame(&self) -> bool {
        self.game_mode == GameMode::Shell
    }

    /// Replay keeps its own mode; original map-vs-lobby preparation observes
    /// actual receiving GameInfo, not a process Recorder or a mode-only guess.
    pub(crate) fn has_recorded_lobby_sides(&self) -> bool {
        self.replay_pending.recorder_owned().is_playback()
            && self
                .replay_pending
                .recorder_owned()
                .get_game_info()
                .slots
                .iter()
                .any(|slot| slot.is_occupied)
    }

    pub(crate) fn uses_lobby_side_foundation(&self) -> bool {
        matches!(
            self.game_mode,
            GameMode::Skirmish | GameMode::Multiplayer | GameMode::Lan | GameMode::Internet
        ) || (self.game_mode == GameMode::Replay
            && (self.has_recorded_lobby_sides()
                || self
                    .initial_skirmish_admission
                    .as_ref()
                    .is_some_and(|initial| initial.config.slots.iter().any(|slot| slot.is_active))))
    }

    pub(crate) fn is_single_player_scenario(&self) -> bool {
        self.game_mode == GameMode::SinglePlayer
            || (self.replay_pending.recorder_owned().is_playback()
                && self.replay_pending.recorder_owned().get_game_mode() == 0)
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
        self.finish_clear_game_data_after_reset();
    }

    /// The native engine already performed its ordered subsystem/world reset.
    /// Finish C++ clearGameData without performing a second reset.
    pub(crate) fn finish_clear_game_data_after_reset(&mut self) {
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
    pub(crate) fn leftover_named_waypoint_host_pos(&self, name: &str) -> Option<Vec3> {
        let wp_name = gamelogic::common::AsciiString::from(name);
        let terrain_owner_handle = self.world_services.terrain();
        let terrain = terrain_owner_handle.read().ok()?;
        let loc = *terrain.get_waypoint_by_name(&wp_name)?.get_location();
        let pos = Vec3::new(loc.x, terrain.get_ground_height(loc.x, loc.y, None), loc.y);
        Some(pos)
    }

    /// C++ `placeNetworkBuildingsForPlayer` `Player_%d_Rally` (1-based start pos).
    /// Driving terrain waypoints first; parsed map cache if this map has none.
    pub(crate) fn player_rally_spawn_pos(&self, start_idx0: u32) -> Option<Vec3> {
        let name = format!("Player_{}_Rally", start_idx0 + 1);
        if let Some(pos) = self.leftover_named_waypoint_host_pos(&name) {
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
        for obj in self.objects.registered_values() {
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
        for obj in self.objects.registered_values() {
            if obj.team == team && obj.is_kind_of(KindOf::Structure) {
                return Some(obj.get_position());
            }
        }
        // Finally, any object owned by the team.
        self.objects
            .registered_values()
            .find(|o| o.team == team)
            .map(|o| o.get_position())
    }

    /// Resolve a base from the controlling player first. A faction fallback is
    /// valid only if the faction has a single active player; otherwise USA-vs-
    /// USA would incorrectly share whichever base happens to be visited first.
    pub fn player_base_position(&self, player_id: u32) -> Option<Vec3> {
        let player = self.players.get(&player_id)?;
        for obj in self.objects.registered_values() {
            if obj.owner_player_id != Some(player_id) {
                continue;
            }
            if obj.is_kind_of(KindOf::Structure)
                && obj.name.to_ascii_lowercase().contains("commandcenter")
            {
                return Some(obj.get_position());
            }
        }
        for obj in self.objects.registered_values() {
            if obj.owner_player_id == Some(player_id) && obj.is_kind_of(KindOf::Structure) {
                return Some(obj.get_position());
            }
        }
        if let Some(position) = self
            .objects
            .registered_values()
            .find(|obj| obj.owner_player_id == Some(player_id))
            .map(|obj| obj.get_position())
        {
            return Some(position);
        }
        (self.unique_player_id_for_team(player.team) == Some(player_id))
            .then(|| self.team_base_position(player.team))
            .flatten()
    }

    /// Initialize a GameLogic world for engine use.
    pub fn initialize() -> GameLogic {
        // A candidate must not select or mutate any other session's services.
        GameLogic::new()
    }

    /// Start a new game with specified mode
    pub fn start_new_game(&mut self, mode: GameMode) {
        self.start_new_game_state(mode, NewGamePlayerAdmission::Bootstrap);
    }

    /// Receiving native NewGame route; the actual dispatcher supplies its
    /// difficulty/rank/FPS payload. Playback already consumed that lifecycle
    /// message and must retain its admitted reader, not synthesize it again.
    pub(crate) fn start_new_game_with_recorder_payload(
        &mut self,
        mode: GameMode,
        difficulty: i32,
        rank: i32,
        max_fps: i32,
    ) {
        self.start_new_game_state(mode, NewGamePlayerAdmission::Bootstrap);
        self.set_rank_points_to_add_at_game_start(rank);
        if !self.replay_pending.recorder_owned().is_playback() {
            self.replay_pending.record_new_game_owned(
                mode,
                self.logic_base_seed,
                difficulty,
                rank,
                max_fps,
            );
        }
    }

    /// Prepare a candidate world for loading a save. C++ GameState::loadGame
    /// resets the engine and does not post MSG_NEW_GAME for ordinary saves;
    /// the candidate must not start a second replay on the live recorder.
    pub(crate) fn start_new_game_for_restore(&mut self, mode: GameMode) {
        self.start_new_game_state(mode, NewGamePlayerAdmission::Resolved);
    }

    /// GameInfo admission supplies the actual participants immediately after
    /// this source reset. Never create temporary bootstrap Players/observer.
    pub(crate) fn start_new_game_for_resolved_players(&mut self, mode: GameMode) {
        self.start_new_game_state(mode, NewGamePlayerAdmission::Resolved);
    }

    pub(crate) fn start_new_game_in_campaign_session(
        &mut self,
        mode: GameMode,
        is_campaign: bool,
        is_challenge: bool,
    ) {
        self.start_new_game_state_with_campaign(
            mode,
            NewGamePlayerAdmission::Bootstrap,
            is_campaign,
            is_challenge,
        );
    }

    pub(crate) fn start_new_game_for_restore_in_campaign_session(
        &mut self,
        mode: GameMode,
        is_campaign: bool,
        is_challenge: bool,
    ) {
        self.start_new_game_state_with_campaign(
            mode,
            NewGamePlayerAdmission::Resolved,
            is_campaign,
            is_challenge,
        );
    }

    fn start_new_game_state(&mut self, mode: GameMode, admission: NewGamePlayerAdmission) {
        self.start_new_game_state_with_campaign(mode, admission, false, false);
    }

    fn start_new_game_state_with_campaign(
        &mut self,
        mode: GameMode,
        admission: NewGamePlayerAdmission,
        is_campaign: bool,
        is_challenge: bool,
    ) {
        log::info!("Starting new game: {:?}", mode);
        // C++ GameLogic.cpp:1254-1256 TheCampaignManager->SetVictorious(FALSE).
        clear_live_campaign_victorious_for_new_game(self);
        self.reset();
        // Source metadata is admitted after reset and before template/player/onCreate work.
        self.admit_campaign_session(is_campaign, is_challenge);
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
        if self.locomotor_definitions.is_none() {
            let loco_seeded = super::locomotor_bootstrap::ensure_host_locomotor_store();
            if loco_seeded > 0 {
                log::info!(
                    "Host LocomotorStore bootstrap registered {} templates",
                    loco_seeded
                );
            }
        }
        if self.receiving_module_content.is_some() {
            let Some(baseline) = &self.receiving_template_baseline else {
                log::error!("Receiving NewGame has no accepted host template baseline");
                return;
            };
            self.templates = baseline.clone();
            self.refresh_receiving_client_templates();
        } else {
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
        }
        if matches!(admission, NewGamePlayerAdmission::Bootstrap) {
            self.create_default_players();
            if matches!(mode, GameMode::Skirmish | GameMode::Replay) {
                let _ = self.ensure_replay_observer_player();
                self.install_replay_observer_side();
                if matches!(mode, GameMode::Skirmish) {
                    self.set_install_multiplayer_scripts(true);
                }
            }
        }
        if matches!(mode, GameMode::Replay) {
            if matches!(admission, NewGamePlayerAdmission::Bootstrap) {
                self.apply_replay_observer_as_local_player();
            }
            // C++ GameLogic.cpp:2340-2343 startNewGame replay hint.
            #[cfg(feature = "game_client")]
            {
                self.push_script_ui_message("GUI:FastForwardInstructions");
            }
        }
        log::info!("New game started successfully");
    }

    /// Whether this world was admitted from a Challenge campaign descriptor.
    pub fn is_challenge_session(&self) -> bool {
        self.campaign_session_kind == super::host::CampaignSessionKind::Challenge
    }

    pub fn campaign_session_kind(&self) -> super::host::CampaignSessionKind {
        self.campaign_session_kind
    }

    pub fn has_campaign_session(&self) -> bool {
        self.campaign_session_kind != super::host::CampaignSessionKind::None
    }

    /// Install Challenge classification from the validated launch or restore
    /// payload before new-map relationship setup observes the session.
    pub(crate) fn admit_challenge_session(&mut self, is_challenge: bool) {
        self.host_trigger_world
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .admit_campaign_context(is_challenge);
        self.campaign_session_kind = if is_challenge {
            super::host::CampaignSessionKind::Challenge
        } else {
            super::host::CampaignSessionKind::None
        };
    }

    pub(crate) fn admit_campaign_session(&mut self, is_campaign: bool, is_challenge: bool) {
        self.host_trigger_world
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .admit_campaign_context(is_challenge);
        self.campaign_session_kind = if is_challenge {
            super::host::CampaignSessionKind::Challenge
        } else if is_campaign {
            super::host::CampaignSessionKind::Campaign
        } else {
            super::host::CampaignSessionKind::None
        };
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

#[cfg(test)]
mod clear_finalization_tests {
    use super::*;

    #[test]
    fn clear_finalization_changes_only_receiving_lifecycle_without_second_reset() {
        let mut receiving = GameLogic::new();
        let mut foreign = GameLogic::new();
        receiving.game_mode = GameMode::Replay;
        receiving.map_name = "receiving.map".into();
        receiving.map_loaded = true;
        receiving.frame = 41;
        foreign.game_mode = GameMode::Skirmish;
        foreign.map_name = "foreign.map".into();
        foreign.map_loaded = true;
        foreign.frame = 93;
        receiving.finish_clear_game_data_after_reset();
        assert_eq!(receiving.game_mode(), GameMode::None);
        assert!(receiving.map_name.is_empty());
        assert!(!receiving.map_loaded);
        assert_eq!(receiving.frame, 41, "finalization must not reset twice");
        assert_eq!(foreign.game_mode(), GameMode::Skirmish);
        assert_eq!(foreign.map_name, "foreign.map");
        assert!(foreign.map_loaded);
        assert_eq!(foreign.frame, 93);
    }
}
