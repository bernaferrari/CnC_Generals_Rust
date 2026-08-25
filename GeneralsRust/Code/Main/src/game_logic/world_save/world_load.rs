//! Map loading, validation helpers, and load side effects.
#![allow(unused_imports, non_snake_case)]
use super::*;

impl GameLogic {
    pub(in super::super) fn ground_loaded_map_objects_to_terrain(
        &mut self,
        objects: &[super::script_loader::PlacedObject],
        spawned_object_ids: &[(ObjectId, usize)],
    ) {
        if self.terrain.is_none() || spawned_object_ids.is_empty() {
            return;
        }

        let mut grounded_positions = Vec::with_capacity(spawned_object_ids.len());
        for &(_, index) in spawned_object_ids {
            let object = &objects[index];
            let ground_height = self
                .terrain_height_at(Vec3::new(object.position.x, 0.0, object.position.y))
                .unwrap_or(0.0);
            grounded_positions.push((
                index,
                object.position.x,
                object.position.z + ground_height,
                object.position.y,
            ));
        }

        for ((object_id, _), (_, x, y, z)) in
            spawned_object_ids.iter().copied().zip(grounded_positions)
        {
            if let Some(object) = self.objects.get_mut(&object_id) {
                object.set_position(Vec3::new(x, y, z));
                // y is world height after grounding; residual height sample = y when terrain present.
                object.set_ground_height_residual(y, true);
                crate::game_logic::host_ground_height_log::record(object_id, y, true);
                if crate::gameworld_shadow::gameworld_movement_authority_live() {
                    crate::game_logic::host_move_log::record(object_id, Some([x, y, z]));
                    object.record_host_movement();
                }
            }
        }
        // Wave 475: host map grounding is host-ObjectStore only.
        // Dual-world OBJECT_REGISTRY pose writes retired — bridge/shadow materialize owns GW poses.
        let _ = objects;
    }

