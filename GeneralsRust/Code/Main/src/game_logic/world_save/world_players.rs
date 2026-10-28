//! Host player, team, and authored build-list state.
#![allow(unused_imports, non_snake_case)]
use super::*;

// Normal skirmish admission retains the exact typed GameSlot name. Direct
// host construction/restored players still use Main's documented slot-ID
// contract; that compatibility route does not establish prepared side ordinals.
fn host_slot_script_name(player: &Player) -> String {
    if player.map_side.map_player_name.is_empty() {
        format!("player{}", player.id)
    } else {
        player.map_side.map_player_name.clone()
    }
}

impl GameLogic {
    pub(in super::super) fn sync_legacy_runtime_from_chunky(
        &mut self,
        map_path: &Path,
        map_bytes: &[u8],
    ) {
        match super::script_loader::ChunkyMap::decode(map_path.to_path_buf(), map_bytes.to_vec()) {
            Ok(chunky) => {
                self.sync_legacy_runtime_from_fast_chunky(map_path, &chunky);
            }
            Err(error) => {
                log::error!(
                    "Map foundation decode failed for '{}': {}",
                    map_path.display(),
                    error
                );
            }
        }
    }

    pub(crate) fn sync_legacy_runtime_from_fast_chunky(
        &mut self,
        map_path: &Path,
        chunky: &super::script_loader::ChunkyMap,
    ) {
        self.sync_legacy_runtime_from_fast_chunky_with_progress(map_path, chunky, |_, _| {});
    }

    pub(crate) fn sync_legacy_runtime_from_fast_chunky_with_progress<F>(
        &mut self,
        map_path: &Path,
        chunky: &super::script_loader::ChunkyMap,
        report_progress: F,
    ) -> bool
    where
        F: FnMut(f32, &str),
    {
        let difficulty = self
            .owned_script_engine
            .as_ref()
            .map(|engine| engine.get_global_difficulty());
        self.sync_legacy_runtime_from_fast_chunky_with_script_difficulty(
            map_path,
            chunky,
            difficulty,
            report_progress,
        )
    }

    pub(in super::super) fn sync_legacy_runtime_from_fast_chunky_with_script_difficulty<F>(
        &mut self,
        map_path: &Path,
        chunky: &super::script_loader::ChunkyMap,
        difficulty: Option<gamelogic::player::GameDifficulty>,
        mut report_progress: F,
    ) -> bool
    where
        F: FnMut(f32, &str),
    {
        self.sync_legacy_runtime_from_fast_chunky_with_water_delivery(
            map_path,
            chunky,
            difficulty,
            None,
            None,
            None,
            |_, _audio_policy, value, phase| {
                report_progress(value, phase);
                Ok(())
            },
        )
        .unwrap_or_else(|error| {
            log::error!("Map foundation progress failed: {error}");
            false
        })
    }

