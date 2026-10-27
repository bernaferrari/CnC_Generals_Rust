//! Additional `impl GameLogic` methods. Child of `game_logic.rs`.
#![allow(unused_imports, non_snake_case)]
use super::*;

#[cfg(test)]
#[path = "script_status_owner_tests.rs"]
mod script_status_owner_tests;

#[cfg(test)]
#[path = "replay_observer_color_tests.rs"]
mod replay_observer_color_tests;

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
    }

    pub(in super::super) fn apply_spawned_object_weather(
        &mut self,
        id: ObjectId,
        object_weather: i32,
    ) {
        let follow = self.lighting_rules().force_models_to_follow_weather;
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
        // Classification only. The immutable map definition is admitted by
        // the receiving native terrain owner, never by detached world loading.
        obj.terrain_scorch().is_some()
    }

    /// C++ `Object::updateObjValuesFromMapProperties` at live map spawn.
    /// Apply the Dict to this session's actual host object.
    pub(in super::super) fn apply_update_obj_values_from_map_properties(
        &mut self,
        id: ObjectId,
        properties: &game_engine::common::dict::Dict,
    ) {
        self.apply_update_obj_values_from_map_properties_with_effects(
            id,
            properties,
            &mut Default::default(),
        )
    }

    pub(in super::super) fn apply_update_obj_values_from_map_properties_with_effects(
        &mut self,
        id: ObjectId,
        properties: &game_engine::common::dict::Dict,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use game_engine::common::dict::DictType;
        use game_engine::common::name_key_generator::NameKeyGenerator;
        use game_engine::common::well_known_keys;

        if properties.get_pair_count() == 0 {
            return;
        }

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
                }
            }
        }

        if let Some(max_hps) = get_int(well_known_keys::key_object_max_hps()) {
            if max_hps >= 0 {
                if let Some(created) = self.objects.get_mut(&id) {
                    let new_max = max_hps as f32;
                    created.set_body_max_health_with_mode(
                        new_max,
                        crate::game_logic::object::MaxHealthChange::PreserveRatio,
                    );
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
            let health_bonus = self.veterancy_health_bonus;
            self.with_object_veterancy_feedback(id, |created, audio, feedback| {
                if created.is_trainable() {
                    let level = match veterancy.clamp(0, 3) {
                        0 => crate::game_logic::VeterancyLevel::Rookie,
                        1 => crate::game_logic::VeterancyLevel::Veteran,
                        2 => crate::game_logic::VeterancyLevel::Elite,
                        _ => crate::game_logic::VeterancyLevel::Heroic,
                    };
                    let _ = created.set_min_veterancy_level_with_health_bonus_table(
                        level,
                        &health_bonus,
                        audio,
                        feedback,
                    );
                }
            });
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
                created.set_script_disabled_with_partition_owner(
                    !enabled,
                    &mut self.partition_manager,
                );
            }
        }

        if let Some(powered) = get_bool(well_known_keys::key_object_powered()) {
            if let Some(created) = self.objects.get_mut(&id) {
                created.set_script_underpowered_with_partition_owner(
                    !powered,
                    &mut self.partition_manager,
                );
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

        if let Some(targetable) = get_bool(well_known_keys::key_object_targetable()) {
            if let Some(created) = self.objects.get_mut(&id) {
                created.set_script_targetable(targetable);
            }
        }

        if let Some(visual_range) = get_int(well_known_keys::key_object_visual_range()) {
            if let Some(created) = self.objects.get_mut(&id) {
                created.vision_range = (visual_range as f32).max(0.0);
            }
        }

        if let Some(shroud_range) = get_int(well_known_keys::key_object_shroud_clearing_distance())
        {
            if let Some(created) = self.objects.get_mut(&id) {
                created.set_shroud_clearing_range_with_partition_owner(
                    (shroud_range as f32).max(0.0),
                    &mut self.partition_manager,
                );
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
            self.apply_upgrade_to_object_with_effects(id, &upgrade_name, effects);
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

    /// Authored presentation intent; an explicit Client loan delivers physical snow.
    pub fn set_weather_visible(&mut self, visible: bool) {
        self.weather_state.visible = visible;
    }

    pub fn queue_pending_special_ability(
        &mut self,
        object_id: ObjectId,
        ability: PendingSpecialAbility,
    ) {
        self.queue_pending_special_ability_with_effects(object_id, ability, &mut Default::default())
    }

    pub(crate) fn queue_pending_special_ability_with_effects(
        &mut self,
        object_id: ObjectId,
        ability: PendingSpecialAbility,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if Self::leftover_sa_exclusive_pending(ability) {
            self.abort_leftover_sa_channel_on_new_order_with_effects(object_id, effects);
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
        if let Some(id) = self.replay_observer_player_id() {
            return id;
        }
        let id = self.next_side_player_id();
        let mut observer = Player::new(id, Team::Neutral, "ReplayObserver", true);
        // C++ GameLogic.cpp:2222-2230 selects this human observer only in GAME_REPLAY.
        // Admission alone must preserve the local participant in a live match.
        observer.is_local = false;
        observer.resources.supplies = self.skirmish_rules.starting_cash;
        observer.map_side.role = PlayerSideRole::ReplayObserver;
        observer.map_side.map_player_name = "ReplayObserver".into();
        observer.is_alive = false;
        observer.start_position = 0;
        observer.alliance_team = -1;
        // CPP GameLogic.cpp1449–1450: the synthetic ReplayObserver uses
        // authored palette index zero, independently of occupied observer slots.
        // Retain the existing fallback when definitions are unavailable.
        // Native boot admits the authored palette before player setup.
        let (day, night) =
            game_engine::common::ini::ini_multiplayer::with_multiplayer_settings(|settings| {
                let rgb = |packed: u32| {
                    (
                        ((packed >> 16) & 255) as u8,
                        ((packed >> 8) & 255) as u8,
                        (packed & 255) as u8,
                    )
                };
                settings
                    .color_definitions
                    .first()
                    .map(|color| (rgb(color.get_color()), rgb(color.night_color)))
                    .unwrap_or(((170, 170, 170), (170, 170, 170)))
            });
        observer.color_rgb = day;
        observer.color_night_rgb = night;

        self.add_player(observer);
        if let Some(identity) = PlayerTemplateIdentity::from_exact_name("FactionObserver") {
            let _ = self.bind_player_template_identity(id, identity);
            if let Some(player) = self.players.get_mut(&id) {
                player.is_alive = false;
                player.name = "ReplayObserver".into();
                player.team = Team::Neutral;
                // Template initialization must not overwrite selected side facts.
                player.color_rgb = day;
                player.color_night_rgb = night;
            }
        }
        id
    }

    /// C++ GameLogic.cpp:1703-1705 permanent reveal for ReplayObserver.
    pub fn reveal_replay_observer_map(&mut self) {
        let Some(id) = self.replay_observer_player_id() else {
            return;
        };
        if let Ok(mut shroud) = self.world_services.shroud().lock() {
            let _ = shroud.reveal_map_for_player_permanently(id);
        }
    }

    /// C++ GameLogic.cpp:1436-1459 + PlayerList::newGame for ReplayObserver.
    pub fn install_replay_observer_side(&mut self) {
        let host_id = self.ensure_replay_observer_player();
        // Project the receiving host's already admitted colors. Do not re-read
        // definitions or infer a palette index from RGB at this bridge.
        let host = self.players.get(&host_id).expect("admitted ReplayObserver");
        let (day, night) = (host.color_rgb, host.color_night_rgb);
        let packed = |(r, g, b): (u8, u8, u8)| {
            (0xff00_0000u32 | ((r as u32) << 16) | ((g as u32) << 8) | b as u32) as i32
        };
        let mut d = Dict::new();
        d.set_ascii_string(key_player_name(), "ReplayObserver");
        d.set_bool(key_player_is_human(), true);
        d.set_unicode_string(key_player_display_name(), "Observer");
        d.set_ascii_string(key_player_faction(), "FactionObserver");
        d.set_ascii_string(key_player_allies(), String::new());
        d.set_ascii_string(key_player_enemies(), String::new());
        d.set_int(key_multiplayer_start_index(), 0);
        d.set_int(key_player_color(), packed(day));
        d.set_int(key_player_night_color(), packed(night));

        {
            let sides = &mut self.prepared_map_sides;
            if let Some(index) = sides.find_side_info("ReplayObserver") {
                let existing = sides
                    .get_side_info_mut(index)
                    .expect("existing observer side")
                    .get_dict_mut();
                existing.set_int(key_player_color(), packed(day));
                existing.set_int(key_player_night_color(), packed(night));
            } else {
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
        // Temporary adapter for the still-live native observer/score consumers.
        // All identity and color facts came from this receiving Main Player.
        let core_colors = (
            gamelogic::common::Color::rgb(day.0, day.1, day.2),
            gamelogic::common::Color::rgb(night.0, night.1, night.2),
        );
        let existing = ThePlayerList()
            .read()
            .ok()
            .and_then(|list| list.get_player(host_id as i32).cloned());
        if let Some(existing) = existing {
            if let Ok(mut player) = existing.write() {
                player.set_colors(core_colors.0, core_colors.1);
            }
        } else {
            let mut logic_player = LogicPlayer::new(host_id as i32);
            logic_player.set_player_name_key(NameKeyGenerator::name_to_key("ReplayObserver"));
            logic_player.set_display_name("Observer");
            logic_player.set_side("Observer");
            logic_player.set_base_side("Observer");
            logic_player.set_observer(true);
            logic_player.set_player_type(LogicPlayerType::Observer, false);
            game_engine::common::ini::ensure_player_templates_loaded();
            let observer_template = {
                let templates = get_player_template_store();
                templates
                    .find_template("FactionObserver")
                    .map(|common| std::sync::Arc::new(LogicPlayerTemplate::from_common(common)))
            };
            if let Some(template) = observer_template {
                logic_player.init(template);
                logic_player.set_observer(true);
                logic_player.set_player_type(LogicPlayerType::Observer, false);
            }
            logic_player.set_colors(core_colors.0, core_colors.1);
            if let Ok(mut list) = ThePlayerList().write() {
                list.add_player(std::sync::Arc::new(std::sync::RwLock::new(logic_player)));
            }
        }
    }

    /// C++ GameLogic.cpp:2222-2230 GAME_REPLAY: setLocalPlayer(ReplayObserver),
    /// TheRadar->forceOn(TRUE), refreshShroudForLocalPlayer, observer control bar.
    /// Select only an already admitted observer on a detached restore owner.
    /// No default Player, Core publication, UI, or shroud mutation occurs here.
    pub(crate) fn select_admitted_replay_observer_local(&mut self) -> Result<(), String> {
        if !matches!(self.game_mode, GameMode::Replay) {
            return Ok(());
        }
        let id = self
            .replay_observer_player_id()
            .ok_or_else(|| "Replay restore has no admitted ReplayObserver Player".to_string())?;
        for player in self.players.values_mut() {
            player.is_local = player.id == id;
        }
        Ok(())
    }

    pub fn apply_replay_observer_as_local_player(&mut self) {
        if !matches!(self.game_mode, GameMode::Replay) {
            return;
        }
        let observer_id = self.ensure_replay_observer_player();
        for player in self.players.values_mut() {
            player.is_local = player.id == observer_id;
        }
        // Temporary native consumer adapter; Main selected the identity above.
        if let Ok(mut list) = ThePlayerList().write() {
            if list.get_player(observer_id as i32).is_some() {
                list.set_local_player_index(observer_id as i32);
            }
        }
        self.set_radar_forced(true);
        {
            let radar = &mut self.radar;
            radar.force_on(true);
        }
        if let Ok(mut shroud) = self.world_services.shroud().lock() {
            shroud.refresh_radar_shroud_for_player_into(observer_id, self.frame, &mut self.radar);
        }
        // Native admission selects the receiving observer scheme after this
        // world candidate commits; constructing/loading a candidate never emits UI.
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
        {
            let sides = &mut self.prepared_map_sides;
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
        if let Some(engine) = &self.owned_script_engine {
            for (idx, list) in self.loaded_script_lists.iter().enumerate() {
                let _ = engine.set_script_list_for_player(idx, Some(Box::new(list.clone())));
            }
        }
    }

    pub fn replay_observer_player_id(&self) -> Option<u32> {
        self.players
            .iter()
            .filter_map(|(&id, player)| player.is_replay_observer().then_some(id))
            .min()
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
        let owner = self.world_services.terrain();
        let terrain = owner.read().unwrap_or_else(|error| error.into_inner());
        self.terrain_height_at_with_terrain_logic(world_pos, Some(&terrain))
    }

    pub(in super::super) fn terrain_height_at_with_terrain_logic(
        &self,
        world_pos: Vec3,
        terrain: Option<&gamelogic::terrain::TerrainLogic>,
    ) -> Option<f32> {
        // None means no loan was supplied, not that this world has no map.
        // In particular, older callers using read().ok() must not turn a
        // poisoned known owner into stale detached ground or a missing deck.
        if terrain.is_none() {
            return self.terrain_height_at(world_pos);
        }
        // A supplied receiving loan must not reenter the same terrain lock.
        let ground = self.terrain_ground_height_with_terrain_logic(world_pos, terrain);
        // C++ getLayerHeight: non-rubble deck plane wins when the XY is on a span.
        if let Some(tl) = terrain {
            if let Some(deck) = tl.host_deck_height_at(world_pos.x, world_pos.z) {
                if ground.map_or(true, |g| deck > g) {
                    return Some(deck);
                }
            }
        }
        ground
    }

    /// C++ raw map ground sampling; no bridge layer or presentation remapping.
    pub(crate) fn raw_terrain_ground_height_at(&self, position: Vec3) -> f32 {
        let owner = self.world_services.terrain();
        let terrain = owner.read().unwrap_or_else(|error| error.into_inner());
        if terrain.has_height_map() {
            return terrain.get_ground_height(position.x, position.z, None);
        }
        // Detached fixtures without an admitted Core map retain their raw
        // fixed-cell sampler. Loaded production never reads this copy.
        crate::game_logic::game_logic::FlightTerrainView {
            // This fallback runs while the receiving read loan is held.
            logical: None,
            grid: &self.pathfinding_system.grid,
            terrain: self.terrain.as_ref(),
            samples: self.pathfinding_height_samples.as_ref(),
        }
        .raw_ground(position, 0.0)
    }

    /// Original getGroundHeight without the bridge deck override used by
    /// getLayerHeight. Positional effects/crushing must not climb a deck.
    pub(crate) fn terrain_ground_height_at(&self, world_pos: Vec3) -> Option<f32> {
        let owner = self.world_services.terrain();
        let terrain = owner.read().unwrap_or_else(|error| error.into_inner());
        self.terrain_ground_height_with_terrain_logic(world_pos, Some(&terrain))
    }

    fn terrain_ground_height_with_terrain_logic(
        &self,
        world_pos: Vec3,
        terrain: Option<&gamelogic::terrain::TerrainLogic>,
    ) -> Option<f32> {
        if let Some(terrain) = terrain.filter(|terrain| terrain.has_height_map()) {
            return Some(terrain.get_ground_height(world_pos.x, world_pos.z, None));
        }
        // Preserve authored detached/no-map fixture behavior only. There is no
        // second height authority once this world admits its Core map.
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
    }

    /// Query the driving logical terrain. Detached host terrain retains its
    /// local water fallback only when this owner has no admitted logical map.
    pub(crate) fn is_underwater_at(&self, world_pos: Vec3) -> bool {
        let owner = self.world_services.terrain();
        let terrain = owner.read().unwrap_or_else(|e| e.into_inner());
        if terrain.has_height_map() || terrain.get_water_handle(world_pos.x, world_pos.z).is_some()
        {
            return terrain.is_underwater(world_pos.x, world_pos.z, None, None);
        }
        self.terrain
            .as_ref()
            .is_some_and(|t| t.is_underwater_at_world(world_pos))
    }

    pub(crate) fn water_surface_at(&self, world_pos: Vec3) -> Option<f32> {
        let owner = self.world_services.terrain();
        let terrain = owner.read().unwrap_or_else(|e| e.into_inner());
        if terrain.get_water_handle(world_pos.x, world_pos.z).is_some() {
            let mut height = 0.0;
            terrain.is_underwater(world_pos.x, world_pos.z, Some(&mut height), None);
            return Some(height);
        }
        if terrain.has_height_map() {
            return None;
        }
        self.terrain
            .as_ref()
            .and_then(|t| t.water_surface_at_world(world_pos))
    }

    /// C++ `Locomotor::getSurfaceHtAtPt` — water surface when underwater, else terrain.
    pub fn surface_ht_at(&self, world_pos: Vec3) -> Option<f32> {
        if self.is_underwater_at(world_pos) {
            if let Some(water_y) = self.water_surface_at(world_pos) {
                return Some(water_y);
            }
        }
        self.terrain_height_at(world_pos)
    }

    /// C++ `PartitionManager::getGroundOrStructureHeight` (`PartitionManager.cpp:4674`).
    /// Tallest live `KINDOF_STRUCTURE` whose 2D bounding sphere is within 1wu,
    /// resolved only from this receiving partition and canonical roster.
    pub fn ground_or_structure_height_at(&self, world_pos: Vec3, terrain_y: f32) -> f32 {
        const RANGE: f32 = 1.0;
        let structure_height = |other: &Object| -> Option<f32> {
            if !other.is_kind_of(KindOf::Structure) {
                return None;
            }
            let op = other.get_position();
            let dx = op.x - world_pos.x;
            let dz = op.z - world_pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let geom = &(*other.geometry_info());
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
            for other in self.objects.registered_values() {
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
        terrain_y + tallest
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
        let owner = self.world_services.terrain();
        let terrain = owner.read().unwrap_or_else(|error| error.into_inner());
        self.terrain_heightmap_snapshot_with_terrain_logic(&terrain)
    }

    /// Reuse the caller's actual receiving loan; the cache reads revision and
    /// freezes these exact bytes under one loan rather than recursively locking.
    #[cfg(feature = "game_client")]
    pub(crate) fn terrain_heightmap_snapshot_with_terrain_logic(
        &self,
        terrain: &gamelogic::terrain::TerrainLogic,
    ) -> Option<game_client::terrain::height_map::HeightMap> {
        self.try_terrain_heightmap_snapshot_with_terrain_logic(terrain)
            .expect("admitted Core terrain byte grid must remain valid")
    }

    #[cfg(feature = "game_client")]
    pub(crate) fn try_terrain_heightmap_snapshot_with_terrain_logic(
        &self,
        terrain: &gamelogic::terrain::TerrainLogic,
    ) -> Result<Option<game_client::terrain::height_map::HeightMap>, String> {
        if terrain.has_height_map() {
            let (width, height) = terrain.logic_height_map_extents();
            return crate::game_logic::terrain::TerrainData::heightmap_from_logic_samples(
                self.terrain.as_ref(),
                width,
                height,
                terrain.logic_height_map_border_size(),
                terrain.logic_height_map_bytes(),
            )
            .map(Some);
        }
        Ok(self
            .terrain
            .as_ref()
            .map(|terrain| terrain.heightmap_clone()))
    }

    /// Snapshot map bridge spans converted to runtime world-space vectors for visual road parity.
    ///
    /// C++ `W3DBridge::load` (`W3DBridgeBuffer.cpp:182-191`) looks up
    /// `TheTerrainRoads->findBridge` and uses `BridgeModelName` + `BridgeScale`
    /// plus the four `TowerObjectName*` slots. Freeze that authored identity
    /// here so presentation bake cannot invent granite `RoadType::StoneBridge`.
    pub fn terrain_bridge_segments_snapshot(&self) -> Vec<(Vec3, Vec3, f32, String)> {
        let terrain_owner_handle = self.world_services.terrain().clone();
        let Ok(terrain) = terrain_owner_handle.read() else {
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

    /// C++ WaterRenderObjClass::renderWater uses every authored water polygon,
    /// including river flag/start and per-point Z. Instance owned map data.
    pub fn terrain_water_triggers_snapshot(&self) -> &[gamelogic::polygon_trigger::PolygonTrigger] {
        &self.runtime_water_triggers
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
        let width = self.pathfinding_system.grid.width().max(0) as u32;
        let height = self.pathfinding_system.grid.height().max(0) as u32;
        if width == 0 || height == 0 {
            return None;
        }
        let terrain = self
            .world_services
            .terrain()
            .read()
            .unwrap_or_else(|error| error.into_inner());
        if terrain.has_height_map() || self.terrain.is_some() {
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
                    samples
                        .push(self.terrain_ground_height_with_terrain_logic(pos, Some(&terrain))?);
                }
            }
            return Some(samples);
        }
        let cache = self.pathfinding_height_samples.as_ref()?;
        (cache.width == width && cache.height == height).then_some(cache.values.clone())
    }

    /// Restore coarse terrain heights from a grid snapshot (used to recover post-load height queries).
    pub fn restore_terrain_heights_from_grid(
        &mut self,
        width: u32,
        height: u32,
        heights: &[f32],
    ) -> bool {
        self.restore_terrain_heights_from_grid_with_effects(
            width,
            height,
            heights,
            &mut Default::default(),
        )
    }

    pub(crate) fn restore_terrain_heights_from_grid_with_effects(
        &mut self,
        width: u32,
        height: u32,
        heights: &[f32],
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
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

        // Raw loaded bytes remain canonical. Coarse save samples are a
        // mapless/old-save fallback, never replacement authored geometry.
        if self
            .world_services
            .terrain()
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .has_height_map()
        {
            return true;
        }

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
            self.seed_pathfinding_from_terrain_with_effects(effects);
            let wall_height = self.pathfind_ai_rules().wall_height;
            ({
                let ground_owner = self.world_services.terrain();
                let ground_loan = ground_owner
                    .read()
                    .unwrap_or_else(|error| error.into_inner());
                let ground_source = crate::game_logic::pathfinding::PathfindingGround::for_terrain(
                    &ground_loan,
                    self.map_loaded,
                    self.terrain.as_ref(),
                    self.pathfinding_height_samples.as_ref(),
                    &self.pathfinding_system.grid,
                );
                self.pathfinding_system.apply_structure_static_blocks(
                    self.objects.read(),
                    wall_height,
                    &ground_source,
                )
            });
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
            let owner = self.world_services.terrain().clone();
            let logic = owner.read().unwrap_or_else(|e| e.into_inner());
            terrain.copy_water_from_terrain_logic(&logic);
        }
    }

    /// Re-apply structure footprints onto the static path/LOS grid.
    /// Call after map object spawn bulk and when a structure dies.
    pub fn sync_structure_path_blocks(&mut self) {
        if self.terrain.is_some() {
            // Reclassification observes admitted bridges. Bridge factories
            // belong to map/restore admission: rebuilding footprints after
            // deletion must not resurrect the Object that was just retired.
            let owner = self.world_services.terrain().clone();
            let terrain = owner.read().unwrap_or_else(|error| error.into_inner());
            self.reclassify_pathfinding_from_terrain(&terrain);
        } else {
            self.pathfinding_system.grid.reclassify_ground();
            self.refresh_owned_bridge_pathfinder_states();
        }
        let wall_height = self.pathfind_ai_rules().wall_height;
        ({
            let ground_owner = self.world_services.terrain();
            let ground_loan = ground_owner
                .read()
                .unwrap_or_else(|error| error.into_inner());
            let ground_source = crate::game_logic::pathfinding::PathfindingGround::for_terrain(
                &ground_loan,
                self.map_loaded,
                self.terrain.as_ref(),
                self.pathfinding_height_samples.as_ref(),
                &self.pathfinding_system.grid,
            );
            self.pathfinding_system.apply_structure_static_blocks(
                self.objects.read(),
                wall_height,
                &ground_source,
            )
        });
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
        ({
            let ground_owner = self.world_services.terrain();
            let ground_loan = ground_owner
                .read()
                .unwrap_or_else(|error| error.into_inner());
            let ground_source = crate::game_logic::pathfinding::PathfindingGround::for_terrain(
                &ground_loan,
                self.map_loaded,
                self.terrain.as_ref(),
                self.pathfinding_height_samples.as_ref(),
                &self.pathfinding_system.grid,
            );
            self.pathfinding_system
                .classify_and_pinch_object(obj, &ground_source)
        });
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
        self.seed_pathfinding_from_terrain_with_effects(&mut Default::default())
    }

    pub(in super::super) fn seed_pathfinding_from_terrain_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // Independent callers retain the existing nonfallible surface until their
        // operation boundary is migrated; the selected factory never uses this adapter.
        if let Err(error) = self.seed_pathfinding_from_terrain_with_effects_checked(effects) {
            log::error!("{error}");
        }
    }

    pub(in super::super) fn seed_pathfinding_from_terrain_with_effects_checked(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        self.register_landmark_bridges_from_spawned_objects_with_effects_checked(effects)?;
        self.ensure_generic_bridge_objects_with_effects_checked(effects)?;
        let owner = self.world_services.terrain().clone();
        let terrain = owner.read().unwrap_or_else(|e| e.into_inner());
        self.reclassify_pathfinding_from_terrain(&terrain);

        Ok(())
    }

    /// Rebuild classified cells without changing original bridge slot identity.
    pub(crate) fn reclassify_pathfinding_from_terrain(
        &mut self,
        logical: &gamelogic::terrain::TerrainLogic,
    ) {
        let Some(terrain) = self.terrain.as_ref() else {
            return;
        };
        let extent = logical.get_extent();
        self.pathfinding_system.grid.admit_logical_extent(
            Vec3::new(extent.lo.x, extent.lo.z, extent.lo.y),
            Vec3::new(extent.hi.x, extent.hi.z, extent.hi.y),
        );
        self.pathfinding_system.grid.reclassify_ground();
        let grid = &mut self.pathfinding_system.grid;
        let size = grid.grid_size();
        let origin = grid.origin();
        let (min, max) = terrain.world_bounds();
        for y in 0..grid.height() {
            for x in 0..grid.width() {
                let tl = Vec3::new(origin.x + x as f32 * size, 0.0, origin.z + y as f32 * size);
                let center = tl + Vec3::new(size * 0.5, 0.0, size * 0.5);
                let pos = super::pathfinding::GridPos::new(x, y);
                let ty =
                    if center.x < min.x || center.x > max.x || center.z < min.z || center.z > max.z
                    {
                        gamelogic::ai::pathfind_astar::PathfindCellType::Impassable
                    } else {
                        let water = [
                            (tl.x, tl.z),
                            (tl.x, tl.z + size),
                            (tl.x + size, tl.z + size),
                            (tl.x + size, tl.z),
                        ]
                        .into_iter()
                        .any(|(x, z)| logical.is_underwater(x, z, None, None));
                        super::pathfinding::PathfindingGrid::classify_map_cell(
                            terrain.is_cliff_at_world(tl),
                            water,
                        )
                    };
                grid.set_cell_type(pos, ty);
            }
        }
        grid.pinch_tighten_cliffs();
        self.refresh_owned_bridge_pathfinder_states_with_ground(&PathfindingGround::receiving(
            logical,
        ));
        self.pathfinding_system.grid.rebuild_terrain_zones();
        self.pathfinding_system.grid.rebuild_path_zones();
    }

    pub(in super::super) fn stamp_live_bridge_decks_and_zones(&mut self) {
        self.stamp_live_bridge_decks_and_zones_with_effects(&mut Default::default())
    }

    pub(in super::super) fn stamp_live_bridge_decks_and_zones_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.register_landmark_bridges_from_spawned_objects_with_effects(effects);
        self.ensure_generic_bridge_objects_with_effects(effects);
        self.pathfinding_system.grid.pinch_tighten_cliffs();
        self.refresh_owned_bridge_pathfinder_states();
        self.pathfinding_system.grid.rebuild_terrain_zones();
        self.pathfinding_system.grid.rebuild_path_zones();
    }

    pub(crate) fn ensure_generic_bridge_objects(&mut self) {
        self.ensure_generic_bridge_objects_with_effects(&mut Default::default())
    }

    pub(crate) fn ensure_generic_bridge_objects_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // Independent callers retain the existing nonfallible surface until their
        // operation boundary is migrated; the selected factory never uses this adapter.
        if let Err(error) = self.ensure_generic_bridge_objects_with_effects_checked(effects) {
            log::error!("{error}");
        }
    }

    pub(crate) fn ensure_generic_bridge_objects_with_effects_checked(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let mut jobs = Vec::new();
        let owner = self.world_services.terrain().clone();
        if let Ok(tl) = owner.read() {
            tl.for_each_bridge(|bridge| {
                let info = bridge.get_bridge_info();
                if info.bridge_object_id == 0
                    || !self.objects.contains_key(&ObjectId(info.bridge_object_id))
                {
                    jobs.push((bridge.get_layer() as u8, info.clone()));
                }
            });
        }
        for (layer, info) in jobs {
            // Only bridge creation needs a bridge definition. Reconstructing an
            // empty terrain must not alter the receiving world's catalog.
            self.ensure_named_bridge_template("GenericBridge", 300.0);
            let cx = (info.from_left.x + info.to_right.x) * 0.5;
            let cy = (info.from_left.y + info.to_right.y) * 0.5;
            let cz = (info.from_left.z + info.to_right.z) * 0.5;
            let pos = Vec3::new(cx, cz, cy);
            let Some(id) = self.create_object_with_script_effects_checked(
                "GenericBridge",
                Team::Neutral,
                pos,
                None,
                effects,
            )?
            else {
                continue;
            };
            let angle =
                (info.to_left.y - info.from_left.y).atan2(info.to_left.x - info.from_left.x);
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.set_orientation_with_partition_owner(angle, &mut self.partition_manager);
            }
            self.pathfinding_system
                .grid
                .bind_reserved_bridge_object(layer, id.0);
            self.bridge_behavior.register_span(
                id,
                Vec3::new(info.from_left.x, info.from_left.z, info.from_left.y),
                Vec3::new(info.from_right.x, info.from_right.z, info.from_right.y),
                Vec3::new(info.to_left.x, info.to_left.z, info.to_left.y),
                Vec3::new(info.to_right.x, info.to_right.z, info.to_right.y),
            );
            if let Ok(mut tl) = owner.write() {
                tl.for_each_bridge_mut(|bridge| {
                    if bridge.get_layer() as u8 == layer
                        && bridge.get_bridge_info().bridge_index == info.bridge_index
                    {
                        bridge.set_bridge_object_id(id.0);
                    }
                });
            }
        }

        Ok(())
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
        self.register_landmark_bridges_from_spawned_objects_with_effects(&mut Default::default())
    }

    pub(crate) fn register_landmark_bridges_from_spawned_objects_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // Independent callers retain the existing nonfallible surface until their
        // operation boundary is migrated; the selected factory never uses this adapter.
        if let Err(error) =
            self.register_landmark_bridges_from_spawned_objects_with_effects_checked(effects)
        {
            log::error!("{error}");
        }
    }

    pub(crate) fn register_landmark_bridges_from_spawned_objects_with_effects_checked(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let jobs: Vec<(ObjectId, String)> = self
            .objects
            .registered_values()
            .filter(|obj| Self::host_object_is_landmark_bridge(obj))
            .map(|obj| (obj.id, obj.template_name.clone()))
            .collect();
        for (id, template_name) in jobs {
            self.register_landmark_bridge_object_with_effects_checked(id, &template_name, effects)?;
        }

        Ok(())
    }

    /// C++ GameLogic.cpp:1640-1688 post-map-load landmark-bridge pass.
    /// `thingTemplate->isBridge()` / `KINDOF_LANDMARK_BRIDGE` map objects
    /// become leftover TerrainLogic spans via `addLandmarkBridgeToLogic`.
    pub(crate) fn register_spawned_landmark_bridges(
        &mut self,
        objects: &[super::script_loader::PlacedObject],
        spawned_object_ids: &[(ObjectId, usize)],
    ) {
        self.register_spawned_landmark_bridges_with_effects(
            objects,
            spawned_object_ids,
            &mut Default::default(),
        )
    }

    pub(crate) fn register_spawned_landmark_bridges_with_effects(
        &mut self,
        objects: &[super::script_loader::PlacedObject],
        spawned_object_ids: &[(ObjectId, usize)],
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // Independent callers retain the existing nonfallible surface until their
        // operation boundary is migrated; the selected factory never uses this adapter.
        if let Err(error) = self.register_spawned_landmark_bridges_with_effects_checked(
            objects,
            spawned_object_ids,
            effects,
        ) {
            log::error!("{error}");
        }
    }

    pub(crate) fn register_spawned_landmark_bridges_with_effects_checked(
        &mut self,
        objects: &[super::script_loader::PlacedObject],
        spawned_object_ids: &[(ObjectId, usize)],
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
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
            self.register_landmark_bridge_object_with_effects_checked(id, &template_name, effects)?;
        }

        Ok(())
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
        self.register_landmark_bridge_object_with_effects(
            id,
            template_name,
            &mut Default::default(),
        )
    }

    pub(in super::super) fn register_landmark_bridge_object_with_effects(
        &mut self,
        id: ObjectId,
        template_name: &str,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // Independent callers retain the existing nonfallible surface until their
        // operation boundary is migrated; the selected factory never uses this adapter.
        if let Err(error) =
            self.register_landmark_bridge_object_with_effects_checked(id, template_name, effects)
        {
            log::error!("{error}");
        }
    }

    pub(in super::super) fn register_landmark_bridge_object_with_effects_checked(
        &mut self,
        id: ObjectId,
        template_name: &str,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        if self
            .pathfinding_system
            .grid
            .bridge_layer_for_object(id.0)
            .is_some()
        {
            return Ok(());
        }
        let Some(obj) = self.objects.get(&id) else {
            return Ok(());
        };
        let pos = obj.get_position();
        let angle = obj.get_orientation();
        let (half_x, half_y) = landmark_bridge_half_sizes(template_name, obj);
        let team = obj.team;
        let info = gamelogic::terrain::TerrainLogic::bridge_info_from_parts(
            gamelogic::common::Coord3D::new(pos.x, pos.z, pos.y),
            angle,
            half_x,
            half_y,
            id.0,
        );
        let layer = self.reserve_owned_bridge(&info);
        let owner = self.world_services.terrain().clone();
        owner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .prepend_bridge_on_layer(
                info.clone(),
                gamelogic::common::AsciiString::from(template_name),
                gamelogic::path::PathfindLayerEnum::from_u32(layer as u32),
            );
        self.bridge_behavior.register_span(
            id,
            Vec3::new(info.from_left.x, info.from_left.z, info.from_left.y),
            Vec3::new(info.from_right.x, info.from_right.z, info.from_right.y),
            Vec3::new(info.to_left.x, info.to_left.z, info.to_left.y),
            Vec3::new(info.to_right.x, info.to_right.z, info.to_right.y),
        );
        self.create_landmark_bridge_towers_with_effects_checked(id, team, angle, &info, effects)?;

        Ok(())
    }

    /// C++ `Bridge::Bridge(Object*)` creates four targetable tower objects.
    pub(in super::super) fn create_landmark_bridge_towers(
        &mut self,
        bridge_id: ObjectId,
        team: Team,
        bridge_angle: f32,
        info: &gamelogic::terrain::BridgeInfo,
    ) {
        self.create_landmark_bridge_towers_with_effects(
            bridge_id,
            team,
            bridge_angle,
            info,
            &mut Default::default(),
        )
    }

    pub(in super::super) fn create_landmark_bridge_towers_with_effects(
        &mut self,
        bridge_id: ObjectId,
        team: Team,
        bridge_angle: f32,
        info: &gamelogic::terrain::BridgeInfo,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // Independent callers retain the existing nonfallible surface until their
        // operation boundary is migrated; the selected factory never uses this adapter.
        if let Err(error) = self.create_landmark_bridge_towers_with_effects_checked(
            bridge_id,
            team,
            bridge_angle,
            info,
            effects,
        ) {
            log::error!("{error}");
        }
    }

    pub(in super::super) fn create_landmark_bridge_towers_with_effects_checked(
        &mut self,
        bridge_id: ObjectId,
        team: Team,
        bridge_angle: f32,
        info: &gamelogic::terrain::BridgeInfo,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(roads) = game_engine::common::ini::try_get_terrain_roads() else {
            return Ok(());
        };
        let Some(bridge_tmpl) = self
            .objects
            .get(&bridge_id)
            .and_then(|obj| roads.find_bridge(&obj.template_name).cloned())
        else {
            return Ok(());
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
            let Some(tower_id) = self
                .create_object_with_script_effects_checked(name, team, host_pos, None, effects)?
            else {
                continue;
            };
            let tower_angle = if index < 2 {
                bridge_angle + std::f32::consts::PI
            } else {
                bridge_angle
            };
            if let Some(tower) = self.objects.get_mut(&tower_id) {
                tower
                    .set_orientation_with_partition_owner(tower_angle, &mut self.partition_manager);
                if bridge_indestructible {
                    tower.set_indestructible(true);
                }
            }

            tower_ids[index] = tower_id.0;
        }
        self.bridge_behavior
            .bind_towers(bridge_id, tower_ids.map(ObjectId));
        let owner = self.world_services.terrain().clone();
        if let Ok(mut terrain) = owner.write() {
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
        };

        Ok(())
    }
}

#[cfg(test)]
#[path = "terrain_height_owner_tests.rs"]
mod terrain_height_owner_tests;

#[cfg(test)]
#[path = "native_player_owner_tests.rs"]
mod native_player_owner_tests;
