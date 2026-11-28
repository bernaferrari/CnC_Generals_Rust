//! Host script state, audio, query, and player request behavior.
#![allow(unused_imports, non_snake_case)]
use super::*;

impl GameLogic {
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
        let pos = obj.get_position();
        let event = crate::game_logic::audio_dispatch_impl::resolve_receiving_per_unit_sound(
            obj.get_template(),
            slot,
        );
        let Some(event) = event else {
            return;
        };
        self.queue_per_unit_sound_event(
            &event,
            attach_object.then_some(object_id),
            attach_position.then_some(pos),
            player_index,
            priority,
        );
    }

    fn queue_per_unit_sound_event(
        &mut self,
        event: &str,
        object_id: Option<crate::game_logic::ObjectId>,
        position: Option<glam::Vec3>,
        player_index: Option<i32>,
        priority: u8,
    ) {
        let mut req = AudioEventRequest::new(event).with_priority(priority);
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

    /// Resolve queued audio events after a native logic step and return the
    /// complete requests to the owning application audio subsystem.
    pub(crate) fn process_audio_events(
        &mut self,
        observer_look_at: Option<i32>,
    ) -> Vec<crate::game_logic::audio_dispatch_impl::AdmittedAudioRequest> {
        self.process_audio_events_with_receiving_audio(observer_look_at, None)
    }

    pub(crate) fn process_audio_events_with_receiving_audio(
        &mut self,
        observer_look_at: Option<i32>,
        audio: Option<&mut dyn super::super::super::NativeAudioEffects>,
    ) -> Vec<crate::game_logic::audio_dispatch_impl::AdmittedAudioRequest> {
        self.retry_dropped_ambient_sounds_with_receiving_audio(audio);
        if self.queued_audio_events.is_empty() {
            return Vec::new();
        }
        let pending = self.queued_audio_events.drain_requests();
        let locality = self.receiving_audio_locality(observer_look_at, &pending);
        let mut outgoing = Vec::new();

        for event in pending {
            #[cfg(feature = "game_client")]
            let names = self.resolve_owned_audio_event_names(&event.event_type);
            #[cfg(not(feature = "game_client"))]
            let names = crate::game_logic::resolve_audio_event_names(&event.event_type);
            for name in names {
                let mut event = event.clone();
                event.event_type = name;
                let Some(event) = crate::game_logic::audio_dispatch_impl::admit_audio_request(
                    event,
                    locality.as_ref(),
                    &mut |player, position| {
                        self.receiving_audio_cell_shroud_clear(player, position)
                    },
                ) else {
                    continue;
                };
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

                outgoing.push(event);
            }
        }
        outgoing
    }

    pub(crate) fn receiving_audio_locality(
        &self,
        observer_look_at: Option<i32>,
        requests: &[AudioEventRequest],
    ) -> Option<crate::game_logic::audio_dispatch_impl::LiveAudioLocality> {
        let mut snap = self.receiving_audio_player_locality(observer_look_at)?;
        // Sound admission only resolves object ownership when the queued
        // event names an Object and did not already specify a player.
        let source_ids = requests
            .iter()
            .filter(|event| event.player_index.is_none())
            .filter_map(|event| event.object_id);
        for id in source_ids {
            if let Some(pid) = self.objects.get_registered(&id).and_then(|obj| obj.owner_player_id)
            {
                snap.object_owners.insert(id.0, pid as i32);
            }
        }
        Some(snap)
    }

    /// Non-positional speech needs player facts, not an object-owner census.
    pub(crate) fn receiving_audio_player_locality(
        &self,
        observer_look_at: Option<i32>,
    ) -> Option<crate::game_logic::audio_dispatch_impl::LiveAudioLocality> {
        use crate::game_logic::audio_dispatch_impl::{LiveAudioLocality, LiveAudioPlayer};
        use game_engine::common::audio::AudioLocalityRelationship;
        let local_id = self.local_player_id()?;
        let local_active = self
            .players
            .get(&local_id)
            .map(|p| p.is_alive)
            .unwrap_or(false);
        let mut snap = LiveAudioLocality {
            local_player_index: local_id as i32,
            local_player_active: local_active,
            observer_look_at,
            players: std::collections::HashMap::new(),
            object_owners: std::collections::HashMap::new(),
        };
        let relationship_local = if local_active {
            local_id
        } else {
            observer_look_at
                .and_then(|id| u32::try_from(id).ok())
                .unwrap_or(local_id)
        };
        for p in self.players.values() {
            let rel = if p.id == relationship_local {
                AudioLocalityRelationship::Allies
            } else {
                match self.player_relationship(p.id, relationship_local) {
                    game_engine::common::game_common::Relationship::Allies => AudioLocalityRelationship::Allies,
                    game_engine::common::game_common::Relationship::Enemies => AudioLocalityRelationship::Enemies,
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
        Some(snap)
    }

    /// C++ GameSounds.cpp203-205 queries the receiving partition cell shroud.
    /// Host coordinates are Y-up; the actual shroud owner consumes C++ Z-up.
    pub(crate) fn receiving_audio_cell_shroud_clear(
        &self,
        player: i32,
        position: glam::Vec3,
    ) -> bool {
        let Ok(player) = u32::try_from(player) else {
            return false;
        };
        let owner = self.world_services.shroud();
        let shroud = owner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let position = glam::Vec3::new(position.x, position.z, position.y);
        matches!(
            shroud.get_shroud_state(player, &position),
            gamelogic::system::shroud_manager::ShroudState::Visible
        )
    }

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

    pub fn host_named_unit_id(&self, name: &str) -> Option<ObjectId> {
        self.host_object_id_by_script_name(name)
    }

    /// Host team query: live host objects on `team`.
    pub fn host_team_unit_ids(&self, team: crate::game_logic::Team) -> Vec<ObjectId> {
        self.host_objects()
            .registered()
            .filter(|(_, o)| o.team == team && o.is_alive() && !o.status.destroyed)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Host area query: live host objects whose XZ is inside `min..=max`.
    pub fn host_area_unit_ids(&self, min: glam::Vec3, max: glam::Vec3) -> Vec<ObjectId> {
        self.host_objects()
            .registered()
            .filter(|(_, o)| {
                if !o.is_alive() || o.status.destroyed {
                    return false;
                }
                let p = o.get_position();
                p.x >= min.x && p.x <= max.x && p.z >= min.z && p.z <= max.z
            })
            .map(|(id, _)| *id)
            .collect()
    }

    /// Project this roster into its trigger owner without callbacks under the guard.
    /// C++ Object.cpp:2563–2635 preserves integer pose, skips and frame edges.
    fn project_host_trigger_flags(&self) {
        let objects = self.host_objects();
        if objects.registered_is_empty() {
            return;
        }
        let mut trigger_world = self
            .host_trigger_world
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        trigger_world.set_current_frame(self.frame);
        let mut teams_with_edges = Vec::new();
        for (id, object) in objects.registered() {
            let skip = object.is_kind_of(crate::game_logic::KindOf::Projectile)
                || object.is_kind_of(crate::game_logic::KindOf::Inert);
            let team = if object.team_instance_name.is_empty() {
                None
            } else {
                Some(object.team_instance_name.as_str())
            };
            if trigger_world.update_object_flags_and_report(
                id.0,
                object.get_position().x,
                object.get_position().z,
                self.frame,
                skip,
                team,
            ) {
                if let Some(team_id) = object.team_instance_id {
                    teams_with_edges.push(team_id);
                }
            }
        }
        drop(trigger_world);

        // The synchronous pose callback normally marks Team at the source
        // event. This reconciliation path also admits edges discovered during
        // initial projection or trigger-owner restore. Keep the event on the
        // receiving Team so it survives roster edits and follows Team xfer.
        for team_id in teams_with_edges {
            self.team_factory.mark_team_entered_exited(team_id);
        }
    }

    /// Project this instance's roster without selecting or publishing an active world.
    /// Compatibility publication remains a separate operation below.
    pub(crate) fn build_host_script_query_snapshot(
        &self,
    ) -> gamelogic::scripting::HostScriptQuerySnapshot {
        use gamelogic::scripting::HostScriptQuerySnapshot;
        let objects = self.host_objects();
        let mut snap = HostScriptQuerySnapshot {
            named: self.host_named_unit_id_map(),
            objects: Vec::with_capacity(objects.registered_len()),
            ..Default::default()
        };
        // C++ script conditions query the live Object and Team. Preserve the
        // registration order in each list, and retain dead actors in the Team
        // instance census even though the faction queries only see living actors.
        let mut census: std::collections::HashMap<String, Vec<u32>> =
            std::collections::HashMap::new();
        {
            let shroud = self.world_services.shroud().lock().ok();
            for (id, object) in objects.registered() {
                let observation = self.receiving_script_query_object_with_shroud(
                    *id, object, shroud.as_deref(),
                );
                if observation.alive {
                    snap.team_ids.entry(observation.team).or_default().push(id.0);
                }
                snap.objects.push(observation);

                let instance_name = &object.team_instance_name;
                let census_name = if instance_name.is_empty() {
                    self.default_host_team_instance_name(object.owner_player_id, object.team)
                        .to_ascii_lowercase()
                } else {
                    if let Some(members) = snap.team_instance_ids.get_mut(instance_name) {
                        members.push(id.0);
                    } else {
                        snap.team_instance_ids.insert(instance_name.clone(), vec![id.0]);
                    }
                    instance_name.to_ascii_lowercase()
                };
                if !census_name.is_empty() {
                    census.entry(census_name).or_default().push(object.id.0);
                }
            }
        }
        let census_ids = |name: &str| -> Vec<u32> {
            let needle = name.trim();
            if needle.is_empty() {
                return Vec::new();
            }
            census
                .get(&needle.to_ascii_lowercase())
                .cloned()
                .unwrap_or_default()
        };
        if let Ok(factory) = self.team_factory.lock() {
            for name in factory.prototype_names() {
                if snap.team_instance_ids.contains_key(&name) {
                    continue;
                }
                let ids = census_ids(&name);
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
            let ids = census_ids(&name);
            if !ids.is_empty() {
                snap.team_instance_ids.insert(name, ids);
            }
        }
        snap
    }

    /// One immutable observation of the canonical receiving Object and services.
    /// Debug KindOf names shared per KindOf set (built once per distinct set).
    fn script_kind_names(&self, kinds: &crate::game_logic::KindOfSet) -> std::sync::Arc<Vec<String>> {
        let key = kinds.bits();
        let build = || std::sync::Arc::new(kinds.iter().map(|k| format!("{k:?}")).collect::<Vec<_>>());
        if let Some(hit) = self.script_kind_names_cache.borrow().get(&key) {
            return std::sync::Arc::clone(hit);
        }
        let names = build();
        self.script_kind_names_cache
            .borrow_mut()
            .insert(key, std::sync::Arc::clone(&names));
        names
    }

    pub(super) fn receiving_script_query_object(
        &self,
        id: ObjectId,
        obj: &Object,
    ) -> generals_sim::scripting::world::HostScriptQueryObject {
        let shroud_manager = self.world_services.shroud();
        let shroud = shroud_manager.lock().ok();
        self.receiving_script_query_object_with_shroud(id, obj, shroud.as_deref())
    }

    /// `receiving_script_query_object` with the shroud manager already locked
    /// (the per-frame snapshot locks it once for every object).
    pub(super) fn receiving_script_query_object_with_shroud(
        &self,
        id: ObjectId,
        obj: &Object,
        shroud: Option<&gamelogic::system::shroud_manager::ShroudManager>,
    ) -> generals_sim::scripting::world::HostScriptQueryObject {
        use generals_sim::scripting::world::HostScriptQueryObject;
        let position = obj.get_position();
        let alive = obj.is_alive() && !obj.status.destroyed;
        let occupants = obj.garrison_count();
        let transport_capacity = obj.transport_capacity();
        let damage_source = obj.last_damage_source.and_then(|id| self.objects.get(&id));
        HostScriptQueryObject {
            id: id.0,
            name: obj.name.clone(),
            team: obj.team as u32,
            x: position.x,
            y: position.y,
            z: position.z,

            alive,
            effectively_dead: obj.status.effectively_dead || obj.status.destroyed,
            health: obj.health.current,
            initial_health: obj.body_initial_health(),
            owner_player: obj
                .owner_player_id
                .and_then(|pid| self.player_name(pid))
                .unwrap_or_default(),
            template_name: obj.template_name.clone(),
            has_contain: obj.can_contain()
                || occupants > 0
                || transport_capacity > 0
                || obj
                    .building_data
                    .as_ref()
                    .is_some_and(|b| b.max_garrison > 0),
            contain_count: occupants as u32,
            contain_max: obj
                .building_data
                .as_ref()
                .map(|b| b.max_garrison)
                .unwrap_or(0)
                .max(transport_capacity) as u32,
            last_damage_source_id: obj.last_damage_source.map(|sid| sid.0).unwrap_or(0),
            last_damage_template: if !obj.last_damage_source_template.is_empty() {
                obj.last_damage_source_template.clone()
            } else {
                damage_source
                    .map(|source| source.template_name.clone())
                    .unwrap_or_default()
            },
            last_damage_player: damage_source
                .and_then(|source| source.owner_player_id)
                .and_then(|player| self.player_name(player))
                .unwrap_or_default(),
            kind_structure: obj.is_kind_of(crate::game_logic::KindOf::Structure),
            kind_projectile: obj.is_kind_of(crate::game_logic::KindOf::Projectile),
            kind_inert: leftover_host_template_is_inert(&obj.template_name),
            kind_mine: obj.is_kind_of(crate::game_logic::KindOf::Mine),
            held: obj.status.disabled_held,
            stealthed_hidden: obj.is_effectively_stealthed(),
            discovered_by: host_discovered_by_player_names(self, shroud, id.0, obj),
            waypoint_labels: obj.completed_waypoint_labels.clone(),
            selected: obj.selected,
            idle: matches!(obj.ai_state(), crate::game_logic::AIState::Idle)
                && !obj.status.moving
                && !obj.status.attacking,
            vision_range: obj.vision_range,
            kind_names: self.script_kind_names(&obj.thing().template.kind_of),
            special_power_ready: obj.special_power_ready && alive && !obj.is_disabled(),
            special_power_templates: obj
                .thing()
                .template
                .special_power_modules
                .iter()
                .map(|module| module.special_power_template.clone())
                .collect(),
            locomotor_surfaces: obj.locomotor_surfaces,
            captured: obj.status.private_captured,
            unmanned: obj.status.disabled_unmanned,
            garrisonable: obj.is_garrison_contain(),
            build_cost: obj.thing().template.build_cost.supplies as i32,
            status_bits: host_query_object_status_bits(obj),
            player_who_entered: host_query_player_who_entered(self, obj),
            is_supply_warehouse: obj.thing().template.dock_kind
                == crate::game_logic::DockKind::SupplyWarehouse,
            warehouse_boxes: obj.warehouse_boxes(),
            off_map: position.x < self.world_min.x
                || position.x > self.world_max.x
                || position.z < self.world_min.z
                || position.z > self.world_max.z,
            contained_by: obj.contained_by.map(|cid| cid.0).unwrap_or(0),
            // Live has no AI_EXIT state; leftover Exit is pretend-contained.
            ai_exiting: false,

            ..Default::default()
        }
    }

    /// Temporary condition adapter; explicit queries use the same owned projection.
    pub fn inject_host_script_query_snapshot(&self) {
        self.with_own_services(|| self.inject_host_script_query_snapshot_bound())
    }

    fn inject_host_script_query_snapshot_bound(&self) {
        self.project_host_trigger_flags();
        let mut snap = self.build_host_script_query_snapshot();
        // Legacy snapshot geometry is projected from the same receiving
        // polygon owner; first name wins, like PolygonTriggerList lookup.
        {
            let terrain = self
                .world_services
                .terrain()
                .read()
                .expect("owned terrain script query");
            for trigger in terrain.get_trigger_areas().get_triggers() {
                let bounds = trigger.get_bounds();
                snap.areas
                    .entry(trigger.get_trigger_name().to_string())
                    .or_insert((
                        bounds.lo.x as f32,
                        bounds.lo.y as f32,
                        bounds.hi.x as f32,
                        bounds.hi.y as f32,
                    ));
            }
        }
        self.capture_host_player_and_tech_census(&mut snap);
        gamelogic::scripting::HostScriptQuery::of(&self.world_services).set_host_script_query_snapshot(snap);
    }

    /// Sample the receiving player's registered buildings and build facilities.
    pub(super) fn owned_script_building_census_for(
        &self,
        player_id: u32,
    ) -> Option<generals_sim::scripting::world::HostScriptPlayerCensus> {
        let player = self.players.get(&player_id)?;
        let mut census = self.script_census_money_and_power(player);
        let facilities = self.build_facility_template_names();
        // Team.cpp:1620–1648,2493–2502 observes the registered membership.
        for object in self.host_objects().registered_values() {
            if !self.object_owned_by_player(object, player_id) {
                continue;
            }
            if object.is_kind_of(KindOf::Structure) {
                census.building_count += 1;
                if object.is_kind_of(KindOf::MpCountForVictory) {
                    census.faction_building_count += 1;
                }
            }
            census.has_any_build_facility |= facilities.contains(&object.template_name.to_ascii_lowercase());
        }
        Some(census)
    }

    pub(super) fn owned_host_player_economy_for(
        &self,
        player_id: u32,
    ) -> Option<generals_sim::scripting::world::HostScriptPlayerCensus> {
        let player = self.players.get(&player_id)?;
        Some(self.script_census_money_and_power(player))
    }

    fn script_census_money_and_power(
        &self,
        player: &Player,
    ) -> generals_sim::scripting::world::HostScriptPlayerCensus {
        generals_sim::scripting::world::HostScriptPlayerCensus {
            money: player.effective_supplies() as i32,
            energy_production: player.energy_production_at(self.frame),
            energy_consumption: player.power_consumed,
            science_purchase_points: player.science_purchase_points,
            power_sabotaged: player.power_sabotaged_till_frame != 0
                && self.frame < player.power_sabotaged_till_frame,
            ..Default::default()
        }
    }

    /// Publish census facts for legacy conditions using the compatibility view.
    pub(super) fn owned_host_player_census(
        &self,
    ) -> std::collections::HashMap<String, generals_sim::scripting::world::HostScriptPlayerCensus> {
        use crate::game_logic::KindOf;
        use generals_sim::scripting::world::HostScriptPlayerCensus;

        // C++ Player/Team counters are maintained incrementally. We still
        // rebuild from the living object set, but do it in one pass over
        // objects (O(objects + players)) instead of O(players * objects).
        let build_facilities = self.build_facility_template_names();
        let mut by_player_id: std::collections::HashMap<u32, HostScriptPlayerCensus> =
            std::collections::HashMap::new();
        for player in self.players.values() {
            by_player_id.insert(
                player.id,
                HostScriptPlayerCensus {
                    science_purchase_points: player.science_purchase_points,
                    unlocked_sciences: player.unlocked_sciences.iter().cloned().collect(),
                    supply_box_value: self.supply_box_value(),
                    ..self.script_census_money_and_power(player)
                },
            );
        }

        #[derive(Default)]
        struct TemplateTally {
            count: i32,
            alive: i32,
            any_not_destroyed: bool,
        }
        let mut template_tallies: std::collections::HashMap<
            u32,
            std::collections::HashMap<&str, TemplateTally>,
        > = std::collections::HashMap::new();
        for obj in self.host_objects().registered_values() {
            // Resolve ownership once (same rules as object_owned_by_player).
            let owner_id = match obj.owner_player_id {
                Some(owner) => owner,
                None => {
                    let mut same_faction = self
                        .players
                        .values()
                        .filter(|player| player.team == obj.team);
                    match (same_faction.next(), same_faction.next()) {
                        (Some(player), None) => player.id,
                        _ => continue,
                    }
                }
            };
            let Some(census) = by_player_id.get_mut(&owner_id) else {
                continue;
            };
            let dead = !obj.is_alive() || obj.status.destroyed || obj.status.effectively_dead;
            if !census.has_any_objects
                && !dead
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
            if obj.template_name.is_empty() {
                continue;
            }
            // Tally by the object's own template name (no per-object
            // lowercase allocation); keys are folded once per template below.
            let tally = template_tallies
                .entry(owner_id)
                .or_default()
                .entry(obj.template_name.as_str())
                .or_default();
            tally.any_not_destroyed |= !obj.status.destroyed;
            if !obj.status.under_construction {
                tally.count += 1;
                if !dead {
                    tally.alive += 1;
                }
            }
        }

        for (owner_id, tallies) in template_tallies {
            let Some(census) = by_player_id.get_mut(&owner_id) else {
                continue;
            };
            for (name, tally) in tallies {
                let key = name.to_ascii_lowercase();
                if tally.any_not_destroyed && build_facilities.contains(&key) {
                    census.has_any_build_facility = true;
                }
                if tally.alive > 0 {
                    *census.template_counts_ignore_dead.entry(key.clone()).or_insert(0) += tally.alive;
                }
                if tally.count > 0 {
                    *census.template_counts.entry(key).or_insert(0) += tally.count;
                }
            }
        }

        let mut player_census = std::collections::HashMap::new();
        for player in self.players.values() {
            if player.map_side.map_player_name.is_empty() {
                continue;
            }
            if let Some(census) = by_player_id.remove(&player.id) {
                player_census.insert(player.map_side.map_player_name.clone(), census);
            }
        }
        player_census
    }

    fn capture_host_player_and_tech_census(
        &self,
        snap: &mut gamelogic::scripting::HostScriptQuerySnapshot,
    ) {
        use crate::game_logic::KindOf;
        use gamelogic::scripting::HostTechBuildingCensus;
        let player_census = self.owned_host_player_census();
        let mut tech_buildings = Vec::new();
        for obj in self.host_objects().registered_values() {
            if !obj.is_kind_of(KindOf::TechBuilding) {
                continue;
            }
            if !obj.is_alive() || obj.status.destroyed || obj.status.effectively_dead {
                continue;
            }
            tech_buildings.push(HostTechBuildingCensus {
                x: obj.get_position().x,
                z: obj.get_position().z,
                owner_player: obj
                    .owner_player_id
                    .and_then(|pid| self.player_name(pid))
                    .unwrap_or_default(),
                team: obj.team as u32,
                off_map: crate::game_logic::host_deliver_payload::is_off_map_default_residual(
                    obj.get_position(),
                ),
            });
        }
        snap.player_census = player_census;
        snap.tech_buildings = tech_buildings;
    }

    /// Observe this owner's body before borrowing its AI queue. The manager
    /// stays installed throughout synchronous script execution.
    pub(super) fn apply_owned_script_ai_repair(&mut self, player: &str, structure: &str) {
        let Some(pid) = self.host_player_id_for_script_token(player) else {
            return;
        };
        let Some(id) = self.host_object_id_by_script_name(structure) else {
            return;
        };
        let Some(object) = self.host_object(id) else {
            return;
        };
        if matches!(
            object.body_damage_state,
            generals_sim::object::BodyDamageType::Pristine
        ) && object.health.current + 0.01 >= object.health.maximum
        {
            return;
        }
        if let Some(ai) = self.ai_manager.ai_players.get_mut(&pid) {
            ai.queue_damaged_structure_repair(id);
        }
    }

    /// C++ Player::setObjectsEnabled — SCRIPT_DISABLED on matching templates.
    pub(super) fn host_set_objects_enabled(
        &mut self,
        player_id: u32,
        template_name: &str,
        enable: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // C++ Player::setObjectsEnabled walks the receiving Player's ordered
        // prototype list, then each prototype's live instance list and member
        // order. Re-resolve both membership and object identity before every
        // status callback, since the callback may re-enter world mutation.
        let teams = self
            .team_factory
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .owned_team_instances_for_controller(player_id);
        for team in teams {
            let members: Vec<_> = team
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .get_members()
                .iter()
                .filter_map(|raw_id| {
                    let id = ObjectId(*raw_id);
                    self.host_object(id).map(|object| {
                        (
                            *raw_id,
                            std::sync::Arc::downgrade(&object.visual_draw_identity_token),
                        )
                    })
                })
                .collect();
            for (raw_id, object_birth) in members {
                let still_owned = {
                    let factory = self
                        .team_factory
                        .lock()
                        .unwrap_or_else(|error| error.into_inner());
                    factory
                        .owned_team_instances_for_controller(player_id)
                        .iter()
                        .any(|current| std::sync::Arc::ptr_eq(current, &team))
                };
                let still_member = team
                    .read()
                    .unwrap_or_else(|error| error.into_inner())
                    .get_members()
                    .contains(&raw_id);
                if !still_owned || !still_member {
                    continue;
                }
                let id = ObjectId(raw_id);
                let Some(object) = self.host_object(id) else {
                    continue;
                };
                let Some(expected_birth) = object_birth.upgrade() else {
                    continue;
                };
                if !std::sync::Arc::ptr_eq(&object.visual_draw_identity_token, &expected_birth)
                    || object.template_name != template_name
                {
                    continue;
                }
                self.set_owned_script_disable(
                    id, crate::game_logic::object::ScriptDisable::Disabled, !enable, effects,
                );
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
        let rank_level_limit = self.rank_level_limit();
        let ranks = self.authored_ranks();
        let Some(player) = self.players.get_mut(&player_id) else {
            return false;
        };
        player.set_rank_level_from_template(new_level, template.as_ref(), &ranks, rank_level_limit)
    }

    /// C++ `Player::transferAssetsFromThat` — non-beacon objects onto dest
    /// default team, then withdraw/deposit all cash.
    pub(super) fn host_script_transfer_assets_from_with_effects(
        &mut self,
        dest_player: u32,
        source_player: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        // Player::transferAssetsFromThat does nothing at all when the
        // destination has no default Team, including leaving its money alone.
        let Some(destination_id) = self.host_default_team_id(dest_player) else {
            return Ok(());
        };
        if let Some(identity) = self.player_template_identity(source_player) {
            if self
                .player_template_definitions
                .as_ref()
                .and_then(|definitions| identity.resolve_in(definitions))
                .is_none()
            {
                return Err(generals_sim::GameLogicError::Configuration(
                    "player asset transfer requires its source PlayerTemplate".into(),
                ));
            }
        }
        let destination = self
            .team_factory
            .with_read_without_flushing(|factory| factory.find_team_by_id(destination_id))
            .map_err(|_| {
                generals_sim::GameLogicError::Threading("receiving TeamFactory lock poisoned".into())
            })?
            .ok_or_else(|| {
                generals_sim::GameLogicError::Configuration(
                    "selected player transfer Team is unavailable".into(),
                )
            })?;
        let destination_birth = std::sync::Arc::downgrade(&destination);
        drop(destination);
        // The original first captures the source player's prototype/Team/member
        // traversal, then changes each captured Object's Team synchronously.
        let members: Vec<_> = {
            let teams = self
                .team_factory
                .with_read_without_flushing(|factory| {
                    factory.owned_team_instances_for_controller(source_player)
                })
                .map_err(|_| {
                    generals_sim::GameLogicError::Threading(
                        "receiving TeamFactory lock poisoned".into(),
                    )
                })?;
            let mut members = Vec::new();
            for team in teams {
                let roster = team.read().map_err(|_| {
                    generals_sim::GameLogicError::Threading("receiving Team lock poisoned".into())
                })?;
                for &raw_id in roster.get_members() {
                    let id = ObjectId(raw_id);
                    if let Some(object) = self.host_object(id) {
                        if !crate::command_executor::host_object_is_beacon_for_player(
                            self,
                            id,
                            source_player,
                        ) {
                            members.push((
                                id,
                                std::sync::Arc::downgrade(&object.visual_draw_identity_token),
                            ));
                        }
                    }
                }
            }
            members
        };
        for (id, birth) in members {
            if !self.object_birth_matches(id, &birth) {
                continue;
            }
            let same_destination = self
                .team_factory
                .with_read_without_flushing(|factory| {
                    factory
                        .find_team_by_id(destination_id)
                        .is_some_and(|current| {
                            destination_birth.ptr_eq(&std::sync::Arc::downgrade(&current))
                                && current.read().is_ok_and(|team| {
                                    team.get_controlling_player_id() == Some(dest_player)
                                })
                        })
                })
                .map_err(|_| {
                    generals_sim::GameLogicError::Threading(
                        "receiving TeamFactory lock poisoned".into(),
                    )
                })?;
            if !same_destination
                || !self.transfer_object_to_selected_team_with_effects(
                    id,
                    dest_player,
                    destination_id,
                    effects,
                )?
            {
                return Err(generals_sim::GameLogicError::Configuration(
                    "player asset transfer lost its selected Team or Object".into(),
                ));
            }
        }
        let amount = self
            .players
            .get(&source_player)
            .map(|p| p.effective_supplies())
            .unwrap_or(0);
        if amount == 0 {
            return Ok(());
        }
        if let Some(src) = self.players.get_mut(&source_player) {
            self.economy_events.record_money_audio(
                source_player,
                crate::game_logic::host_economy_log::HostMoneyAudio::Withdraw,
            );
            src.apply_supply_spend_unchecked(amount, &mut self.economy_events);
        }
        if let Some(dst) = self.players.get_mut(&dest_player) {
            self.economy_events.record_money_audio(
                dest_player,
                crate::game_logic::host_economy_log::HostMoneyAudio::Deposit,
            );
            dst.apply_supply_gain(amount, &mut self.economy_events);
        }
        Ok(())
    }

    /// C++ Money::withdraw(countMoney()) then Money::deposit(amount).
    pub(super) fn host_script_set_player_money(&mut self, pid: u32, amount: i32) {
        let Some(player) = self.players.get_mut(&pid) else {
            return;
        };
        let current = player.effective_supplies();
        if current > 0 {
            self.economy_events.record_money_audio(
                pid,
                crate::game_logic::host_economy_log::HostMoneyAudio::Withdraw,
            );
            player.apply_supply_spend_unchecked(current, &mut self.economy_events);
        }
        let deposit = amount.max(0) as u32;
        if deposit > 0 {
            self.economy_events.record_money_audio(
                pid,
                crate::game_logic::host_economy_log::HostMoneyAudio::Deposit,
            );
            player.apply_supply_gain(deposit, &mut self.economy_events);
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
                self.economy_events.record_money_audio(
                    pid,
                    crate::game_logic::host_economy_log::HostMoneyAudio::Withdraw,
                );
                player.apply_supply_spend_unchecked(actual, &mut self.economy_events);
            }
        } else if amount > 0 {
            self.economy_events.record_money_audio(
                pid,
                crate::game_logic::host_economy_log::HostMoneyAudio::Deposit,
            );
            player.apply_supply_gain(amount as u32, &mut self.economy_events);
        }
    }

    pub(super) fn host_player_id_for_script_token(&self, token: &str) -> Option<u32> {
        let needle = token;
        if needle.is_empty() {
            return None;
        }
        // C++ ScriptEngine::getPlayerFromAsciiString: LOCAL_PLAYER / THIS_PLAYER.
        // Tokens are "<Local Player>" / "<This Player>", not THIS_PLAYER/THE_PLAYER.
        if needle.eq_ignore_ascii_case("<This Player>")
            || needle.eq_ignore_ascii_case("<Local Player>")
        {
            return self.local_player_id();
        }
        // C++ NameKeyGenerator::nameToKey uses strcmp; PlayerList compares
        // that exact canonical playerName key. Display labels and faction names
        // are not Player identities.
        self.players
            .values()
            .find(|player| {
                !player.map_side.map_player_name.is_empty()
                    && player.map_side.map_player_name == needle
            })
            .map(|player| player.id)
    }
}

#[cfg(test)]
mod trigger_projection_tests;

#[cfg(test)]
mod pathfind_relationship_refresh_tests {
    use super::*;

    #[test]
    fn player_relation_resolves_script_enemy_in_the_receiving_world() {
        use generals_sim::scripting::program::{
            Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
            ScriptActionType, ScriptList, THIS_PLAYER, THIS_PLAYER_ENEMY,
        };

        let (mut receiving, _, _) = super::named_command_test_support::world();
        let (peer, _, _) = super::named_command_test_support::world();
        assert!(!receiving.pathfinding_system.players_are_pathfind_allies(2, 1));
        assert!(!peer.pathfinding_system.players_are_pathfind_allies(2, 1));

        // ScriptEngine.cpp:5779–5831 chooses the receiving player's current
        // enemy, then its first human player. Slot 2 is the authored caller.
        let mut relate = ScriptAction::new(ScriptActionType::PlayerRelatesPlayer);
        relate
            .add_parameter(Parameter::with_string(ParameterType::Side, THIS_PLAYER.into()))
            .unwrap();
        relate
            .add_parameter(Parameter::with_string(
                ParameterType::Side,
                THIS_PLAYER_ENEMY.into(),
            ))
            .unwrap();
        relate.add_parameter(Parameter::with_int(ParameterType::Int, 2)).unwrap();
        let mut branch = OrCondition::new();
        branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
        let mut script = Script::new();
        script.is_one_shot = true;
        script.condition = Some(Box::new(branch));
        script.action = Some(Box::new(relate));
        let mut list = ScriptList::new();
        list.append_script(Box::new(script));
        let engine = gamelogic::scripting::engine::ScriptEngine::new_in_published_world().unwrap();
        engine.set_script_list_for_player(2, Some(Box::new(list))).unwrap();

        gamelogic::system::engine_stores::with_world_services(&peer.world_services, || {
            receiving.update_local_script_engine_with_owner(&engine).unwrap();
        });

        assert!(
            receiving.pathfinding_system.players_are_pathfind_allies(2, 1),
            "the authored relationship to the current script enemy must reach the pathfinder"
        );
        assert!(!peer.pathfinding_system.players_are_pathfind_allies(2, 1));
    }
}
