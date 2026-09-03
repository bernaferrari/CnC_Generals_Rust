//! Host script state, audio, query, and player request behavior.
#![allow(unused_imports, non_snake_case)]
use super::*;

impl GameLogic {
    pub(in crate::game_logic::game_logic) fn build_script_game_state_context(
        &self,
    ) -> gamelogic::scripting::GameStateContext {
        let players = self
            .players
            .values()
            .map(|player| {
                let color = color_for_player(player.id as u8);
                gamelogic::scripting::PlayerInfo {
                    id: player.id,
                    name: player.name.clone(),
                    team: player.team as u32,
                    color: format!("{:02X}{:02X}{:02X}", color.r, color.g, color.b),
                    is_human: player.is_local,
                    is_alive: player.is_alive,
                    score: 0,
                }
            })
            .collect();

        gamelogic::scripting::GameStateContext {
            map_name: self.map_name.clone(),
            game_mode: format!("{:?}", self.game_mode),
            players,
            objectives: Vec::new(),
        }
    }

    /// Queue an audio event to be processed by the audio system
    /// Mirrors C++ TheAudio->addAudioEvent() pattern
    /// Test/honesty: pending audio events not yet process_audio_events drained.
    pub fn queued_audio_event_count_for_test(&self) -> usize {
        self.queued_audio_events.len()
    }

    pub fn queue_audio_event(&mut self, event: AudioEventRequest) {
        self.queued_audio_events.push(event);
    }

    /// C++ `ThingTemplate::getPerUnitSound(slot)` + `TheAudio->addAudioEvent`.
    /// Missing or empty UnitSpecificSounds stay silent — never queue the slot key.
    pub(crate) fn queue_resolved_per_unit_sound(
        &mut self,
        object_id: crate::game_logic::ObjectId,
        slot: &str,
        attach_object: bool,
        attach_position: bool,
        player_index: Option<i32>,
        priority: u8,
    ) {
        let Some(obj) = self.objects.get(&object_id) else {
            return;
        };
        let template = obj.thing.template.name.clone();
        let pos = obj.get_position();
        self.queue_resolved_per_unit_sound_named(
            &template,
            slot,
            attach_object.then_some(object_id),
            attach_position.then_some(pos),
            player_index,
            priority,
        );
    }

    pub(crate) fn queue_resolved_per_unit_sound_named(
        &mut self,
        template_name: &str,
        slot: &str,
        object_id: Option<crate::game_logic::ObjectId>,
        position: Option<glam::Vec3>,
        player_index: Option<i32>,
        priority: u8,
    ) {
        let Some(event) =
            crate::game_logic::audio_dispatch_impl::resolve_per_unit_sound(template_name, slot)
        else {
            return;
        };
        let mut req = AudioEventRequest::new(&event).with_priority(priority);
        if let Some(id) = object_id {
            req = req.with_object(id);
        }
        if let Some(pos) = position {
            req = req.with_position(pos);
        }
        if let Some(idx) = player_index {
            req = req.with_player_index(idx);
        }
        self.queue_audio_event(req);
    }

    pub fn play_ui_sound(&mut self, event_type: &str) {
        let translated = translate_audio_event(event_type);
        self.queue_audio_event(AudioEventRequest::new(translated));
    }

    /// Process all queued audio events (called once per frame).
    /// Also invoked after presentation `apply_events_to_audio` so same-frame
    /// presentation residual is not delayed one tick.
    pub(crate) fn process_audio_events(&mut self) {
        self.refresh_live_audio_locality();
        for ev in crate::game_logic::host_voice_fear_log::drain() {
            self.queued_audio_events.push(
                AudioEventRequest::new(&ev.event_name)
                    .with_object(ev.victim)
                    .with_position(ev.position)
                    .with_priority(150),
            );
        }
        for ev in crate::game_logic::host_unit_training::drain_promote_audio() {
            self.queued_audio_events.push(
                AudioEventRequest::new(&ev.event_name)
                    .with_object(ev.object)
                    .with_position(ev.position)
                    .with_priority(160),
            );
        }
        self.drain_pending_move_ambient_audio();

        for event in self.queued_audio_events.drain(..) {
            let names = crate::game_logic::resolve_audio_event_names(&event.event_type);
            for name in names {
                let mut event = event.clone();
                event.event_type = name;
                if !crate::game_logic::audio_dispatch_impl::should_dispatch_audio_request(&event) {
                    continue;
                }
                if let Some(obj_id) = event.object_id {
                    if let Some(pos) = event.position {
                        log::trace!(
                            "🔊 Audio: {} at {:?} from object {}",
                            event.event_type,
                            pos,
                            obj_id
                        );
                    } else {
                        log::trace!("🔊 Audio: {} from object {}", event.event_type, obj_id);
                    }
                } else if let Some(pos) = event.position {
                    log::trace!("🔊 Audio: {} at {:?}", event.event_type, pos);
                } else {
                    log::trace!("🔊 Audio: {}", event.event_type);
                }

                let _ = crate::subsystem_manager::with_subsystem_mut::<
                    crate::subsystem_manager::AudioManagerSubsystem,
                    _,
                >(|audio| audio.queue_event(event.clone()));
            }
        }
    }

    pub(super) fn refresh_live_audio_locality(&self) {
        use crate::game_logic::audio_dispatch_impl::{
            set_live_audio_locality, LiveAudioLocality, LiveAudioPlayer,
        };
        use game_engine::common::audio::AudioLocalityRelationship;
        let Some(local_id) = self.local_player_id() else {
            return;
        };
        let local_active = self
            .players
            .get(&local_id)
            .map(|p| p.is_alive)
            .unwrap_or(false);
        let mut snap = LiveAudioLocality {
            local_player_index: local_id as i32,
            local_player_active: local_active,
            observer_look_at: None,
            players: std::collections::HashMap::new(),
            object_owners: std::collections::HashMap::new(),
        };
        for p in self.players.values() {
            let rel = if p.id == local_id {
                AudioLocalityRelationship::Allies
            } else {
                match self.player_relationship(p.id, local_id) {
                    gamelogic::common::Relationship::Allies => AudioLocalityRelationship::Allies,
                    gamelogic::common::Relationship::Enemies => AudioLocalityRelationship::Enemies,
                    _ => AudioLocalityRelationship::Neutral,
                }
            };
            snap.players.insert(
                p.id as i32,
                LiveAudioPlayer {
                    exists: true,
                    active: p.is_alive,
                    has_default_team: true,
                    relationship_to_local: rel,
                },
            );
        }
        for (id, obj) in &self.objects {
            if let Some(pid) = obj.owner_player_id {
                snap.object_owners.insert(id.0, pid as i32);
            }
        }
        set_live_audio_locality(snap);
    }