    pub(in super::super) fn sync_legacy_runtime_from_fast_chunky_with_water_delivery<F>(
        &mut self,
        map_path: &Path,
        chunky: &super::script_loader::ChunkyMap,
        difficulty: Option<gamelogic::player::GameDifficulty>,
        water_delivery: Option<&mut dyn FnMut(&gamelogic::terrain_water::WaterGridState)>,
        mut native: Option<&mut dyn crate::game_logic::NativeMapAdmission>,
        metadata: Option<&super::script_loader::MapMetadata>,
        mut report_progress: F,
    ) -> Result<bool, String>
    where
        F: FnMut(
            &mut game_engine::common::random_value::RandomState,
            &game_engine::common::audio::script_policy::ScriptAudioPolicy,
            f32,
            &str,
        ) -> Result<(), String>,
    {
        let sync_started = Instant::now();
        report_progress(
            &mut self.session_view.audio_random,
            self.queued_audio_events.policy(),
            0.41,
            "Fast sync heightmap",
        )?;
        log::info!(
            "Fast legacy runtime sync started for '{}'",
            map_path.display()
        );
        let heightmap = match super::script_loader::parse_heightmap_data_from_chunky(chunky) {
            Ok(value) => value,
            Err(err) => {
                log::warn!(
                    "Fast legacy runtime sync heightmap parse failed for '{}': {}",
                    map_path.display(),
                    err
                );
                None
            }
        };
        log::info!(
            "Fast legacy runtime sync heightmap parsed for '{}' in {:.2}s",
            map_path.display(),
            sync_started.elapsed().as_secs_f32()
        );
        report_progress(
            &mut self.session_view.audio_random,
            self.queued_audio_events.policy(),
            0.415,
            "Fast sync waypoints",
        )?;

        let (waypoints, waypoint_links) =
            match super::script_loader::parse_runtime_waypoints_from_chunky(chunky) {
                Ok(value) => value,
                Err(err) => {
                    log::warn!(
                        "Fast legacy runtime sync waypoint parse failed for '{}': {}",
                        map_path.display(),
                        err
                    );
                    (Vec::new(), Vec::new())
                }
            };
        log::info!(
            "Fast legacy runtime sync waypoints parsed for '{}' (count={}, links={}) in {:.2}s",
            map_path.display(),
            waypoints.len(),
            waypoint_links.len(),
            sync_started.elapsed().as_secs_f32()
        );
        report_progress(
            &mut self.session_view.audio_random,
            self.queued_audio_events.policy(),
            0.42,
            "Fast sync bridges",
        )?;
        let bridges = match super::script_loader::parse_runtime_bridges_from_chunky(
            chunky,
            &self.templates,
        ) {
            Ok(value) => value,
            Err(err) => {
                log::warn!(
                    "Fast legacy runtime sync bridge parse failed for '{}': {}",
                    map_path.display(),
                    err
                );
                Vec::new()
            }
        };
        report_progress(
            &mut self.session_view.audio_random,
            self.queued_audio_events.policy(),
            0.425,
            "Fast sync water",
        )?;
        let water_height =
            match super::script_loader::parse_runtime_water_height_from_chunky(chunky) {
                Ok(value) => value,
                Err(err) => {
                    log::warn!(
                        "Fast legacy runtime sync water-height parse failed for '{}': {}",
                        map_path.display(),
                        err
                    );
                    None
                }
            };
        let map_weather = match super::script_loader::parse_runtime_weather_from_chunky(chunky) {
            Ok(value) => value,
            Err(err) => {
                log::warn!(
                    "Fast legacy runtime sync WorldInfo weather parse failed for '{}': {}",
                    map_path.display(),
                    err
                );
                None
            }
        };
        self.apply_world_info_weather(map_weather);

        report_progress(
            &mut self.session_view.audio_random,
            self.queued_audio_events.policy(),
            0.43,
            "Fast sync polygons",
        )?;
        let polygon_triggers =
            match super::script_loader::parse_runtime_polygon_triggers_from_chunky_with_lighting_rules(
                chunky,
                self.lighting_rules_baseline(),
            ) {
                Ok(value) => value,
                Err(err) => {
                    log::warn!(
                        "Fast legacy runtime sync polygon-trigger parse failed for '{}': {}",
                        map_path.display(),
                        err
                    );
                    Vec::new()
                }
            };
        // Trigger membership uses each GameLogic world's authored geometry,
        // so later map loads cannot change another world's enter/exit edges.
        self.host_trigger_world
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .set_trigger_areas(&polygon_triggers);
        // Freeze the same authored polygons that C++ WaterRenderObjClass::renderWater
        // traverses. Rendering must not depend on a later read of process-global
        // TerrainLogic, which can be a different instance or unavailable.
        self.runtime_water_triggers = polygon_triggers
            .iter()
            .filter(|trigger| trigger.is_water_area() && trigger.get_num_points() > 2)
            .cloned()
            .collect();
        {
            let terrain_owner = self.world_services.terrain();
            let mut terrain = terrain_owner
                .write()
                .unwrap_or_else(|error| error.into_inner());
            super::script_loader::install_runtime_polygon_triggers(&mut terrain, &polygon_triggers);
        } // Release before progress can dispatch authored load-screen callbacks.

        report_progress(
            &mut self.session_view.audio_random,
            self.queued_audio_events.policy(),
            0.435,
            "Fast sync roads",
        )?;
        self.runtime_road_segments = match super::script_loader::parse_runtime_roads_from_chunky(
            chunky,
            self.effective_terrain_roads(),
        ) {
            Ok(value) => value,
            Err(err) => {
                log::warn!(
                    "Fast legacy runtime sync road parse failed for '{}': {}",
                    map_path.display(),
                    err
                );
                Vec::new()
            }
        };
        self.runtime_terrain_texture_classes.clear();
        report_progress(
            &mut self.session_view.audio_random,
            self.queued_audio_events.policy(),
            0.44,
            "Fast sync sides",
        )?;
        let sides_data = match super::script_loader::parse_runtime_sides_from_chunky(chunky) {
            Ok(value) => value,
            Err(err) => {
                log::warn!(
                    "Fast legacy runtime sync sides parse failed for '{}': {}",
                    map_path.display(),
                    err
                );
                super::script_loader::RuntimeSidesData::default()
            }
        };
        log::info!(
            "Fast legacy runtime sync sides parsed for '{}' (sides={}, teams={}) in {:.2}s",
            map_path.display(),
            sides_data.side_dicts.len(),
            sides_data.team_dicts.len(),
            sync_started.elapsed().as_secs_f32()
        );
        report_progress(
            &mut self.session_view.audio_random,
            self.queued_audio_events.policy(),
            0.445,
            "Fast sync terrain write",
        )?;

        let has_terrain_payload = heightmap.is_some()
            || water_height.is_some()
            || !bridges.is_empty()
            || !polygon_triggers.is_empty()
            || !waypoints.is_empty();
        if has_terrain_payload {
            let mut map_data = gamelogic::system::map_loader::MapData::new();
            map_data.water_height = water_height;
            map_data.bridges = bridges;
            map_data.polygon_triggers = polygon_triggers;
            map_data.waypoints = waypoints
                .iter()
                .map(|waypoint| gamelogic::system::map_loader::MapWaypoint {
                    id: waypoint.id,
                    name: waypoint.name.clone(),
                    location: gamelogic::system::map_loader::Coord3D::new(
                        waypoint.location.x,
                        waypoint.location.y,
                        waypoint.location.z,
                    ),
                    path_label1: waypoint.path_label1.clone(),
                    path_label2: waypoint.path_label2.clone(),
                    path_label3: waypoint.path_label3.clone(),
                    bi_directional: waypoint.bi_directional,
                })
                .collect();
            map_data.waypoint_links = waypoint_links;
            if let Some(heightmap) = heightmap {
                map_data.width = heightmap.width.max(0) as u32;
                map_data.height = heightmap.height.max(0) as u32;
                map_data.heightmap = heightmap.data;
                map_data.border_size = heightmap.border_size;
                map_data.boundaries = heightmap
                    .boundaries
                    .into_iter()
                    .map(|(x, y)| gamelogic::common::ICoord2D::new(x, y))
                    .collect();
            }

            let terrain_owner = self.world_services.terrain().clone();
            {
                let mut terrain = terrain_owner.write().unwrap_or_else(|e| e.into_inner());
                if let Err(error) = self.admit_map_bridge_geometry(&mut terrain, map_data) {
                    log::error!("Cannot admit receiving terrain: {error}");
                    return Ok(false);
                }
                // C++ GameLogic.cpp:1629 TheTerrainLogic->newMap after load.
                // The effective definitions are borrowed only during local
                // configuration; no GlobalData loan crosses native delivery.
                let definitions =
                    game_engine::common::ini::get_global_data().map(|global| global.read().clone());
                let source = self
                    .map_definition_source
                    .as_deref()
                    .unwrap_or_else(|| map_path.to_str().unwrap_or(""));
                terrain.new_map_with_water_configuration(
                    false,
                    source,
                    definitions.as_ref(),
                    water_delivery,
                );
            }
            self.copy_crate_water_into_host_terrain();
        } else {
            log::info!(
                "Fast legacy runtime sync skipped terrain write for '{}' (no payload) in {:.2}s",
                map_path.display(),
                sync_started.elapsed().as_secs_f32()
            );
        }
        if let Some(native) = native.as_mut() {
            if let Err(error) = native.prepare(self, metadata) {
                log::error!("Receiving native terrain admission failed: {error}");
                return Ok(false);
            }
        }
        report_progress(
            &mut self.session_view.audio_random,
            self.queued_audio_events.policy(),
            0.45,
            "Fast sync sides write",
        )?;

        // The fast parser owns its decoded side/team dictionaries instead of
        // going through LogicMapLoader's global SidesList callback. Retain the
        // prepared value on this world before deriving players and teams;
        // observer and script setup consume these same ordered map rows.
        if !self.admit_map_script_definitions_with_difficulty(
            &map_path.to_string_lossy(),
            Some(chunky),
            difficulty,
        ) {
            return Ok(false);
        }
        let script_lists = self.loaded_script_lists.clone();
        let prepared_team_definitions = self.sync_legacy_sides_list_from_dicts(
            &sides_data.side_dicts,
            &sides_data.team_dicts,
            &sides_data.side_builds,
            &script_lists,
        );
        log::info!(
            "Fast legacy runtime sync sides write finished for '{}' in {:.2}s",
            map_path.display(),
            sync_started.elapsed().as_secs_f32()
        );
        if !self.uses_lobby_side_foundation() {
            if let Err(error) = self.admit_direct_map_players_from_prepared_sides() {
                log::error!("Receiving authored Player admission failed: {error}");
                return Ok(false);
            }
        }
        self.sync_legacy_player_list_from_sides();
        self.transfer_side_build_lists_to_players();
        let mut effects = native
            .as_mut()
            .map(|native| native.effects())
            .unwrap_or_default();
        if !self.sync_legacy_team_factory_from_team_dicts_with_effects(
            &prepared_team_definitions,
            &mut effects,
        ) {
            return Ok(false);
        }
        if !self.uses_lobby_side_foundation() {
            let mut ids = self.players.keys().copied().collect::<Vec<_>>();
            ids.sort_unstable();
            for id in ids {
                self.ensure_host_default_team(id);
            }
        }
        if !self.finish_map_script_foundation() {
            return Ok(false);
        }

        let waypoint_count = self
            .world_services
            .terrain()
            .clone()
            .try_read()
            .ok()
            .map(|terrain| {
                let mut count = 0usize;
                let mut current = terrain.get_first_waypoint();
                while let Some(waypoint) = current {
                    count += 1;
                    current = waypoint.get_next();
                }
                count
            })
            .unwrap_or(0);
        let team_count = self
            .team_factory
            .try_lock()
            .map(|factory| factory.get_all_teams().len())
            .unwrap_or(0);

        log::info!(
            "Fast legacy runtime sync complete for '{}': waypoints={}, live_teams={}, elapsed={:.2}s",
            map_path.display(),
            waypoint_count,
            team_count,
            sync_started.elapsed().as_secs_f32()
        );
        Ok(true)
    }