    /// Load a map with optional milestone progress reporting.
    pub fn load_map_with_progress<F>(&mut self, map_name: &str, mut report_progress: F) -> bool
    where
        F: FnMut(f32, &str),
    {
        report_progress(0.26, "Preparing map data");
        log::info!("Loading map: {}", map_name);
        let load_started = Instant::now();
        // A failed decode must never leave the previous map marked playable.
        // `load_map_with_progress` mutates the world incrementally, so the
        // successful tail below is the only place allowed to set this back to
        // true. This matters especially for a selected-map → default-map
        // fallback: if both attempts fail, callers must see that no map loaded.
        self.map_loaded = false;
        self.map_name = map_name.to_string();
        self.pathfinding_height_samples = None;
        self.runtime_terrain_texture_classes.clear();
        self.configure_victory_rules_for_map(map_name);
        self.scripts_loaded = false;
        self.script_event_pump_in_flight
            .store(false, Ordering::Release);
        self.script_event_pump_busy_frames = 0;
        self.loaded_script_lists.clear();
        self.script_source_path = None;
        self.mission_scripts.install_lists(&[]);
        self.script_broadcasts.clear();
        self.new_script_messages.clear();
        self.pending_popup_messages.clear();
        self.pending_view_guardband = None;
        self.pending_camera_bw_mode = None;
        self.pending_camera_motion_blur.clear();
        self.script_skybox_enabled = true;
        self.script_cameo_flash_count.clear();
        self.script_named_timers.clear();
        self.script_named_timer_display_shown = true;
        self.script_superweapon_display_enabled = true;
        self.script_superweapon_hidden_objects.clear();
        self.eva_superweapon_science_hidden.clear();
        self.mission_objectives = self.load_campaign_objectives(map_name);
        self.rebuild_objective_lookup();

        // Try to locate the real map file so scripts and future terrain loaders have a source.
        report_progress(0.30, "Resolving map resources");
        let resolved_map = super::script_loader::find_map_file(map_name);
        if let Some(path) = &resolved_map {
            log::info!("Resolved map '{}' to '{}'", map_name, path.display());
            if let Ok(Some(chunky)) = super::script_loader::load_chunky_map(map_name) {
                if let Some(chunks) = super::script_loader::inspect_map_chunks_from_chunky(&chunky)
                {
                    log::debug!(
                        "Map '{}' contains chunky sections: {}",
                        map_name,
                        chunks.join(", ")
                    );
                }
                report_progress(0.34, "Parsing map chunks");
                log::info!(
                    "Map '{}' parsed: {} TOC entries, body offset {} bytes",
                    map_name,
                    chunky.toc.len(),
                    chunky.body_offset
                );
                if self.game_mode != GameMode::Shell {
                    report_progress(0.40, "Syncing runtime objects");
                } else {
                    report_progress(0.40, "Syncing shell runtime");
                }
                let sync_started = Instant::now();
                self.sync_legacy_runtime_from_fast_chunky_with_progress(
                    path,
                    &chunky,
                    &mut report_progress,
                );
                log::info!(
                    "Map '{}' legacy runtime sync finished in {:.2}s (fast path)",
                    map_name,
                    sync_started.elapsed().as_secs_f32()
                );

                let heightmap_started = Instant::now();
                report_progress(0.46, "Parsing terrain heightmap");
                let heightmap_data =
                    super::script_loader::parse_heightmap_data_from_chunky(&chunky)
                        .ok()
                        .flatten();
                let blend_tile_data = heightmap_data.as_ref().and_then(|hm| {
                    match super::script_loader::parse_blend_tile_data_from_chunky(&chunky, hm) {
                        Ok(value) => value,
                        Err(err) => {
                            log::warn!("Map '{}' BlendTileData parse failed: {}", map_name, err);
                            None
                        }
                    }
                });
                self.runtime_terrain_texture_classes = blend_tile_data
                    .as_ref()
                    .map(|blend| blend.texture_classes.clone())
                    .unwrap_or_default();
                log::info!(
                    "Map '{}' heightmap parse finished in {:.2}s (heightmap_present={}, blend_tiles_present={})",
                    map_name,
                    heightmap_started.elapsed().as_secs_f32(),
                    heightmap_data.is_some(),
                    blend_tile_data.is_some()
                );

                // Replace the test map with parsed object placements for basic fidelity.
                let settings_started = Instant::now();
                report_progress(0.52, "Reading map settings");
                let parsed = super::script_loader::parse_map_settings_from_chunky(&chunky);
                let parsed_settings = parsed.ok();
                log::info!(
                    "Map '{}' settings parse finished in {:.2}s (present={})",
                    map_name,
                    settings_started.elapsed().as_secs_f32(),
                    parsed_settings.is_some()
                );
                if let Some(meta) = parsed_settings.as_ref() {
                    self.last_map_settings = Some(meta.clone());
                    log::info!(
                        "Map '{}' metadata: objects={}, heightmap_hint={:?}, world_min={:?}, world_max={:?}",
                        map_name,
                        meta.objects.len(),
                        meta.heightmap_path,
                        meta.world_min,
                        meta.world_max
                    );
                    let objects = &meta.objects;
                    if !objects.is_empty() {
                        let named_count = objects.iter().filter(|obj| obj.name.is_some()).count();
                        if named_count > 0 {
                            log::info!(
                                "Map '{}' contains {} named object placements",
                                map_name,
                                named_count
                            );
                        }
                        let object_spawn_started = Instant::now();
                        report_progress(0.58, "Spawning world objects");
                        self.objects.clear();
                        // Build a mapping from map-defined player IDs to teams.
                        let mut map_player_to_team: HashMap<u32, Team> = HashMap::new();
                        for obj in objects {
                            if let Some(pid) = obj.player_id {
                                if let Some(team) =
                                    obj.team_name.as_deref().and_then(Self::team_from_string)
                                {
                                    map_player_to_team.entry(pid).or_insert(team);
                                }
                            }
                        }
                        // Wave 830: seed player→team from skirmish slots when map
                        // team_name strings are missing / unparseable (Lone Eagle).
                        for (pid, player) in &self.players {
                            if *pid == 0 || player.team != Team::Neutral {
                                map_player_to_team.entry(*pid).or_insert(player.team);
                            }
                        }
                        // Common skirmish residual: player 0 human USA, 1 AI GLA.
                        map_player_to_team.entry(0).or_insert(Team::USA);
                        map_player_to_team.entry(1).or_insert(Team::GLA);
                        // Seed players from map ownership only when no skirmish/host
                        // players were already configured. Wiping would destroy
                        // apply_skirmish_config slots/AI on Lone Eagle-style loads.
                        if !map_player_to_team.is_empty() {
                            let preserve_host_players = matches!(
                                self.game_mode,
                                GameMode::Skirmish | GameMode::SinglePlayer
                            ) && !self.players.is_empty();
                            if preserve_host_players {
                                log::info!(
                                    "Preserving {} host player(s) across map load (skirmish/SP config)",
                                    self.players.len()
                                );
                                self.apply_host_players_from_sides_list(false);
                            } else {
                                self.players.clear();
                                for (&pid, &team) in &map_player_to_team {
                                    let is_local = pid == 0;
                                    let name = format!("Player{}", pid + 1);
                                    self.players
                                        .insert(pid, Player::new(pid, team, &name, is_local));
                                }
                                self.apply_host_players_from_sides_list(true);
                            }
                            self.stash_side_builds_on_host(&meta.side_builds);
                        }

                        let mut spawned_object_ids: Vec<(ObjectId, usize)> = Vec::new();
                        let total_objects = objects.len().max(1) as f32;
                        for (index, obj) in objects.iter().enumerate() {
                            if index % 4 == 0 {
                                let t = (index as f32 / total_objects).clamp(0.0, 1.0);
                                report_progress(0.58 + t * 0.20, "Spawning world objects");
                            }
                            if Self::apply_map_object_scorch(obj) {
                                continue;
                            }

                            let team = obj
                                .team_name
                                .as_deref()
                                .and_then(Self::team_from_string)
                                .or_else(|| {
                                    obj.player_id
                                        .and_then(|pid| map_player_to_team.get(&pid).cloned())
                                })
                                .or_else(|| Self::team_from_template_name(obj.template.as_str()))
                                .unwrap_or(Team::Neutral);
                            let mut spawn_position =
                                Vec3::new(obj.position.x, obj.position.z, obj.position.y);
                            if let Some(ground_height) = self.terrain_height_at(Vec3::new(
                                spawn_position.x,
                                0.0,
                                spawn_position.z,
                            )) {
                                // Match C++ map-object placement: map z-offset sits on top of terrain.
                                spawn_position.y += ground_height;
                            }
                            let owner_player_id = obj.player_id.filter(|player_id| {
                                self.players
                                    .get(player_id)
                                    .is_some_and(|player| player.is_alive && player.team == team)
                            });
                            let created = match owner_player_id {
                                Some(player_id) => self.create_object_for_player(
                                    obj.template.as_str(),
                                    player_id,
                                    spawn_position,
                                ),
                                None => {
                                    self.create_object(obj.template.as_str(), team, spawn_position)
                                }
                            };
                            if let Some(id) = created {
                                if let Some(team_name) = obj
                                    .team_name
                                    .as_deref()
                                    .map(str::trim)
                                    .filter(|name| !name.is_empty())
                                {
                                    if let Some(created) = self.objects.get_mut(&id) {
                                        created.team_instance_name = team_name.to_string();
                                    }
                                    self.activate_leftover_team_for_host_object(id);
                                }
                                spawned_object_ids.push((id, index));
                                self.apply_update_obj_values_from_map_properties(
                                    id,
                                    &obj.properties,
                                );
                                if obj.unsellable == Some(true) {
                                    if let Some(created) = self.objects.get_mut(&id) {
                                        created.set_script_unsellable(true);
                                    }
                                }
                                if obj.enabled == Some(false) {
                                    if let Some(created) = self.objects.get_mut(&id) {
                                        created.set_script_disabled(true);
                                    }
                                }
                                if obj.powered == Some(false) {
                                    if let Some(created) = self.objects.get_mut(&id) {
                                        created.set_script_underpowered(true);
                                    }
                                }
                                if let Some(indestructible) = obj.indestructible {
                                    self.set_object_indestructible(id, indestructible);
                                }

                                self.apply_spawned_object_weather(
                                    id,
                                    obj.object_weather.unwrap_or(0),
                                );

                                if let Some(name) =
                                    obj.name.as_deref().map(str::trim).filter(|n| !n.is_empty())
                                {
                                    if let Some(created) = self.objects.get_mut(&id) {
                                        created.name = name.to_string();
                                        created.record_host_identity();
                                    }
                                }
                                self.sync_named_shell_object_into_legacy_runtime(obj, id);
                                if let Some(rot) = obj.rotation {
                                    if let Some(created) = self.objects.get_mut(&id) {
                                        created.set_orientation(rot);
                                    }
                                }
                                if let Some(upgrade) = obj.upgrade.as_deref() {
                                    // ObjectCreationList encodes upgrade/facing hints in a freeform string.
                                    // Apply all upgrades separated by commas/semicolons and treat a numeric-only
                                    // token as a facing override if the chunk omitted rotation.
                                    let mut applied_facing = false;
                                    for token in upgrade.split(&[',', ';'][..]) {
                                        let trimmed = token.trim();
                                        if trimmed.is_empty() {
                                            continue;
                                        }
                                        if !applied_facing && obj.rotation.is_none() {
                                            if let Ok(angle) = trimmed.parse::<f32>() {
                                                if let Some(created) = self.objects.get_mut(&id) {
                                                    created.set_orientation(angle);
                                                }
                                                applied_facing = true;
                                                continue;
                                            }
                                        }
                                        self.apply_upgrade_to_object(id, trimmed);
                                    }
                                }
                            }
                        }
                        report_progress(0.80, "World objects spawned");
                        let spawned_ids = spawned_object_ids;
                        self.spawned_map_object_ids = spawned_ids.clone();
                        self.register_spawned_landmark_bridges(objects, &spawned_ids);
                        // Wave 831: SidesList build-list faction bases (skirmish armies).
                        let side_spawned =
                            self.spawn_side_build_list(&meta.side_builds, &map_player_to_team);
                        if side_spawned > 0 {
                            log::info!(
                                "Spawned {} SidesList build-list objects for '{}'",
                                side_spawned,
                                map_name
                            );
                        }
                        // Wave 831: starting yard+dozer at Player_N_Start waypoints.
                        self.spawn_skirmish_starting_units();
                        report_progress(0.82, "Finalizing world objects");
                        self.ensure_non_shell_player_presence(parsed_settings.as_ref());
                        log::info!(
                            "Spawned {} objects from map placement data for '{}' in {:.2}s",
                            self.objects.len(),
                            map_name,
                            object_spawn_started.elapsed().as_secs_f32()
                        );
                    }
                }
                let bounds_started = Instant::now();
                report_progress(0.84, "Building world bounds");
                let mut bounds_override = parsed_settings.as_ref().and_then(|m| {
                    m.world_min.zip(m.world_max).map(|(min, max)| {
                        (
                            Vec3::new(min.x, min.y, min.z),
                            Vec3::new(max.x, max.y, max.z),
                        )
                    })
                });
                if let Some((min, max)) = bounds_override {
                    let extent_x = (max.x - min.x).abs();
                    let extent_z = (max.z - min.z).abs();
                    if extent_x < 1.0 || extent_z < 1.0 {
                        log::warn!(
                            "Map '{}' reported degenerate bounds ({:.2}x{:.2}); deriving bounds from terrain/object data",
                            map_name,
                            extent_x,
                            extent_z
                        );
                        bounds_override = None;
                    }
                }
                if bounds_override.is_none() {
                    if let Some(hm) = heightmap_data.as_ref() {
                        use gamelogic::common::MAP_XY_FACTOR;
                        let playable_w = (hm.width - 2 * hm.border_size).max(1) as f32;
                        let playable_h = (hm.height - 2 * hm.border_size).max(1) as f32;
                        bounds_override = Some((
                            Vec3::new(0.0, 0.0, 0.0),
                            Vec3::new(playable_w * MAP_XY_FACTOR, 0.0, playable_h * MAP_XY_FACTOR),
                        ));
                    }
                }
                if bounds_override.is_none() && !self.objects.is_empty() {
                    // Derive bounds from placed objects when map metadata is missing.
                    let mut min = Vec3::splat(f32::MAX);
                    let mut max = Vec3::splat(f32::MIN);
                    for obj in self.objects.values() {
                        let pos = obj.get_position();
                        min.x = min.x.min(pos.x);
                        min.y = min.y.min(pos.y);
                        min.z = min.z.min(pos.z);
                        max.x = max.x.max(pos.x);
                        max.y = max.y.max(pos.y);
                        max.z = max.z.max(pos.z);
                    }
                    // Add a small margin to keep camera from clipping edges.
                    let margin = 50.0;
                    min -= Vec3::splat(margin);
                    max += Vec3::splat(margin);
                    bounds_override = Some((min, max));
                }

                if let Some((min, max)) = bounds_override {
                    self.world_min = min;
                    self.world_max = max;
                    self.world_width = (self.world_max.x - self.world_min.x).max(1.0);
                    self.world_height = (self.world_max.z - self.world_min.z).max(1.0);
                    self.pathfinding_system = PathfindingSystem::new_with_origin(
                        self.world_min,
                        self.world_width,
                        self.world_height,
                    );
                    log::info!(
                        "Map '{}' bounds set to min({:.1},{:.1},{:.1}) max({:.1},{:.1},{:.1})",
                        map_name,
                        self.world_min.x,
                        self.world_min.y,
                        self.world_min.z,
                        self.world_max.x,
                        self.world_max.y,
                        self.world_max.z
                    );

                    #[cfg(feature = "game_client")]
                    if let Some(hm) = heightmap_data.as_ref() {
                        use gamelogic::common::{MAP_HEIGHT_SCALE, MAP_XY_FACTOR};
                        let width = hm.width.max(1) as u32;
                        let height = hm.height.max(1) as u32;
                        if hm.data.len() == (width * height) as usize {
                            let max_height = 255.0 * MAP_HEIGHT_SCALE;
                            // C++ MapObject.h MAP_XY_FACTOR=10; WorldHeightMap
                            // stores map-authored border (ZH Alpine = 70).
                            // HeightMap::new defaults scale=1 / border=0; the
                            // visual freeze copies these fields verbatim.
                            let mut heightmap = game_client::terrain::height_map::HeightMap::new(
                                width,
                                height,
                                max_height,
                                MAP_XY_FACTOR,
                            );
                            apply_cpp_heightmap_xy_and_border(&mut heightmap, hm.border_size);
                            heightmap.heights = hm.data.iter().map(|h| *h as f32 / 255.0).collect();
                            if let Some(blend) = blend_tile_data.as_ref() {
                                if blend.tile_ndxes.len() == heightmap.tile_ndxes.len() {
                                    heightmap.tile_ndxes = blend.tile_ndxes.clone();
                                }
                                if blend.blend_tile_ndxes.len() == heightmap.blend_tile_ndxes.len()
                                {
                                    heightmap.blend_tile_ndxes = blend.blend_tile_ndxes.clone();
                                }
                                if blend.extra_blend_tile_ndxes.len()
                                    == heightmap.extra_blend_tile_ndxes.len()
                                {
                                    heightmap.extra_blend_tile_ndxes =
                                        blend.extra_blend_tile_ndxes.clone();
                                }
                                if !blend.blended_tiles.is_empty() {
                                    let mut tiles =
                                        vec![game_client::terrain::textures::BlendTileInfo::new()];
                                    tiles.extend(blend.blended_tiles.iter().map(|t| {
                                        game_client::terrain::textures::BlendTileInfo {
                                            blend_ndx: t.blend_ndx,
                                            horiz: t.horiz,
                                            vert: t.vert,
                                            right_diagonal: t.right_diagonal,
                                            left_diagonal: t.left_diagonal,
                                            inverted: t.inverted,
                                            long_diagonal: t.long_diagonal,
                                            custom_blend_edge_class: t.custom_blend_edge_class,
                                        }
                                    }));
                                    heightmap.assign_blended_tiles(tiles);
                                }
                            }

                            let border = heightmap.border_size.max(0) as u32;
                            self.terrain = Some(super::terrain::TerrainData::from_heightmap(
                                heightmap,
                                self.world_min,
                                self.world_max,
                                border,
                            ));
                            self.copy_crate_water_into_host_terrain();
                            if let Some(meta) = self.last_map_settings.clone() {
                                let spawned_map_object_ids = self.spawned_map_object_ids.clone();
                                self.ground_loaded_map_objects_to_terrain(
                                    &meta.objects,
                                    &spawned_map_object_ids,
                                );
                            }
                            self.seed_pathfinding_from_terrain();
                            self.pathfinding_system
                                .apply_structure_static_blocks(&self.objects);
                        }
                    }
                } else {
                    // Default symmetrical bounds based on current width/height.
                    self.world_min =
                        Vec3::new(-self.world_width * 0.5, 0.0, -self.world_height * 0.5);
                    self.world_max =
                        Vec3::new(self.world_width * 0.5, 0.0, self.world_height * 0.5);
                    self.pathfinding_system = PathfindingSystem::new_with_origin(
                        self.world_min,
                        self.world_width,
                        self.world_height,
                    );
                }

                if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
                    shroud_mgr.init_shroud_grid(self.world_width, self.world_height);
                }
                report_progress(0.88, "Initializing shroud and pathfinding");
                log::info!(
                    "Map '{}' bounds/terrain/shroud hookup finished in {:.2}s",
                    map_name,
                    bounds_started.elapsed().as_secs_f32()
                );
            } else {
                log::error!(
                    "Map '{}' was found at '{}' but could not be decoded as a chunky map",
                    map_name,
                    path.display()
                );
                return false;
            }
        } else {
            // Development-only fallback maps: keep the legacy test layout for demos.
            if matches!(map_name, "TestMap" | "demo_map") {
                log::warn!(
                    "Map '{}' not found on disk; using built-in test layout",
                    map_name
                );
                self.create_test_map();
            } else {
                log::warn!("Map '{}' not found on disk", map_name);
                return false;
            }
        }