    /// C++ `Eva` is the sole consumer of `setShouldPlay` flags (`Eva.cpp:264-525`).
    /// Leftover `TheEva` stays queued so live `Eva::update` / `ingest_logic_events`
    /// can play Eva.ini `SideSounds` (`EvaUSA_BuildingLost`, …). Host HUD still
    /// uses `host_eva_log` copies and must not drain this queue.
    pub(in crate::game_logic::game_logic) fn process_eva_events(&mut self) {}

    /// Evaluate and execute scripts each frame
    /// This is called from the main game loop (update_simulation)
    /// Phase 8 of game loop update sequence (C++ Generals compatibility)
    /// Count scripts currently installed from the last map load (groups + free lists).
    pub(in crate::game_logic::game_logic) fn mission_script_count(&self) -> usize {
        let mut count = 0usize;
        for list in &self.loaded_script_lists {
            let mut script = list.first_script.as_deref();
            while let Some(s) = script {
                count += 1;
                script = s.get_next();
            }
            let mut group = list.first_group.as_deref();
            while let Some(g) = group {
                let mut script = g.get_script();
                while let Some(s) = script {
                    count += 1;
                    script = s.get_next();
                }
                group = g.get_next();
            }
        }
        count
    }

    /// Read-only host name→ObjectId map for crate script evaluator.
    /// Does **not** populate OBJECT_REGISTRY or wrap crate `Object`s.
    pub fn host_named_unit_id_map(&self) -> std::collections::HashMap<String, u32> {
        let mut map = std::collections::HashMap::new();
        for (id, obj) in self.host_objects() {
            if obj.name.is_empty() {
                continue;
            }
            map.insert(obj.name.clone(), id.0);
        }
        map
    }

    /// Host named-unit query (scripts/AI). Prefer this over empty crate the_ai groups.
    pub fn host_named_unit_id(&self, name: &str) -> Option<ObjectId> {
        if name.is_empty() {
            return None;
        }
        self.host_objects()
            .iter()
            .find(|(_, o)| o.name == name)
            .map(|(id, _)| *id)
    }

