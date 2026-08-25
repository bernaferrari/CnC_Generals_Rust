//! Additional `impl GameLogic` methods. Child of `game_logic.rs`.
#![allow(unused_imports, non_snake_case)]
use super::*;

// Host weather, replay, terrain, and bridge subsystem state.
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

    /// C++ `WorldHeightMap::ParseWorldDictDataChunk` writes `m_weather` when
    /// WorldInfo carries `TheKey_weather` (`WEATHER_NORMAL=0` / `WEATHER_SNOWY=1`).
    pub fn apply_world_info_weather(&mut self, weather: Option<i32>) {
        let Some(weather) = weather else {
            return;
        };
        let snowy = weather == 1;
        let name = if snowy { "snowy" } else { "normal" };
        let intensity = self.weather_state.intensity;
        let duration_remaining = self.weather_state.duration_remaining;
        let next_change_time = self.weather_state.next_change_time;
        self.set_weather_state(name, intensity, duration_remaining, next_change_time);
        if let Some(global) = game_engine::common::ini::get_global_data() {
            global.write().weather = if snowy {
                game_engine::common::ini::Weather::Snowy
            } else {
                game_engine::common::ini::Weather::Normal
            };
        }
        if let Ok(mut runtime) = game_engine::common::global_data::write_safe() {
            runtime.weather = if snowy { 1 } else { 0 };
        }
    }

    pub(in super::super) fn apply_spawned_object_weather(
        &mut self,
        id: ObjectId,
        object_weather: i32,
    ) {
        let follow = game_engine::common::ini::get_global_data()
            .map(|global| global.read().force_models_to_follow_weather)
            .unwrap_or(true);
        let world_is_snow = follow
            && self
                .weather_state
                .current_weather
                .to_ascii_lowercase()
                .contains("snow");
        let snow = super::script_loader::resolve_object_weather_snow(object_weather, world_is_snow);
        let snow_b = crate::game_logic::host_enum_table_residual::snow_model_bit();
        if let Some(created) = self.objects.get_mut(&id) {
            created.object_weather = object_weather;
            if snow {
                created.model_condition_bits |= 1u128 << snow_b;
            } else {
                created.model_condition_bits &= !(1u128 << snow_b);
            }
        }
    }

    /// C++ `W3DTerrainVisual::load` (`W3DTerrainVisual.cpp:671-678`): map
    /// objects whose Dict has `scorchType` are `MO_SCORCH` and stamp
    /// `TheTerrainRenderObject->addScorch(loc, objectRadius, scorchType)`.
    /// No logic Object is created.
    pub(in super::super) fn apply_map_object_scorch(
        obj: &super::script_loader::PlacedObject,
    ) -> bool {
        use game_engine::common::dict::DictType;
        use game_engine::common::name_key_generator::NameKeyGenerator;

        let scorch_key = NameKeyGenerator::name_to_key("scorchType");
        if obj.properties.get_type(scorch_key).is_none() {
            return false;
        }
        let scorch_type = match obj.properties.get_type(scorch_key) {
            Some(DictType::Int) => obj.properties.get_int(scorch_key),
            Some(DictType::Real) => obj.properties.get_real(scorch_key) as i32,
            _ => 0,
        };
        let radius_key = NameKeyGenerator::name_to_key("objectRadius");
        let radius = match obj.properties.get_type(radius_key) {
            Some(DictType::Real) => obj.properties.get_real(radius_key),
            Some(DictType::Int) => obj.properties.get_int(radius_key) as f32,
            _ => 0.0,
        };
        let loc = gamelogic::common::Coord3D::new(obj.position.x, obj.position.y, obj.position.z);
        if let Some(client) = gamelogic::helpers::TheGameClient::get() {
            client.add_scorch(&loc, radius, scorch_type);
        }
        true
    }

    /// C++ `Object::updateObjValuesFromMapProperties` at live map spawn.
    /// Leftover already matches C++; call it when a dual-world object exists,
    /// then stamp the same Dict onto the live host object.
    pub(in super::super) fn apply_update_obj_values_from_map_properties(
        &mut self,
        id: ObjectId,
        properties: &game_engine::common::dict::Dict,
    ) {
        use game_engine::common::dict::DictType;
        use game_engine::common::name_key_generator::NameKeyGenerator;
        use game_engine::common::well_known_keys;

        if properties.get_pair_count() == 0 {
            return;
        }

        let _ = gamelogic::object::registry::OBJECT_REGISTRY.with_object_mut(id.0, |obj| {
            obj.update_obj_values_from_map_properties(properties);
        });

        let get_bool = |key| {
            if properties.get_type(key) == Some(DictType::Bool) {
                Some(properties.get_bool(key))
            } else {
                None
            }
        };
        let get_int = |key| {
            if properties.get_type(key) == Some(DictType::Int) {
                Some(properties.get_int(key))
            } else {
                None
            }
        };
        let get_ascii = |key| {
            if properties.get_type(key) == Some(DictType::AsciiString) {
                Some(properties.get_ascii_string(key))
            } else {
                None
            }
        };

        if let Some(name) = get_ascii(well_known_keys::key_object_name()) {
            if !name.is_empty() {
                if let Some(created) = self.objects.get_mut(&id) {
                    created.name = name;
                    created.record_host_identity();
                }
            }
        }

        if let Some(max_hps) = get_int(well_known_keys::key_object_max_hps()) {
            if max_hps >= 0 {
                if let Some(created) = self.objects.get_mut(&id) {
                    let new_max = max_hps as f32;
                    let old_max = created.health.maximum.max(created.max_health).max(1.0);
                    let ratio = created.health.current / old_max;
                    created.set_body_max_health(new_max);
                    created.health.current = (new_max * ratio).clamp(0.0, new_max);
                    created.record_host_max_health();
                }
            }
        }

        if let Some(initial_health) = get_int(well_known_keys::key_object_initial_health()) {
            if let Some(created) = self.objects.get_mut(&id) {
                // C++ setInitialHealth: percent of stored InitialHealth, current HP only.
                created.set_initial_health_percent(initial_health);
            }
        }

        if let Some(veterancy) = get_int(well_known_keys::key_object_veterancy()) {
            if let Some(created) = self.objects.get_mut(&id) {
                if created.is_trainable() {
                    let level = match veterancy.clamp(0, 3) {
                        0 => crate::game_logic::VeterancyLevel::Rookie,
                        1 => crate::game_logic::VeterancyLevel::Veteran,
                        2 => crate::game_logic::VeterancyLevel::Elite,
                        _ => crate::game_logic::VeterancyLevel::Heroic,
                    };
                    let _ = created.set_min_veterancy_level(level);
                }
            }
        }

        if let Some(attitude) = get_int(well_known_keys::key_object_aggressiveness()) {
            if let Some(created) = self.objects.get_mut(&id) {
                created.set_ai_attitude_i8(attitude as i8);
            }
        }

        if let Some(selectable) = get_bool(well_known_keys::key_object_selectable()) {
            if let Some(created) = self.objects.get_mut(&id) {
                if selectable != created.is_selectable() {
                    created.set_status_unselectable(!selectable);
                }
            }
        }

        if let Some(enabled) = get_bool(well_known_keys::key_object_enabled()) {
            if let Some(created) = self.objects.get_mut(&id) {
                created.set_script_disabled(!enabled);
            }
        }

        if let Some(powered) = get_bool(well_known_keys::key_object_powered()) {
            if let Some(created) = self.objects.get_mut(&id) {
                created.set_script_underpowered(!powered);
            }
        }

        if let Some(indestructible) = get_bool(well_known_keys::key_object_indestructible()) {
            self.set_object_indestructible(id, indestructible);
        }

        if let Some(unsellable) = get_bool(well_known_keys::key_object_unsellable()) {
            if let Some(created) = self.objects.get_mut(&id) {
                created.set_script_unsellable(unsellable);
            }
        }

        if leftover_object_script_targetable(id)
            || get_bool(well_known_keys::key_object_targetable()) == Some(true)
        {
            if let Some(created) = self.objects.get_mut(&id) {
                created.set_script_targetable(true);
            }
        } else if get_bool(well_known_keys::key_object_targetable()) == Some(false) {
            if let Some(created) = self.objects.get_mut(&id) {
                created.set_script_targetable(false);
            }
        }

        if let Some(visual_range) = get_int(well_known_keys::key_object_visual_range()) {
            if let Some(created) = self.objects.get_mut(&id) {
                created.vision_range = (visual_range as f32).max(0.0);
                created.record_host_crush_vision();
            }
        }

        if let Some(shroud_range) = get_int(well_known_keys::key_object_shroud_clearing_distance())
        {
            if let Some(created) = self.objects.get_mut(&id) {
                created.shroud_clearing_range = (shroud_range as f32).max(0.0);
                created.record_host_crush_vision();
            }
        }

        let mut grant_upgrades = Vec::new();
        for upgrade_num in 0.. {
            let key = NameKeyGenerator::name_to_key(&format!("objectGrantUpgrade{upgrade_num}"));
            let Some(upgrade_name) = get_ascii(key) else {
                break;
            };
            if upgrade_name.is_empty() {
                break;
            }
            grant_upgrades.push(upgrade_name);
        }
        for upgrade_name in grant_upgrades {
            self.apply_upgrade_to_object(id, &upgrade_name);
        }

        if let Some(time_val) = get_int(well_known_keys::key_object_time()) {
            let night_b = crate::game_logic::host_enum_table_residual::night_model_bit();
            if let Some(created) = self.objects.get_mut(&id) {
                match time_val {
                    1 => created.model_condition_bits &= !(1u128 << night_b),
                    2 => created.model_condition_bits |= 1u128 << night_b,
                    _ => {}
                }
            }
        }

        if let Some(weather_val) = get_int(well_known_keys::key_object_weather()) {
            self.apply_spawned_object_weather(id, weather_val);
        }

        let mut sound_enabled_exists = false;
        let mut sound_enabled = false;
        if let Some(enabled) = get_bool(well_known_keys::key_object_sound_ambient_enabled()) {
            sound_enabled_exists = true;
            sound_enabled = enabled;
        }

        let custom_ambient = get_ascii(well_known_keys::key_object_sound_ambient());
        if let Some(sound_name) = &custom_ambient {
            if sound_name.is_empty() {
                sound_enabled_exists = true;
                sound_enabled = false;
            } else if !sound_enabled_exists {
                // C++ defaults to isPermanentSound(); map-custom names are loops.
                sound_enabled_exists = true;
                sound_enabled = true;
            }
        }

        if let Some(sound_name) = &custom_ambient {
            if sound_name.is_empty() {
                self.stop_ambient_sound(id);
                if let Some(created) = self.objects.get_mut(&id) {
                    created.ambient_audio = None;
                    created.ambient_sound_enabled_from_script = false;
                }
            } else if sound_enabled {
                let pos = self.objects.get(&id).map(|o| o.get_position());
                if let Some(created) = self.objects.get_mut(&id) {
                    created.ambient_sound_enabled_from_script = true;
                    created.ambient_audio = Some(sound_name.clone());
                }
                if let Some(pos) = pos {
                    self.queue_audio_event(
                        crate::game_logic::AudioEventRequest::new(sound_name)
                            .with_object(id)
                            .with_position(pos)
                            .with_priority(80)
                            .looping(),
                    );
                }
            } else {
                self.stop_ambient_sound(id);
                if let Some(created) = self.objects.get_mut(&id) {
                    created.ambient_sound_enabled_from_script = false;
                    created.ambient_audio = Some(sound_name.clone());
                }
            }
        } else if sound_enabled_exists {
            if let Some(created) = self.objects.get_mut(&id) {
                created.ambient_sound_enabled_from_script = sound_enabled;
            }
            if sound_enabled {
                self.start_ambient_sound(id);
            } else {
                self.stop_ambient_sound(id);
            }
        }
    }

    pub fn set_weather_visible(&mut self, visible: bool) {
        self.weather_state.visible = visible;
        #[cfg(feature = "game_client")]
        {
            // C++ ScriptActions.cpp:3804 TheSnowManager->setVisible(showWeather)
            let snow = game_client::snow::get_snow_manager()
                .unwrap_or_else(game_client::snow::initialize_snow_manager);
            if let Ok(mut guard) = snow.lock() {
                guard.set_visible(visible);
            }
        }
    }

    pub fn queue_pending_special_ability(
        &mut self,
        object_id: ObjectId,
        ability: PendingSpecialAbility,
    ) {
        if Self::leftover_sa_exclusive_pending(ability) {
            self.abort_leftover_sa_channel_on_new_order(object_id);
        }
        self.pending_special_abilities.insert(object_id, ability);
    }

    pub fn clear_pending_special_ability(&mut self, object_id: ObjectId) {
        self.pending_special_abilities.remove(&object_id);
    }

    /// Live pending special-ability command (C++ SpecialAbilityUpdate target).
    pub fn pending_special_ability(&self, object_id: ObjectId) -> Option<PendingSpecialAbility> {
        self.pending_special_abilities.get(&object_id).copied()
    }

    /// Load-time pending ability insert. Does not abort an already-restored leftover channel.
    pub fn restore_pending_special_ability(
        &mut self,
        object_id: ObjectId,
        ability: PendingSpecialAbility,
    ) {
        self.pending_special_abilities.insert(object_id, ability);
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
        observer.color_night_rgb = (170, 170, 170);

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
            logic_player.init(std::sync::Arc::new(LogicPlayerTemplate::from_common(
                common,
            )));
            logic_player.set_observer(true);
            logic_player.set_player_type(LogicPlayerType::Observer, false);
        }
        if let Ok(mut list) = ThePlayerList().write() {
            list.add_player(std::sync::Arc::new(std::sync::RwLock::new(logic_player)));
        }
    }

    /// C++ GameLogic.cpp:2222-2230 GAME_REPLAY: setLocalPlayer(ReplayObserver),
    /// TheRadar->forceOn(TRUE), refreshShroudForLocalPlayer, observer control bar.
    pub fn apply_replay_observer_as_local_player(&mut self) {
        if !matches!(self.game_mode, GameMode::Replay) {
            return;
        }
        let observer_id = self.ensure_replay_observer_player();
        for player in self.players.values_mut() {
            player.is_local = player.id == observer_id;
        }
        if let Ok(mut list) = ThePlayerList().write() {
            if let Some(observer) = list.find_player_by_name("ReplayObserver") {
                if let Ok(guard) = observer.read() {
                    list.set_local_player_index(guard.get_player_index());
                }
            }
        }
        self.set_radar_forced(true);
        if let Ok(mut radar) = game_engine::common::system::radar::get_radar_system().write() {
            radar.force_on(true);
        }
        if let Ok(mut shroud) = get_shroud_manager().lock() {
            shroud.refresh_shroud_for_local_player();
        }
        #[cfg(feature = "game_client")]
        {
            game_client::helpers::TheControlBar::set_control_bar_scheme_by_player("Observer");
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
                let mut dest = side
                    .get_script_list()
                    .cloned()
                    .unwrap_or_else(ScriptList::new);
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
        let ground = {
            #[cfg(feature = "game_client")]
            {
                if let Some(h) = self.terrain.as_ref().map(|t| t.height_at_world(world_pos)) {
                    Some(h)
                } else {
                    self.pathfinding_height_samples
                        .as_ref()
                        .and_then(|cache| self.cached_pathfind_height(world_pos, cache))
                }
            }
            #[cfg(not(feature = "game_client"))]
            {
                self.pathfinding_height_samples
                    .as_ref()
                    .and_then(|cache| self.cached_pathfind_height(world_pos, cache))
            }
        };
        // C++ getLayerHeight: non-rubble deck plane wins when the XY is on a span.
        if let Ok(tl) = gamelogic::terrain::get_terrain_logic().read() {
            if let Some(deck) = tl.host_deck_height_at(world_pos.x, world_pos.z) {
                if ground.map_or(true, |g| deck > g) {
                    return Some(deck);
                }
            }
        }
        ground
    }

    /// C++ `Locomotor::getSurfaceHtAtPt` — water surface when underwater, else terrain.
    pub fn surface_ht_at(&self, world_pos: Vec3) -> Option<f32> {
        if let Some(terrain) = self.terrain.as_ref() {
            if terrain.is_underwater_at_world(world_pos) {
                if let Some(water_y) = terrain.water_surface_at_world(world_pos) {
                    return Some(water_y);
                }
            }
        }
        self.terrain_height_at(world_pos)
    }

    /// C++ `PartitionManager::getGroundOrStructureHeight` (`PartitionManager.cpp:4674`).
    /// Tallest live `KINDOF_STRUCTURE` whose 2D bounding sphere is within 1wu,
    /// plus leftover partition when that world is populated.
    pub fn ground_or_structure_height_at(&self, world_pos: Vec3, terrain_y: f32) -> f32 {
        const RANGE: f32 = 1.0;
        let leftover = gamelogic::object::collide::partition_manager::PARTITION_MANAGER
            .read()
            .ok()
            .map(|pm| pm.get_ground_or_structure_height(world_pos.x, world_pos.z));
        let structure_height = |other: &Object| -> Option<f32> {
            if !other.is_kind_of(KindOf::Structure) {
                return None;
            }
            let op = other.get_position();
            let dx = op.x - world_pos.x;
            let dz = op.z - world_pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let geom = &other.thing.template.geometry_info;
            let radius = if geom.authored {
                // C++ FROM_BOUNDINGSPHERE_2D subtracts bounding-circle
                // (sqrt(major^2+minor^2) for boxes).
                geom.bounding_circle_radius()
            } else {
                other.selection_radius
            };

            if dist - radius > RANGE {
                return None;
            }
            Some(if geom.authored {
                geom.max_height_above_position()
            } else {
                other.selection_radius.max(1.0)
            })
        };
        let mut tallest = 0.0f32;
        let neighbors = self
            .partition_manager
            .neighbor_object_ids(world_pos.x, world_pos.z);
        if neighbors.is_empty() {
            for other in self.objects.values() {
                if let Some(h) = structure_height(other) {
                    tallest = tallest.max(h);
                }
            }
        } else {
            for id in neighbors {
                if let Some(other) = self.objects.get(&ObjectId(id)) {
                    if let Some(h) = structure_height(other) {
                        tallest = tallest.max(h);
                    }
                }
            }
        }
        let live = terrain_y + tallest;
        match leftover {
            Some(h) if h > live + 1.0e-3 => h,
            _ => live,
        }
    }

    /// Terrain argument for `tick_height_die` so HAT matches C++ HeightDieUpdate
    /// (`getGroundHeight` + optional structure rooftop, HeightDieUpdate.cpp:132-195).
    pub fn height_die_terrain_at(&self, world_pos: Vec3, template: &str, fallback: f32) -> f32 {
        use crate::game_logic::host_height_die::{
            height_die_ini_for_template, height_die_target_world_y,
        };
        let terrain = self.terrain_height_at(world_pos).unwrap_or(fallback);
        let Some(ini) = height_die_ini_for_template(template) else {
            return terrain;
        };
        if !ini.includes_structures {
            return terrain;
        }
        let surface = self.ground_or_structure_height_at(world_pos, terrain);
        let structure_h = (surface - terrain).max(0.0);
        height_die_target_world_y(terrain, &ini, structure_h) - ini.target_height
    }

    pub(in super::super) fn cached_pathfind_height(
        &self,
        world_pos: Vec3,
        cache: &PathfindingHeightSamples,
    ) -> Option<f32> {
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
    ///
    /// C++ `W3DBridge::load` (`W3DBridgeBuffer.cpp:182-191`) looks up
    /// `TheTerrainRoads->findBridge` and uses `BridgeModelName` + `BridgeScale`
    /// plus the four `TowerObjectName*` slots. Freeze that authored identity
    /// here so presentation bake cannot invent granite `RoadType::StoneBridge`.
    pub fn terrain_bridge_segments_snapshot(&self) -> Vec<(Vec3, Vec3, f32, String)> {
        let Ok(terrain) = gamelogic::terrain::get_terrain_logic().read() else {
            return Vec::new();
        };
        let roads = game_engine::common::ini::try_get_terrain_roads();
        terrain
            .bridge_data_snapshot()
            .into_iter()
            .map(|bridge| {
                let identity = roads
                    .as_ref()
                    .and_then(|roads| roads.find_bridge(&bridge.template_name))
                    .map(|tmpl| {
                        encode_authored_bridge_visual(
                            &bridge.template_name,
                            tmpl.bridge_model_name.as_str(),
                            tmpl.bridge_scale,
                            [
                                tmpl.tower_object_name[0].as_str(),
                                tmpl.tower_object_name[1].as_str(),
                                tmpl.tower_object_name[2].as_str(),
                                tmpl.tower_object_name[3].as_str(),
                            ],
                        )
                    })
                    .unwrap_or(bridge.template_name);
                (
                    Vec3::new(bridge.from.x, bridge.from.z, bridge.from.y),
                    Vec3::new(bridge.to.x, bridge.to.z, bridge.to.y),
                    bridge.width,
                    identity,
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

    /// C++ `Pathfinder::queueForPath` residual for save/load.
    pub fn snapshot_pending_host_paths(&self) -> Vec<super::pathfinding::PendingHostPath> {
        self.pathfinding_system.pending_paths().cloned().collect()
    }

    /// Re-queue deferred pathfind requests after load.
    pub fn restore_pending_host_paths(
        &mut self,
        paths: impl IntoIterator<Item = super::pathfinding::PendingHostPath>,
    ) {
        for req in paths {
            let _ = self.pathfinding_system.queue_path(req);
        }
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
        self.pathfinding_system.set_terrain_height_samples(
            width as i32,
            height as i32,
            heights.to_vec(),
        );

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
            self.copy_crate_water_into_host_terrain();
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

    /// Copy crate `TerrainLogic` water handles/polygons into live host
    /// `TerrainData.water_plane_y` / water polygons.
    /// C++ `TerrainLogic::isUnderwater` / `getWaterHandle` (TerrainLogic.cpp:2119-2160).
    pub(in super::super) fn copy_crate_water_into_host_terrain(&mut self) {
        if let Some(terrain) = self.terrain.as_mut() {
            terrain.copy_water_from_global_crate_terrain_logic();
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

    /// Block one structure footprint without full rebuild.
    /// C++ `addObjectToPathfindMap` at dozer/worker placement, including scaffolds.
    pub(in super::super) fn block_structure_object_path(&mut self, object_id: ObjectId) {
        let Some(obj) = self.objects.get(&object_id) else {
            return;
        };
        if !obj.is_kind_of(KindOf::Structure) || !obj.is_alive() {
            return;
        }
        self.pathfinding_system.classify_and_pinch_object(obj);
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
    pub(in super::super) fn seed_pathfinding_from_terrain(&mut self) {
        #[cfg(feature = "game_client")]
        {
            let Some(terrain) = self.terrain.as_ref() else {
                return;
            };

            self.pathfinding_system.clear_static_blocks();

            // C++ Pathfinder::classifyMapCell (AIPathfind.cpp:4491-4521):
            // cliff at top-left, water if any of 4 corners (water wins).
            // No terrain-slope Impassable gate.
            let grid_size = self.pathfinding_system.grid.grid_size();
            let grid_origin = self.pathfinding_system.grid.origin();

            let (min, max) = terrain.world_bounds();
            let min_x = min.x;
            let min_z = min.z;
            let max_x = max.x;
            let max_z = max.z;

            let width = self.pathfinding_system.grid.width();
            let height = self.pathfinding_system.grid.height();
            for y in 0..height {
                for x in 0..width {
                    let tl = Vec3::new(
                        grid_origin.x + x as f32 * grid_size,
                        0.0,
                        grid_origin.z + y as f32 * grid_size,
                    );
                    let pos = super::pathfinding::GridPos::new(x, y);
                    let center = Vec3::new(tl.x + 0.5 * grid_size, 0.0, tl.z + 0.5 * grid_size);

                    if center.x < min_x || center.x > max_x || center.z < min_z || center.z > max_z
                    {
                        self.pathfinding_system.grid.set_cell_type(
                            pos,
                            gamelogic::ai::pathfind_astar::PathfindCellType::Impassable,
                        );
                        continue;
                    }

                    let brx = tl.x + grid_size;
                    let brz = tl.z + grid_size;
                    let cliff = terrain.is_cliff_at_world(tl);
                    let water = terrain.is_underwater_at_world(tl)
                        || terrain.is_underwater_at_world(Vec3::new(tl.x, 0.0, brz))
                        || terrain.is_underwater_at_world(Vec3::new(brx, 0.0, brz))
                        || terrain.is_underwater_at_world(Vec3::new(brx, 0.0, tl.z));
                    let ty = super::pathfinding::PathfindingGrid::classify_map_cell(cliff, water);
                    self.pathfinding_system.grid.set_cell_type(pos, ty);
                }
            }
            self.stamp_live_bridge_decks_and_zones();
        }
    }

    /// C++ addBridge classifyCells + classifyMap pinch + zone rebuild on the live host grid.
    pub(in super::super) fn stamp_live_bridge_decks_and_zones(&mut self) {
        self.register_landmark_bridges_from_spawned_objects();
        self.ensure_generic_bridge_objects();
        self.pathfinding_system.grid.pinch_tighten_cliffs();
        if let Ok(terrain) = gamelogic::terrain::get_terrain_logic().read() {
            terrain.for_each_bridge(|bridge| {
                let info = bridge.get_bridge_info();
                let destroyed = info.cur_damage_state == gamelogic::common::BodyDamageType::Rubble;
                // C++ Coord3D ground is XY / height Z; host path grid is XZ / height Y.
                self.pathfinding_system.grid.stamp_bridge_deck(
                    Vec3::new(info.from_left.x, info.from_left.z, info.from_left.y),
                    Vec3::new(info.from_right.x, info.from_right.z, info.from_right.y),
                    Vec3::new(info.to_left.x, info.to_left.z, info.to_left.y),
                    Vec3::new(info.to_right.x, info.to_right.z, info.to_right.y),
                    destroyed,
                );
                self.pathfinding_system.grid.bind_bridge_layer_object_id(
                    Vec3::new(info.from_left.x, info.from_left.z, info.from_left.y),
                    Vec3::new(info.from_right.x, info.from_right.z, info.from_right.y),
                    Vec3::new(info.to_left.x, info.to_left.z, info.to_left.y),
                    Vec3::new(info.to_right.x, info.to_right.z, info.to_right.y),
                    info.bridge_object_id,
                );
            });
        }
        self.sync_host_bridge_rubble_and_scaffolds();
        self.pathfinding_system.grid.rebuild_terrain_zones();
        self.pathfinding_system.grid.rebuild_path_zones();
    }

    pub(crate) fn ensure_generic_bridge_objects(&mut self) {
        self.ensure_named_bridge_template("GenericBridge", 300.0);
        let mut jobs = Vec::new();
        if let Ok(tl) = gamelogic::terrain::get_terrain_logic().read() {
            tl.for_each_bridge(|bridge| {
                let info = bridge.get_bridge_info();
                if info.bridge_object_id == 0
                    || !self.objects.contains_key(&ObjectId(info.bridge_object_id))
                {
                    jobs.push(info.clone());
                }
            });
        }
        for info in jobs {
            let cx = (info.from_left.x + info.to_right.x) * 0.5;
            let cy = (info.from_left.y + info.to_right.y) * 0.5;
            let cz = (info.from_left.z + info.to_right.z) * 0.5;
            let pos = Vec3::new(cx, cz, cy);
            let Some(id) = self.create_object("GenericBridge", Team::Neutral, pos) else {
                continue;
            };
            let angle =
                (info.to_left.y - info.from_left.y).atan2(info.to_left.x - info.from_left.x);
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.set_orientation(angle);
            }
            self.bridge_behavior.register_span(
                id,
                Vec3::new(info.from_left.x, info.from_left.z, info.from_left.y),
                Vec3::new(info.from_right.x, info.from_right.z, info.from_right.y),
                Vec3::new(info.to_left.x, info.to_left.z, info.to_left.y),
                Vec3::new(info.to_right.x, info.to_right.z, info.to_right.y),
            );
            if let Ok(mut tl) = gamelogic::terrain::get_terrain_logic().write() {
                tl.bind_bridge_object_id_at(info.from_left, id.0);
            }
        }
    }

    pub(crate) fn ensure_named_bridge_template(&mut self, name: &str, health: f32) {
        if self.templates.contains_key(name) {
            return;
        }
        let mut t = ThingTemplate::new(name);
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Bridge)
            .add_kind_of(KindOf::Attackable)
            .set_health(health);
        self.templates.insert(name.to_string(), t);
    }

    pub(in super::super) fn host_object_is_landmark_bridge(obj: &super::Object) -> bool {
        if obj.is_kind_of(KindOf::BridgeTower) {
            return false;
        }
        let n = obj.template_name.to_ascii_lowercase();
        if n.contains("tower")
            || n.contains("scaffold")
            || n.eq_ignore_ascii_case("genericbridge")
            || n.contains("waterwave")
        {
            return false;
        }
        leftover_template_is_landmark_bridge(&obj.template_name)
            || obj.is_kind_of(KindOf::LandmarkBridge)
            || obj.get_template().is_kind_of(KindOf::LandmarkBridge)
            || obj.is_kind_of(KindOf::Bridge)
            || n.contains("landmark")
    }

    /// Safety net for objects not in `spawned_map_object_ids` (side builds).
    pub(crate) fn register_landmark_bridges_from_spawned_objects(&mut self) {
        let jobs: Vec<(ObjectId, String)> = self
            .objects
            .values()
            .filter(|obj| Self::host_object_is_landmark_bridge(obj))
            .map(|obj| (obj.id, obj.template_name.clone()))
            .collect();
        for (id, template_name) in jobs {
            self.register_landmark_bridge_object(id, &template_name);
        }
    }

    /// C++ GameLogic.cpp:1640-1688 post-map-load landmark-bridge pass.
    /// `thingTemplate->isBridge()` / `KINDOF_LANDMARK_BRIDGE` map objects
    /// become leftover TerrainLogic spans via `addLandmarkBridgeToLogic`.
    pub(crate) fn register_spawned_landmark_bridges(
        &mut self,
        objects: &[super::script_loader::PlacedObject],
        spawned_object_ids: &[(ObjectId, usize)],
    ) {
        let jobs: Vec<(ObjectId, String)> = spawned_object_ids
            .iter()
            .filter_map(|&(id, index)| {
                let placed = objects.get(index)?;
                let name = placed.template.as_str();
                if self.template_is_landmark_bridge(name, id) {
                    Some((id, name.to_string()))
                } else {
                    None
                }
            })
            .collect();
        for (id, template_name) in jobs {
            self.register_landmark_bridge_object(id, &template_name);
        }
    }

    pub(in super::super) fn template_is_landmark_bridge(
        &self,
        template_name: &str,
        object_id: ObjectId,
    ) -> bool {
        if leftover_template_is_landmark_bridge(template_name) {
            return true;
        }
        if let Some(tmpl) = self.templates.get(template_name) {
            if tmpl.is_kind_of(KindOf::LandmarkBridge) {
                return true;
            }
        }
        self.objects.get(&object_id).is_some_and(|obj| {
            obj.is_kind_of(KindOf::LandmarkBridge)
                || obj.get_template().is_kind_of(KindOf::LandmarkBridge)
        })
    }

    pub(in super::super) fn register_landmark_bridge_object(
        &mut self,
        id: ObjectId,
        template_name: &str,
    ) {
        if leftover_bridge_info_for_object(id.0).is_some() {
            return;
        }
        let Some(obj) = self.objects.get(&id) else {
            return;
        };
        let pos = obj.get_position();
        let leftover_pos = gamelogic::common::Coord3D::new(pos.x, pos.z, pos.y);
        let angle = obj.get_orientation();
        let (half_x, half_y) = landmark_bridge_half_sizes(template_name, obj);
        let team = obj.team;
        if let Ok(mut terrain) = gamelogic::terrain::get_terrain_logic().write() {
            terrain.add_landmark_bridge_from_geometry(
                leftover_pos,
                angle,
                half_x,
                half_y,
                id.0,
                gamelogic::common::AsciiString::from(template_name),
            );
        }
        let Some(info) = leftover_bridge_info_for_object(id.0) else {
            return;
        };
        self.bridge_behavior.register_span(
            id,
            Vec3::new(info.from_left.x, info.from_left.z, info.from_left.y),
            Vec3::new(info.from_right.x, info.from_right.z, info.from_right.y),
            Vec3::new(info.to_left.x, info.to_left.z, info.to_left.y),
            Vec3::new(info.to_right.x, info.to_right.z, info.to_right.y),
        );
        self.create_landmark_bridge_towers(id, team, angle, &info);
    }

    /// C++ `Bridge::Bridge(Object*)` creates four targetable tower objects.
    pub(in super::super) fn create_landmark_bridge_towers(
        &mut self,
        bridge_id: ObjectId,
        team: Team,
        bridge_angle: f32,
        info: &gamelogic::terrain::BridgeInfo,
    ) {
        let Some(roads) = game_engine::common::ini::try_get_terrain_roads() else {
            return;
        };
        let Some(bridge_tmpl) = leftover_bridge_template_name(bridge_id.0)
            .and_then(|name| roads.find_bridge(&name).cloned())
            .or_else(|| {
                self.objects
                    .get(&bridge_id)
                    .and_then(|obj| roads.find_bridge(&obj.template_name).cloned())
            })
        else {
            return;
        };
        drop(roads);

        let bridge_indestructible = self
            .objects
            .get(&bridge_id)
            .is_some_and(|o| o.indestructible);

        let mut width = gamelogic::common::Coord3D::new(
            info.to_left.x - info.to_right.x,
            info.to_left.y - info.to_right.y,
            0.0,
        );
        let len = (width.x * width.x + width.y * width.y).sqrt();
        if len > f32::EPSILON {
            width.x /= len;
            width.y /= len;
        }
        let corners = [info.from_left, info.from_right, info.to_left, info.to_right];
        let mut tower_ids = [0u32; 4];
        for (index, corner) in corners.iter().enumerate() {
            let name = bridge_tmpl.tower_object_name[index].as_str();
            if name.is_empty() {
                continue;
            }
            let mut offset = gamelogic::path::PATHFIND_CELL_SIZE_F * 0.5;
            if let Some(factory) =
                game_engine::common::thing::thing_factory::try_get_thing_factory()
            {
                if let Some(factory) = factory.as_ref() {
                    if let Some(tower_tmpl) = factory.find_template(name, false) {
                        let radius = tower_tmpl.get_template_geometry_info().major_radius();
                        if radius > 0.0 {
                            offset = radius;
                        }
                    }
                }
            }
            let sign = if index == 0 || index == 2 { 1.0 } else { -1.0 };
            let leftover_pos = gamelogic::common::Coord3D::new(
                corner.x + width.x * offset * sign,
                corner.y + width.y * offset * sign,
                corner.z,
            );
            let host_pos = Vec3::new(leftover_pos.x, leftover_pos.z, leftover_pos.y);
            let Some(tower_id) = self.create_object(name, team, host_pos) else {
                continue;
            };
            let tower_angle = if index < 2 {
                bridge_angle + std::f32::consts::PI
            } else {
                bridge_angle
            };
            if let Some(tower) = self.objects.get_mut(&tower_id) {
                tower.set_orientation(tower_angle);
                if bridge_indestructible {
                    tower.set_indestructible(true);
                }
            }

            tower_ids[index] = tower_id.0;
        }
        if let Ok(mut terrain) = gamelogic::terrain::get_terrain_logic().write() {
            terrain.for_each_bridge_mut(|bridge| {
                if bridge.get_bridge_info().bridge_object_id != bridge_id.0 {
                    return;
                }
                for (index, tower_id) in tower_ids.iter().copied().enumerate() {
                    if tower_id == 0 {
                        continue;
                    }
                    if let Some(which) = gamelogic::common::BridgeTowerType::from_index(index) {
                        bridge.set_tower_object_id(tower_id, which);
                    }
                }
            });
        }
    }
}