        // Terrain hookup: if a heightmap path was discovered next to the map, load it for height
        // queries and derive a first-pass impassability mask for the pathfinding grid.
        #[cfg(feature = "game_client")]
        {
            if self.terrain.is_none() {
                if let Some(heightmap_path) = self.heightmap_hint() {
                    if let Some(path_str) = heightmap_path.to_str() {
                        let loaded = if path_str.ends_with(".hmp") {
                            game_client::terrain::height_map::HeightMap::load_hmp(path_str).ok()
                        } else if path_str.ends_with(".tga") {
                            game_client::terrain::height_map::HeightMap::load_tga(path_str).ok()
                        } else if path_str.ends_with(".raw") {
                            game_client::terrain::height_map::HeightMap::load_raw(path_str).ok()
                        } else {
                            None
                        };

                        if let Some(mut heightmap) = loaded {
                            let border_size = heightmap.border_size;
                            apply_cpp_heightmap_xy_and_border(&mut heightmap, border_size);
                            let border = heightmap.border_size.max(0) as u32;
                            let terrain = super::terrain::TerrainData::from_heightmap(
                                heightmap,
                                self.world_min,
                                self.world_max,
                                border,
                            );
                            self.terrain = Some(terrain);
                            self.copy_crate_water_into_host_terrain();
                            if let Some(meta) = self.last_map_settings.clone() {
                                let spawned_map_object_ids = self.spawned_map_object_ids.clone();
                                self.ground_loaded_map_objects_to_terrain(
                                    &meta.objects,
                                    &spawned_map_object_ids,
                                );
                            }
                            self.seed_pathfinding_from_terrain();
                            self.pathfinding_system
                                .apply_structure_static_blocks(&self.objects);
                        } else {
                            log::warn!("Failed to load heightmap '{}'", path_str);
                        }
                    }
                }
            }
        }