    // Temporary compatibility publication while actual native ScoreScreen,
    // honors, science/rank and observer readers still consume Core PlayerList.
    // Receiving Main Player remains authoritative; remove only with those readers.
    pub(in super::super) fn sync_legacy_player_list_from_side_dicts(&self, side_dicts: &[Dict]) {
        let mut logic_list = LogicPlayerList::new();

        for (index, dict) in side_dicts.iter().enumerate() {
            let player_name = dict.get_ascii_string(key_player_name());
            let faction = dict.get_ascii_string(key_player_faction());
            let display_name = dict.get_unicode_string(key_player_display_name());
            let host_id = self
                .host_player_id_for_side_index(index as u32)
                .unwrap_or(index as u32);
            let admitted = self.players.get(&host_id);
            let is_human = admitted.map_or_else(
                || dict.get_bool(key_player_is_human()),
                |player| player.is_human,
            );

            // The temporary Core adapter gets facts from the receiving world.
            // Retain the old nonblocking lookup only for unconverted callers.
            let template_from_store = match &self.player_template_definitions {
                Some(store) => store
                    .find_template(&faction)
                    .map(LogicPlayerTemplate::from_common),
                None => try_get_player_template_store().and_then(|store| {
                    store
                        .find_template(&faction)
                        .map(LogicPlayerTemplate::from_common)
                }),
            };
            let template = template_from_store.unwrap_or_else(|| {
                let mut template = LogicPlayerTemplate::new(player_name.clone());
                template.side = faction.clone();
                template.base_side = faction.clone();
                template.display_name = if display_name.is_empty() {
                    player_name.clone()
                } else {
                    display_name.clone()
                };
                template
            });

            let mut player = LogicPlayer::new(host_id as i32);
            if !player_name.is_empty() {
                player.set_player_name_key(NameKeyGenerator::name_to_key(&player_name));
            }
            player.set_display_name(if display_name.is_empty() {
                if player_name.is_empty() {
                    "Neutral".to_string()
                } else {
                    player_name.clone()
                }
            } else {
                display_name
            });
            player.set_side(&faction);
            player.set_base_side(faction);
            player.set_difficulty(LogicGameDifficulty::Normal);

            let player_type = if player_name.is_empty() {
                LogicPlayerType::Neutral
            } else if is_human {
                LogicPlayerType::Human
            } else {
                LogicPlayerType::Computer
            };
            player.set_player_type(
                player_type,
                admitted.is_some_and(|player| player.map_side.is_skirmish_ai),
            );
            player.init(Arc::new(template));
            player.init_from_dict_defaults();
            player.apply_handicap_from_dict(dict);
            // C++ Player.cpp:859 m_mpStartIndex = d->getInt(TheKey_multiplayerStartIndex).
            // Host leftover PlayerList must carry this so SKIRMISH_TECH_BUILDING
            // [Skirmish]MyInnerPerimeter qualifies to InnerPerimeter{start+1}.
            if dict.get_type(key_multiplayer_start_index()).is_some() {
                player.set_mp_start_index(dict.get_int(key_multiplayer_start_index()));
            }
            if dict.get_type(key_player_color()).is_some() {
                let color = (dict.get_int(key_player_color()) as u32) | 0xff00_0000;
                let night = if dict.get_type(key_player_night_color()).is_some() {
                    (dict.get_int(key_player_night_color()) as u32) | 0xff00_0000
                } else {
                    color
                };
                let to_color = |argb: u32| {
                    gamelogic::common::Color::new(
                        ((argb >> 16) & 0xff) as u8,
                        ((argb >> 8) & 0xff) as u8,
                        (argb & 0xff) as u8,
                        ((argb >> 24) & 0xff) as u8,
                    )
                };
                player.set_colors(to_color(color), to_color(night));
            }
            if dict.get_type(key_player_start_money()).is_some() {
                // C++ Player.cpp:1007-1009 deposits map playerStartMoney.
                player
                    .get_money_mut()
                    .deposit_money(dict.get_int(key_player_start_money()));
            }
            logic_list.add_player(Arc::new(RwLock::new(player)));

            if self.local_player_id() == Some(host_id)
                || (self.local_player_id().is_none()
                    && is_human
                    && logic_list.get_local_player_index() < 0)
            {
                logic_list.set_local_player_index(host_id as i32);
            }
        }

        // C++ PlayerList.cpp:167-199 — apply playerAllies / playerEnemies after
        // every side exists so name lookup can resolve.
        apply_logic_player_list_relationships(&mut logic_list, side_dicts);

        match ThePlayerList().try_write() {
            Ok(mut guard) => *guard = logic_list,
            Err(_) => {
                log::warn!(
                    "Fast legacy runtime sync skipped PlayerList write (ThePlayerList busy)"
                );
            }
        }
    }

