//! Additional `impl GameLogic` methods. Child of `game_logic.rs`.
#![allow(unused_imports, non_snake_case)]
use super::*;

impl GameLogic {
    pub fn weather_state(&self) -> &RuntimeWeatherState {
        &self.weather_state
    }

    pub fn set_weather_state(
        &mut self,
        current_weather: impl Into<String>,
        intensity: f32,
        duration_remaining: f32,
        next_change_time: f32,
    ) {
        let mut weather = current_weather.into();
        weather = weather.trim().to_string();
        if weather.is_empty() {
            weather = "clear".to_string();
        }

        self.weather_state.current_weather = weather;
        self.weather_state.intensity = intensity.clamp(0.0, 1.0);
        self.weather_state.duration_remaining = duration_remaining.max(0.0);
        self.weather_state.next_change_time = next_change_time.max(0.0);
    }

    pub fn set_weather_visible(&mut self, visible: bool) {
        self.weather_state.visible = visible;
    }

    pub fn queue_pending_special_ability(
        &mut self,
        object_id: ObjectId,
        ability: PendingSpecialAbility,
    ) {
        self.pending_special_abilities.insert(object_id, ability);
    }

    pub fn clear_pending_special_ability(&mut self, object_id: ObjectId) {
        self.pending_special_abilities.remove(&object_id);
    }

    /// C++ GameLogic.cpp:1436-1459 always adds a ReplayObserver side/team.
    pub fn ensure_replay_observer_player(&mut self) -> u32 {
        if let Some(existing) = self.replay_observer_player_id {
            if self
                .players
                .get(&existing)
                .is_some_and(|p| p.name == "ReplayObserver")
            {
                return existing;
            }
        }
        if let Some((&id, _)) = self
            .players
            .iter()
            .find(|(_, p)| p.name == "ReplayObserver")
        {
            self.replay_observer_player_id = Some(id);
            return id;
        }
        let id = self
            .players
            .keys()
            .copied()
            .max()
            .map(|max| max.saturating_add(1))
            .unwrap_or(0);
        let mut observer = Player::new(id, Team::Neutral, "ReplayObserver", false);
        observer.is_alive = false;
        observer.start_position = 0;
        observer.alliance_team = -1;
        observer.color_rgb = (170, 170, 170);
        self.add_player(observer);
        if let Some(identity) = PlayerTemplateIdentity::from_exact_name("FactionObserver") {
            let _ = self.bind_player_template_identity(id, identity);
            if let Some(player) = self.players.get_mut(&id) {
                player.is_alive = false;
                player.name = "ReplayObserver".into();
                player.team = Team::Neutral;
            }
        }
        self.replay_observer_player_id = Some(id);
        id
    }

    /// C++ GameLogic.cpp:1703-1705 permanent reveal for ReplayObserver.
    pub fn reveal_replay_observer_map(&mut self) {
        let Some(id) = self.replay_observer_player_id.or_else(|| {
            self.players
                .iter()
                .find(|(_, p)| p.name == "ReplayObserver")
                .map(|(&id, _)| id)
        }) else {
            return;
        };
        if let Ok(mut shroud) = get_shroud_manager().lock() {
            let _ = shroud.reveal_map_for_player_permanently(id);
        }
    }

    /// C++ GameLogic.cpp:1436-1459 + PlayerList::newGame for ReplayObserver.
    pub fn install_replay_observer_side(&mut self) {
        let host_id = self.ensure_replay_observer_player();
        let mut d = Dict::new();
        d.set_ascii_string(key_player_name(), "ReplayObserver");
        d.set_bool(key_player_is_human(), true);
        d.set_unicode_string(key_player_display_name(), "Observer");
        d.set_ascii_string(key_player_faction(), "FactionObserver");
        d.set_ascii_string(key_player_allies(), String::new());
        d.set_ascii_string(key_player_enemies(), String::new());
        d.set_int(key_multiplayer_start_index(), 0);

        if let Ok(mut sides) = get_sides_list().write() {
            if sides.find_side_info("ReplayObserver").is_none() {
                sides.add_side(&d);
            }
            if sides.find_team_info("teamReplayObserver").is_none() {
                let mut team = Dict::new();
                team.set_ascii_string(key_team_name(), "teamReplayObserver");
                team.set_ascii_string(key_team_owner(), "ReplayObserver");
                team.set_bool(key_team_is_singleton(), true);
                sides.add_team(&team);
            }
        }

        if let Ok(list) = ThePlayerList().read() {
            if list.find_player_by_name("ReplayObserver").is_some() {
                return;
            }
        }

        let mut logic_player = LogicPlayer::new(host_id as i32);
        logic_player.set_player_name_key(NameKeyGenerator::name_to_key("ReplayObserver"));
        logic_player.set_display_name("Observer");
        logic_player.set_side("Observer");
        logic_player.set_base_side("Observer");
        logic_player.set_observer(true);
        logic_player.set_player_type(LogicPlayerType::Observer, false);
        game_engine::common::ini::ensure_player_templates_loaded();
        if let Some(common) = get_player_template_store().find_template("FactionObserver") {
            logic_player.init(std::sync::Arc::new(LogicPlayerTemplate::from_common(common)));
            logic_player.set_observer(true);
            logic_player.set_player_type(LogicPlayerType::Observer, false);
        }
        if let Ok(mut list) = ThePlayerList().write() {
            list.add_player(std::sync::Arc::new(std::sync::RwLock::new(logic_player)));
        }
    }