        let scripts_started = Instant::now();
        report_progress(0.92, "Initializing mission scripts");
        self.initialize_scripts(map_name);
        if matches!(self.game_mode, GameMode::Skirmish | GameMode::Replay) {
            // C++ startNewGame adds ReplayObserver after sides, then installs
            // MultiplayerScripts.scb (numTeams>1) and permanently reveals the map.
            self.install_replay_observer_side();
            self.install_multiplayer_scripts_if_needed();
            self.reveal_replay_observer_map();
        }
        if matches!(self.game_mode, GameMode::Replay) {
            self.apply_replay_observer_as_local_player();
        }
        log::info!(
            "Map '{}' script init finished in {:.2}s",
            map_name,
            scripts_started.elapsed().as_secs_f32()
        );

        // Skirmish/SP: map spawn clears world objects. Rebind host AI (stale
        // object/factory refs, rebuild budget) and re-ensure GLA_*/faction templates
        // without wiping players, cash, difficulty, or is_active.
        if matches!(self.game_mode, GameMode::Skirmish | GameMode::SinglePlayer) {
            self.rebind_host_ai_after_map_load();
            // C++ GameLogic.cpp placeObjectAtPosition loop for PlayerTemplate StartingUnitN.
            // Without this, Lone Eagle-style maps keep buildings but no dozers/workers.
            self.spawn_skirmish_starting_units();
        }