    pub(in super::super) fn sync_legacy_player_list_from_sides(&self) {
        let sides_guard = &self.prepared_map_sides;

        let side_dicts: Vec<Dict> = (0..sides_guard.get_num_sides())
            .filter_map(|index| {
                sides_guard
                    .get_side_info(index)
                    .map(|side| side.get_dict().clone())
            })
            .collect();
        self.sync_legacy_player_list_from_side_dicts(&side_dicts);
    }

    pub(in super::super) fn add_host_players_as_sides(
        &self,
        sides: &mut gamelogic::sides_list::SidesList,
    ) {
        let mut pids: Vec<u32> = self.players.keys().copied().collect();
        pids.sort_unstable();
        // Side names of the slots that become MP sides, with their lobby team.
        let slot_sides: Vec<(String, i32)> = pids
            .iter()
            .filter_map(|pid| {
                let player = self.players.get(pid)?;
                let skipped = player.map_side.role != PlayerSideRole::Participant;
                (!skipped).then(|| (host_slot_script_name(player), player.alliance_team))
            })
            .collect();
        for pid in &pids {
            let Some(player) = self.players.get(pid) else {
                continue;
            };
            // C++ GameLogic1291–1425 appends occupied GameInfo slots only.
            // Authored map owners already come from the prepared input rows;
            // re-emitting them would carry a previous map into a retry.
            if player.map_side.role != PlayerSideRole::Participant {
                continue;
            }
            let player_name = host_slot_script_name(player);
            if sides.find_side_info(&player_name).is_some() {
                continue;
            }
            let faction = self
                .player_template_identity(*pid)
                .map(|identity| identity.template_name.as_str());
            let faction = faction.unwrap_or(match player.team {
                Team::USA => "FactionAmerica",
                Team::China => "FactionChina",
                Team::GLA => "FactionGLA",
                Team::Neutral => "FactionCivilian",
            });
            let mut dict = Dict::new();
            dict.set_ascii_string(key_player_name(), player_name.clone());
            dict.set_bool(key_player_is_human(), player.is_human);
            dict.set_unicode_string(key_player_display_name(), player.name.clone());
            dict.set_ascii_string(key_player_faction(), faction);
            // C++ GameLogic.cpp:1332-1368: another occupied slot is an enemy
            // when our team is None (-1) or differs from theirs, else an ally.
            let (mut allies, mut enemies) = (Vec::new(), Vec::new());
            for (other_name, other_team) in &slot_sides {
                if *other_name == player_name {
                    continue;
                }
                if player.alliance_team == -1 || *other_team != player.alliance_team {
                    enemies.push(other_name.as_str());
                } else {
                    allies.push(other_name.as_str());
                }
            }
            dict.set_ascii_string(key_player_allies(), allies.join(" "));
            dict.set_ascii_string(key_player_enemies(), enemies.join(" "));
            // CPP GameLogic1400 copies GameSlot::getStartPos verbatim, including
            // an unresolved -1; it never substitutes the occupied-slot rank.
            dict.set_int(key_multiplayer_start_index(), player.start_position);
            if matches!(self.game_mode, GameMode::Skirmish) {
                dict.set_bool(key_player_is_skirmish(), !player.is_human);
            }
            sides.add_side(&dict);

            let mut team = Dict::new();
            let mut team_name = String::from("team");
            team_name.push_str(&player_name);
            team.set_ascii_string(key_team_name(), team_name);
            team.set_ascii_string(key_team_owner(), player_name);
            team.set_bool(key_team_is_singleton(), true);
            sides.add_team(&team);
        }
        // C++ adds ReplayObserver after every occupied slot, independently of host IDs.
        if self.replay_observer_player_id().is_some() {
            if sides.find_side_info("ReplayObserver").is_none() {
                let mut dict = Dict::new();
                dict.set_ascii_string(key_player_name(), "ReplayObserver");
                dict.set_bool(key_player_is_human(), true);
                dict.set_unicode_string(key_player_display_name(), "Observer");
                dict.set_ascii_string(key_player_faction(), "FactionObserver");
                dict.set_ascii_string(key_player_allies(), String::new());
                dict.set_ascii_string(key_player_enemies(), String::new());
                dict.set_int(key_multiplayer_start_index(), 0);
                sides.add_side(&dict);
                let mut team = Dict::new();
                team.set_ascii_string(key_team_name(), "teamReplayObserver");
                team.set_ascii_string(key_team_owner(), "ReplayObserver");
                team.set_bool(key_team_is_singleton(), true);
                sides.add_team(&team);
            }
        }
    }