    /// Host team query: live host objects on `team`.
    pub fn host_team_unit_ids(&self, team: crate::game_logic::Team) -> Vec<ObjectId> {
        self.host_objects()
            .iter()
            .filter(|(_, o)| o.team == team && o.is_alive() && !o.status.destroyed)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Host area query: live host objects whose XZ is inside `min..=max`.
    pub fn host_area_unit_ids(&self, min: glam::Vec3, max: glam::Vec3) -> Vec<ObjectId> {
        self.host_objects()
            .iter()
            .filter(|(_, o)| {
                if !o.is_alive() || o.status.destroyed {
                    return false;
                }
                let p = o.position;
                p.x >= min.x && p.x <= max.x && p.z >= min.z && p.z <= max.z
            })
            .map(|(id, _)| *id)
            .collect()
    }

    /// Inject host names into the crate NamedObjectTracker (IDs only).
    /// Crate evaluator can resolve names; it still must not require crate Objects.
    /// CAMERA_FOLLOW_NAMED / TETHER_NAMED / LOOK_TOWARD_OBJECT read this tracker
    /// even when leftover OBJECT_REGISTRY is empty.
    pub fn inject_host_named_unit_map_into_crate_tracker(&self) {
        use gamelogic::scripting::engine::get_named_object_tracker;
        let tracker = get_named_object_tracker();
        for (name, id) in self.host_named_unit_id_map() {
            let _ = tracker.register_named_object(name, id);
        }
        self.inject_host_script_query_snapshot();
    }

    /// Fill crate condition host-query snapshot from `host_named_unit_id*`.
    pub fn inject_host_script_query_snapshot(&self) {
        use gamelogic::scripting::{HostScriptQueryObject, HostScriptQuerySnapshot};
        let mut snap = HostScriptQuerySnapshot::default();
        snap.named = self.host_named_unit_id_map();
        for team in [
            crate::game_logic::Team::USA,
            crate::game_logic::Team::China,
            crate::game_logic::Team::GLA,
            crate::game_logic::Team::Neutral,
        ] {
            let ids = self.host_team_unit_ids(team);
            if !ids.is_empty() {
                snap.team_ids
                    .insert(team as u32, ids.iter().map(|id| id.0).collect());
            }
        }
        for (id, obj) in self.host_objects() {
            snap.objects.push(HostScriptQueryObject {
                id: id.0,
                name: obj.name.clone(),
                team: obj.team as u32,
                x: obj.position.x,
                y: obj.position.y,
                z: obj.position.z,

                alive: obj.is_alive() && !obj.status.destroyed,
                effectively_dead: obj.status.effectively_dead || obj.status.destroyed,
                health: obj.health.current,
                initial_health: obj.body_initial_health(),
                owner_player: obj
                    .owner_player_id
                    .and_then(|pid| self.player_name(pid))
                    .unwrap_or_default(),
                template_name: obj.template_name.clone(),
                has_contain: obj.can_contain()
                    || obj.garrison_count() > 0
                    || obj.transport_capacity() > 0
                    || obj
                        .building_data
                        .as_ref()
                        .is_some_and(|b| b.max_garrison > 0),
                contain_count: obj.garrison_count() as u32,
                contain_max: obj
                    .building_data
                    .as_ref()
                    .map(|b| b.max_garrison)
                    .unwrap_or(0)
                    .max(obj.transport_capacity()) as u32,
                last_damage_source_id: obj.last_damage_source.map(|sid| sid.0).unwrap_or(0),
                last_damage_template: obj
                    .last_damage_source
                    .and_then(|sid| self.objects.get(&sid).map(|src| src.template_name.clone()))
                    .unwrap_or_default(),
                last_damage_player: obj
                    .last_damage_source
                    .and_then(|sid| {
                        self.objects.get(&sid).and_then(|src| {
                            src.owner_player_id.and_then(|pid| self.player_name(pid))
                        })
                    })
                    .unwrap_or_default(),
                kind_structure: obj.is_kind_of(crate::game_logic::KindOf::Structure),
                kind_projectile: obj.is_kind_of(crate::game_logic::KindOf::Projectile),
                kind_inert: leftover_host_template_is_inert(&obj.template_name),
                kind_mine: obj.is_kind_of(crate::game_logic::KindOf::Mine),
                held: obj.status.disabled_held,
                stealthed_hidden: obj.is_effectively_stealthed(),
                discovered_by: host_discovered_by_player_names(self, id.0, obj),
                waypoint_labels: obj.completed_waypoint_labels.clone(),
                selected: obj.selected,
                idle: matches!(obj.ai_state, crate::game_logic::AIState::Idle)
                    && !obj.status.moving
                    && !obj.status.attacking,
                vision_range: obj.vision_range,
                kind_names: obj
                    .thing
                    .template
                    .kind_of
                    .iter()
                    .map(|k| format!("{k:?}"))
                    .collect(),
                special_power_ready: obj.special_power_ready
                    && obj.is_alive()
                    && !obj.is_disabled()
                    && !obj.status.destroyed,
                special_power_templates: obj
                    .thing
                    .template
                    .special_power_modules
                    .iter()
                    .map(|module| module.special_power_template.clone())
                    .collect(),
                locomotor_surfaces: obj.locomotor_surfaces,
                captured: obj.status.private_captured,
                unmanned: obj.status.disabled_unmanned,
                garrisonable: obj.is_garrison_contain(),
                build_cost: obj.thing.template.build_cost.supplies as i32,
                status_bits: host_query_object_status_bits(obj),
                player_who_entered: host_query_player_who_entered(self, obj),
                is_supply_warehouse: obj.thing.template.dock_kind
                    == crate::game_logic::DockKind::SupplyWarehouse,
                warehouse_boxes: if obj.thing.template.dock_kind
                    == crate::game_logic::DockKind::SupplyWarehouse
                {
                    // C++ SupplyWarehouseDockUpdate::getBoxesStored.
                    // drawable_supply_boxes is kept in sync by set_stored_supplies
                    // (including 0). Cash fallback only if never initialized.
                    if obj.drawable_supply_max_boxes > 0 || obj.drawable_supply_boxes > 0 {
                        obj.drawable_supply_boxes as i32
                    } else {
                        let value = crate::game_logic::host_structure_economy_residual::VALUE_PER_SUPPLY_BOX;
                        let value = if value > 0 { value as u32 } else { 75 };
                        (obj.stored_resources.supplies / value) as i32
                    }
                } else {
                    0
                },
                off_map: obj.position.x < self.world_min.x
                    || obj.position.x > self.world_max.x
                    || obj.position.z < self.world_min.z
                    || obj.position.z > self.world_max.z,
                contained_by: obj.contained_by.map(|cid| cid.0).unwrap_or(0),
                // Live has no AI_EXIT state; leftover Exit is pretend-contained.
                ai_exiting: false,

                ..Default::default()
            });
            if !obj.team_instance_name.is_empty() {
                snap.team_instance_ids
                    .entry(obj.team_instance_name.clone())
                    .or_default()
                    .push(id.0);
            }
            let skip = obj.is_kind_of(crate::game_logic::KindOf::Projectile);
            let team = if obj.team_instance_name.is_empty() {
                None
            } else {
                Some(obj.team_instance_name.as_str())
            };
            gamelogic::scripting::update_host_object_trigger_flags(
                id.0,
                obj.position.x,
                obj.position.z,
                self.frame,
                skip,
                team,
            );
        }
        if let Ok(factory) = gamelogic::team::get_team_factory().lock() {
            for name in factory.prototype_names() {
                if snap.team_instance_ids.contains_key(&name) {
                    continue;
                }
                let ids = self.host_script_team_census_member_ids(&name);
                if !ids.is_empty() {
                    snap.team_instance_ids.insert(name, ids);
                }
            }
        }
        for player in self.players.values() {
            let name = format!("team{}", player.name.trim());
            if name == "team" || snap.team_instance_ids.contains_key(&name) {
                continue;
            }
            let ids = self.host_script_team_census_member_ids(&name);
            if !ids.is_empty() {
                snap.team_instance_ids.insert(name, ids);
            }
        }
        for (name, aabb) in gamelogic::scripting::engine::get_area_tracker().all_area_aabbs() {
            snap.areas.insert(name, aabb);
        }
        merge_host_bridge_states(self, &mut snap);
        gamelogic::scripting::set_host_script_query_snapshot(snap);
        self.merge_host_player_census_into_snapshot();
    }

    /// C++ `Player::isSupplySourceAttacked` / `isSupplySourceSafe` for leftover
    /// conditions when crate `OBJECT_REGISTRY` is empty.
    pub(super) fn inject_host_supply_source_queries(&mut self) {
        let mut attacked = std::collections::HashMap::new();
        let mut cash_map = std::collections::HashMap::new();
        let mut safe_map = std::collections::HashMap::new();
        let mut ai_mgr = std::mem::take(&mut self.ai_manager);
        let player_rows: Vec<(u32, String, crate::game_logic::Team)> = self
            .players
            .values()
            .map(|p| (p.id, p.name.clone(), p.team))
            .collect();
        for (pid, name, _team) in player_rows {
            let key = name.trim().to_ascii_lowercase();
            if key.is_empty() {
                continue;
            }
            let Some(ai) = ai_mgr.ai_players.get_mut(&pid) else {
                continue;
            };
            let is_attacked = ai.is_supply_source_attacked(self);
            let warehouse = ai.find_supply_center(self, 0);
            let snapshot = warehouse.and_then(|id| {
                self.host_object(id).map(|obj| {
                    (
                        obj.stored_resources.supplies as i32,
                        obj.get_position(),
                        obj.template_name.clone(),
                    )
                })
            });
            let (cash, location_safe) = match snapshot {
                Some((cash, pos, template_name)) => {
                    let template = self.templates.get(&template_name);
                    (cash, ai.is_location_safe(self, pos, template))
                }
                None => (-1, true),
            };
            attacked.insert(key.clone(), is_attacked);
            cash_map.insert(key.clone(), cash);
            safe_map.insert(key, location_safe);
        }
        self.ai_manager = ai_mgr;
        gamelogic::scripting::merge_host_script_query_snapshot(|snap| {
            snap.supply_source_attacked = attacked;
            snap.supply_center_cash = cash_map;
            snap.supply_center_location_safe = safe_map;
        });
    }

    /// C++ Player::getMoney / getEnergy / hasAnyObjects for leftover conditions
    /// when crate `OBJECT_REGISTRY` is empty (live host Player is authoritative).
    pub(super) fn merge_host_player_census_into_snapshot(&self) {
        use crate::game_logic::KindOf;
        use gamelogic::scripting::{HostScriptPlayerCensus, HostTechBuildingCensus};

        let mut player_census = std::collections::HashMap::new();
        for player in self.players.values() {
            let mut census = HostScriptPlayerCensus {
                money: player.effective_supplies() as i32,
                energy_production: player.power_produced,
                energy_consumption: player.power_consumed,
                power_sabotaged: player.power_sabotaged_till_frame != 0
                    && self.frame < player.power_sabotaged_till_frame,
                science_purchase_points: player.science_purchase_points,
                unlocked_sciences: player.unlocked_sciences.iter().cloned().collect(),
                supply_box_value:
                    crate::game_logic::host_structure_economy_residual::VALUE_PER_SUPPLY_BOX,
                ..Default::default()
            };
            for obj in self.host_objects().values() {
                let owned = match obj.owner_player_id {
                    Some(pid) => pid == player.id,
                    None => obj.team == player.team,
                };
                if !owned {
                    continue;
                }
                let dead = !obj.is_alive() || obj.status.destroyed || obj.status.effectively_dead;
                // C++ Team::hasAnyObjects / leftover Player::has_any_objects:
                // skip dead, projectile, inert (radiation fields), and mine.
                // leftover_host_template_is_inert matches the kind_inert already
                // stamped onto HostScriptQueryObject in this same inject.
                if !dead
                    && !obj.is_kind_of(KindOf::Projectile)
                    && !obj.is_kind_of(KindOf::Mine)
                    && !obj.is_kind_of(KindOf::SmallMissile)
                    && !obj.is_kind_of(KindOf::BallisticMissile)
                    && !obj.is_kind_of(KindOf::Inert)
                    && !leftover_host_template_is_inert(&obj.template_name)
                {
                    census.has_any_objects = true;
                }
                if !dead && obj.is_kind_of(KindOf::Structure) {
                    census.building_count += 1;
                    if obj.is_kind_of(KindOf::MpCountForVictory) {
                        census.faction_building_count += 1;
                    }
                }
                if !dead
                    && (obj.is_kind_of(KindOf::CommandCenter)
                        || obj.is_kind_of(KindOf::FSBarracks)
                        || obj.is_kind_of(KindOf::FSWarFactory)
                        || obj.is_kind_of(KindOf::FSAirfield))
                {
                    census.has_any_build_facility = true;
                }
                if !obj.status.under_construction && !obj.template_name.is_empty() {
                    let key = obj.template_name.to_ascii_lowercase();
                    *census.template_counts.entry(key.clone()).or_insert(0) += 1;
                    if !dead {
                        *census.template_counts_ignore_dead.entry(key).or_insert(0) += 1;
                    }
                }
            }
            let mut insert = |name: &str| {
                let key = name.trim().to_ascii_lowercase();
                if !key.is_empty() {
                    player_census.insert(key, census.clone());
                }
            };
            insert(&player.name);
            insert(&player.map_side.map_player_name);
            match player.team {
                crate::game_logic::Team::USA => {
                    insert("PlyrAmerica");
                    insert("America");
                    insert("USA");
                }
                crate::game_logic::Team::China => {
                    insert("PlyrChina");
                    insert("China");
                }
                crate::game_logic::Team::GLA => {
                    insert("PlyrGLA");
                    insert("GLA");
                }
                _ => {}
            }
        }
        let mut tech_buildings = Vec::new();
        for obj in self.host_objects().values() {
            if !obj.is_kind_of(KindOf::TechBuilding) {
                continue;
            }
            if !obj.is_alive() || obj.status.destroyed || obj.status.effectively_dead {
                continue;
            }
            tech_buildings.push(HostTechBuildingCensus {
                x: obj.position.x,
                z: obj.position.z,
                owner_player: obj
                    .owner_player_id
                    .and_then(|pid| self.player_name(pid))
                    .unwrap_or_default(),
                team: obj.team as u32,
                off_map: crate::game_logic::host_deliver_payload::is_off_map_default_residual(
                    obj.position,
                ),
            });
        }
        gamelogic::scripting::merge_host_script_query_snapshot(|snap| {
            snap.player_census = player_census;
            snap.tech_buildings = tech_buildings;
        });
    }

    /// Leftover `OBJECT_REGISTRY` is empty on the live path. Script actions
    /// queue named SW / priority-build requests for host AIPlayer.
    pub fn apply_host_skirmish_script_requests(&mut self) {
        let fires = gamelogic::scripting::take_host_skirmish_fire_special_requests();
        let builds = gamelogic::scripting::take_host_skirmish_build_requests();
        let supply_centers =
            gamelogic::scripting::take_host_ai_player_build_supply_center_requests();
        let upgrades = gamelogic::scripting::take_host_ai_player_build_upgrade_requests();
        let nearest = gamelogic::scripting::take_host_ai_player_build_type_nearest_team_requests();
        let cave_indexes = gamelogic::scripting::take_host_set_cave_index_requests();
        let unit_flags = gamelogic::scripting::take_host_object_panel_flag_requests();
        let team_flags = gamelogic::scripting::take_host_team_panel_flag_requests();
        let sciences = gamelogic::scripting::take_host_science_action_requests();
        let defenses = gamelogic::scripting::take_host_skirmish_base_defense_requests();
        if fires.is_empty()
            && builds.is_empty()
            && supply_centers.is_empty()
            && upgrades.is_empty()
            && nearest.is_empty()
            && cave_indexes.is_empty()
            && unit_flags.is_empty()
            && team_flags.is_empty()
            && sciences.is_empty()
            && defenses.is_empty()
        {
            return;
        }
        let mut ai_mgr = std::mem::take(&mut self.ai_manager);
        for (player_token, power_name) in fires {
            ai_mgr.fire_skirmish_special_power_at_most_cost(self, &player_token, &power_name);
        }
        for thing_name in builds {
            let _ = ai_mgr.build_specific_ai_building_for_token(self, "", &thing_name);
        }
        for (player_token, thing_name, cash) in supply_centers {
            let _ = ai_mgr.build_by_supplies_for_token(self, &player_token, cash, &thing_name);
        }
        for (player_token, upgrade_name) in upgrades {
            let _ = ai_mgr.build_upgrade_for_token(self, &player_token, &upgrade_name);
        }
        for (player_token, thing_name, team_name) in nearest {
            let _ = ai_mgr.build_specific_building_nearest_team_for_token(
                self,
                &player_token,
                &thing_name,
                &team_name,
            );
        }
        for req in defenses {
            match req.structure.as_deref() {
                None => {
                    let _ = ai_mgr.build_ai_base_defense_for_token(self, &req.player, req.flank);
                }
                Some(thing_name) => {
                    let _ = ai_mgr.build_ai_base_defense_structure_for_token(
                        self,
                        &req.player,
                        thing_name,
                        req.flank,
                    );
                }
            }
        }
        self.ai_manager = ai_mgr;
        for (cave_name, index) in cave_indexes {
            let _ = self.set_named_cave_index(&cave_name, index);
        }
        for (unit_name, flag_name, enable) in unit_flags {
            if let Some(id) = self.host_object_id_by_script_name(&unit_name) {
                if panel_flag_is_indestructible(&flag_name) {
                    self.set_object_indestructible(id, enable);
                } else if let Some(obj) = self.objects.get_mut(&id) {
                    obj.apply_object_panel_flag(&flag_name, enable);
                }
            }
        }
        for (team_name, flag_name, enable) in team_flags {
            let needle = team_name.trim();
            if needle.is_empty() {
                continue;
            }
            let ids: Vec<ObjectId> = self
                .objects
                .values()
                .filter(|obj| {
                    (!obj.team_instance_name.is_empty()
                        && obj.team_instance_name.eq_ignore_ascii_case(needle))
                        || obj.team.get_name().eq_ignore_ascii_case(needle)
                })
                .map(|obj| obj.id)
                .collect();
            for id in ids {
                if panel_flag_is_indestructible(&flag_name) {
                    self.set_object_indestructible(id, enable);
                } else if let Some(obj) = self.objects.get_mut(&id) {
                    obj.apply_object_panel_flag(&flag_name, enable);
                }
            }
        }
        for (player_token, science_name, grant) in sciences {
            let Some(pid) = self.host_player_id_for_script_token(&player_token) else {
                continue;
            };
            let applied = if grant {
                if let Some(player) = self.players.get_mut(&pid) {
                    let granted = player.grant_science(&science_name);
                    if granted {
                        crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::sync_host_science_to_crate_player(
                            pid,
                            &science_name,
                        );
                    }
                    granted
                } else {
                    false
                }
            } else if let Some(player) = self.players.get_mut(&pid) {
                player.attempt_to_purchase_science(&science_name)
            } else {
                false
            };
            // C++ Player::addScience walks owned SpecialPowerModules and
            // expresses sharedNSync ready-now. ControlBar purchase already
            // calls this; script grant/purchase must too.
            if applied {
                self.on_special_power_science_creation(pid, &science_name);
            }
        }
    }

    /// C++ `SET_BASE_CONSTRUCTION_SPEED` → `Player::setTeamDelaySeconds`.
    pub(super) fn apply_host_set_base_construction_speed_requests(&mut self) {
        for (player, delay) in
            gamelogic::scripting::take_host_set_base_construction_speed_requests()
        {
            let Some(pid) = self.host_player_id_for_script_token(&player) else {
                continue;
            };
            if let Some(ai) = self.ai_manager.ai_players.get_mut(&pid) {
                ai.set_team_delay_seconds(delay);
            }
        }
    }

    /// C++ `SET_TRAIN_HELD` → `RailroadBehavior::setHeld`.
    pub(super) fn apply_host_set_train_held_requests(&mut self) {
        for (unit, held) in gamelogic::scripting::take_host_set_train_held_requests() {
            if let Some(id) = self.host_object_id_by_script_name(&unit) {
                self.set_railroad_held(id, held);
            }
        }
    }

    /// C++ ScriptActions::doSetMoney / doGiveMoney live drain.
    /// Leftover mutates crate player_list; live cash lives on host Player.
    pub(super) fn apply_host_money_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptMoneyRequest;
        for req in gamelogic::scripting::take_host_money_requests() {
            match req {
                HostScriptMoneyRequest::Set { player, amount } => {
                    let Some(pid) = self.host_player_id_for_script_token(&player) else {
                        continue;
                    };
                    self.host_script_set_player_money(pid, amount);
                }
                HostScriptMoneyRequest::Give { player, amount } => {
                    let Some(pid) = self.host_player_id_for_script_token(&player) else {
                        continue;
                    };
                    self.host_script_give_player_money(pid, amount);
                }
            }
        }
    }