        // C++ TerrainLogic.cpp:2589 TheRadar->newMap after bounds, then
        // re-add live objects. Stay off sides_list / polygon.
        self.host_radar_on_map_loaded();

        self.map_loaded = true;
        // C++ start-of-match residual: reveal FOW around loaded units/structures
        // immediately so build placement / presentation FOW are not stuck LBC_SHROUD
        // until the first logic tick. Same XZ→shroud mapping as update path.
        // Wave 827: under coupled shadow, host system residuals sole-tick after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_main_crate_vision();
        }
        report_progress(0.96, "Map load complete");
        log::info!(
            "Map loaded successfully in {:.2}s",
            load_started.elapsed().as_secs_f32()
        );
        true
    }

    /// Load a map without external progress reporting.
    pub fn load_map(&mut self, map_name: &str) -> bool {
        self.load_map_with_progress(map_name, |_progress, _phase| {})
    }

    /// Load a requested map, then an explicit fallback, returning the identity
    /// that actually loaded. `None` means neither attempt produced a playable
    /// map; it is deliberately not an alias for the fallback name.
    #[inline]
    pub fn load_map_or_fallback(&mut self, map_name: &str, fallback: &str) -> Option<String> {
        self.load_map_or_fallback_with_progress(map_name, fallback, |_progress, _phase| {})
    }

    /// Load a requested map, then an explicit fallback, while exposing the
    /// real map-load milestones from each attempt. The fallback order and
    /// successful-tail requirement intentionally match `load_map_or_fallback`.
    #[inline]
    pub fn load_map_or_fallback_with_progress<F>(
        &mut self,
        map_name: &str,
        fallback: &str,
        mut report_progress: F,
    ) -> Option<String>
    where
        F: FnMut(f32, &str),
    {
        if self.load_map_with_progress(map_name, |progress, phase| report_progress(progress, phase))
            && self.map_loaded
        {
            return Some(self.map_name.clone());
        }

        // Do not pretend a second attempt occurred when the requested identity
        // already was the fallback. More importantly, never report a fallback
        // name unless that attempt really reached the successful load tail.
        if map_name != fallback
            && self.load_map_with_progress(fallback, |progress, phase| {
                report_progress(progress, phase)
            })
            && self.map_loaded
        {
            return Some(self.map_name.clone());
        }

        // No world is available. Clear the transient identity set at the start
        // of either failed attempt so UI/save/render callers cannot describe an
        // unloaded map as the active match.
        self.map_name.clear();
        self.map_loaded = false;
        None
    }
}