    pub(in super::super) fn transfer_side_build_lists_to_players(&mut self) {
        let owners: Vec<_> = (0..self.prepared_map_sides.get_num_sides())
            .map(|index| self.host_player_id_for_side_index(index as u32))
            .collect();
        for (index, owner) in owners.into_iter().enumerate() {
            let Some(side) = self.prepared_map_sides.get_side_info_mut(index) else {
                continue;
            };
            let mut node = side.take_build_list();
            let mut list = Vec::new();
            while let Some(mut entry) = node {
                let location = entry.get_location();
                list.push(HostAuthoredBuild {
                    template: entry.get_template_name().to_string(),
                    position: (location.x, location.y, location.z),
                    num_rebuilds: entry.get_num_rebuilds(),
                    initially_built: entry.is_initially_built(),
                    completion: crate::ai::AuthoredStructureCompletion {
                        name: entry.get_building_name().to_string(),
                        script: entry.get_script().to_string(),
                        health_percent: entry.get_health(),
                        unsellable: entry.get_unsellable(),
                    },
                });
                node = entry.take_next_build_list();
            }
            if let Some(player) = owner.and_then(|id| self.players.get_mut(&id)) {
                player.map_side.build_list = list;
            }
        }
        self.feed_host_ai_from_authored_build_lists();
    }

    pub(in super::super) fn host_player_id_for_side_index(&self, side_index: u32) -> Option<u32> {
        if self.prepared_map_sides.get_num_sides() > 0 {
            let side = self.prepared_map_sides.get_side_info(side_index as usize)?;
            let name = side.get_dict().get_ascii_string(key_player_name());
            return self.players.iter().find_map(|(&id, player)| {
                (player.script_identity() == Some(name.as_str())).then_some(id)
            });
        }
        if self.players.contains_key(&side_index) {
            return Some(side_index);
        }
        let mut pids: Vec<u32> = self.players.keys().copied().collect();
        pids.sort_unstable();
        pids.get(side_index as usize).copied()
    }

    pub(in super::super) fn apply_host_players_from_sides_list(
        &mut self,
        replace_default_money: bool,
    ) {
        let sides_guard = &self.prepared_map_sides;
        let side_dicts: Vec<Dict> = (0..sides_guard.get_num_sides())
            .filter_map(|index| {
                sides_guard
                    .get_side_info(index)
                    .map(|side| side.get_dict().clone())
            })
            .collect();
        self.apply_host_players_from_side_dicts(&side_dicts, replace_default_money);
    }

    /// C++ Player::initFromDict + PlayerList relationship pass onto live host players.
    pub(in super::super) fn apply_host_players_from_side_dicts(
        &mut self,
        side_dicts: &[Dict],
        replace_default_money: bool,
    ) {
        if side_dicts.is_empty() {
            return;
        }
        self.admit_retained_map_civilians(side_dicts);
        for dict in side_dicts {
            let name = dict.get_ascii_string(key_player_name());
            if name.is_empty() {
                continue;
            }
            let pid = {
                self.players
                    .iter()
                    .find(|(_, player)| !name.is_empty() && player.map_side.map_player_name == name)
                    .map(|(id, _)| *id)
            };
            let Some(pid) = pid else {
                continue;
            };
            let controller = self.prepared_player_controllers.for_side(&name);
            if let Some(player) = self.players.get_mut(&pid) {
                // Retained Civilians were initialized and received the authored
                // deposit above. The relationship pass must not deposit twice.
                if player.map_side.role != PlayerSideRole::Authored
                    || dict.get_ascii_string(key_player_faction()) != "FactionCivilian"
                {
                    player.apply_map_side_dict(
                        dict,
                        replace_default_money,
                        &mut self.economy_events,
                    );
                }
                if let Some(controller) = controller {
                    controller.apply(player);
                }
            }
        }

        let mut by_name: HashMap<String, u32> = HashMap::new();
        for (&id, player) in &self.players {
            if !player.map_side.map_player_name.is_empty() {
                by_name.insert(player.map_side.map_player_name.clone(), id);
            }
            if !player.name.is_empty() {
                by_name.entry(player.name.clone()).or_insert(id);
            }
        }
        for dict in side_dicts {
            let name = dict.get_ascii_string(key_player_name());
            let Some(&pid) = by_name.get(&name) else {
                continue;
            };
            let enemies = dict.get_ascii_string(key_player_enemies());
            let allies = dict.get_ascii_string(key_player_allies());
            if let Some(player) = self.players.get_mut(&pid) {
                player.set_map_relationship(pid, gamelogic::common::Relationship::Allies);
                for token in enemies.split_whitespace() {
                    if let Some(&eid) = by_name.get(token) {
                        player.set_map_relationship(eid, gamelogic::common::Relationship::Enemies);
                    }
                }
                for token in allies.split_whitespace() {
                    if let Some(&aid) = by_name.get(token) {
                        player.set_map_relationship(aid, gamelogic::common::Relationship::Allies);
                    }
                }
            }
        }
        // C++ GameLogic.cpp:2073-2119 after PlayerList::newMap.
        self.apply_challenge_the_player_relationships();
    }