    /// C++ PLAYER_DISABLE/ENABLE unit/base/factory construction drain.
    /// Leftover writes leftover PlayerList; live flags live on host Player.
    pub(super) fn apply_host_can_build_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptCanBuildRequest;
        for req in gamelogic::scripting::take_host_can_build_requests() {
            match req {
                HostScriptCanBuildRequest::Units { player, enable } => {
                    let Some(pid) = self.host_player_id_for_script_token(&player) else {
                        continue;
                    };
                    if let Some(p) = self.players.get_mut(&pid) {
                        p.set_can_build_units(enable);
                    }
                }
                HostScriptCanBuildRequest::Base { player, enable } => {
                    let Some(pid) = self.host_player_id_for_script_token(&player) else {
                        continue;
                    };
                    if let Some(p) = self.players.get_mut(&pid) {
                        p.set_can_build_base(enable);
                    }
                }
                HostScriptCanBuildRequest::Factories {
                    player,
                    template,
                    enable,
                } => {
                    let Some(pid) = self.host_player_id_for_script_token(&player) else {
                        continue;
                    };
                    self.host_set_objects_enabled(pid, &template, enable);
                }
            }
        }
    }

    /// C++ doNamedFlash: NULL color → `Object::getIndicatorColor`; white → RGB 1,1,1 `getAsInt`.
    pub(super) fn host_script_flash_color(
        &self,
        id: crate::game_logic::ObjectId,
        white: bool,
    ) -> u32 {
        if white {
            return 0x00FF_FFFF;
        }
        let Some(obj) = self.objects.get(&id) else {
            return 0xFF00_0000;
        };
        if let Some(c) = obj.custom_indicator_color {
            return c;
        }
        obj.owner_player_id
            .and_then(|pid| self.players.get(&pid))
            .map(|p| crate::game_logic::host_radar::pack_player_color_argb(p.color_rgb))
            .unwrap_or(0xFF00_0000)
    }

    /// C++ NAMED_RECEIVE_UPGRADE / FLASH / EMOTICON / HELD / CUSTOM_COLOR /
    /// NAMED_SET_ATTITUDE / REPULSOR / STOPPING_DISTANCE / FORCE_SELECT /
    /// PLAYER_SELL_EVERYTHING / REPAIR_NAMED / EXCLUDE_FROM_SCORE / SELECT_SKILLSET.
    pub(super) fn apply_host_script_visual_status_requests(&mut self) {
        use crate::game_logic::KindOf;
        use gamelogic::scripting::{
            HostScriptEmoticonRequest, HostScriptFlashRequest, HostScriptPlayerMiscRequest,
            HostScriptRepulsorRequest, HostScriptStoppingDistanceRequest,
        };

        for req in gamelogic::scripting::take_host_script_named_upgrade_requests() {
            if let Some(id) = self.host_object_id_by_script_name(&req.unit) {
                self.apply_upgrade_to_object(id, &req.upgrade);
            }
        }

        // C++ script actions execute sequentially: CUSTOM_INDICATOR_COLOR
        // writes land before a later FLASH reads getIndicatorColor
        // (ScriptEngine/DoNamedFlash order). Apply the color batch first.
        for req in gamelogic::scripting::take_host_script_custom_color_requests() {
            if let Some(id) = self.host_object_id_by_script_name(&req.unit) {
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.set_custom_indicator_color_raw(req.color_raw);
                }
            }
        }

        for req in gamelogic::scripting::take_host_script_flash_requests() {
            let (ids, seconds, white) = match req {
                HostScriptFlashRequest::Named {
                    unit,
                    seconds,
                    white,
                } => (
                    self.host_object_id_by_script_name(&unit)
                        .into_iter()
                        .collect::<Vec<_>>(),
                    seconds,
                    white,
                ),
                HostScriptFlashRequest::Team {
                    team,
                    seconds,
                    white,
                } => (self.host_script_team_member_ids(&team), seconds, white),
            };
            // C++ doNamedFlash: timeInSeconds > 0; count = frames / DRAWABLE_FRAMES_PER_FLASH.
            if seconds <= 0 {
                continue;
            }
            for id in ids {
                let color = self.host_script_flash_color(id, white);
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.set_script_flash(seconds, color);
                }
            }
        }

        for req in gamelogic::scripting::take_host_script_emoticon_requests() {
            let (ids, name, frames) = match req {
                HostScriptEmoticonRequest::Named {
                    unit,
                    emoticon,
                    duration_frames,
                } => (
                    self.host_object_id_by_script_name(&unit)
                        .into_iter()
                        .collect::<Vec<_>>(),
                    emoticon,
                    duration_frames,
                ),
                HostScriptEmoticonRequest::Team {
                    team,
                    emoticon,
                    duration_frames,
                } => (
                    self.host_script_team_member_ids(&team),
                    emoticon,
                    duration_frames,
                ),
            };
            for id in ids {
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.set_emoticon(&name, frames);
                }
            }
        }

        for req in gamelogic::scripting::take_host_script_held_requests() {
            if let Some(id) = self.host_object_id_by_script_name(&req.unit) {
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.set_status_disabled_held(req.held);
                }
            }
        }

        for req in gamelogic::scripting::take_host_script_named_attitude_requests() {
            if let Some(id) = self.host_object_id_by_script_name(&req.unit) {
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.set_ai_attitude_i8(req.mood as i8);
                }
            }
        }

        for req in gamelogic::scripting::take_host_script_repulsor_requests() {
            let (ids, enabled) = match req {
                HostScriptRepulsorRequest::Named { unit, enabled } => (
                    self.host_object_id_by_script_name(&unit)
                        .into_iter()
                        .collect::<Vec<_>>(),
                    enabled,
                ),
                HostScriptRepulsorRequest::Team { team, enabled } => {
                    (self.host_script_team_member_ids(&team), enabled)
                }
            };
            for id in ids {
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.repulsor_until_frame = 0;
                    obj.set_status_repulsor(enabled);
                }
            }
        }

        for req in gamelogic::scripting::take_host_script_stopping_distance_requests() {
            let (mut ids, distance) = match req {
                HostScriptStoppingDistanceRequest::Named { unit, distance } => (
                    self.host_object_id_by_script_name(&unit)
                        .into_iter()
                        .collect::<Vec<_>>(),
                    distance,
                ),
                HostScriptStoppingDistanceRequest::Team { team, distance } => {
                    (self.host_script_team_member_ids(&team), distance)
                }
            };
            if distance < 0.5 {
                continue;
            }
            // C++ doSetStoppingDistance: team member list order. Live HashMap
            // iteration is not stable; ObjectId is spawn/join order.
            ids.sort_by_key(|id| id.0);
            for id in ids {
                let Some(obj) = self.objects.get(&id) else {
                    continue;
                };
                // C++ `if (!aiUpdate || !aiUpdate->getCurLocomotor()) { return; }`
                // — first structure/hulk aborts, later members keep old dist.
                if !Self::host_script_member_has_cur_locomotor(obj) {
                    break;
                }
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.close_enough_dist = Some(distance);
                }
            }
        }

        for req in gamelogic::scripting::take_host_script_force_select_requests() {
            let members = self.host_script_team_member_ids(&req.team);
            let mut best: Option<ObjectId> = None;
            for id in members {
                let Some(obj) = self.objects.get(&id) else {
                    continue;
                };
                if !obj.template_name.eq_ignore_ascii_case(&req.object_type) {
                    continue;
                }
                if best.is_none_or(|cur| id.0 < cur.0) {
                    best = Some(id);
                }
            }
            let Some(selected_id) = best else {
                continue;
            };
            let pos = self
                .objects
                .get(&selected_id)
                .map(|o| o.get_position())
                .unwrap_or(glam::Vec3::ZERO);
            if let Some(pid) = self.players.values().find(|p| p.is_local).map(|p| p.id) {
                self.select_objects(pid, vec![selected_id]);
            }
            if !req.audio.is_empty() {
                self.queue_audio_event(crate::game_logic::AudioEventRequest::new(&req.audio));
            }
            if req.center_in_view {
                self.request_camera_focus(pos);
            }
        }

        for req in gamelogic::scripting::take_host_script_player_misc_requests() {
            match req {
                HostScriptPlayerMiscRequest::SellEverything { player } => {
                    let Some(pid) = self.host_player_id_for_script_token(&player) else {
                        continue;
                    };
                    let ids: Vec<ObjectId> = self
                        .objects
                        .values()
                        .filter(|obj| {
                            obj.owner_player_id == Some(pid)
                                && obj.is_alive()
                                && !obj.status.effectively_dead
                                && (obj.is_faction_structure()
                                    || obj.is_kind_of(KindOf::CommandCenter)
                                    || obj.is_kind_of(KindOf::FSPower))
                        })
                        .map(|obj| obj.id)
                        .collect();
                    for id in ids {
                        let _ = self.start_sell_object(id);
                    }
                }
                HostScriptPlayerMiscRequest::RepairNamed { player, structure } => {
                    let Some(pid) = self.host_player_id_for_script_token(&player) else {
                        continue;
                    };
                    let Some(id) = self.host_object_id_by_script_name(&structure) else {
                        continue;
                    };
                    let mut ai_mgr = std::mem::take(&mut self.ai_manager);
                    if let Some(ai) = ai_mgr.ai_players.get_mut(&pid) {
                        ai.repair_structure(self, id);
                    }
                    self.ai_manager = ai_mgr;
                }
                HostScriptPlayerMiscRequest::ExcludeFromScore { player } => {
                    let Some(pid) = self.host_player_id_for_script_token(&player) else {
                        continue;
                    };
                    if let Some(p) = self.players.get_mut(&pid) {
                        p.list_in_score_screen = false;
                    }
                }
                HostScriptPlayerMiscRequest::SelectSkillset { player, skillset } => {
                    let Some(pid) = self.host_player_id_for_script_token(&player) else {
                        continue;
                    };
                    if let Some(ai) = self.ai_manager.ai_players.get_mut(&pid) {
                        ai.select_skillset(skillset - 1);
                    }
                }
                HostScriptPlayerMiscRequest::Kill { player } => {
                    let Some(pid) = self.host_player_id_for_script_token(&player) else {
                        continue;
                    };
                    self.kill_player_for_victory(pid);
                }
            }
        }
    }

    /// C++ Player::setObjectsEnabled — SCRIPT_DISABLED on matching templates.
    pub(super) fn host_set_objects_enabled(
        &mut self,
        player_id: u32,
        template_name: &str,
        enable: bool,
    ) {
        let ids: Vec<ObjectId> = self
            .objects
            .values()
            .filter(|obj| {
                obj.owner_player_id == Some(player_id)
                    && obj.template_name.eq_ignore_ascii_case(template_name)
            })
            .map(|obj| obj.id)
            .collect();
        for id in ids {
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.set_script_disabled(!enable);
            }
        }
    }

    /// C++ TECHTREE_MODIFY_BUILDABILITY_OBJECT drain.
    pub(super) fn apply_host_buildable_override_script_requests(&mut self) {
        for req in gamelogic::scripting::take_host_buildable_status_override_requests() {
            gamelogic::helpers::TheGameLogic::set_buildable_status_override(
                &req.template,
                req.status,
            );
            if let Some(template) = self.template_mut_by_name(&req.template) {
                template.buildable_status = req.status.max(0) as u32;
            }
        }
    }

    pub(super) fn template_mut_by_name(&mut self, name: &str) -> Option<&mut ThingTemplate> {
        if self.templates.contains_key(name) {
            return self.templates.get_mut(name);
        }
        let key = self
            .templates
            .keys()
            .find(|k| k.eq_ignore_ascii_case(name))
            .cloned()?;
        self.templates.get_mut(&key)
    }

    /// C++ `Player::setRankLevel` with the bound PlayerTemplate so rank-down
    /// re-seeds IntrinsicSciences / IntrinsicSciencePurchasePoints.
    pub fn set_player_rank_level(&mut self, player_id: u32, new_level: u32) -> bool {
        let template = self.resolved_player_template(player_id);
        let Some(player) = self.players.get_mut(&player_id) else {
            return false;
        };
        player.set_rank_level_from_template(new_level, template.as_ref())
    }

    /// C++ ScriptActions skill/rank leftover drain.
    /// Leftover mutates crate player_list / leftover GameLogic; live rank lives on host Player.
    pub(super) fn apply_host_rank_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptRankRequest;
        for req in gamelogic::scripting::take_host_rank_requests() {
            match req {
                HostScriptRankRequest::AddSkillPoints { player, delta } => {
                    let Some(pid) = self.host_player_id_for_script_token(&player) else {
                        continue;
                    };
                    let _ = self.add_player_skill_points(pid, delta);
                }
                HostScriptRankRequest::AddRankLevel { player, delta } => {
                    let Some(pid) = self.host_player_id_for_script_token(&player) else {
                        continue;
                    };
                    let current = self.players.get(&pid).map(|p| p.rank_level).unwrap_or(1);
                    let changed =
                        self.set_player_rank_level(pid, (current as i32 + delta).max(1) as u32);
                    if changed {
                        self.try_eva_general_level_up(pid);
                    }
                }
                HostScriptRankRequest::SetRankLevel { player, level } => {
                    let Some(pid) = self.host_player_id_for_script_token(&player) else {
                        continue;
                    };
                    let changed = self.set_player_rank_level(pid, level.max(1) as u32);
                    if changed {
                        self.try_eva_general_level_up(pid);
                    }
                }
                HostScriptRankRequest::SetRankLevelLimit { limit } => {
                    gamelogic::helpers::TheGameLogic::set_rank_level_limit(limit);
                }
                HostScriptRankRequest::AffectReceivingExperience { player, modifier } => {
                    let Some(pid) = self.host_player_id_for_script_token(&player) else {
                        continue;
                    };
                    if let Some(p) = self.players.get_mut(&pid) {
                        p.skill_points_modifier = modifier;
                    }
                }
            }
        }
    }

    /// C++ ScriptActions TEAM/PLAYER/NAMED TRANSFER live drain.
    /// Leftover mutates empty `OBJECT_REGISTRY`; live objects/money live on host.
    pub(super) fn apply_host_transfer_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptTransferRequest;
        for req in gamelogic::scripting::take_host_script_transfer_requests() {
            match req {
                HostScriptTransferRequest::Player { from, to } => {
                    let Some(src) = self.host_player_id_for_script_token(&from) else {
                        continue;
                    };
                    let Some(dst) = self.host_player_id_for_script_token(&to) else {
                        continue;
                    };
                    self.host_script_transfer_assets_from(dst, src);
                }
                HostScriptTransferRequest::Named { unit, player } => {
                    let Some(pid) = self.host_player_id_for_script_token(&player) else {
                        continue;
                    };
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let _ = self.transfer_object_to_player(id, pid);
                }
                HostScriptTransferRequest::Team { team, player } => {
                    let Some(pid) = self.host_player_id_for_script_token(&player) else {
                        continue;
                    };
                    self.host_script_transfer_team_to_player(&team, pid);
                }
            }
        }
    }

    /// C++ ScriptActions::updatePlayerRelationTowardPlayer live drain.
    /// Leftover writes leftover ThePlayerList; live relations live on host Player.
    pub(super) fn apply_host_player_relates_script_requests(&mut self) {
        for req in gamelogic::scripting::take_host_player_relates_requests() {
            let Some(src) = self.host_player_id_for_script_token(&req.source) else {
                continue;
            };
            let Some(dst) = self.host_player_id_for_script_token(&req.dest) else {
                continue;
            };
            if let Some(player) = self.players.get_mut(&src) {
                player.set_map_relationship(dst, req.relationship);
            }
        }
    }

    /// C++ TEAM_SET/REMOVE_OVERRIDE_RELATION_* live drain.
    /// Leftover writes leftover Team maps; live combat reads host Player maps.
    pub(super) fn apply_host_team_override_relation_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptTeamOverrideRelationRequest;
        for req in gamelogic::scripting::take_host_team_override_relation_requests() {
            match req {
                HostScriptTeamOverrideRelationRequest::SetTeam {
                    source,
                    dest_team,
                    relationship,
                } => {
                    for player in self.players.values_mut() {
                        player.set_team_instance_team_override(&source, &dest_team, relationship);
                    }
                }
                HostScriptTeamOverrideRelationRequest::RemoveTeam { source, dest_team } => {
                    for player in self.players.values_mut() {
                        let _ = player.remove_team_instance_team_override(&source, &dest_team);
                    }
                }
                HostScriptTeamOverrideRelationRequest::SetPlayer {
                    source,
                    dest_player,
                    relationship,
                } => {
                    let Some(pid) = self.host_player_id_for_script_token(&dest_player) else {
                        continue;
                    };
                    for player in self.players.values_mut() {
                        player.set_team_instance_player_override(&source, pid, relationship);
                    }
                }
                HostScriptTeamOverrideRelationRequest::RemovePlayer {
                    source,
                    dest_player,
                } => {
                    let Some(pid) = self.host_player_id_for_script_token(&dest_player) else {
                        continue;
                    };
                    for player in self.players.values_mut() {
                        let _ = player.remove_team_instance_player_override(&source, pid);
                    }
                }
                HostScriptTeamOverrideRelationRequest::RemoveAll { source } => {
                    for player in self.players.values_mut() {
                        player.clear_team_instance_overrides(&source);
                    }
                }
                HostScriptTeamOverrideRelationRequest::SetPlayerToTeam {
                    source_player,
                    dest_team,
                    relationship,
                } => {
                    let Some(pid) = self.host_player_id_for_script_token(&source_player) else {
                        continue;
                    };
                    if let Some(player) = self.players.get_mut(&pid) {
                        player.set_team_relationship_override(&dest_team, relationship);
                    }
                }
                HostScriptTeamOverrideRelationRequest::RemovePlayerToTeam {
                    source_player,
                    dest_team,
                } => {
                    let Some(pid) = self.host_player_id_for_script_token(&source_player) else {
                        continue;
                    };
                    if let Some(player) = self.players.get_mut(&pid) {
                        let _ = player.remove_team_relationship_override(&dest_team);
                    }
                }
            }
        }
    }

    /// C++ `Player::transferAssetsFromThat` — non-beacon objects onto dest
    /// default team, then withdraw/deposit all cash.
    pub(super) fn host_script_transfer_assets_from(
        &mut self,
        dest_player: u32,
        source_player: u32,
    ) {
        let ids: Vec<ObjectId> = self
            .host_objects()
            .iter()
            .filter_map(|(id, obj)| {
                (obj.owner_player_id == Some(source_player)
                    && obj.is_alive()
                    && !obj.template_name.to_ascii_lowercase().contains("beacon"))
                .then_some(*id)
            })
            .collect();
        for id in ids {
            let _ = self.transfer_object_to_player(id, dest_player);
        }
        let amount = self
            .players
            .get(&source_player)
            .map(|p| p.effective_supplies())
            .unwrap_or(0);
        if amount == 0 {
            return;
        }
        if let Some(src) = self.players.get_mut(&source_player) {
            crate::game_logic::host_economy_log::record_money_audio(
                source_player,
                crate::game_logic::host_economy_log::HostMoneyAudio::Withdraw,
            );
            src.apply_supply_spend_unchecked(amount);
        }
        if let Some(dst) = self.players.get_mut(&dest_player) {
            crate::game_logic::host_economy_log::record_money_audio(
                dest_player,
                crate::game_logic::host_economy_log::HostMoneyAudio::Deposit,
            );
            dst.apply_supply_gain(amount);
        }
    }

    /// C++ `Team::setControllingPlayer` — members stay on the same team.
    pub(super) fn host_script_transfer_team_to_player(
        &mut self,
        team_name: &str,
        dest_player: u32,
    ) {
        let members = self.host_script_team_member_ids(team_name);
        for id in members {
            if let Some(obj) = self.host_object_mut(id) {
                obj.owner_player_id = Some(dest_player);
            }
        }
        if let Ok(mut factory) = gamelogic::team::get_team_factory().lock() {
            if let Some(team) = factory.find_team(team_name) {
                if let Ok(mut guard) = team.write() {
                    guard.set_controlling_player_id(Some(dest_player));
                }
            }
        }
    }

    /// C++ Money::withdraw(countMoney()) then Money::deposit(amount).
    pub(super) fn host_script_set_player_money(&mut self, pid: u32, amount: i32) {
        let Some(player) = self.players.get_mut(&pid) else {
            return;
        };
        let current = player.effective_supplies();
        if current > 0 {
            crate::game_logic::host_economy_log::record_money_audio(
                pid,
                crate::game_logic::host_economy_log::HostMoneyAudio::Withdraw,
            );
            player.apply_supply_spend_unchecked(current);
        }
        let deposit = amount.max(0) as u32;
        if deposit > 0 {
            crate::game_logic::host_economy_log::record_money_audio(
                pid,
                crate::game_logic::host_economy_log::HostMoneyAudio::Deposit,
            );
            player.apply_supply_gain(deposit);
        }
    }

    /// C++ doGiveMoney: negative withdraws, else deposits.
    pub(super) fn host_script_give_player_money(&mut self, pid: u32, amount: i32) {
        let Some(player) = self.players.get_mut(&pid) else {
            return;
        };
        if amount < 0 {
            let want = amount.unsigned_abs();
            let actual = want.min(player.effective_supplies());
            if actual > 0 {
                crate::game_logic::host_economy_log::record_money_audio(
                    pid,
                    crate::game_logic::host_economy_log::HostMoneyAudio::Withdraw,
                );
                player.apply_supply_spend_unchecked(actual);
            }
        } else if amount > 0 {
            crate::game_logic::host_economy_log::record_money_audio(
                pid,
                crate::game_logic::host_economy_log::HostMoneyAudio::Deposit,
            );
            player.apply_supply_gain(amount as u32);
        }
    }

    pub(super) fn host_player_id_for_script_token(&self, token: &str) -> Option<u32> {
        let needle = token.trim();
        if needle.is_empty() {
            return None;
        }
        // C++ ScriptEngine::getPlayerFromAsciiString: LOCAL_PLAYER / THIS_PLAYER.
        // Tokens are "<Local Player>" / "<This Player>", not THIS_PLAYER/THE_PLAYER.
        if needle.eq_ignore_ascii_case("<This Player>")
            || needle.eq_ignore_ascii_case("<Local Player>")
        {
            return self.players.values().find(|p| p.is_local).map(|p| p.id);
        }
        self.players
            .values()
            .find(|p| {
                p.name.eq_ignore_ascii_case(needle)
                    || p.map_side.map_player_name.eq_ignore_ascii_case(needle)
                    || format!("Plyr{}", p.team.get_name()).eq_ignore_ascii_case(needle)
            })
            .map(|p| p.id)
    }
}