pub(in super::super) fn leftover_template_is_landmark_bridge(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.contains("landmarkbridge") || n.contains("landmark_bridge") {
        return true;
    }
    let Some(guard) = game_engine::common::thing::thing_factory::try_get_thing_factory() else {
        return false;
    };
    let Some(factory) = guard.as_ref() else {
        return false;
    };
    let Some(tmpl) = factory.find_template(template_name, false) else {
        return false;
    };
    tmpl.is_bridge()
        || tmpl.is_kind_of_mask(
            game_engine::common::system::kind_of::KindOfMask::LANDMARK_BRIDGE.bits(),
        )
}

pub(in super::super) fn landmark_bridge_half_sizes(
    template_name: &str,
    obj: &Object,
) -> (f32, f32) {
    if let Some(guard) = game_engine::common::thing::thing_factory::try_get_thing_factory() {
        if let Some(factory) = guard.as_ref() {
            if let Some(tmpl) = factory.find_template(template_name, false) {
                let geometry = tmpl.get_template_geometry_info();
                let half_x = geometry.major_radius();
                let half_y = geometry.minor_radius();
                if half_x > 0.0 && half_y > 0.0 {
                    return (half_x, half_y);
                }
            }
        }
    }
    let geom = &obj.thing.template.geometry_info;
    if geom.authored && geom.major_radius > 0.0 && geom.minor_radius > 0.0 {
        return (geom.major_radius, geom.minor_radius);
    }
    let radius = obj.selection_radius.max(20.0);
    (radius, (radius * 0.25).max(8.0))
}