    /// Copy dummy ThePlayer alliances onto the local Challenge general.
    pub(in super::super) fn apply_challenge_the_player_relationships(&mut self) {
        if !self.is_challenge_session() {
            return;
        }
        if !self.is_single_player_scenario() {
            return;
        }
        use gamelogic::common::Relationship;
        let Some(local_id) = self.local_player_id() else {
            return;
        };
        let the_player_id = self
            .players
            .iter()
            .find(|(_, player)| {
                player
                    .map_side
                    .map_player_name
                    .eq_ignore_ascii_case("ThePlayer")
                    || player.name.eq_ignore_ascii_case("ThePlayer")
            })
            .map(|(&id, _)| id);

        if let Some(placeholder_id) = the_player_id {
            let enemy_ids: Vec<u32> = self
                .players
                .keys()
                .copied()
                .filter(|&id| {
                    id != placeholder_id
                        && self
                            .players
                            .get(&placeholder_id)
                            .and_then(|player| player.map_relationship(id))
                            == Some(Relationship::Enemies)
                })
                .collect();
            for eid in enemy_ids {
                if let Some(enemy) = self.players.get_mut(&eid) {
                    enemy.set_map_relationship(local_id, Relationship::Enemies);
                }
                if let Some(local) = self.players.get_mut(&local_id) {
                    local.set_map_relationship(eid, Relationship::Enemies);
                }
            }
            return;
        }

        let ids: Vec<u32> = self.players.keys().copied().collect();
        for id in ids {
            let rel = if id == local_id {
                Relationship::Allies
            } else {
                let player = &self.players[&id];
                let name = player.map_side.map_player_name.as_str();
                let civilian_or_neutral = player.team == Team::Neutral
                    || name.eq_ignore_ascii_case("PlyrCivilian")
                    || name.to_ascii_lowercase().contains("civilian")
                    || name.to_ascii_lowercase().contains("neutral");
                if civilian_or_neutral {
                    Relationship::Neutral
                } else {
                    Relationship::Enemies
                }
            };
            if let Some(local) = self.players.get_mut(&local_id) {
                local.set_map_relationship(id, rel);
            }
        }
    }

    pub(in super::super) fn stash_side_builds_on_host(
        &mut self,
        builds: &[super::script_loader::SideBuildEntry],
    ) {
        if builds.is_empty() {
            return;
        }
        let mut grouped: HashMap<u32, Vec<HostAuthoredBuild>> = HashMap::new();
        for entry in builds {
            let Some(pid) = self.host_player_id_for_side_index(entry.side_index) else {
                continue;
            };
            grouped.entry(pid).or_default().push(HostAuthoredBuild {
                template: entry.template.clone(),
                position: (entry.position.x, entry.position.y, entry.position.z),
                num_rebuilds: entry.num_rebuilds.max(0) as u32,
                initially_built: entry.initially_built,
                completion: crate::ai::AuthoredStructureCompletion {
                    name: entry.building_name.clone(),
                    script: entry.script_name.clone().unwrap_or_default(),
                    health_percent: entry.health.unwrap_or(100),
                    unsellable: entry.unsellable.unwrap_or(false),
                },
            });
        }
        for (pid, list) in grouped {
            if let Some(player) = self.players.get_mut(&pid) {
                player.map_side.build_list = list;
            }
        }
        self.feed_host_ai_from_authored_build_lists();
    }

    /// C++ AIPlayer::newMap consumes Player build list + numRebuilds.
    pub(in super::super) fn feed_host_ai_from_authored_build_lists(&mut self) {
        let authored: Vec<(u32, Vec<HostAuthoredBuild>)> = self
            .players
            .iter()
            .filter_map(|(&id, player)| {
                if player.map_side.build_list.is_empty() {
                    None
                } else {
                    Some((id, player.map_side.build_list.clone()))
                }
            })
            .collect();
        for (pid, list) in authored {
            let Some(ai) = self.ai_manager.ai_players.get_mut(&pid) else {
                continue;
            };
            ai.building_queue.clear();
            for entry in list {
                let pos = glam::Vec3::new(entry.position.0, entry.position.2, entry.position.1);
                ai.add_building(&entry.template, pos, entry.num_rebuilds);
                if let Some(last) = ai.building_queue.last_mut() {
                    last.completion = Some(entry.completion);
                    last.is_built = entry.initially_built;
                }
            }
        }
    }

    pub(in super::super) fn sync_legacy_team_factory_from_team_dicts(
        &mut self,
        team_dicts: &[Dict],
    ) -> bool {
        self.sync_legacy_team_factory_from_team_dicts_with_effects(
            team_dicts,
            &mut Default::default(),
        )
    }