    /// C++ GameLogic.cpp:1479-1532 — MultiplayerScripts.scb when numTeams > 1.
    pub fn install_multiplayer_scripts_if_needed(&mut self) {
        if !self.install_multiplayer_scripts {
            return;
        }
        let Some(scripts) = load_multiplayer_scripts_scb() else {
            return;
        };
        let mut next = scripts.get_script();
        if next.is_none() {
            return;
        }
        if let Ok(mut sides) = get_sides_list().write() {
            if let Some(side) = sides.get_side_info_mut(0) {
                let mut dest = side.get_script_list().cloned().unwrap_or_else(ScriptList::new);
                while let Some(script) = next {
                    dest.add_script(Box::new(script.clone()), 0);
                    next = script.get_next();
                }
                side.set_script_list(Some(Box::new(dest)));
            }
        }
        if self.loaded_script_lists.is_empty() {
            self.loaded_script_lists.push(scripts);
        } else {
            let dest = &mut self.loaded_script_lists[0];
            let mut next = scripts.get_script();
            while let Some(script) = next {
                dest.add_script(Box::new(script.clone()), 0);
                next = script.get_next();
            }
        }
        self.mission_scripts
            .install_lists(&self.loaded_script_lists);
        if let Ok(mut engine_guard) = gamelogic::scripting::engine::get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                for (idx, list) in self.loaded_script_lists.iter().enumerate() {
                    let _ = engine.set_script_list_for_player(idx, Some(Box::new(list.clone())));
                }
            }
        }
    }

    pub fn replay_observer_player_id(&self) -> Option<u32> {
        self.replay_observer_player_id
    }

    pub fn will_install_multiplayer_scripts(&self) -> bool {
        self.install_multiplayer_scripts
    }

    pub fn set_install_multiplayer_scripts(&mut self, install: bool) {
        self.install_multiplayer_scripts = install;
    }

    pub fn loaded_script_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for list in &self.loaded_script_lists {
            let mut cur = list.get_script();
            while let Some(script) = cur {
                names.push(script.get_name().to_string());
                cur = script.get_next();
            }
        }
        names
    }



    pub fn has_loaded_multiplayer_script(&self, name: &str) -> bool {
        self.loaded_script_lists.iter().any(|list| {
            let mut cur = list.get_script();
            while let Some(script) = cur {
                if script.get_name() == name {
                    return true;
                }
                cur = script.get_next();
            }
            false
        })
    }


    pub fn terrain_height_at(&self, world_pos: Vec3) -> Option<f32> {
        #[cfg(feature = "game_client")]
        {
            if let Some(h) = self.terrain.as_ref().map(|t| t.height_at_world(world_pos)) {
                return Some(h);
            }
        }
        // Coarse pathfinding height cache residual (save/load + synthetic maps).
        let cache = self.pathfinding_height_samples.as_ref()?;
        let width = self.pathfinding_system.grid.width().max(0) as u32;
        let height = self.pathfinding_system.grid.height().max(0) as u32;
        if cache.width != width || cache.height != height || width == 0 || height == 0 {
            return None;
        }
        let cell = self.pathfinding_system.grid.world_to_grid(world_pos);
        if cell.x < 0 || cell.y < 0 || cell.x >= width as i32 || cell.y >= height as i32 {
            return None;
        }
        let idx = (cell.y as u32 * width + cell.x as u32) as usize;
        cache.values.get(idx).copied()
    }

    #[cfg(feature = "game_client")]
    pub fn terrain_heightmap_snapshot(
        &self,
    ) -> Option<game_client::terrain::height_map::HeightMap> {
        self.terrain
            .as_ref()
            .map(|terrain| terrain.heightmap_clone())
    }

    /// Snapshot map bridge spans converted to runtime world-space vectors for visual road parity.
    pub fn terrain_bridge_segments_snapshot(&self) -> Vec<(Vec3, Vec3, f32, String)> {
        let Ok(terrain) = gamelogic::terrain::get_terrain_logic().read() else {
            return Vec::new();
        };
        terrain
            .bridge_data_snapshot()
            .into_iter()
            .map(|bridge| {
                (
                    Vec3::new(bridge.from.x, bridge.from.z, bridge.from.y),
                    Vec3::new(bridge.to.x, bridge.to.z, bridge.to.y),
                    bridge.width,
                    bridge.template_name,
                )
            })
            .collect()
    }

    /// Snapshot map road spans parsed from map-object ROAD_POINT flags.
    pub fn terrain_road_segments_snapshot(&self) -> Vec<super::script_loader::RuntimeRoadSegment> {
        self.runtime_road_segments.clone()
    }

    pub fn terrain_texture_classes_snapshot(
        &self,
    ) -> Vec<super::script_loader::BlendTileTextureClass> {
        self.runtime_terrain_texture_classes.clone()
    }

    /// Export terrain/pathing passability as a compact grid mask for save/load parity.
    pub fn snapshot_pathfinding_passability(&self) -> (u32, u32, Vec<bool>) {
        let width = self.pathfinding_system.grid.width().max(0) as u32;
        let height = self.pathfinding_system.grid.height().max(0) as u32;
        let mask = self.pathfinding_system.grid.export_static_block_mask();
        (width, height, mask)
    }

    /// Restore terrain/pathing passability from a saved grid mask.
    pub fn restore_pathfinding_passability(
        &mut self,
        width: u32,
        height: u32,
        mask: &[bool],
    ) -> bool {
        if width == 0 || height == 0 {
            return false;
        }

        self.pathfinding_system
            .grid
            .import_static_block_mask(width as i32, height as i32, mask)
    }

    /// Sample terrain heights into the current pathfinding grid resolution for save/load parity.
    pub fn snapshot_terrain_heights_for_path_grid(&self) -> Option<Vec<f32>> {
        #[cfg(feature = "game_client")]
        {
            let terrain = self.terrain.as_ref()?;
            let width = self.pathfinding_system.grid.width().max(0);
            let height = self.pathfinding_system.grid.height().max(0);
            if width == 0 || height == 0 {
                return None;
            }

            let grid_size = self.pathfinding_system.grid.grid_size();
            let origin = self.pathfinding_system.grid.origin();
            let mut samples = Vec::with_capacity((width * height) as usize);
            for y in 0..height {
                for x in 0..width {
                    let pos = Vec3::new(
                        origin.x + (x as f32 + 0.5) * grid_size,
                        0.0,
                        origin.z + (y as f32 + 0.5) * grid_size,
                    );
                    samples.push(terrain.height_at_world(pos));
                }
            }
            Some(samples)
        }
        #[cfg(not(feature = "game_client"))]
        {
            let cache = self.pathfinding_height_samples.as_ref()?;
            let width = self.pathfinding_system.grid.width().max(0) as u32;
            let height = self.pathfinding_system.grid.height().max(0) as u32;

            (cache.width == width && cache.height == height).then_some(cache.values.clone())
        }
    }

    /// Restore coarse terrain heights from a grid snapshot (used to recover post-load height queries).
    pub fn restore_terrain_heights_from_grid(
        &mut self,
        width: u32,
        height: u32,
        heights: &[f32],
    ) -> bool {
        let expected_len = (width as usize).saturating_mul(height as usize);
        if width == 0 || height == 0 || heights.len() != expected_len {
            return false;
        }

        self.pathfinding_height_samples = Some(PathfindingHeightSamples {
            width,
            height,
            values: heights.to_vec(),
        });

        #[cfg(feature = "game_client")]
        {
            let max_height = heights.iter().copied().fold(0.0_f32, f32::max).max(1.0_f32);
            let mut heightmap =
                game_client::terrain::height_map::HeightMap::new(width, height, max_height, 1.0);

            for (dst, src) in heightmap.heights.iter_mut().zip(heights.iter().copied()) {
                *dst = (src / max_height).clamp(0.0, 1.0);
            }

            let terrain = super::terrain::TerrainData::from_heightmap(
                heightmap,
                self.world_min,
                self.world_max,
                0,
            );
            self.terrain = Some(terrain);
            self.seed_pathfinding_from_terrain();
            self.pathfinding_system
                .apply_structure_static_blocks(&self.objects);
            true
        }
        #[cfg(not(feature = "game_client"))]
        {
            true
        }
    }

    /// Re-apply structure footprints onto the static path/LOS grid.
    /// Call after map object spawn bulk and when a structure dies.
    pub fn sync_structure_path_blocks(&mut self) {
        #[cfg(feature = "game_client")]
        let had_terrain = self.terrain.is_some();
        #[cfg(not(feature = "game_client"))]
        let had_terrain = false;
        if had_terrain {
            self.seed_pathfinding_from_terrain();
            self.pathfinding_system
                .apply_structure_static_blocks(&self.objects);
        } else {
            self.pathfinding_system.clear_static_blocks();
        }
        self.pathfinding_system
            .apply_structure_static_blocks(&self.objects);
    }

    /// Block one constructed structure footprint without full rebuild.
    pub(super) fn block_structure_object_path(&mut self, object_id: ObjectId) {
        let Some(obj) = self.objects.get(&object_id) else {
            return;
        };
        if !obj.is_kind_of(KindOf::Structure) || !obj.is_alive() || obj.status.under_construction {
            return;
        }
        let pos = obj.get_position();
        let radius = obj.selection_radius;
        let gs = self.pathfinding_system.grid.grid_size();
        let r = ((radius / gs.max(1.0)).ceil() as i32).max(1).min(4);
        self.pathfinding_system.block_structure_at_world(pos, r);
    }

    pub fn set_pathfinding_static_block(&mut self, x: i32, y: i32, blocked: bool) {
        self.pathfinding_system
            .grid
            .set_blocked(super::pathfinding::GridPos::new(x, y), blocked);
    }

    pub fn is_pathfinding_static_blocked(&self, x: i32, y: i32) -> bool {
        self.pathfinding_system
            .grid
            .is_static_blocked(super::pathfinding::GridPos::new(x, y))
    }

    pub(super) fn seed_pathfinding_from_terrain(&mut self) {
        #[cfg(feature = "game_client")]
        {
            let Some(terrain) = self.terrain.as_ref() else {
                return;
            };

            // Reset static blocks to the terrain-derived mask each map load.
            self.pathfinding_system.clear_static_blocks();

            // C++ Pathfinder::classifyMap residual (AIPathfind.h:233-242):
            // stamp CELL_WATER / CELL_CLIFF / CELL_IMPASSABLE. Water and cliff
            // stay walk-costed; only extreme slope / OOB is IMPASSABLE.
            // Incomplete heightmaps that over-classify cliffs still fail-open
            // the IMPASSABLE mask (hq-hneu: do not reopen that fail-open).
            const MAX_SLOPE: f32 = 4.0; // only block cliffs-ish grades
            let grid_size = self.pathfinding_system.grid.grid_size();
            let grid_origin = self.pathfinding_system.grid.origin();

            let (min, max) = terrain.world_bounds();
            let min_x = min.x;
            let min_z = min.z;
            let max_x = max.x;
            let max_z = max.z;

            let width = self.pathfinding_system.grid.width();
            let height = self.pathfinding_system.grid.height();
            let mut blocked_slopes = 0u32;
            let mut total_cells = 0u32;
            for y in 0..height {
                for x in 0..width {
                    total_cells += 1;
                    let center = Vec3::new(
                        grid_origin.x + (x as f32 + 0.5) * grid_size,
                        0.0,
                        grid_origin.z + (y as f32 + 0.5) * grid_size,
                    );
                    let pos = super::pathfinding::GridPos::new(x, y);

                    if center.x < min_x || center.x > max_x || center.z < min_z || center.z > max_z
                    {
                        self.pathfinding_system
                            .grid
                            .set_cell_type(pos, gamelogic::ai::pathfind_astar::PathfindCellType::Impassable);
                        continue;
                    }

                    if terrain.is_underwater_at_world(center) {
                        self.pathfinding_system
                            .grid
                            .set_cell_type(pos, gamelogic::ai::pathfind_astar::PathfindCellType::Water);
                    }

                    let slope = terrain.slope_at_world(center);
                    let cliff = terrain.is_cliff_at_world(center);
                    if slope > MAX_SLOPE {
                        blocked_slopes += 1;
                        self.pathfinding_system
                            .grid
                            .set_cell_type(pos, gamelogic::ai::pathfind_astar::PathfindCellType::Impassable);
                    } else if cliff {
                        self.pathfinding_system
                            .grid
                            .set_cell_type(pos, gamelogic::ai::pathfind_astar::PathfindCellType::Cliff);
                    }
                }
            }

            // If the slope heuristic blocked most of the map, terrain data is incomplete —
            // clear slope blocks and keep only out-of-bounds so infantry can still march.
            if total_cells > 0 && blocked_slopes as f32 / total_cells as f32 > 0.35 {
                log::warn!(
                    "Pathfinding slope mask blocked {:.0}% of cells; clearing static blocks (terrain incomplete)",
                    100.0 * blocked_slopes as f32 / total_cells as f32
                );
                self.pathfinding_system.clear_static_blocks();
                for y in 0..height {
                    for x in 0..width {
                        let center = Vec3::new(
                            grid_origin.x + (x as f32 + 0.5) * grid_size,
                            0.0,
                            grid_origin.z + (y as f32 + 0.5) * grid_size,
                        );
                        if center.x < min_x
                            || center.x > max_x
                            || center.z < min_z
                            || center.z > max_z
                        {
                            self.pathfinding_system.grid.set_cell_type(
                                super::pathfinding::GridPos::new(x, y),
                                gamelogic::ai::pathfind_astar::PathfindCellType::Impassable,
                            );
                        }
                    }
                }
            }
            self.stamp_live_bridge_decks_and_zones();
        }
    }

    /// C++ addBridge classifyCells + classifyMap pinch + zone rebuild on the live host grid.
    pub(super) fn stamp_live_bridge_decks_and_zones(&mut self) {
        self.pathfinding_system.grid.pinch_tighten_cliffs();
        if let Ok(terrain) = gamelogic::terrain::get_terrain_logic().read() {
            terrain.for_each_bridge(|bridge| {
                let info = bridge.get_bridge_info();
                let destroyed = info.cur_damage_state
                    == gamelogic::common::BodyDamageType::Rubble;
                // C++ Coord3D ground is XY; host path grid is XZ.
                self.pathfinding_system.grid.stamp_bridge_deck(
                    Vec3::new(info.from_left.x, 0.0, info.from_left.y),
                    Vec3::new(info.from_right.x, 0.0, info.from_right.y),
                    Vec3::new(info.to_left.x, 0.0, info.to_left.y),
                    Vec3::new(info.to_right.x, 0.0, info.to_right.y),
                    destroyed,
                );
            });
        }
        self.pathfinding_system.grid.rebuild_terrain_zones();
    }

    /// C++ AIFollowWaypointPathExact residual — use waypoints as-is (no A* smoothing).
    pub fn assign_unit_path_exact(
        &mut self,
        unit_id: ObjectId,
        destination: Vec3,
        waypoints: &[Vec3],
    ) -> bool {
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            if unit.is_deployed() {
                unit.set_deployed(false);
            }
        }
        let can_move = match self.objects.get(&unit_id) {
            Some(unit) => unit.is_alive() && unit.can_move(),
            None => return false,
        };
        if !can_move {
            return false;
        }
        let mut full_path: Vec<Vec3> = Vec::with_capacity(waypoints.len() + 1);
        for wp in waypoints {
            if !wp.x.is_finite() || !wp.z.is_finite() {
                continue;
            }
            if let Some(last) = full_path.last() {
                let dx = last.x - wp.x;
                let dz = last.z - wp.z;
                if dx * dx + dz * dz < 0.01 {
                    continue;
                }
            }
            full_path.push(*wp);
        }
        if let Some(last) = full_path.last() {
            let dx = last.x - destination.x;
            let dz = last.z - destination.z;
            if dx * dx + dz * dz >= 0.01 {
                full_path.push(destination);
            }
        } else {
            full_path.push(destination);
        }
        if full_path.is_empty() {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            unit.waiting_for_path = false;
            unit.movement.current_path_index = 0;
            unit.movement.path = full_path;
            unit.movement.target_position = unit.movement.path.first().copied();
            unit.is_exact_path = true;
            unit.set_ai_state(AIState::Moving);
            true
        } else {
            false
        }
    }

    pub fn assign_unit_path(
        &mut self,
        unit_id: ObjectId,
        destination: Vec3,
        waypoints: &[Vec3],
    ) -> bool {
        self.assign_unit_path_inner(unit_id, destination, waypoints, false)
    }

    #[cfg(test)]
    pub fn force_map_loaded_for_path_test(&mut self, loaded: bool) {
        self.map_loaded = loaded;
    }

    fn assign_unit_path_inner(
        &mut self,
        unit_id: ObjectId,
        destination: Vec3,
        waypoints: &[Vec3],
        compute_now: bool,
    ) -> bool {
        // C++ DeployStyle: move order packs unit before pathing residual.
        let mut started_undeploy = false;
        let mut block_path = false;
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            if let Some(ds) = unit.deploy_style.as_mut() {
                if !ds.is_ready_to_move() {
                    if ds.begin_undeploy(self.frame) {
                        started_undeploy = true;
                    }
                    unit.set_deployed(false);
                    unit.stop_moving();
                    block_path = true;
                }
            } else if unit.is_deployed() {
                unit.set_deployed(false);
            }
        }
        if started_undeploy {
            self.deploy_style_reg.record_undeploy();
        }
        if block_path {
            self.deploy_style_reg.record_blocked_move();
            // Path blocked until pack completes; re-issue move after ReadyToMove.
            return false;
        }
        let (start, can_move, is_aircraft, surfaces) = match self.objects.get(&unit_id) {
            Some(unit) => (
                unit.get_position(),
                unit.can_move(),
                unit.is_kind_of(crate::game_logic::KindOf::Aircraft)
                    || unit.object_type == crate::game_logic::ObjectType::Aircraft,
                unit.locomotor_surfaces,
            ),
            None => return false,
        };
        if !can_move {
            return false;
        }

        // C++ Pathfinder::queueForPath: loaded maps wait one frame
        // (AI.cpp:332-339, AIPathfind.h:418). Mapless / test compute now.
        let defer = self.map_loaded && !compute_now;
        if defer {
            if let Some(unit) = self.objects.get_mut(&unit_id) {
                unit.waiting_for_path = true;
                unit.movement.target_position = Some(destination);
                unit.set_ai_state(AIState::Moving);
                unit.set_status_moving(true);
                let mut dir = destination - start;
                dir.y = 0.0;
                let dir = dir.normalize_or_zero();
                unit.movement.velocity = dir * unit.movement.max_speed;
                unit.record_host_movement();
            }
            self.pathfinding_system
                .queue_path(super::pathfinding::PendingHostPath {
                    unit_id,
                    start,
                    destination,
                    waypoints: waypoints.to_vec(),
                    aircraft: is_aircraft,
                    surfaces,
                });
            crate::game_logic::host_move_log::record(
                unit_id,
                Some([destination.x, destination.y, destination.z]),
            );
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 1);
            }
            return true;
        }

        let Some(full_path) =
            self.compute_assigned_unit_path(unit_id, start, destination, waypoints, is_aircraft, surfaces)
        else {
            return false;
        };
        self.apply_computed_unit_path(unit_id, start, destination, full_path)
    }

    fn compute_assigned_unit_path(
        &mut self,
        unit_id: ObjectId,
        start: Vec3,
        destination: Vec3,
        waypoints: &[Vec3],
        is_aircraft: bool,
        surfaces: u32,
    ) -> Option<Vec<Vec3>> {
        let horiz = |a: Vec3, b: Vec3| {
            let dx = a.x - b.x;
            let dz = a.z - b.z;
            (dx * dx + dz * dz).sqrt()
        };

        let mut goals: Vec<Vec3> = waypoints.to_vec();
        goals.push(destination);

        let mut full_path: Vec<Vec3> = Vec::new();
        let mut segment_start = start;
        let loco = if is_aircraft {
            gamelogic::ai::pathfind_complete::SURFACE_AIR
        } else if surfaces != 0 {
            surfaces
        } else {
            gamelogic::ai::pathfind_complete::SURFACE_GROUND
        };
        for goal in goals {
            if horiz(segment_start, goal) < 0.1 {
                segment_start = goal;
                continue;
            }

            // Never fail-open through blocked cells: always ask the pathfinder.
            let straight = horiz(segment_start, goal);
            let segment = self.pathfinding_system.find_path_ex_surfaces(
                segment_start,
                goal,
                &self.objects,
                is_aircraft,
                loco,
                false,
            );

            match segment {
                Some(mut segment_path) => {
                    // Keep the found path even if it is long — do not walk through walls.
                    let path_len: f32 = segment_path.windows(2).map(|w| horiz(w[0], w[1])).sum();
                    if straight > 1.0 && path_len > straight * 3.5 {
                        log::debug!(
                            "Path detour {:.0} vs straight {:.0} for {:?}",
                            path_len,
                            straight,
                            unit_id
                        );
                    }
                    {
                        if let Some(first) = segment_path.first_mut() {
                            *first = segment_start;
                        }
                        if let Some(last) = segment_path.last_mut() {
                            *last = goal;
                        }
                        if !full_path.is_empty()
                            && !segment_path.is_empty()
                            && full_path
                                .last()
                                .is_some_and(|prev| horiz(*prev, segment_path[0]) < 0.01)
                        {
                            segment_path.remove(0);
                        }
                        full_path.extend(segment_path);
                    }
                }
                None => {
                    log::debug!(
                        "No path found for unit {:?} from {:?} to {:?}; refuse fail-open march",
                        unit_id,
                        segment_start,
                        goal
                    );
                    // C++ accepts a direct movement request before a map has
                    // installed its terrain/path graph. Preserve the normal
                    // fail-closed path policy for loaded maps, but keep the
                    // mapless host-authority path usable during startup and
                    // command validation.
                    if !self.map_loaded {
                        if full_path.is_empty() {
                            full_path.push(segment_start);
                        }
                        full_path.push(goal);
                    } else {
                        return None;
                    }
                }
            }

            segment_start = goal;
        }

        if full_path.is_empty() {
            // Already at goal (all segments < 0.1) is not a fail-open march.
            return None;
        }
        Some(full_path)
    }

    fn apply_computed_unit_path(
        &mut self,
        unit_id: ObjectId,
        start: Vec3,
        destination: Vec3,
        full_path: Vec<Vec3>,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        unit.waiting_for_path = false;
        unit.is_exact_path = false;
        unit.movement.path = full_path;
        unit.record_host_movement();
        unit.movement.current_path_index = 0;
        unit.record_host_movement();
        unit.movement.target_position = Some(destination);
        crate::game_logic::host_move_log::record(
            unit_id,
            Some([destination.x, destination.y, destination.z]),
        );
        // Kick toward destination at full speed so large-map marches do not
        // burn seconds on the acceleration ramp (was a combat_no_teleport residual).
        {
            let mut dir = destination - start;
            dir.y = 0.0;
            let dir = dir.normalize_or_zero();
            unit.movement.velocity = dir * unit.movement.max_speed;
            unit.record_host_movement();
        }
        unit.set_ai_state(AIState::Moving);
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 1);
            // Moving
        }
        unit.set_status_moving(true);
        true
    }

    /// C++ `AIGroup::friend_computeGroundPath` + per-member slot:
    /// one A* from the nearest member to `destination`, then each unit
    /// follows that spine with last waypoint = its formation/column goal.
    pub fn assign_shared_group_paths(
        &mut self,
        goals: &[(ObjectId, Vec3)],
        destination: Vec3,
    ) -> bool {
        if goals.is_empty() {
            return false;
        }
        let leader = goals
            .iter()
            .filter_map(|(id, _)| {
                self.objects.get(id).map(|o| {
                    let p = o.get_position();
                    let d = (p.x - destination.x).hypot(p.z - destination.z);
                    (*id, p, d, o.locomotor_surfaces, o.is_kind_of(crate::game_logic::KindOf::Aircraft)
                        || o.object_type == crate::game_logic::ObjectType::Aircraft)
                })
            })
            .min_by(|a, b| a.2.total_cmp(&b.2));
        let Some((leader_id, start, _, surfaces, aircraft)) = leader else {
            return false;
        };
        let Some(spine) =
            self.compute_assigned_unit_path(leader_id, start, destination, &[], aircraft, surfaces)
        else {
            return false;
        };
        let mut any = false;
        for &(unit_id, goal) in goals {
            let Some(unit_start) = self.objects.get(&unit_id).map(|o| o.get_position()) else {
                continue;
            };
            let mut path = spine.clone();
            if let Some(last) = path.last_mut() {
                *last = goal;
            } else {
                path.push(goal);
            }
            if self.apply_computed_unit_path(unit_id, unit_start, goal, path) {
                any = true;
            }
        }
        any
    }

    /// C++ Pathfinder::processPathfindQueue residual (AI.cpp:332-339).
    pub(crate) fn process_pathfind_queue(&mut self) {
        let pending = self.pathfinding_system.take_pending_paths();
        for req in pending {
            let (start, can_move, is_aircraft, surfaces) = match self.objects.get(&req.unit_id) {
                Some(unit) if unit.is_alive() => (
                    unit.get_position(),
                    unit.can_move(),
                    req.aircraft
                        || unit.is_kind_of(crate::game_logic::KindOf::Aircraft)
                        || unit.object_type == crate::game_logic::ObjectType::Aircraft,
                    if req.surfaces != 0 {
                        req.surfaces
                    } else {
                        unit.locomotor_surfaces
                    },
                ),
                _ => continue,
            };
            if !can_move {
                if let Some(unit) = self.objects.get_mut(&req.unit_id) {
                    unit.waiting_for_path = false;
                }
                continue;
            }
            match self.compute_assigned_unit_path(
                req.unit_id,
                start,
                req.destination,
                &req.waypoints,
                is_aircraft,
                surfaces,
            ) {
                Some(path) => {
                    let _ = self.apply_computed_unit_path(
                        req.unit_id,
                        start,
                        req.destination,
                        path,
                    );
                }
                None => {
                    if let Some(unit) = self.objects.get_mut(&req.unit_id) {
                        unit.waiting_for_path = false;
                    }
                }
            }
        }
    }

    #[cfg(test)]
    pub fn assign_unit_path_for_test(
        &mut self,
        unit_id: ObjectId,
        destination: Vec3,
        waypoints: &[Vec3],
    ) -> bool {
        self.assign_unit_path_inner(unit_id, destination, waypoints, true)
    }

    /// Pathfind to goal then set AI state. Falls back to set_destination if A* fails.
    /// C++ Pathfinder::isAttackViewBlockedByObstacle residual for host combat.
    /// Units with AttackNeedsLineOfSight cannot fire through static obstacles.
    /// Aircraft / non-LOS kinds always clear. Fail-closed: not full weapon terrain LOS.
    /// Path toward a firing position with LOS (C++ findAttackPath residual).
    /// Falls back to path-to-target if no in-range LOS cell is found.
    pub fn assign_unit_attack_path(
        &mut self,
        unit_id: ObjectId,
        target_id: Option<ObjectId>,
        target_pos: Vec3,
    ) -> bool {
        let (from, range, can_move, contact) = match self.objects.get(&unit_id) {
            Some(u) => {
                let range = u
                    .weapon
                    .as_ref()
                    .map(|w| w.range)
                    .or_else(|| u.secondary_weapon.as_ref().map(|w| w.range))
                    .unwrap_or(50.0)
                    * u.battle_plan_range_multiplier();
                let wname = u.thing.template.primary_weapon_name.as_deref().or(u
                    .thing
                    .template
                    .secondary_weapon_name
                    .as_deref());
                let contact = wname
                    .map(crate::game_logic::weapon_bootstrap::host_is_contact_weapon_name)
                    .unwrap_or(false)
                    || crate::game_logic::weapon_bootstrap::is_contact_weapon_range(range);
                (
                    u.get_position(),
                    range,
                    u.can_move() && u.is_alive(),
                    contact,
                )
            }
            None => return false,
        };
        if !can_move {
            return false;
        }
        // Contact residual: path onto the target cell (C++ approach = victim pos).
        // Non-contact: path to in-range firing cell via find_attack_firing_position.
        // Callers should pass approach-adjusted goal for non-contact when known.
        let path_range = if contact { range.max(1.0) } else { range };
        let _ = contact;
        // Snapshot objects for dynamic occupancy during search.
        let mut path = self.pathfinding_system.find_attack_firing_position(
            from,
            target_pos,
            path_range,
            &self.objects,
        );
        // LOS_TERRAIN residual: reject firing cell if terrain occludes eye-line.
        if let Some(ref full_path) = path {
            if let Some(&goal) = full_path.last() {
                let eye_r = self
                    .objects
                    .get(&unit_id)
                    .map(|o| o.selection_radius.max(5.0) * 0.5)
                    .unwrap_or(5.0);
                let eye_to = target_id
                    .and_then(|tid| self.objects.get(&tid))
                    .map(|o| o.selection_radius.max(5.0) * 0.5)
                    .unwrap_or(5.0);
                let a_eye = Vec3::new(goal.x, goal.y + eye_r, goal.z);
                let b_eye = Vec3::new(target_pos.x, target_pos.y + eye_to, target_pos.z);
                if !self.is_clear_line_of_sight_terrain(a_eye, b_eye) {
                    path = None;
                }
            }
        }
        let decision_auth = crate::gameworld_shadow::gameworld_ai_decision_authority_live();
        if let Some(full_path) = path {
            if full_path.len() >= 2 {
                if let Some(unit) = self.objects.get_mut(&unit_id) {
                    // Path integrate stays host (movement authority peels separately).
                    unit.movement.path = full_path;
                    unit.record_host_movement();
                    unit.movement.current_path_index = 1;
                    unit.record_host_movement();
                    unit.movement.target_position = Some(unit.movement.path[1]);
                    unit.set_status_moving(true);
                    if !decision_auth {
                        unit.set_ai_state(AIState::Attacking);
                        unit.set_status_attacking(true);
                        if let Some(tid) = target_id {
                            unit.target = Some(tid);
                        }
                    }
                    crate::game_logic::host_move_log::record(
                        unit_id,
                        Some([target_pos.x, target_pos.y, target_pos.z]),
                    );
                }
                if decision_auth {
                    if let Some(tid) = target_id {
                        crate::game_logic::host_ai_decision_log::record_attack(unit_id, tid);
                    }
                    // Attacking ordinal = 2
                    crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 2);
                }
                return true;
            }
        }
        // Fallback: path to target footprint (prior residual).
        if self.assign_unit_path(unit_id, target_pos, &[]) {
            if decision_auth {
                if let Some(tid) = target_id {
                    crate::game_logic::host_ai_decision_log::record_attack(unit_id, tid);
                }
                crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 2);
            } else if let Some(unit) = self.objects.get_mut(&unit_id) {
                unit.set_ai_state(AIState::Attacking);
                unit.set_status_attacking(true);
                if let Some(tid) = target_id {
                    unit.target = Some(tid);
                }
            }
            return true;
        }
        false
    }

    #[cfg(test)]
    pub fn assign_unit_attack_path_for_test(
        &mut self,
        unit_id: ObjectId,
        target_id: Option<ObjectId>,
        target_pos: Vec3,
    ) -> bool {
        self.assign_unit_attack_path(unit_id, target_id, target_pos)
    }

    /// C++ TerrainLogic/PartitionManager isClearLineOfSightTerrain residual.
    /// Samples ground height along the XZ segment; blocked when terrain rises above
    /// the eye-line + clearance. Uses `terrain_height_at` / pathfinding height cache.
    /// Fail-closed: returns true (clear) when no height data is available.
    pub fn is_clear_line_of_sight_terrain(&self, from: Vec3, to: Vec3) -> bool {
        let dx = to.x - from.x;
        let dz = to.z - from.z;
        let dist_xz = (dx * dx + dz * dz).sqrt();
        if dist_xz <= 0.001 {
            return true;
        }
        // Eye height residual: geometry top ~ selection_radius*0.5 fallback + 5.
        // Callers should pass elevated from/to; default add small eye fudge here.
        let from_y = from.y;
        let to_y = to.y;
        let step_len = 10.0_f32;
        let steps = (dist_xz / step_len).ceil().clamp(2.0, 512.0) as u32;
        const CLEARANCE: f32 = 5.0;
        let mut any_sample = false;
        for i in 1..steps {
            let tfrac = i as f32 / steps as f32;
            let x = from.x + dx * tfrac;
            let z = from.z + dz * tfrac;
            let expected_y = from_y + (to_y - from_y) * tfrac;
            let Some(ground) = self.terrain_height_at(Vec3::new(x, 0.0, z)) else {
                continue;
            };
            any_sample = true;
            if ground > expected_y + CLEARANCE {
                return false;
            }
        }
        // No height data along segment → fail-open clear (flat/synthetic maps).
        let _ = any_sample;
        true
    }

    pub fn attack_view_blocked(
        &self,
        attacker_id: ObjectId,
        target_id: Option<ObjectId>,
        target_pos: Vec3,
    ) -> bool {
        let Some(attacker) = self.objects.get(&attacker_id) else {
            return false;
        };
        // C++ KINDOF_ATTACK_NEEDS_LINE_OF_SIGHT gate.
        // Host residual: Infantry/Vehicle default-need LOS unless Immobile structure.
        let needs_los = attacker.is_kind_of(KindOf::AttackNeedsLineOfSight)
            || ((attacker.is_kind_of(KindOf::Infantry) || attacker.is_kind_of(KindOf::Vehicle))
                && !attacker.is_kind_of(KindOf::Structure /* immobile residual */)
                && !attacker.is_kind_of(KindOf::Structure)
                && !attacker.is_kind_of(KindOf::Aircraft));
        if !needs_los {
            return false;
        }
        // Flying victim residual: significantly above terrain → not blocked.
        if let Some(tid) = target_id {
            if let Some(t) = self.objects.get(&tid) {
                if t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target {
                    return false;
                }
            }
        }
        let from = attacker.get_position();
        // Tiny range residual (C++ AIStates close-range skip).
        let dx = from.x - target_pos.x;
        let dz = from.z - target_pos.z;
        if (dx * dx + dz * dz).sqrt() < 15.0 {
            return false;
        }
        // LOS_TERRAIN residual (C++ Weapon::isClearGoalFiringLineOfSightTerrain):
        // immobile attackers skip terrain LOS (cannot path around).
        let immobile = attacker.is_kind_of(KindOf::Structure /* immobile residual */)
            || attacker.is_kind_of(KindOf::Structure);
        if !immobile {
            // Eye-line: lift by geometry height residual (selection_radius as proxy).
            let eye_from = from.y + attacker.selection_radius.max(5.0) * 0.5;
            let eye_to = {
                let th = target_id
                    .and_then(|tid| self.objects.get(&tid))
                    .map(|t| t.selection_radius.max(5.0) * 0.5)
                    .unwrap_or(5.0);
                target_pos.y + th
            };
            let from_eye = Vec3::new(from.x, eye_from, from.z);
            let to_eye = Vec3::new(target_pos.x, eye_to, target_pos.z);
            if !self.is_clear_line_of_sight_terrain(from_eye, to_eye) {
                return true;
            }
        }
        // Structure/static obstacle Bresenham residual.
        self.pathfinding_system
            .is_attack_view_blocked(from, target_pos)
    }

    pub(super) fn path_approach_with_state(
        &mut self,
        object_id: ObjectId,
        goal: Vec3,
        state: AIState,
    ) {
        let decision_auth = crate::gameworld_shadow::gameworld_ai_decision_authority_live();
        let ordinal = crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(&state);
        if self.assign_unit_path(object_id, goal, &[]) {
            if decision_auth {
                crate::game_logic::host_ai_decision_log::record_set_state(object_id, ordinal);
            } else if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.set_ai_state(state);
            }
        } else if decision_auth {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.set_destination(goal);
            }
            crate::game_logic::host_ai_decision_log::record_set_state(object_id, ordinal);
        } else if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.set_destination(goal);
            obj.set_ai_state(state);
        }
    }

    #[cfg(test)]
    pub fn path_approach_with_state_for_test(
        &mut self,
        object_id: ObjectId,
        goal: Vec3,
        state: AIState,
    ) {
        self.path_approach_with_state(object_id, goal, state);
    }

    pub fn append_unit_waypoint(&mut self, unit_id: ObjectId, waypoint: Vec3) -> bool {
        let (unit_pos, current_path, can_move) = match self.objects.get(&unit_id) {
            Some(unit) => (
                unit.get_position(),
                unit.movement.path.clone(),
                unit.can_move(),
            ),
            None => return false,
        };
        if !can_move {
            return false;
        }

        let last_goal = current_path.last().copied().unwrap_or(unit_pos);

        let segment = self
            .pathfinding_system
            .find_path(last_goal, waypoint, &self.objects);

        let mut appended = current_path;
        match segment {
            Some(mut segment_path) => {
                if let Some(first) = segment_path.first_mut() {
                    *first = last_goal;
                }
                if !appended.is_empty()
                    && !segment_path.is_empty()
                    && appended
                        .last()
                        .is_some_and(|prev| prev.distance(segment_path[0]) < 0.01)
                {
                    segment_path.remove(0);
                }
                appended.extend(segment_path);
            }
            None => {
                log::debug!(
                    "No path found for unit {:?} from {:?} to {:?}; falling back to direct segment",
                    unit_id,
                    last_goal,
                    waypoint
                );
                if appended.is_empty() {
                    appended.push(last_goal);
                }
                appended.push(waypoint);
            }
        }

        let Some(unit) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        unit.movement.path = appended;
        unit.movement.target_position = Some(waypoint);
        crate::game_logic::host_move_log::record(
            unit_id,
            Some([waypoint.x, waypoint.y, waypoint.z]),
        );
        unit.set_ai_state(AIState::Moving);
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 1);
            // Moving
        }
        unit.set_status_moving(true);
        true
    }

    #[cfg(test)]
    pub fn append_unit_waypoint_for_test(&mut self, unit_id: ObjectId, waypoint: Vec3) -> bool {
        self.append_unit_waypoint(unit_id, waypoint)
    }

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

    /// Player_N_Start spots already decoded with map settings.
    /// Avoids a second RefPack decode of the same `.map` during spawn.
    pub(crate) fn cached_player_start_waypoints(
        &self,
    ) -> Option<Vec<(u32, gamelogic::scripting::core::Coord3D, Option<gamelogic::scripting::core::Coord3D>)>>
    {
        let meta = self.last_map_settings.as_ref()?;
        if meta.start_waypoints.is_empty() {
            return None;
        }
        let mut starts = Vec::new();
        for (name, pos) in &meta.start_waypoints {
            let lower = name.trim().to_ascii_lowercase();
            let Some(rest) = lower.strip_prefix("player_") else {
                continue;
            };
            let Some((num, kind)) = rest.split_once('_') else {
                continue;
            };
            if !kind.starts_with("start") {
                continue;
            }
            if let Ok(idx1) = num.parse::<u32>() {
                if idx1 >= 1 {
                    starts.push((idx1 - 1, *pos, None));
                }
            }
        }
        if starts.is_empty() {
            None
        } else {
            Some(starts)
        }
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
        self.reset();
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
        self.create_default_players();
        if matches!(mode, GameMode::Skirmish) {
            let _ = self.ensure_replay_observer_player();
            self.install_replay_observer_side();
            self.set_install_multiplayer_scripts(true);
        }
        log::info!("New game started successfully");
    }

    pub fn game_mode(&self) -> GameMode {
        self.game_mode
    }

    /// Wave 831: spawn SidesList build-list entries (initiallyBuilt faction bases).

    pub(super) fn spawn_side_build_list(
        &mut self,
        _builds: &[super::script_loader::SideBuildEntry],
        _map_player_to_team: &std::collections::HashMap<u32, Team>,
    ) -> u32 {
        // C++ never instantiates SidesList BuildListInfo at map load.
        // initiallyBuilt entries are already placed via ObjectsList; the list
        // is transferred onto Player for AI rebuild (see sync + take_build_list).
        0
    }

    pub(super) fn team_from_string(name: &str) -> Option<Team> {
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
    pub(super) fn team_from_template_name(template: &str) -> Option<Team> {
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

    pub(super) fn sync_legacy_runtime_from_chunky(&mut self, map_path: &Path, map_bytes: &[u8]) {
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
        }
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
        let script_lists =
            match super::script_loader::load_map_scripts_from_chunky(chunky) {
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

    pub(super) fn sync_legacy_player_list_from_side_dicts(&self, side_dicts: &[Dict]) {
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
                log::warn!("Fast legacy runtime sync skipped PlayerList write (ThePlayerList busy)");
            }
        }
    }

    pub(super) fn sync_legacy_sides_list_from_dicts(
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
            build.set_template_name(gamelogic::common::AsciiString::from(entry.template.as_str()));
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

    fn add_host_players_as_sides(&self, sides: &mut gamelogic::sides_list::SidesList) {
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
            dict.set_int(key_multiplayer_start_index(), index as i32);
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

    fn transfer_side_build_lists_to_players(&self) {
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

    fn host_player_id_for_side_index(&self, side_index: u32) -> Option<u32> {
        if self.players.contains_key(&side_index) {
            return Some(side_index);
        }
        let mut pids: Vec<u32> = self.players.keys().copied().collect();
        pids.sort_unstable();
        pids.get(side_index as usize).copied()
    }

    fn apply_host_players_from_sides_list(&mut self, replace_default_money: bool) {
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
    pub(super) fn apply_host_players_from_side_dicts(
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
                        player.set_map_relationship(
                            eid,
                            gamelogic::common::Relationship::Enemies,
                        );
                    }
                }
                for token in allies.split_whitespace() {
                    if let Some(&aid) = by_name.get(token) {
                        player.set_map_relationship(aid, gamelogic::common::Relationship::Allies);
                    }
                }
            }
        }
    }

    pub(super) fn stash_side_builds_on_host(
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
    pub(super) fn feed_host_ai_from_authored_build_lists(&mut self) {
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


    pub(super) fn sync_legacy_player_list_from_sides(&self) {
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

    pub(super) fn sync_legacy_team_factory_from_team_dicts(&self, team_dicts: &[Dict]) {
        let Ok(mut team_factory) = get_team_factory().try_lock() else {
            log::warn!("Fast legacy runtime sync skipped TeamFactory write (THE_TEAM_FACTORY busy)");
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

            let team = team_factory
                .find_team(&team_name)
                .or_else(|| team_factory.create_team(&team_name));

            let Some(team_arc) = team else {
                log::warn!("Failed to instantiate legacy team '{}'", team_name);
                continue;
            };

            if let Ok(mut team_guard) = team_arc.write() {
                if !owner.is_empty() {
                    if let Ok(player_list) = ThePlayerList().try_read() {
                        if let Some(player_arc) = player_list.find_player_by_name(&owner) {
                            if let Ok(player_guard) = player_arc.try_read() {
                                team_guard.set_controlling_player_id(Some(
                                    player_guard.get_player_index() as u32,
                                ));
                            }
                        }
                    }
                }
                if singleton {
                    team_guard.set_active();
                }
            };
        }
    }

    pub(super) fn sync_legacy_team_factory_from_sides(&self) {
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

    pub(super) fn sync_named_shell_object_into_legacy_runtime(
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

    pub(super) fn ground_loaded_map_objects_to_terrain(
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
                                self.players.get(player_id).is_some_and(|player| {
                                    player.is_alive && player.team == team
                                })
                            });
                            let created = match owner_player_id {
                                Some(player_id) => self.create_object_for_player(
                                    obj.template.as_str(),
                                    player_id,
                                    spawn_position,
                                ),
                                None => self.create_object(
                                    obj.template.as_str(),
                                    team,
                                    spawn_position,
                                ),
                            };
                            if let Some(id) = created {
                                spawned_object_ids.push((id, index));
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
                        self.spawned_map_object_ids = spawned_object_ids;
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
                                width, height, max_height, MAP_XY_FACTOR,
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
                                    let mut tiles = vec![
                                        game_client::terrain::textures::BlendTileInfo::new(),
                                    ];
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
        if matches!(self.game_mode, GameMode::Skirmish) {
            // C++ startNewGame adds ReplayObserver after sides, then installs
            // MultiplayerScripts.scb (numTeams>1) and permanently reveals the map.
            self.install_replay_observer_side();
            self.install_multiplayer_scripts_if_needed();
            self.reveal_replay_observer_map();
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

/// C++ PlayerList.cpp:167-199.
fn apply_logic_player_list_relationships(logic_list: &mut LogicPlayerList, side_dicts: &[Dict]) {
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
        for token in dict.get_ascii_string(key_player_enemies()).split_whitespace() {
            if let Some(&enemy) = name_to_index.get(token) {
                player.set_player_relationship_by_index(
                    enemy,
                    gamelogic::common::Relationship::Enemies,
                );
            }
        }
        for token in dict.get_ascii_string(key_player_allies()).split_whitespace() {
            if let Some(&ally) = name_to_index.get(token) {
                player.set_player_relationship_by_index(
                    ally,
                    gamelogic::common::Relationship::Allies,
                );
            }
        }
    }
}


fn load_multiplayer_scripts_scb() -> Option<ScriptList> {
    use game_engine::common::system::file::FileAccess;
    use game_engine::common::system::file_system::get_file_system;
    use game_engine::common::system::DataChunkInput;
    use gamelogic::scripting::core::{parse_player_scripts_list_chunk, ScriptListReadInfo};

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

#[cfg(feature = "game_client")]
fn apply_cpp_heightmap_xy_and_border(
    heightmap: &mut game_client::terrain::height_map::HeightMap,
    map_border: i32,
) {
    use gamelogic::common::MAP_XY_FACTOR;
    // C++ MapObject.h MAP_XY_FACTOR; WorldHeightMap::m_borderSize else ZH 70.
    heightmap.scale = MAP_XY_FACTOR;
    heightmap.border_size = if map_border > 0 { map_border } else { 70 };
}

#[cfg(test)]
mod sides_host_apply_tests {
    use super::*;

    #[test]
    fn map_side_dict_sets_host_money_color_and_enemies() {
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "PlyrAmerica", true));
        logic.add_player(Player::new(1, Team::GLA, "PlyrGLA", false));

        let mut america = Dict::new();
        america.set_ascii_string(key_player_name(), "PlyrAmerica");
        america.set_int(key_player_start_money(), 3_000);
        america.set_int(key_player_color(), 0x0000_00ff);
        america.set_ascii_string(key_player_enemies(), "PlyrGLA");
        america.set_ascii_string(key_player_allies(), "");

        let mut gla = Dict::new();
        gla.set_ascii_string(key_player_name(), "PlyrGLA");
        gla.set_int(key_player_start_money(), 1_500);
        gla.set_ascii_string(key_player_enemies(), "PlyrAmerica");

        logic.apply_host_players_from_side_dicts(&[america, gla], true);

        let usa = logic.get_player(0).expect("usa");
        assert_eq!(usa.resources.supplies, 3_000);
        assert_eq!(usa.color_rgb, (0, 0, 0xff));
        assert_eq!(
            logic.player_relationship(0, 1),
            gamelogic::common::Relationship::Enemies
        );
    }

    #[test]
    fn authored_build_list_replaces_hardcoded_ai_layout() {
        let mut logic = GameLogic::new();
        logic.add_player(Player::new(1, Team::USA, "PlyrAmerica", false));
        logic.add_ai_opponent(1, Team::USA, crate::ai::AIDifficulty::Medium);
        let before = logic
            .ai_manager
            .ai_players
            .get(&1)
            .map(|ai| ai.building_queue.len())
            .unwrap_or(0);
        assert!(before > 0, "hardcoded layout seeds a queue");

        let builds = [super::super::script_loader::SideBuildEntry {
            building_name: "cc".into(),
            template: "AmericaCommandCenter".into(),
            position: gamelogic::scripting::core::Coord3D {
                x: 10.0,
                y: 20.0,
                z: 0.0,
            },
            angle: 0.0,
            initially_built: true,
            num_rebuilds: 3,
            side_index: 1,
            script_name: None,
            health: None,
            whiner: None,
            unsellable: None,
            repairable: None,
        }];
        logic.stash_side_builds_on_host(&builds);
        let ai = logic.ai_manager.ai_players.get(&1).expect("ai");
        assert_eq!(ai.building_queue.len(), 1);
        assert_eq!(ai.building_queue[0].template_name, "AmericaCommandCenter");
        assert_eq!(ai.building_queue[0].max_rebuilds, 3);
        assert!(ai.building_queue[0].is_built);
    }
}