pub(in super::super) fn leftover_bridge_info_for_object(
    object_id: u32,
) -> Option<gamelogic::terrain::BridgeInfo> {
    let terrain = gamelogic::terrain::get_terrain_logic().read().ok()?;
    let mut found = None;
    terrain.for_each_bridge(|bridge| {
        if bridge.get_bridge_info().bridge_object_id == object_id {
            found = Some(bridge.get_bridge_info().clone());
        }
    });
    found
}

pub(in super::super) fn leftover_bridge_template_name(object_id: u32) -> Option<String> {
    let terrain = gamelogic::terrain::get_terrain_logic().read().ok()?;
    let mut found = None;
    terrain.for_each_bridge(|bridge| {
        if bridge.get_bridge_info().bridge_object_id == object_id {
            found = Some(bridge.get_bridge_template_name().as_str().to_string());
        }
    });
    found
}

/// Presentation identity for C++ `W3DBridge::load` (`W3DBridgeBuffer.cpp:182-191`).
/// Unit-separated so map template names stay unambiguous.
pub(crate) fn encode_authored_bridge_visual(
    template_name: &str,
    model_name: &str,
    scale: f32,
    towers: [&str; 4],
) -> String {
    format!(
        "AUTHBR\u{1f}{template_name}\u{1f}{model_name}\u{1f}{scale}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        towers[0], towers[1], towers[2], towers[3]
    )
}