    pub(in super::super) fn sync_legacy_team_factory_from_team_dicts_with_effects(
        &mut self,
        team_dicts: &[Dict],
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        if self.owned_script_engine.is_none() {
            log::error!("Team admission requires this map's script interpreter");
            return false;
        }
        {
            let Ok(mut factory) = self.team_factory.try_lock() else {
                return false;
            };
            factory.reset();
        }
        // TeamFactory reset retires every Team identity. Clear only this
        // receiving world's Player bindings before definitions are rebuilt.
        for player in self.players.values_mut() {
            player.default_team_id = None;
        }
        for dict in team_dicts {
            let name = dict.get_ascii_string(game_engine::common::well_known_keys::key_team_name());
            if name.is_empty() {
                continue;
            }
            let owner =
                dict.get_ascii_string(game_engine::common::well_known_keys::key_team_owner());
            let singleton =
                dict.get_bool(game_engine::common::well_known_keys::key_team_is_singleton());
            let controller = self
                .players
                .values()
                .find(|player| !owner.is_empty() && player.map_side.map_player_name == owner)
                .or_else(|| {
                    self.players.values().find(|player| {
                        player.map_side.map_player_name.is_empty()
                            && !owner.is_empty()
                            && player.name == owner
                    })
                })
                .or_else(|| {
                    self.players
                        .values()
                        .find(|player| player.is_reserved_neutral())
                })
                .map(|player| player.id);
            let default_player_id = self.players.values().find_map(|player| {
                (self.default_host_team_instance_name(Some(player.id), player.team) == name)
                    .then_some(player.id)
            });
            let (on_create_scripts, admitted_default_id) = {
                let Ok(mut factory) = self.team_factory.try_lock() else {
                    return false;
                };
                let previous_ids = if default_player_id.is_some() {
                    factory
                        .find_team_instances(&name)
                        .iter()
                        .filter_map(|team| team.read().ok().map(|team| team.get_id()))
                        .collect::<std::collections::HashSet<_>>()
                } else {
                    std::collections::HashSet::new()
                };
                let Some((_, effects)) = factory.init_team_owned_with_effects(
                    name.clone().into(),
                    owner.clone().into(),
                    singleton,
                    Some(dict),
                    controller,
                ) else {
                    return false;
                };
                let admitted_default_id = if singleton {
                    default_player_id.and_then(|player_id| {
                        let candidates = factory
                            .find_team_instances(&name)
                            .into_iter()
                            .filter_map(|team| {
                                let team = team.read().ok()?;
                                (team.get_controlling_player_id() == Some(player_id)).then_some((
                                    team.get_id(),
                                    !previous_ids.contains(&team.get_id()),
                                ))
                            })
                            .collect::<Vec<_>>();
                        let newly_admitted = candidates
                            .iter()
                            .filter_map(|(id, is_new)| is_new.then_some(*id))
                            .collect::<Vec<_>>();
                        match newly_admitted.as_slice() {
                            [identity] => Some(*identity),
                            [] if candidates.len() == 1 => Some(candidates[0].0),
                            _ => None,
                        }
                    })
                } else {
                    None
                };
                (effects, admitted_default_id)
            };
            if let (Some(player_id), Some(identity)) = (default_player_id, admitted_default_id) {
                if let Some(player) = self.players.get_mut(&player_id) {
                    player.default_team_id = Some(identity);
                }
            }
            // createInactiveTeam executes with NULL team before initFromSides
            // advances to the next definition. The factory is available again
            // for recursive actions and all session owners remain installed.
            if self
                .with_owned_script_engine(|engine, world| {
                    for script in on_create_scripts {
                        world.execute_owned_ai_condition_with_effects(
                            engine, &script, None, effects,
                        );
                    }
                })
                .is_none()
            {
                return false;
            }
        }
        let mut player_ids = self.players.keys().copied().collect::<Vec<_>>();
        player_ids.sort_unstable();
        for player_id in player_ids {
            self.ensure_host_default_team(player_id);
        }
        true
    }
}

#[cfg(test)]
mod map_singleton_owner_tests {
    use super::*;
    use game_engine::common::well_known_keys::*;
    use gamelogic::scripting::core::{
        Parameter, ParameterType, Script, ScriptAction, ScriptActionType, ScriptList,
    };
    use gamelogic::scripting::engine::ScriptEngine;

