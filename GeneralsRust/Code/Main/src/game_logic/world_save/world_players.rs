//! Host player, team, and authored build-list state.
#![allow(unused_imports, non_snake_case)]
use super::*;

impl GameLogic {
    pub(in super::super) fn sync_legacy_runtime_from_chunky(
        &mut self,
        map_path: &Path,
        map_bytes: &[u8],
    ) {
        let sync_started = Instant::now();
        let mut loader = LogicMapLoader::new();
        self.runtime_road_segments.clear();
        log::info!("Legacy runtime sync started for '{}'", map_path.display());
        if loader.load_runtime_support_from_bytes(map_bytes).is_err() {
            log::warn!(
                "Legacy GameLogic map load failed for '{}'",
                map_path.display()
            );
            return;
        }
        log::info!(
            "Legacy runtime support parse finished for '{}' in {:.2}s",
            map_path.display(),
            sync_started.elapsed().as_secs_f32()
        );

        let map_data = loader.to_map_data();

        if let Ok(mut terrain) = gamelogic::terrain::get_terrain_logic().write() {
            terrain.reset();
            terrain.load_map_data(map_data);
            // C++ GameLogic.cpp:1629 TheTerrainLogic->newMap after load.
            terrain.new_map(false);
        }
        self.copy_crate_water_into_host_terrain();
        log::info!(
            "Legacy terrain sync finished for '{}' in {:.2}s",
            map_path.display(),
            sync_started.elapsed().as_secs_f32()
        );

        self.sync_legacy_player_list_from_sides();
        log::info!(
            "Legacy player-list sync finished for '{}' in {:.2}s",
            map_path.display(),
            sync_started.elapsed().as_secs_f32()
        );
        self.sync_legacy_team_factory_from_sides();
        log::info!(
            "Legacy team-factory sync finished for '{}' in {:.2}s",
            map_path.display(),
            sync_started.elapsed().as_secs_f32()
        );

        let waypoint_count = gamelogic::terrain::get_terrain_logic()
            .read()
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
        let team_count = get_team_factory()
            .lock()
            .map(|factory| factory.get_all_teams().len())
            .unwrap_or(0);

        log::info!(
            "Legacy runtime sync complete for '{}': waypoints={}, live_teams={}",
            map_path.display(),
            waypoint_count,
            team_count
        );
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
        mut report_progress: F,
    ) where
        F: FnMut(f32, &str),
    {
        let sync_started = Instant::now();
        report_progress(0.41, "Fast sync heightmap");
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
        report_progress(0.415, "Fast sync waypoints");

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
        report_progress(0.42, "Fast sync bridges");
        let bridges = match super::script_loader::parse_runtime_bridges_from_chunky(chunky) {
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
        report_progress(0.425, "Fast sync water");
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

        report_progress(0.43, "Fast sync polygons");
        let polygon_triggers =
            match super::script_loader::parse_runtime_polygon_triggers_from_chunky(chunky) {
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
        super::script_loader::install_runtime_polygon_triggers(&polygon_triggers);

        report_progress(0.435, "Fast sync roads");
        self.runtime_road_segments =
            match super::script_loader::parse_runtime_roads_from_chunky(chunky) {
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
        report_progress(0.44, "Fast sync sides");
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
        report_progress(0.445, "Fast sync terrain write");

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

            match gamelogic::terrain::get_terrain_logic().try_write() {
                Ok(mut terrain) => {
                    terrain.reset();
                    terrain.load_map_data(map_data);
                    // C++ GameLogic.cpp:1629 TheTerrainLogic->newMap after load.
                    terrain.new_map(false);
                    log::info!(
                        "Fast legacy runtime sync terrain write finished for '{}' in {:.2}s",
                        map_path.display(),
                        sync_started.elapsed().as_secs_f32()
                    );
                }
                Err(_) => {
                    log::warn!(
                        "Fast legacy runtime sync skipped terrain write for '{}' (THE_TERRAIN_LOGIC busy)",
                        map_path.display()
                    );
                }
            }
            self.copy_crate_water_into_host_terrain();
        } else {
            log::info!(
                "Fast legacy runtime sync skipped terrain write for '{}' (no payload) in {:.2}s",
                map_path.display(),
                sync_started.elapsed().as_secs_f32()
            );
        }
        report_progress(0.45, "Fast sync sides write");

        // The fast parser owns its decoded side/team dictionaries instead of
        // going through LogicMapLoader's global SidesList callback.  Publish
        // the same map-owned SidesList before deriving PlayerList/TeamFactory,
        // otherwise a staged restore would commit an empty (or stale) side
        // singleton even though its player/team globals came from this map.
        // Abandoned boot workers can still hold these globals after generation
        // bump; fail-open so load_map returns and host objects still spawn.
        let script_lists = match super::script_loader::load_map_scripts_from_chunky(chunky) {
            Ok(Some(result)) => result.script_lists,
            _ => Vec::new(),
        };
        self.sync_legacy_sides_list_from_dicts(
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
        self.sync_legacy_player_list_from_sides();
        // C++ GameLogic newMap: PlayerList then TeamFactory::initFromSides.
        // Fast path previously published sides/players and skipped factory init,
        // so selectTeamToBuild saw an empty getPlayerTeams() list.
        self.sync_legacy_team_factory_from_sides();

        if !self.players.is_empty() {
            self.apply_host_players_from_side_dicts(&sides_data.side_dicts, false);
            self.stash_side_builds_on_host(&sides_data.side_builds);
        }
        self.transfer_side_build_lists_to_players();

        let waypoint_count = gamelogic::terrain::get_terrain_logic()
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
        let team_count = get_team_factory()
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
    }

    pub(in super::super) fn sync_legacy_player_list_from_side_dicts(&self, side_dicts: &[Dict]) {
        let mut logic_list = LogicPlayerList::new();

        for (index, dict) in side_dicts.iter().enumerate() {
            let player_name = dict.get_ascii_string(key_player_name());
            let faction = dict.get_ascii_string(key_player_faction());
            let display_name = dict.get_unicode_string(key_player_display_name());
            let is_human = dict.get_bool(key_player_is_human());

            // Keep player-template store locking narrow so Player::init can lazily hydrate
            // templates without deadlocking on the same global RwLock.
            let template_from_store = try_get_player_template_store().and_then(|store| {
                store
                    .find_template(&faction)
                    .map(LogicPlayerTemplate::from_common)
            });
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

            let mut player = LogicPlayer::new(index as i32);
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
            player.set_player_type(player_type, false);
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

            if is_human && logic_list.get_local_player_index() < 0 {
                logic_list.set_local_player_index(index as i32);
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

    pub(in super::super) fn sync_legacy_sides_list_from_dicts(
        &self,
        side_dicts: &[Dict],
        team_dicts: &[Dict],
        side_builds: &[super::script_loader::SideBuildEntry],
        script_lists: &[ScriptList],
    ) {
        let sides_list = get_sides_list();
        let Ok(mut sides) = sides_list.try_write() else {
            log::warn!("Fast legacy runtime sync skipped SidesList write (THE_SIDES_LIST busy)");
            return;
        };
        sides.reset();
        for dict in side_dicts {
            sides.add_side(dict);
        }
        for dict in team_dicts {
            sides.add_team(dict);
        }

        let mut pos_by_side: HashMap<u32, i32> = HashMap::new();
        for entry in side_builds {
            let pos = *pos_by_side.get(&entry.side_index).unwrap_or(&0);
            let mut build = gamelogic::build_list_info::BuildListInfo::new();
            build.set_building_name(gamelogic::common::AsciiString::from(
                entry.building_name.as_str(),
            ));
            build.set_template_name(gamelogic::common::AsciiString::from(
                entry.template.as_str(),
            ));
            build.set_location(gamelogic::common::Coord3D::new(
                entry.position.x,
                entry.position.y,
                0.0,
            ));
            build.set_angle(entry.angle);
            build.set_initially_built(entry.initially_built);
            build.set_num_rebuilds(entry.num_rebuilds.max(0) as u32);
            if let Some(script) = &entry.script_name {
                build.set_script(gamelogic::common::AsciiString::from(script.as_str()));
            }
            if let Some(health) = entry.health {
                build.set_health(health);
            }
            if let Some(whiner) = entry.whiner {
                build.set_whiner(whiner);
            }
            if let Some(unsellable) = entry.unsellable {
                build.set_unsellable(unsellable);
            }
            if let Some(repairable) = entry.repairable {
                build.set_repairable(repairable);
            }
            if let Some(side) = sides.get_side_info_mut(entry.side_index as usize) {
                side.add_to_build_list(build, pos);
                pos_by_side.insert(entry.side_index, pos + 1);
            }
        }

        for (index, scripts) in script_lists.iter().enumerate() {
            if let Some(side) = sides.get_side_info_mut(index) {
                side.set_script_list(Some(Box::new(scripts.clone())));
            }
        }

        sides.validate_sides();

        if matches!(
            self.game_mode,
            GameMode::Skirmish
                | GameMode::Multiplayer
                | GameMode::Lan
                | GameMode::Internet
                | GameMode::Replay
        ) {
            sides.prepare_for_mp_or_skirmish();
            self.add_host_players_as_sides(&mut sides);
            sides.validate_sides();
        }
    }

    pub(in super::super) fn add_host_players_as_sides(
        &self,
        sides: &mut gamelogic::sides_list::SidesList,
    ) {
        let mut pids: Vec<u32> = self.players.keys().copied().collect();
        pids.sort_unstable();
        for (index, pid) in pids.iter().enumerate() {
            let Some(player) = self.players.get(pid) else {
                continue;
            };
            if player.name == "ReplayObserver" {
                if sides.find_side_info("ReplayObserver").is_none() {
                    let mut dict = Dict::new();
                    dict.set_ascii_string(key_player_name(), "ReplayObserver");
                    dict.set_bool(key_player_is_human(), true);
                    dict.set_unicode_string(key_player_display_name(), "Observer");
                    dict.set_ascii_string(key_player_faction(), "FactionObserver");
                    dict.set_ascii_string(key_player_allies(), String::new());
                    dict.set_ascii_string(key_player_enemies(), String::new());
                    sides.add_side(&dict);
                    let mut team = Dict::new();
                    team.set_ascii_string(key_team_name(), "teamReplayObserver");
                    team.set_ascii_string(key_team_owner(), "ReplayObserver");
                    team.set_bool(key_team_is_singleton(), true);
                    sides.add_team(&team);
                }
                continue;
            }
            if player.team == Team::Neutral && player.name.is_empty() {
                continue;
            }
            let player_name = format!("player{index}");
            if sides.find_side_info(&player_name).is_some() {
                continue;
            }
            let faction = match player.team {
                Team::USA => "FactionAmerica",
                Team::China => "FactionChina",
                Team::GLA => "FactionGLA",
                Team::Neutral => "FactionCivilian",
            };
            let mut dict = Dict::new();
            dict.set_ascii_string(key_player_name(), player_name.clone());
            dict.set_bool(key_player_is_human(), player.is_local);
            let display = if player.name.is_empty() {
                player_name.clone()
            } else {
                player.name.clone()
            };
            dict.set_unicode_string(key_player_display_name(), display);
            dict.set_ascii_string(key_player_faction(), faction);
            dict.set_ascii_string(key_player_allies(), String::new());
            dict.set_ascii_string(key_player_enemies(), String::new());
            let start_index = if player.start_position >= 0 {
                player.start_position
            } else {
                index as i32
            };
            dict.set_int(key_multiplayer_start_index(), start_index);
            if matches!(self.game_mode, GameMode::Skirmish) {
                dict.set_bool(key_player_is_skirmish(), !player.is_local);
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
    }

    pub(in super::super) fn transfer_side_build_lists_to_players(&self) {
        let sides_list = get_sides_list();
        let Ok(mut sides) = sides_list.try_write() else {
            return;
        };
        let player_list = ThePlayerList();
        let Ok(mut players) = player_list.try_write() else {
            return;
        };
        for index in 0..sides.get_num_sides() {
            let Some(side) = sides.get_side_info_mut(index) else {
                continue;
            };
            let Some(build_list) = side.take_build_list() else {
                continue;
            };
            if let Some(player) = players.get_player(index as i32) {
                if let Ok(mut player) = player.write() {
                    player.set_build_list(Some(*build_list));
                }
            }
        }
    }

    pub(in super::super) fn host_player_id_for_side_index(&self, side_index: u32) -> Option<u32> {
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
        let sides_list = get_sides_list();
        let Ok(sides_guard) = sides_list.try_read() else {
            return;
        };
        let side_dicts: Vec<Dict> = (0..sides_guard.get_num_sides())
            .filter_map(|index| {
                sides_guard
                    .get_side_info(index)
                    .map(|side| side.get_dict().clone())
            })
            .collect();
        drop(sides_guard);
        self.apply_host_players_from_side_dicts(&side_dicts, replace_default_money);
    }

    /// C++ Player::initFromDict + PlayerList relationship pass onto live host players.
    pub(in super::super) fn apply_host_players_from_side_dicts(
        &mut self,
        side_dicts: &[Dict],
        replace_default_money: bool,
    ) {
        if self.players.is_empty() || side_dicts.is_empty() {
            return;
        }
        for (index, dict) in side_dicts.iter().enumerate() {
            let name = dict.get_ascii_string(key_player_name());
            let pid = self
                .players
                .iter()
                .find(|(_, player)| {
                    !name.is_empty()
                        && (player.map_side.map_player_name == name || player.name == name)
                })
                .map(|(id, _)| *id)
                .or_else(|| self.host_player_id_for_side_index(index as u32));
            let Some(pid) = pid else {
                continue;
            };
            if let Some(player) = self.players.get_mut(&pid) {
                player.apply_map_side_dict(dict, replace_default_money);
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
        if !game_engine::System::capture_campaign_manager_runtime().is_challenge {
            return;
        }
        if !matches!(self.game_mode, GameMode::SinglePlayer) {
            return;
        }
        use gamelogic::common::Relationship;
        let Some(local_id) = self
            .players
            .iter()
            .find(|(_, player)| player.is_local)
            .map(|(&id, _)| id)
        else {
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
                if entry.initially_built {
                    if let Some(last) = ai.building_queue.last_mut() {
                        last.is_built = true;
                    }
                }
            }
        }
    }

    pub(in super::super) fn sync_legacy_player_list_from_sides(&self) {
        let sides_list = get_sides_list();
        let Ok(sides_guard) = sides_list.try_read() else {
            log::warn!("Fast legacy runtime sync skipped PlayerList derive (THE_SIDES_LIST busy)");
            return;
        };

        let side_dicts: Vec<Dict> = (0..sides_guard.get_num_sides())
            .filter_map(|index| {
                sides_guard
                    .get_side_info(index)
                    .map(|side| side.get_dict().clone())
            })
            .collect();
        self.sync_legacy_player_list_from_side_dicts(&side_dicts);
    }

    pub(in super::super) fn sync_legacy_team_factory_from_team_dicts(&self, team_dicts: &[Dict]) {
        let Ok(mut team_factory) = get_team_factory().try_lock() else {
            log::warn!(
                "Fast legacy runtime sync skipped TeamFactory write (THE_TEAM_FACTORY busy)"
            );
            return;
        };
        team_factory.reset();

        for dict in team_dicts {
            let team_name =
                dict.get_ascii_string(game_engine::common::well_known_keys::key_team_name());
            if team_name.is_empty() {
                continue;
            }

            let owner =
                dict.get_ascii_string(game_engine::common::well_known_keys::key_team_owner());
            let singleton =
                dict.get_bool(game_engine::common::well_known_keys::key_team_is_singleton());

            let _ = team_factory.init_team(
                team_name.clone().into(),
                owner.clone().into(),
                singleton,
                Some(dict),
            );

            // C++ TeamFactory::initFromSides only initTeam: prototype plus an
            // inactive singleton instance. Do not findTeam/createTeam here —
            // that would make getTeamNamed see empty teams at map load so
            // TEAM_DESTROYED is true before any units spawn.
        }
    }

    pub(in super::super) fn sync_legacy_team_factory_from_sides(&self) {
        let sides_list = get_sides_list();
        let Ok(sides_guard) = sides_list.try_read() else {
            log::warn!("Fast legacy runtime sync skipped TeamFactory derive (THE_SIDES_LIST busy)");
            return;
        };

        let team_dicts: Vec<Dict> = (0..sides_guard.get_num_teams())
            .filter_map(|index| {
                sides_guard
                    .get_team_info(index)
                    .map(|team| team.get_dict().clone())
            })
            .collect();
        self.sync_legacy_team_factory_from_team_dicts(&team_dicts);
    }

    pub(in super::super) fn sync_named_shell_object_into_legacy_runtime(
        &self,
        object: &super::script_loader::PlacedObject,
        host_id: ObjectId,
    ) {
        if self.game_mode != GameMode::Shell {
            return;
        }

        let Some(name) = object
            .name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
        else {
            return;
        };

        let tracker = gamelogic::scripting::engine::get_named_object_tracker();
        if tracker.get_object_id(name).ok().flatten().is_some() {
            return;
        }

        // Wave 476: host-only named tracker registration.
        // Dual ObjectManager/OBJECT_REGISTRY mirror retired — host ObjectId is the name key;
        // GameWorld shadow materialize owns any GW entity map when dual-tick is enabled.
        if let Err(err) = tracker.register_named_object(name.to_string(), host_id.0) {
            log::warn!(
                "Failed to register host shell object '{}' -> {}: {}",
                name,
                host_id.0,
                err
            );
        }
    }
}