/// C++ PlayerList.cpp:167-199.
pub(in super::super) fn apply_logic_player_list_relationships(
    logic_list: &mut LogicPlayerList,
    side_dicts: &[Dict],
) {
    let mut name_to_index: HashMap<String, i32> = HashMap::new();
    for dict in side_dicts {
        let name = dict.get_ascii_string(key_player_name());
        if name.is_empty() {
            continue;
        }
        if let Some(player) = logic_list.find_player_by_name(&name) {
            if let Ok(guard) = player.read() {
                name_to_index.insert(name, guard.get_player_index());
            }
        }
    }
    for dict in side_dicts {
        let name = dict.get_ascii_string(key_player_name());
        let Some(&index) = name_to_index.get(&name) else {
            continue;
        };
        let Some(player_arc) = logic_list.get_player(index) else {
            continue;
        };
        let Ok(mut player) = player_arc.write() else {
            continue;
        };
        player.set_player_relationship_by_index(index, gamelogic::common::Relationship::Allies);
        if index != 0 {
            player.set_player_relationship_by_index(0, gamelogic::common::Relationship::Neutral);
        }
        for token in dict
            .get_ascii_string(key_player_enemies())
            .split_whitespace()
        {
            if let Some(&enemy) = name_to_index.get(token) {
                player.set_player_relationship_by_index(
                    enemy,
                    gamelogic::common::Relationship::Enemies,
                );
            }
        }
        for token in dict
            .get_ascii_string(key_player_allies())
            .split_whitespace()
        {
            if let Some(&ally) = name_to_index.get(token) {
                player.set_player_relationship_by_index(
                    ally,
                    gamelogic::common::Relationship::Allies,
                );
            }
        }
    }
}

pub(in super::super) fn load_multiplayer_scripts_scb() -> Option<ScriptList> {
    use game_engine::common::system::DataChunkInput;
    use game_engine::common::system::file::FileAccess;
    use game_engine::common::system::file_system::get_file_system;
    use gamelogic::scripting::core::{ScriptListReadInfo, parse_player_scripts_list_chunk};

    const VIRTUAL: &str = "Data\\Scripts\\MultiplayerScripts.scb";
    let data = {
        let fs = get_file_system();
        let mut guard = fs.lock().ok()?;
        let mut file = guard.open_file(VIRTUAL, FileAccess::READ.combine(FileAccess::BINARY))?;
        file.read_entire_and_close().ok()
    }
    .or_else(|| {
        const CANDIDATES: &[&str] = &[
            "windows_game/Command & Conquer Generals Zero Hour/Data/Scripts/MultiplayerScripts.scb",
            "Data/Scripts/MultiplayerScripts.scb",
        ];
        CANDIDATES.iter().find_map(|path| std::fs::read(path).ok())
    })?;
    let mut input = DataChunkInput::new(data);
    let mut read_info = ScriptListReadInfo::default();
    input.register_parser("PlayerScriptsList", "", parse_player_scripts_list_chunk);
    if !input.parse(&mut read_info) {
        return None;
    }
    read_info.lists.into_iter().next().map(|boxed| *boxed)
}

pub(in super::super) fn clear_live_campaign_victorious_for_new_game() {
    if let Ok(mut guard) = gamelogic::scripting::engine::get_script_engine().write() {
        if let Some(engine) = guard.as_mut() {
            engine.set_campaign_victorious(false);
        }
    }
}

#[cfg(feature = "game_client")]
pub(in super::super) fn apply_cpp_heightmap_xy_and_border(
    heightmap: &mut game_client::terrain::height_map::HeightMap,
    map_border: i32,
) {
    use gamelogic::common::MAP_XY_FACTOR;
    // C++ MapObject.h MAP_XY_FACTOR; WorldHeightMap::m_borderSize else ZH 70.
    heightmap.scale = MAP_XY_FACTOR;
    heightmap.border_size = if map_border > 0 { map_border } else { 70 };
}