    fn definition(name: &str, callback: &str) -> Dict {
        let mut dict = Dict::new();
        dict.set_ascii_string(key_team_name(), name);
        dict.set_ascii_string(key_team_owner(), "MapOwner");
        dict.set_bool(key_team_is_singleton(), true);
        dict.set_ascii_string(key_team_production_condition(), callback);
        dict.set_bool(key_team_executes_actions_on_create(), true);
        dict
    }
    fn interpreter() -> ScriptEngine {
        let engine = ScriptEngine::new().unwrap();
        let mut first = ScriptAction::new(ScriptActionType::SetCounter);
        first
            .add_parameter(Parameter::with_string(
                ParameterType::Counter,
                "AdmissionOrder".into(),
            ))
            .unwrap();
        first
            .add_parameter(Parameter::with_int(ParameterType::Int, 10))
            .unwrap();
        let mut spawn = ScriptAction::new(ScriptActionType::CreateObject);
        spawn
            .add_parameter(Parameter::with_string(
                ParameterType::ObjectType,
                "MapCallbackInfantry".into(),
            ))
            .unwrap();
        spawn
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                "SpawnTeam".into(),
            ))
            .unwrap();
        spawn
            .add_parameter(Parameter::with_coord(
                ParameterType::Coord3D,
                gamelogic::scripting::core::Coord3D::new(17.0, 23.0, 0.0),
            ))
            .unwrap();
        spawn
            .add_parameter(Parameter::with_real(ParameterType::Angle, 0.0))
            .unwrap();
        first.next_action = Some(Box::new(spawn));
        let mut second = ScriptAction::new(ScriptActionType::IncrementCounter);
        second
            .add_parameter(Parameter::with_int(ParameterType::Int, 2))
            .unwrap();
        second
            .add_parameter(Parameter::with_string(
                ParameterType::Counter,
                "AdmissionOrder".into(),
            ))
            .unwrap();
        let mut list = ScriptList::new();
        for (name, action) in [("FirstCallback", first), ("SecondCallback", second)] {
            let mut script = Script::new();
            script.script_name = name.into();
            script.is_subroutine = true;
            script.action = Some(Box::new(action));
            list.append_script(Box::new(script));
        }
        engine
            .set_script_list_for_player(0, Some(Box::new(list)))
            .unwrap();
        engine
    }

    #[test]
    fn team_factory_reload_rebinds_each_players_exact_default_identity() {
        let mut world = GameLogic::new();
        world.install_owned_script_engine(interpreter());
        let mut player = Player::new(1, Team::USA, "MapOwner", true);
        player.map_side.map_player_name = "MapOwner".into();
        world.add_player(player);
        let first = world.host_default_team_id(1).expect("first default Team");
        let mut inserted = Dict::new();
        inserted.set_ascii_string(key_team_name(), "BeforeDefault");
        inserted.set_ascii_string(key_team_owner(), "MapOwner");
        inserted.set_bool(key_team_is_singleton(), false);
        assert!(world.sync_legacy_team_factory_from_team_dicts(&[inserted]));
        let second = world.host_default_team_id(1).expect("rebound default Team");
        assert_ne!(first, second, "new factory admission order changes Team ID");
        let factory = world.team_factory.lock().unwrap();
        let exact = factory.find_team_by_id(second).expect("bound Team exists");
        let exact = exact.read().unwrap();
        assert_eq!(exact.get_name().as_str(), "teamMapOwner");
        assert_eq!(exact.get_controlling_player_id(), Some(1));
    }
    fn world() -> GameLogic {
        let mut world = GameLogic::new();
        world.add_player(
            Player::new(1, Team::USA, "MapOwner", false).with_script_identity("MapOwner"),
        );
        let mut template = ThingTemplate::new("MapCallbackInfantry");
        template.add_kind_of(KindOf::Infantry).set_health(100.0);
        world.templates.insert(template.name.clone(), template);
        world.install_owned_script_engine(interpreter());
        world
    }

    fn prototype_identity(world: &GameLogic) -> Vec<(String, u32)> {
        let factory = world.team_factory.lock().unwrap();
        factory
            .prototype_names()
            .into_iter()
            .map(|name| {
                let id = factory.find_team_prototype(&name).unwrap().get_id();
                (name, id)
            })
            .collect()
    }

    #[test]
    fn singleton_callbacks_run_in_admission_order_before_new_map_reset_and_preserve_objects() {
        let mut owner = world();
        let mut peer = world();
        let peer_definitions = prototype_identity(&peer);
        assert!(
            !peer_definitions.is_empty(),
            "player admission creates its default team"
        );
        peer.owned_script_engine
            .as_ref()
            .unwrap()
            .set_counter("AdmissionOrder", 91)
            .unwrap();
        let mut spawn_definition = Dict::new();
        spawn_definition.set_ascii_string(key_team_name(), "SpawnTeam");
        spawn_definition.set_ascii_string(key_team_owner(), "MapOwner");
        let definitions = [
            spawn_definition,
            definition("FirstSingleton", "FirstCallback"),
            definition("SecondSingleton", "SecondCallback"),
        ];
        assert!(owner.sync_legacy_team_factory_from_team_dicts(&definitions));
        assert_eq!(
            owner
                .owned_script_engine
                .as_ref()
                .unwrap()
                .get_counter("AdmissionOrder")
                .unwrap()
                .value,
            12
        );
        assert_eq!(owner.objects.registered_len(), 1);
        let created = owner.objects.registered_keys().next().copied().unwrap();
        assert_eq!(owner.host_object(created).unwrap().owner_player_id, Some(1));
        assert_eq!(
            owner.team_factory.lock().unwrap().prototype_names(),
            ["SpawnTeam", "FirstSingleton", "SecondSingleton"]
        );

        assert!(owner.finish_map_script_foundation());
        assert!(
            owner
                .owned_script_engine
                .as_ref()
                .unwrap()
                .get_counter("AdmissionOrder")
                .is_none()
        );
        assert!(
            owner
                .owned_script_engine
                .as_ref()
                .unwrap()
                .find_script_clone_by_name("FirstCallback")
                .is_some()
        );
        assert!(owner.host_object(created).is_some());
        owner
            .create_object_for_player("MapCallbackInfantry", 1, Vec3::ZERO)
            .unwrap();
        assert_eq!(
            owner.objects.registered_len(),
            2,
            "placed objects append after callback-created objects"
        );
        assert!(peer.objects.registered_is_empty());
        assert_eq!(prototype_identity(&peer), peer_definitions);
        assert_eq!(
            peer.owned_script_engine
                .as_ref()
                .unwrap()
                .get_counter("AdmissionOrder")
                .unwrap()
                .value,
            91
        );
    }

    #[test]
    fn map_team_admission_without_owned_interpreter_fails_before_factory_mutation() {
        let mut owner = world();
        let definitions = prototype_identity(&owner);
        assert!(
            !definitions.is_empty(),
            "player admission creates its default team"
        );
        owner.owned_script_engine = None;
        assert!(
            !owner.sync_legacy_team_factory_from_team_dicts(&[definition(
                "FirstSingleton",
                "FirstCallback"
            )])
        );
        assert_eq!(prototype_identity(&owner), definitions);
        assert!(owner.objects.registered_is_empty());
        assert!(!owner.finish_map_script_foundation());
    }
}

#[cfg(test)]
#[path = "prepared_team_factory_tests.rs"]
mod prepared_team_factory_tests;

#[cfg(test)]
#[path = "polygon_trigger_owner_tests.rs"]
mod polygon_trigger_owner_tests;
