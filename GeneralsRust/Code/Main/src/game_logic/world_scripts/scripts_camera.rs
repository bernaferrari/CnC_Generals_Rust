//! Host scripts `impl GameLogic` — `scripts_camera`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! script eval / EVA process / camera path / script camera
#![allow(unused_imports, non_snake_case)]
use super::super::*;
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static HOST_PREV_BRIDGE_BROKEN: RefCell<HashMap<String, bool>> =
        RefCell::new(HashMap::new());
}

fn merge_host_bridge_states(
    world: &GameLogic,
    snap: &mut gamelogic::scripting::HostScriptQuerySnapshot,
) {
    use crate::game_logic::host_bridge_behavior::is_bridge_span_template;
    let mut current = HashMap::new();
    for obj in world.host_objects().values() {
        if obj.name.is_empty() || !is_bridge_span_template(&obj.template_name) {
            continue;
        }
        let broken = !obj.is_alive() || obj.status.destroyed || obj.health.current <= 0.0;
        current.insert(obj.name.clone(), broken);
        snap.named_bridge_broken.insert(obj.name.clone(), broken);
        snap.named_bridge_repaired.insert(obj.name.clone(), !broken);
    }
    snap.any_bridges_damage_states_changed = HOST_PREV_BRIDGE_BROKEN.with(|prev| {
        let mut prev = prev.borrow_mut();
        let changed = !prev.is_empty()
            && current
                .iter()
                .any(|(name, broken)| prev.get(name) != Some(broken));
        *prev = current;
        changed
    });
}

/// C++ KINDOF_INERT from leftover ThingTemplate when the factory is already loaded.
/// Never calls TheThingFactory::find_template (that lazy-inits Object INI).
fn leftover_host_template_is_inert(template_name: &str) -> bool {
    if template_name.is_empty() {
        return false;
    }
    game_engine::common::thing::thing_factory::try_get_thing_factory()
        .and_then(|guard| {
            guard
                .as_ref()
                .and_then(|factory| factory.find_template(template_name, false))
        })
        .is_some_and(|template| {
            template.is_kind_of_mask(game_engine::common::system::kind_of::KindOfMask::INERT.bits())
        })
}

/// C++ Object::getShroudedStatus CLEAR|PARTIAL_CLEAR per player (no stealth filter).
fn host_discovered_by_player_names(
    logic: &crate::game_logic::GameLogic,
    object_id: u32,
    obj: &crate::game_logic::object::Object,
) -> Vec<String> {
    use gamelogic::common::ObjectShroudStatus;
    if obj.status.disabled_held {
        return Vec::new();
    }
    let mut names = Vec::new();
    let shroud = gamelogic::system::shroud_manager::get_shroud_manager()
        .lock()
        .ok();
    for player in logic.players.values() {
        let status = shroud
            .as_ref()
            .and_then(|mgr| mgr.get_host_object_shroud_status(player.id, object_id));
        let visible = match status {
            Some(ObjectShroudStatus::Clear | ObjectShroudStatus::PartialClear) => true,
            Some(_) => false,
            None => obj.owner_player_id == Some(player.id),
        };
        if visible && !player.name.is_empty() {
            names.push(player.name.clone());
        }
    }
    names
}

fn leftover_waypoint_path_labels(path_label: &str, last: glam::Vec3) -> Vec<String> {
    let mut labels = Vec::new();
    if !path_label.is_empty() {
        labels.push(path_label.to_string());
    }
    let Ok(terrain) = gamelogic::terrain::get_terrain_logic().read() else {
        return labels;
    };
    let pos = gamelogic::common::Coord3D::new(last.x, last.z, last.y);
    if let Some(wp) = terrain.get_closest_waypoint_on_path(&pos, path_label) {
        for label in [
            wp.get_path_label1().as_str(),
            wp.get_path_label2().as_str(),
            wp.get_path_label3().as_str(),
        ] {
            if !label.is_empty() && !labels.iter().any(|existing| existing == label) {
                labels.push(label.to_string());
            }
        }
    }
    labels
}

/// C++ Object::getStatusBits: packed OBJECT_STATUS_* from live host flags.
fn host_query_object_status_bits(obj: &crate::game_logic::object::Object) -> u64 {
    use crate::game_logic::host_status_bits_upgrade::object_status_mask_from_names;
    let s = &obj.status;
    let mut names: Vec<&str> = Vec::new();
    if s.destroyed {
        names.push("DESTROYED");
    }
    if s.under_construction {
        names.push("UNDER_CONSTRUCTION");
    }
    if s.unselectable {
        names.push("UNSELECTABLE");
    }
    if s.no_collisions {
        names.push("NO_COLLISIONS");
    }
    if s.airborne_target {
        names.push("AIRBORNE_TARGET");
    }
    if s.parachuting {
        names.push("PARACHUTING");
    }
    if s.repulsor {
        names.push("REPULSOR");
    }
    if s.hijacked {
        names.push("HIJACKED");
    }
    if s.wet {
        names.push("WET");
    }
    if s.is_firing_weapon {
        names.push("IS_FIRING_WEAPON");
    }
    if s.stealthed {
        names.push("STEALTHED");
    }
    if s.detected {
        names.push("DETECTED");
    }
    if s.sold {
        names.push("SOLD");
    }
    if s.reconstructing {
        names.push("RECONSTRUCTING");
    }
    if s.masked {
        names.push("MASKED");
    }
    if s.attacking {
        names.push("IS_ATTACKING");
    }
    if s.using_ability {
        names.push("USING_ABILITY");
    }
    if s.is_aiming_weapon {
        names.push("IS_AIMING_WEAPON");
    }
    if s.ignoring_stealth {
        names.push("IGNORING_STEALTH");
    }
    if s.is_carbomb {
        names.push("IS_CARBOMB");
    }
    if s.deck_height_offset {
        names.push("DECK_HEIGHT_OFFSET");
    }
    if s.faerie_fire {
        names.push("FAERIE_FIRE");
    }
    if s.booby_trapped {
        names.push("BOOBY_TRAPPED");
    }
    if s.disguised {
        names.push("DISGUISED");
    }
    if s.deployed {
        names.push("DEPLOYED");
    }
    obj.object_status_bits | object_status_mask_from_names(&names)
}

/// C++ OpenContain::getPlayerWhoEntered — SIDE name, one-frame pulse after enter.
fn host_query_player_who_entered(
    _logic: &GameLogic,
    obj: &crate::game_logic::object::Object,
) -> String {
    obj.player_who_entered.clone()
}




impl GameLogic {
    pub(in super::super) fn build_script_game_state_context(
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

    fn refresh_live_audio_locality(&self) {
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
    pub(in super::super) fn process_eva_events(&mut self) {}

    /// Evaluate and execute scripts each frame
    /// This is called from the main game loop (update_simulation)
    /// Phase 8 of game loop update sequence (C++ Generals compatibility)
    /// Count scripts currently installed from the last map load (groups + free lists).
    pub(in super::super) fn mission_script_count(&self) -> usize {
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

    /// Host named-unit query (scripts/AI). Prefer this over empty crate THE_AI groups.
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
                initial_health: obj.health.maximum.max(obj.max_health).max(1.0),
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
    fn inject_host_supply_source_queries(&mut self) {
        let mut attacked = std::collections::HashMap::new();
        let mut cash_map = std::collections::HashMap::new();
        let mut safe_map = std::collections::HashMap::new();
        let mut ai_mgr = std::mem::take(&mut self.ai_manager);
        let player_rows: Vec<(u32, String, crate::game_logic::Team)> = self
            .players
            .values()
            .map(|p| (p.id, p.name.clone(), p.team))
            .collect();
        for (pid, name, team) in player_rows {
            let key = name.trim().to_ascii_lowercase();
            if key.is_empty() {
                continue;
            }
            let Some(ai) = ai_mgr.ai_players.get_mut(&pid) else {
                continue;
            };
            let is_attacked = ai.is_supply_source_attacked(self);
            let warehouse = ai.find_supply_center(self, 0);
            let (cash, location_safe) = match warehouse.and_then(|id| self.host_object(id)) {
                Some(obj) => {
                    let pos = obj.get_position();
                    let cash = obj.stored_resources.supplies as i32;
                    let enemy_near = self.host_objects().values().any(|other| {
                        other.is_alive()
                            && other.team != team
                            && other.team != crate::game_logic::Team::Neutral
                            && other.is_mobile()
                            && (other.get_position() - pos).length_squared() <= 150.0 * 150.0
                    });
                    (cash, !enemy_near)
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
    fn merge_host_player_census_into_snapshot(&self) {
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
                let dead =
                    !obj.is_alive() || obj.status.destroyed || obj.status.effectively_dead;
                if !dead
                    && !obj.is_kind_of(KindOf::Projectile)
                    && !obj.is_kind_of(KindOf::Mine)
                    && !obj.is_kind_of(KindOf::SmallMissile)
                    && !obj.is_kind_of(KindOf::BallisticMissile)
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
                        *census
                            .template_counts_ignore_dead
                            .entry(key)
                            .or_insert(0) += 1;
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
        let supply_centers = gamelogic::scripting::take_host_ai_player_build_supply_center_requests();
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
    fn apply_host_set_base_construction_speed_requests(&mut self) {
        for (player, delay) in gamelogic::scripting::take_host_set_base_construction_speed_requests()
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
    fn apply_host_set_train_held_requests(&mut self) {
        for (unit, held) in gamelogic::scripting::take_host_set_train_held_requests() {
            if let Some(id) = self.host_object_id_by_script_name(&unit) {
                self.set_railroad_held(id, held);
            }
        }
    }


    /// C++ ScriptActions::doSetMoney / doGiveMoney live drain.
    /// Leftover mutates crate player_list; live cash lives on host Player.
    fn apply_host_money_script_requests(&mut self) {
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
    fn apply_host_can_build_script_requests(&mut self) {
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
    fn host_script_flash_color(&self, id: crate::game_logic::ObjectId, white: bool) -> u32 {
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
    fn apply_host_script_visual_status_requests(&mut self) {
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

        for req in gamelogic::scripting::take_host_script_custom_color_requests() {
            if let Some(id) = self.host_object_id_by_script_name(&req.unit) {
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.set_custom_indicator_color_raw(req.color_raw);
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
            let (ids, distance) = match req {
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
            for id in ids {
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
    fn host_set_objects_enabled(&mut self, player_id: u32, template_name: &str, enable: bool) {
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
    fn apply_host_buildable_override_script_requests(&mut self) {
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

    fn template_mut_by_name(&mut self, name: &str) -> Option<&mut ThingTemplate> {
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
    fn apply_host_rank_script_requests(&mut self) {
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
                    let changed = self
                        .set_player_rank_level(pid, (current as i32 + delta).max(1) as u32);
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
    fn apply_host_transfer_script_requests(&mut self) {
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
    fn apply_host_player_relates_script_requests(&mut self) {
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
    fn apply_host_team_override_relation_script_requests(&mut self) {
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
    fn host_script_transfer_assets_from(&mut self, dest_player: u32, source_player: u32) {
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
    fn host_script_transfer_team_to_player(&mut self, team_name: &str, dest_player: u32) {
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
    fn host_script_set_player_money(&mut self, pid: u32, amount: i32) {
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
    fn host_script_give_player_money(&mut self, pid: u32, amount: i32) {
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

    fn host_player_id_for_script_token(&self, token: &str) -> Option<u32> {
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
        self.players.values().find(|p| {
            p.name.eq_ignore_ascii_case(needle)
                || p.map_side.map_player_name.eq_ignore_ascii_case(needle)
                || format!("Plyr{}", p.team.get_name()).eq_ignore_ascii_case(needle)
        }).map(|p| p.id)
    }

    /// C++ ScriptActions::doSetCaveIndex live drain.
    pub fn apply_host_set_cave_index_requests(&mut self) {
        for (cave_name, index) in gamelogic::scripting::take_host_set_cave_index_requests() {
            let _ = self.set_named_cave_index(&cave_name, index);
        }
    }

    /// C++ ScriptActions TEAM/NAMED move and attack live drain.
    /// Leftover `OBJECT_REGISTRY` is empty on the host path; leftover actions
    /// queue [`gamelogic::scripting::HostScriptMoveAttackRequest`].
    fn apply_host_move_attack_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptMoveAttackRequest;
        for req in gamelogic::scripting::take_host_script_move_attack_requests() {
            match req {
                HostScriptMoveAttackRequest::TeamMove { team, waypoint } => {
                    let Some(dest) = self.host_script_waypoint_position(&waypoint) else {
                        continue;
                    };
                    for id in self.host_script_team_member_ids(&team) {
                        let _ = self.unit_command_move_to(id, dest);
                    }
                }
                HostScriptMoveAttackRequest::NamedMove { unit, waypoint } => {
                    let Some(dest) = self.host_script_waypoint_position(&waypoint) else {
                        continue;
                    };
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let _ = self.apply_unit_locomotor_set(id, "normal");
                    let _ = self.unit_command_move_to(id, dest);
                }
                HostScriptMoveAttackRequest::TeamAttackTeam { attacker, victim } => {
                    let members = self.host_script_team_member_ids(&attacker);
                    for id in members {
                        self.host_script_attack_team(id, &victim);
                    }
                }
                HostScriptMoveAttackRequest::NamedAttackNamed { attacker, victim } => {
                    let Some(aid) = self.host_object_id_by_script_name(&attacker) else {
                        continue;
                    };
                    let Some(vid) = self.host_object_id_by_script_name(&victim) else {
                        continue;
                    };
                    let _ = self.apply_unit_locomotor_set(aid, "normal");
                    let _ = self.unit_command_force_attack(aid, vid);
                }
                HostScriptMoveAttackRequest::NamedAttackArea { unit, area } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let _ = self.apply_unit_locomotor_set(id, "normal");
                    self.host_script_attack_area(id, &area);
                }
                HostScriptMoveAttackRequest::NamedAttackTeam { unit, team } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let _ = self.apply_unit_locomotor_set(id, "normal");
                    self.host_script_attack_team(id, &team);
                }
                HostScriptMoveAttackRequest::TeamAttackArea { team, area } => {
                    let members = self.host_script_team_member_ids(&team);
                    for id in members {
                        self.host_script_attack_area(id, &area);
                    }
                }
                HostScriptMoveAttackRequest::TeamAttackNamed { team, unit } => {
                    let Some(vid) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    for id in self.host_script_team_member_ids(&team) {
                        let _ = self.unit_command_attack(id, vid);
                    }
                }
            }
        }
    }

    /// C++ ScriptActions TEAM/NAMED HUNT, TEAM/NAMED GUARD, PLAYER_HUNT.
    /// Leftover `OBJECT_REGISTRY` is empty on the host path; leftover actions
    /// queue [`gamelogic::scripting::HostScriptHuntGuardRequest`].
    fn apply_host_hunt_guard_script_requests(&mut self) {
        use crate::game_logic::KindOf;
        use gamelogic::scripting::HostScriptHuntGuardRequest;
        for req in gamelogic::scripting::take_host_script_hunt_guard_requests() {
            match req {
                HostScriptHuntGuardRequest::TeamHunt { team } => {
                    for id in self.host_script_hunt_guard_team_member_ids(&team) {
                        let _ = self.unit_command_patrol(id);
                    }
                }
                HostScriptHuntGuardRequest::NamedHunt { unit } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let _ = self.apply_unit_locomotor_set(id, "normal");
                    let _ = self.unit_command_patrol(id);
                }
                HostScriptHuntGuardRequest::TeamGuard { team } => {
                    // C++ doTeamGuard: leftover getTeamNamed instance, every member with AI.
                    let members = self.host_script_hunt_guard_team_member_ids(&team);
                    for id in members {
                        if !self.host_script_unit_can_guard(id) {
                            continue;
                        }
                        let Some(pos) = self.host_object(id).map(|u| u.get_position()) else {
                            continue;
                        };
                        let _ = self.unit_command_guard_position(id, pos);
                    }
                }
                HostScriptHuntGuardRequest::NamedGuard { unit } => {
                    // C++ doNamedGuard: AIUpdateInterface only (Stinger/stun still guard).
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    if !self.host_script_unit_can_guard(id) {
                        continue;
                    }
                    let Some(pos) = self.host_object(id).map(|u| u.get_position()) else {
                        continue;
                    };
                    let _ = self.apply_unit_locomotor_set(id, "normal");
                    let _ = self.unit_command_guard_position(id, pos);
                }
                HostScriptHuntGuardRequest::PlayerHunt { player } => {
                    let Some(pid) = self.host_player_id_for_script_token(&player) else {
                        continue;
                    };
                    if let Some(player) = self.players.get_mut(&pid) {
                        player.units_should_hunt = true;
                    }

                    let team = self.players.get(&pid).map(|p| p.team);
                    let ids: Vec<ObjectId> = self
                        .objects
                        .values()
                        .filter(|obj| {
                            if !obj.is_alive() || obj.status.destroyed {
                                return false;
                            }
                            if obj.is_kind_of(KindOf::Dozer)
                                || obj.is_kind_of(KindOf::Harvester)
                                || obj.is_kind_of(KindOf::IgnoresSelectAll)
                            {
                                return false;
                            }
                            match obj.owner_player_id {
                                Some(oid) => oid == pid,
                                None => team.map(|t| obj.team == t).unwrap_or(false),
                            }
                        })
                        .map(|obj| obj.id)
                        .collect();
                    for id in ids {
                        // C++ Player::setUnitsShouldHunt: leaveGroup then aiHunt.
                        self.host_object_leave_group(id);
                        let _ = self.unit_command_patrol(id);
                    }
                }
                HostScriptHuntGuardRequest::TeamHuntWithCommandButton { team, button } => {
                    self.host_script_team_hunt_with_command_button(&team, &button);
                }
            }
        }
    }

    /// C++ ScriptActions NAMED_STOP / TEAM_STOP / TEAM_STOP_AND_DISBAND.
    /// Leftover `OBJECT_REGISTRY` is empty on the host path; leftover actions
    /// queue [`gamelogic::scripting::HostScriptIdleRequest`] (`aiIdle` / `groupIdle`).
    fn apply_host_idle_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptIdleRequest;
        for req in gamelogic::scripting::take_host_script_idle_requests() {
            match req {
                HostScriptIdleRequest::NamedStop { unit } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let _ = self.unit_command_stop(id);
                }
                HostScriptIdleRequest::TeamStop { team, disband } => {
                    let members = self.host_script_team_member_ids(&team);
                    for id in members {
                        let _ = self.unit_command_stop(id);
                    }
                    if !disband {
                        continue;
                    }
                    let members = self.host_script_team_member_ids(&team);
                    for id in members {
                        let Some((owner, faction)) = self
                            .host_object(id)
                            .map(|obj| (obj.owner_player_id, obj.team))
                        else {
                            continue;
                        };
                        let default = self.default_host_team_instance_name(owner, faction);
                        if let Some(obj) = self.host_object_mut(id) {
                            obj.team_instance_name = default;
                        }
                    }
                }
                HostScriptIdleRequest::IdleAll { player } => {
                    let pids = self.host_script_idle_or_resume_player_ids(&player);
                    let ids: Vec<ObjectId> = self
                        .objects
                        .values()
                        .filter(|obj| {
                            obj.is_alive()
                                && !obj.status.destroyed
                                && !obj.is_kind_of(crate::game_logic::KindOf::Structure)
                                && obj
                                    .owner_player_id
                                    .map(|pid| pids.contains(&pid))
                                    .unwrap_or(false)
                        })
                        .map(|obj| obj.id)
                        .collect();
                    for id in ids {
                        let pos = self
                            .host_object(id)
                            .map(|o| o.get_position())
                            .unwrap_or(glam::Vec3::ZERO);
                        // C++ aiMoveToPosition(self) — stop in place.
                        let _ = self.unit_command_move_to(id, pos);
                    }
                }
                HostScriptIdleRequest::ResumeSupply { player } => {
                    let pids = self.host_script_idle_or_resume_player_ids(&player);
                    let ids: Vec<ObjectId> = self
                        .objects
                        .values()
                        .filter(|obj| {
                            obj.is_alive()
                                && !obj.status.destroyed
                                && !obj.is_kind_of(crate::game_logic::KindOf::Structure)
                                && obj.ai_state == crate::game_logic::AIState::Idle
                                && (obj.is_kind_of(crate::game_logic::KindOf::Harvester)
                                    || obj.is_kind_of(crate::game_logic::KindOf::Dozer))
                                && obj
                                    .owner_player_id
                                    .map(|pid| pids.contains(&pid))
                                    .unwrap_or(false)
                        })
                        .map(|obj| obj.id)
                        .collect();
                    for id in ids {
                        if let Some(obj) = self.host_object_mut(id) {
                            obj.supply_truck_force_pending = true;
                        }
                    }
                }
            }
        }
    }

    /// C++ `doIdleAllPlayerUnits` / `doResumeSupplyTruckingForIdleUnits`.
    /// Empty name walks every local/human player (dispatch always passes empty).
    fn host_script_idle_or_resume_player_ids(&self, player: &str) -> Vec<u32> {
        if let Some(pid) = self.host_player_id_for_script_token(player) {
            return vec![pid];
        }
        let locals: Vec<u32> = self
            .players
            .values()
            .filter(|p| p.is_local && !p.is_observer)
            .map(|p| p.id)
            .collect();
        if !locals.is_empty() {
            return locals;
        }
        self.players
            .values()
            .filter(|p| !p.is_observer && p.is_alive)
            .map(|p| p.id)
            .collect()
    }

    /// C++ `doNamedUseCommandButtonAbility*` / `doTeamUseCommandButtonAbility*`.
    fn apply_host_use_command_button_script_requests(&mut self) {
        use crate::command_executor::CommandExecutor;
        use gamelogic::scripting::HostScriptUseCommandButtonRequest;
        for req in gamelogic::scripting::take_host_script_use_command_button_requests() {
            match req {
                HostScriptUseCommandButtonRequest::Named { unit, button } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let pid = self
                        .host_object(id)
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(0);
                    let _ = CommandExecutor::new(self, pid).execute_do_command_button(
                        &[id],
                        &button,
                        None,
                        None,
                    );
                }
                HostScriptUseCommandButtonRequest::NamedOnNamed {
                    unit,
                    button,
                    target,
                } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let Some(tid) = self.host_object_id_by_script_name(&target) else {
                        continue;
                    };
                    let pid = self
                        .host_object(id)
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(0);
                    let _ = CommandExecutor::new(self, pid).execute_do_command_button(
                        &[id],
                        &button,
                        None,
                        Some(tid),
                    );
                }
                HostScriptUseCommandButtonRequest::NamedAtWaypoint {
                    unit,
                    button,
                    waypoint,
                } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let Some(pos) = self.host_script_waypoint_position(&waypoint) else {
                        continue;
                    };
                    let pid = self
                        .host_object(id)
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(0);
                    let _ = CommandExecutor::new(self, pid).execute_do_command_button(
                        &[id],
                        &button,
                        Some(pos),
                        None,
                    );
                }
                HostScriptUseCommandButtonRequest::NamedUsingWaypointPath {
                    unit,
                    button,
                    path,
                } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let from = self
                        .host_object(id)
                        .map(|o| o.get_position())
                        .unwrap_or(glam::Vec3::ZERO);
                    let Some(wps) = self.host_script_waypoint_path_from(&path, from) else {
                        continue;
                    };
                    let pid = self
                        .host_object(id)
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(0);
                    let _ = CommandExecutor::new(self, pid)
                        .execute_do_command_button_using_waypoints(&[id], &button, &wps);
                }
                HostScriptUseCommandButtonRequest::Team { team, button } => {
                    let ids = self.host_script_team_member_ids(&team);
                    if ids.is_empty() {
                        continue;
                    }
                    let pid = self
                        .host_object(ids[0])
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(0);
                    let _ = CommandExecutor::new(self, pid).execute_do_command_button(
                        &ids,
                        &button,
                        None,
                        None,
                    );
                }
                HostScriptUseCommandButtonRequest::TeamOnNamed {
                    team,
                    button,
                    target,
                } => {
                    let ids = self.host_script_team_member_ids(&team);
                    let Some(tid) = self.host_object_id_by_script_name(&target) else {
                        continue;
                    };
                    if ids.is_empty() {
                        continue;
                    }
                    let pid = self
                        .host_object(ids[0])
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(0);
                    let _ = CommandExecutor::new(self, pid).execute_do_command_button(
                        &ids,
                        &button,
                        None,
                        Some(tid),
                    );
                }
                HostScriptUseCommandButtonRequest::TeamAtWaypoint {
                    team,
                    button,
                    waypoint,
                } => {
                    let ids = self.host_script_team_member_ids(&team);
                    let Some(pos) = self.host_script_waypoint_position(&waypoint) else {
                        continue;
                    };
                    if ids.is_empty() {
                        continue;
                    }
                    let pid = self
                        .host_object(ids[0])
                        .and_then(|o| o.owner_player_id)
                        .unwrap_or(0);
                    let _ = CommandExecutor::new(self, pid).execute_do_command_button(
                        &ids,
                        &button,
                        Some(pos),
                        None,
                    );
                }
                HostScriptUseCommandButtonRequest::TeamOnNearestEnemy { team, button } => {
                    self.host_script_team_use_command_on_nearest(&team, &button, |s, viewer, obj| {
                        s.host_script_affiliation_allows(viewer, obj, true, false)
                    });
                }
                HostScriptUseCommandButtonRequest::TeamOnNearestGarrisonedBuilding {
                    team,
                    button,
                } => {
                    self.host_script_team_use_command_on_nearest(&team, &button, |s, viewer, obj| {
                        s.host_script_affiliation_allows(viewer, obj, true, false)
                            && obj.is_kind_of(crate::game_logic::KindOf::Structure)
                            && obj.is_garrison_contain()
                    });
                }
                HostScriptUseCommandButtonRequest::TeamOnNearestKindof {
                    team,
                    button,
                    kindof,
                } => {
                    let Some(kind) = Self::host_script_kind_from_token(&kindof) else {
                        continue;
                    };
                    self.host_script_team_use_command_on_nearest(&team, &button, |s, viewer, obj| {
                        s.host_script_affiliation_allows(viewer, obj, true, false)
                            && obj.is_kind_of(kind)
                    });
                }
                HostScriptUseCommandButtonRequest::TeamOnNearestEnemyBuilding { team, button } => {
                    self.host_script_team_use_command_on_nearest(&team, &button, |s, viewer, obj| {
                        s.host_script_affiliation_allows(viewer, obj, true, false)
                            && obj.is_kind_of(crate::game_logic::KindOf::Structure)
                    });
                }
                HostScriptUseCommandButtonRequest::TeamOnNearestEnemyBuildingClass {
                    team,
                    button,
                    kindof,
                } => {
                    let Some(kind) = Self::host_script_kind_from_token(&kindof) else {
                        continue;
                    };
                    self.host_script_team_use_command_on_nearest(&team, &button, |s, viewer, obj| {
                        s.host_script_affiliation_allows(viewer, obj, true, false)
                            && obj.is_kind_of(crate::game_logic::KindOf::Structure)
                            && obj.is_kind_of(kind)
                    });
                }
                HostScriptUseCommandButtonRequest::TeamOnNearestObjectType {
                    team,
                    button,
                    object_type,
                } => {
                    self.host_script_team_use_command_on_nearest(&team, &button, |s, viewer, obj| {
                        s.host_script_affiliation_allows(viewer, obj, true, true)
                            && obj.template_name.eq_ignore_ascii_case(&object_type)
                    });
                }
            }
        }
        self.apply_host_team_partial_command_button_requests();
    }

    fn host_script_kind_from_token(token: &str) -> Option<crate::game_logic::KindOf> {
        use crate::game_logic::KindOf;
        let t = token.trim();
        let t = t.strip_prefix("KINDOF_").or_else(|| t.strip_prefix("KINDOF")).unwrap_or(t);
        let u = t.to_ascii_uppercase();
        match u.as_str() {
            "INFANTRY" => Some(KindOf::Infantry),
            "VEHICLE" => Some(KindOf::Vehicle),
            "STRUCTURE" | "BUILDING" => Some(KindOf::Structure),
            "AIRCRAFT" => Some(KindOf::Aircraft),
            "HERO" => Some(KindOf::Hero),
            "DOZER" => Some(KindOf::Dozer),
            "HARVESTER" => Some(KindOf::Harvester),
            "MINE" => Some(KindOf::Mine),
            "PROJECTILE" => Some(KindOf::Projectile),
            "COMMANDCENTER" | "COMMAND_CENTER" => Some(KindOf::CommandCenter),
            "FSBARRACKS" | "FS_BARRACKS" => Some(KindOf::FSBarracks),
            "FSWARFACTORY" | "FS_WARFACTORY" => Some(KindOf::FSWarFactory),
            "FSAIRFIELD" | "FS_AIRFIELD" => Some(KindOf::FSAirfield),
            "FSBASEDEFENSE" | "FS_BASE_DEFENSE" | "BASEDEFENSE" => Some(KindOf::FSBaseDefense),
            "TECHBUILDING" | "TECH_BUILDING" => Some(KindOf::TechBuilding),
            other => KindOf::from_ini_token(other),
        }
    }

    fn host_script_affiliation_allows(
        &self,
        viewer: u32,
        candidate: &crate::game_logic::Object,
        allow_enemies: bool,
        allow_neutral: bool,
    ) -> bool {
        use crate::game_logic::Team;
        use gamelogic::common::Relationship;
        let Some(oid) = candidate.owner_player_id else {
            let vt = self
                .players
                .get(&viewer)
                .map(|p| p.team)
                .unwrap_or(Team::Neutral);
            if candidate.team == Team::Neutral || vt == Team::Neutral {
                return allow_neutral;
            }
            if candidate.team == vt {
                return false;
            }
            return allow_enemies;
        };
        let rel = self
            .players
            .get(&viewer)
            .and_then(|p| p.map_relationship(oid))
            .unwrap_or_else(|| {
                let vt = self
                    .players
                    .get(&viewer)
                    .map(|p| p.team)
                    .unwrap_or(candidate.team);
                let ot = self
                    .players
                    .get(&oid)
                    .map(|p| p.team)
                    .unwrap_or(candidate.team);
                if vt == ot {
                    Relationship::Allies
                } else if vt == Team::Neutral || ot == Team::Neutral {
                    Relationship::Neutral
                } else {
                    Relationship::Enemies
                }
            });
        match rel {
            Relationship::Enemies => allow_enemies,
            Relationship::Neutral => allow_neutral,
            Relationship::Allies => false,
        }
    }

    fn host_script_team_center(&self, ids: &[crate::game_logic::ObjectId]) -> Option<glam::Vec3> {
        let mut acc = glam::Vec3::ZERO;
        let mut n = 0.0;
        for id in ids {
            if let Some(obj) = self.host_object(*id) {
                acc += obj.get_position();
                n += 1.0;
            }
        }
        if n <= 0.0 {
            None
        } else {
            Some(acc / n)
        }
    }

    fn host_script_team_use_command_on_nearest(
        &mut self,
        team: &str,
        button: &str,
        pred: impl Fn(&Self, u32, &crate::game_logic::Object) -> bool,
    ) {
        use crate::command_executor::CommandExecutor;
        let ids = self.host_script_team_member_ids(team);
        if ids.is_empty() {
            return;
        }
        let pid = self
            .host_object(ids[0])
            .and_then(|o| o.owner_player_id)
            .unwrap_or(0);
        let Some(center) = self.host_script_team_center(&ids) else {
            return;
        };
        let team_set: std::collections::HashSet<_> = ids.iter().copied().collect();
        let mut best = None;
        let mut best_d = f32::MAX;
        for obj in self.objects.values() {
            if !obj.is_alive() || obj.status.destroyed || obj.status.effectively_dead {
                continue;
            }
            if team_set.contains(&obj.id) {
                continue;
            }
            if !pred(self, pid, obj) {
                continue;
            }
            let p = obj.get_position();
            let dx = p.x - center.x;
            let dz = p.z - center.z;
            let d = dx * dx + dz * dz;
            if d < best_d {
                best_d = d;
                best = Some(obj.id);
            }
        }
        let Some(tid) = best else {
            return;
        };
        let _ = CommandExecutor::new(self, pid).execute_do_command_button(
            &ids,
            button,
            None,
            Some(tid),
        );
    }

    fn apply_host_team_partial_command_button_requests(&mut self) {
        use crate::command_executor::CommandExecutor;
        use crate::command_system::command_type_from_button_name;
        for req in gamelogic::scripting::take_host_team_partial_command_button_requests() {
            let mut ids = self.host_script_team_member_ids(&req.team);
            if ids.is_empty() || command_type_from_button_name(&req.button).is_none() {
                continue;
            }
            let mut num_to_use = ((req.percentage / 100.0) * ids.len() as f32) as i32;
            if num_to_use <= 0 {
                continue;
            }
            if num_to_use > ids.len() as i32 {
                num_to_use = ids.len() as i32;
            }
            ids.truncate(num_to_use as usize);
            let pid = self
                .host_object(ids[0])
                .and_then(|o| o.owner_player_id)
                .unwrap_or(0);
            for id in ids {
                let _ = CommandExecutor::new(self, pid).execute_do_command_button(
                    &[id],
                    &req.button,
                    None,
                    None,
                );
            }
        }
    }




    /// C++ ScriptActions NAMED/TEAM DELETE / KILL / DAMAGE.
    /// Leftover `OBJECT_REGISTRY` is empty on the host path; leftover actions
    /// queue [`gamelogic::scripting::HostScriptKillDeleteDamageRequest`].
    fn apply_host_kill_delete_damage_script_requests(&mut self) {
        use crate::game_logic::KindOf;
        use gamelogic::scripting::HostScriptKillDeleteDamageRequest;
        const HUGE_DAMAGE_AMOUNT: f32 = 999999.0;
        for req in gamelogic::scripting::take_host_script_kill_delete_damage_requests() {
            match req {
                HostScriptKillDeleteDamageRequest::NamedDelete { unit } => {
                    if let Some(id) = self.host_object_id_by_script_name(&unit) {
                        self.destroy_object(id);
                    }
                }
                HostScriptKillDeleteDamageRequest::NamedKill { unit } => {
                    if let Some(id) = self.host_object_id_by_script_name(&unit) {
                        self.host_script_kill_object(id, HUGE_DAMAGE_AMOUNT);
                    }
                }
                HostScriptKillDeleteDamageRequest::NamedDamage { unit, amount } => {
                    if let Some(id) = self.host_object_id_by_script_name(&unit) {
                        self.host_script_apply_unresistable(id, amount as f32, HUGE_DAMAGE_AMOUNT);
                    }
                }
                HostScriptKillDeleteDamageRequest::TeamDelete { team, ignore_dead } => {
                    let members = self.host_script_team_member_ids(&team);
                    for id in members {
                        if ignore_dead {
                            let skip = self
                                .host_object(id)
                                .map(|o| !o.is_alive() || o.status.destroyed)
                                .unwrap_or(true);
                            if skip {
                                continue;
                            }
                        }
                        self.destroy_object(id);
                    }
                }
                HostScriptKillDeleteDamageRequest::TeamKill { team } => {
                    let members = self.host_script_team_member_ids(&team);
                    for id in members {
                        let is_tech = self
                            .host_object(id)
                            .map(|o| o.is_kind_of(KindOf::TechBuilding))
                            .unwrap_or(false);
                        if is_tech {
                            if let Some(obj) = self.host_object_mut(id) {
                                obj.team = Team::Neutral;
                            }
                            continue;
                        }
                        self.host_script_kill_object(id, HUGE_DAMAGE_AMOUNT);
                    }
                }
                HostScriptKillDeleteDamageRequest::TeamDamage { team, amount } => {
                    let members = self.host_script_team_member_ids(&team);
                    for id in members {
                        let skip = self
                            .host_object(id)
                            .map(|o| !o.is_alive() || o.status.destroyed)
                            .unwrap_or(true);
                        if skip {
                            continue;
                        }
                        self.host_script_apply_unresistable(id, amount, HUGE_DAMAGE_AMOUNT);
                    }
                }
                HostScriptKillDeleteDamageRequest::DestroyAllContained { unit } => {
                    let Some(container) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let mut occupants = self
                        .host_object(container)
                        .map(|o| o.contained_units())
                        .unwrap_or_default();
                    occupants.extend(
                        self.objects
                            .values()
                            .filter(|o| o.contained_by == Some(container) && o.is_alive())
                            .map(|o| o.id),
                    );
                    occupants.sort_by_key(|id| id.0);
                    occupants.dedup();
                    for occ in occupants {
                        self.host_script_kill_object(occ, HUGE_DAMAGE_AMOUNT);
                    }
                    if let Some(obj) = self.host_object_mut(container) {
                        if let Some(building) = obj.building_data.as_mut() {
                            building.garrisoned_units.clear();
                        }
                        obj.occupants.clear();
                    }
                }
            }
        }
    }

    /// C++ `Object::kill()` — HUGE unresistable damage with death effects.
    fn host_script_kill_object(&mut self, id: ObjectId, huge: f32) {
        let dead = self
            .host_object_mut(id)
            .map(|obj| obj.take_damage_from(huge, None))
            .unwrap_or(false);
        if dead {
            self.destroy_object(id);
        }
    }

    /// C++ `attemptDamage` UNRESISTABLE; amount < 0 is `Object::kill()`.
    fn host_script_apply_unresistable(&mut self, id: ObjectId, amount: f32, huge: f32) {
        if amount < 0.0 {
            self.host_script_kill_object(id, huge);
            return;
        }
        let dead = self
            .host_object_mut(id)
            .map(|obj| obj.take_damage_from(amount, None))
            .unwrap_or(false);
        if dead {
            self.destroy_object(id);
        }
    }



    /// C++ ScriptActions TEAM/NAMED FOLLOW_WAYPOINTS and EXACT.
    /// Leftover `OBJECT_REGISTRY` is empty on the host path; leftover actions
    /// queue [`gamelogic::scripting::HostScriptFollowWaypointsRequest`].
    fn apply_host_follow_waypoints_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptFollowWaypointsRequest;
        for req in gamelogic::scripting::take_host_script_follow_waypoints_requests() {
            match req {
                HostScriptFollowWaypointsRequest::NamedFollow {
                    unit,
                    waypoint,
                    exact,
                } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let Some(pos) = self
                        .host_object(id)
                        .filter(|u| u.is_alive())
                        .map(|u| u.get_position())
                    else {
                        continue;
                    };
                    let Some(path) = self.host_script_waypoint_path_from(&waypoint, pos) else {
                        continue;
                    };
                    let _ = self.apply_unit_locomotor_set(id, "normal");
                    self.host_script_issue_follow_waypoint_path(
                        &[id],
                        &path,
                        exact,
                        false,
                        &waypoint,
                    );
                }
                HostScriptFollowWaypointsRequest::TeamFollow {
                    team,
                    waypoint,
                    as_team,
                    exact,
                } => {
                    let members = self.host_script_team_member_ids(&team);
                    if members.is_empty() {
                        continue;
                    }
                    let mut sx = 0.0f32;
                    let mut sy = 0.0f32;
                    let mut sz = 0.0f32;
                    let mut n = 0u32;
                    for id in &members {
                        if let Some(pos) = self
                            .host_object(*id)
                            .filter(|u| u.is_alive())
                            .map(|u| u.get_position())
                        {
                            sx += pos.x;
                            sy += pos.y;
                            sz += pos.z;
                            n += 1;
                        }
                    }
                    if n == 0 {
                        continue;
                    }
                    let inv = 1.0 / n as f32;
                    let center = glam::Vec3::new(sx * inv, sy * inv, sz * inv);
                    let Some(path) = self.host_script_waypoint_path_from(&waypoint, center) else {
                        continue;
                    };
                    self.host_script_issue_follow_waypoint_path(
                        &members,
                        &path,
                        exact,
                        as_team,
                        &waypoint,
                    );
                }
            }
        }
    }

    /// C++ `doTeamFollowSkirmishApproachPath` / `doTeamMoveToSkirmishApproachPath`.
    /// Path label is `label + (enemy mpStartIndex + 1)`.
    fn apply_host_skirmish_approach_path_script_requests(&mut self) {
        for req in gamelogic::scripting::take_host_skirmish_approach_path_requests() {
            let members = self.host_script_team_member_ids(&req.team);
            if members.is_empty() {
                continue;
            }
            let mut sx = 0.0f32;
            let mut sy = 0.0f32;
            let mut sz = 0.0f32;
            let mut n = 0u32;
            for id in &members {
                if let Some(pos) = self
                    .host_object(*id)
                    .filter(|u| u.is_alive())
                    .map(|u| u.get_position())
                {
                    sx += pos.x;
                    sy += pos.y;
                    sz += pos.z;
                    n += 1;
                }
            }
            if n == 0 {
                continue;
            }
            let inv = 1.0 / n as f32;
            let center = glam::Vec3::new(sx * inv, sy * inv, sz * inv);
            let mp_index = self.host_skirmish_enemy_mp_index(&members) + 1;
            let path_label = format!("{}{}", req.path_label, mp_index);
            let Some(path) = self.host_script_waypoint_path_from(&path_label, center) else {
                continue;
            };
            if req.follow {
                self.host_script_issue_follow_waypoint_path(
                    &members,
                    &path,
                    false,
                    req.as_team,
                    &path_label,
                );
            } else if let Some(&dest) = path.first() {
                for id in members {
                    let _ = self.unit_command_move_to(id, dest);
                }
            }
        }
    }

    /// C++ `TheScriptEngine->getSkirmishEnemyPlayer()->getMpStartIndex()`.
    fn host_skirmish_enemy_mp_index(&self, members: &[ObjectId]) -> i32 {
        let owner = members
            .first()
            .and_then(|id| self.host_object(*id))
            .and_then(|obj| obj.owner_player_id);
        for player in self.players.values() {
            if player.is_local && Some(player.id) != owner {
                return player.start_position.max(0);
            }
        }
        for player in self.players.values() {
            if Some(player.id) != owner && player.is_alive && !player.is_observer {
                return player.start_position.max(0);
            }
        }
        0
    }


    /// C++ ScriptActions NAMED/TEAM FACE_NAMED / FACE_WAYPOINT live drain.
    /// Leftover queues [`gamelogic::scripting::HostScriptFaceRequest`].
    fn apply_host_face_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptFaceRequest;
        for req in gamelogic::scripting::take_host_script_face_requests() {
            match req {
                HostScriptFaceRequest::NamedFaceNamed { unit, target } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let Some(tid) = self.host_object_id_by_script_name(&target) else {
                        continue;
                    };
                    let Some(pos) = self
                        .host_object(tid)
                        .filter(|o| o.is_alive())
                        .map(|o| o.get_position())
                    else {
                        continue;
                    };
                    self.host_script_face_unit(id, pos);
                }
                HostScriptFaceRequest::NamedFaceWaypoint { unit, waypoint } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let Some(pos) = self.host_script_waypoint_position(&waypoint) else {
                        continue;
                    };
                    self.host_script_face_unit(id, pos);
                }
                HostScriptFaceRequest::TeamFaceNamed { team, target } => {
                    let Some(tid) = self.host_object_id_by_script_name(&target) else {
                        continue;
                    };
                    let Some(pos) = self
                        .host_object(tid)
                        .filter(|o| o.is_alive())
                        .map(|o| o.get_position())
                    else {
                        continue;
                    };
                    for id in self.host_script_team_member_ids(&team) {
                        self.host_script_face_unit(id, pos);
                    }
                }
                HostScriptFaceRequest::TeamFaceWaypoint { team, waypoint } => {
                    let Some(pos) = self.host_script_waypoint_position(&waypoint) else {
                        continue;
                    };
                    for id in self.host_script_team_member_ids(&team) {
                        self.host_script_face_unit(id, pos);
                    }
                }
            }
        }
    }

    /// C++ `clearWaypointQueue` + `leaveGroup` + `chooseLocomotorSet(NORMAL)` +
    /// `aiFacePosition` (`CMD_FROM_SCRIPT`).
    fn host_script_face_unit(&mut self, id: ObjectId, pos: glam::Vec3) {
        let _ = self.unit_command_stop(id);
        let _ = self.apply_unit_locomotor_set(id, "normal");
        let _ = self.private_face_position(id, pos);
    }


    /// C++ `doTeamGuardPosition` / `Object` / `Area` / `InTunnelNetwork`.
    /// Leftover queues [`gamelogic::scripting::HostScriptGuardVariantRequest`].
    fn apply_host_guard_variant_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptGuardVariantRequest;
        for req in gamelogic::scripting::take_host_script_guard_variant_requests() {
            match req {
                HostScriptGuardVariantRequest::TeamGuardPosition { team, waypoint } => {
                    let Some(dest) = self.host_script_waypoint_position(&waypoint) else {
                        continue;
                    };
                    for id in self.host_script_team_member_ids(&team) {
                        if self.host_script_unit_can_guard(id) {
                            let _ = self.unit_command_guard_position(id, dest);
                        }
                    }
                }
                HostScriptGuardVariantRequest::TeamGuardObject { team, unit } => {
                    let Some(tid) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    for id in self.host_script_team_member_ids(&team) {
                        if self.host_script_unit_can_guard(id) {
                            let _ = self.unit_command_guard_object(id, tid);
                        }
                    }
                }
                HostScriptGuardVariantRequest::TeamGuardArea { team, area } => {
                    let Some(dest) = self.host_script_area_center(&area) else {
                        continue;
                    };
                    let poly_radius = crate::game_logic::GameLogic::host_named_guard_area_polygon(&area)
                        .map(|(_, r, _)| r);
                    for id in self.host_script_team_member_ids(&team) {
                        if self.host_script_unit_can_guard(id) {
                            let _ = self.unit_command_guard_position(id, dest);
                            if let Some(u) = self.objects.get_mut(&id) {
                                u.guard_area_trigger = Some(area.clone());
                                if let Some(r) = poly_radius {
                                    if r > 0.0 {
                                        u.guard_radius = r;
                                    }
                                }
                            }
                        }
                    }
                }
                HostScriptGuardVariantRequest::TeamGuardTunnel { team } => {
                    for id in self.host_script_team_member_ids(&team) {
                        if !self.host_script_unit_can_guard(id) {
                            continue;
                        }
                        if let Some(tid) = self.host_script_nearest_tunnel(id) {
                            let _ = self.unit_command_guard_object(id, tid);
                        }
                    }
                }
            }
        }
    }

    /// C++ `doNamedFireSpecialPowerAtWaypoint` / `AtNamed`.
    /// Leftover queues [`gamelogic::scripting::HostScriptNamedFireSpecialPowerRequest`].
    fn apply_host_named_fire_special_script_requests(&mut self) {
        use crate::command_system::PowerTarget;
        use gamelogic::scripting::HostScriptNamedFireSpecialPowerRequest;
        for req in gamelogic::scripting::take_host_script_named_fire_special_requests() {
            match req {
                HostScriptNamedFireSpecialPowerRequest::AtWaypoint {
                    unit,
                    power,
                    waypoint,
                } => {
                    let Some((wid, dest)) = self.host_script_leftover_waypoint(&waypoint) else {
                        continue;
                    };
                    self.host_script_fire_named_special_power(
                        &unit,
                        &power,
                        PowerTarget::Location(dest),
                        Some(wid),
                    );
                }
                HostScriptNamedFireSpecialPowerRequest::AtNamed {
                    unit,
                    power,
                    target,
                } => {
                    let Some(tid) = self.host_object_id_by_script_name(&target) else {
                        continue;
                    };
                    self.host_script_fire_named_special_power(
                        &unit,
                        &power,
                        PowerTarget::Object(tid),
                        None,
                    );
                }
            }
        }
    }

    /// C++ `Object::getSpecialPowerModule(TheSpecialPowerStore->find…)`.
    fn host_script_special_power_type_for(
        &self,
        id: ObjectId,
        power_name: &str,
    ) -> Option<crate::command_system::SpecialPowerType> {
        let obj = self.host_object(id)?;
        for module in &obj.thing.template.special_power_modules {
            if module
                .special_power_template
                .eq_ignore_ascii_case(power_name)
            {
                if let Some(power) = module.command_power.clone() {
                    return Some(power);
                }
                return crate::command_system::special_power_type_from_template_name(power_name);
            }
        }
        let power = crate::command_system::special_power_type_from_template_name(power_name)?;
        if obj
            .thing
            .template
            .special_power_module_for_command(&power)
            .is_some()
            || obj.special_power_cooldowns.contains_key(&power)
        {
            Some(power)
        } else {
            None
        }
    }

    /// C++ `mod->doSpecialPowerAtLocation/Object(..., COMMAND_FIRED_BY_SCRIPT)`.
    /// When `waypoint_id` is set, PUC leftover `scriptedWaypointMode` drives
    /// the outgoing chain instead of SwathOfDeath on a static point.
    fn host_script_fire_named_special_power(
        &mut self,
        unit: &str,
        power_name: &str,
        target: crate::command_system::PowerTarget,
        waypoint_id: Option<u32>,
    ) {
        let Some(id) = self.host_object_id_by_script_name(unit) else {
            return;
        };
        let Some(power) = self.host_script_special_power_type_for(id, power_name) else {
            return;
        };
        let player_id = {
            let Some(obj) = self.host_object(id) else {
                return;
            };
            if !obj.is_alive() || obj.status.destroyed || obj.status.sold || obj.is_disabled() {
                return;
            }
            if obj.is_special_power_countdown_paused(&power) {
                return;
            }
            obj.owner_player_id.unwrap_or(0)
        };
        // C++ script fire does not consult isReady; only paused/disabled.
        if !self.is_special_power_ready_for(id, &power) {
            let _ = self.script_set_special_power_countdown(id, &power, 0);
            if let Some(obj) = self.host_object(id) {
                if let Some(pid) = self.player_owner_for_host_object(obj) {
                    if let Some(player) = self.get_player_mut(pid) {
                        player.express_shared_special_power_ready_now(&power);
                    }
                }
            }
        }
        // C++ doSpecialPower* COMMAND_FIRED_BY_SCRIPT — location-only stays swath.
        self.special_power_strikes
            .note_script_fired_special_power(id);
        if let Some(wid) = waypoint_id {
            if crate::game_logic::special_power_strikes::HostSuperweaponKind::from_command_power(
                &power,
            ) == Some(
                crate::game_logic::special_power_strikes::HostSuperweaponKind::ParticleCannon,
            ) {
                self.special_power_strikes
                    .note_scripted_waypoint_special_power(id, wid);
            }
        }
        self.queue_command(crate::command_system::GameCommand {
            command_type: crate::command_system::CommandType::DoSpecialPower {
                power_type: power,
                target,
            },
            player_id,
            command_id: 0,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
    }

    fn host_script_unit_can_guard(&self, id: ObjectId) -> bool {
        // C++ doTeamGuard / leftover group_guard: AIUpdateInterface only.
        self.host_unit_can_guard(id)
    }

    /// C++ AITNGuardMachine nearest entrance for `aiGuardTunnelNetwork`.
    fn host_script_nearest_tunnel(&self, from: ObjectId) -> Option<ObjectId> {
        let obj = self.objects.get(&from)?;
        let origin = obj.get_position();
        let key = obj.tunnel_system_key();
        self.objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive()
                    && !o.status.sold
                    && o.tunnel_system_key() == key
                    && (o.is_tunnel_network_style_container()
                        || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                            &o.template_name,
                        ))
            })
            .min_by(|a, b| {
                origin
                    .distance(a.1.get_position())
                    .partial_cmp(&origin.distance(b.1.get_position()))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(id, _)| *id)
    }



    /// C++ ScriptActions CREATE_OBJECT family live drain.
    pub(in super::super) fn apply_host_create_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptCreateRequest;
        for req in gamelogic::scripting::take_host_script_create_requests() {

            match req {
                HostScriptCreateRequest::Object {
                    name,
                    thing,
                    team,
                    x,
                    y,
                    z,
                    angle,
                } => {
                    self.host_script_create_object(name.as_deref(), &thing, &team, x, y, z, angle);
                }
                HostScriptCreateRequest::ReinforcementTeam { team, waypoint } => {
                    self.host_script_create_reinforcement_team(&team, &waypoint);
                }
            }
        }
    }

    /// C++ `ScriptActions::doNamedSetBoobytrapped` / `doTeamSetBoobytrapped`.
    /// Leftover queues [`gamelogic::scripting::HostScriptBoobytrapRequest`].
    fn apply_host_boobytrap_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptBoobytrapRequest;
        for req in gamelogic::scripting::take_host_script_boobytrap_requests() {
            match req {
                HostScriptBoobytrapRequest::Named { thing, unit } => {
                    if let Some(id) = self.host_object_id_by_script_name(&unit) {
                        self.host_script_plant_boobytrap(&thing, id);
                    }
                }
                HostScriptBoobytrapRequest::Team { thing, team } => {
                    for id in self.host_script_team_member_ids(&team) {
                        self.host_script_plant_boobytrap(&thing, id);
                    }
                }
            }
        }
    }

    /// C++ `ScriptActions::doNamedSetUnmanned` / `doTeamSetUnmanned` /
    /// `deleteAllUnmanned`. Leftover queues [`gamelogic::scripting::HostScriptUnmannedRequest`].
    pub(in super::super) fn apply_host_unmanned_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptUnmannedRequest;
        for req in gamelogic::scripting::take_host_script_unmanned_requests() {
            match req {
                HostScriptUnmannedRequest::Named { unit } => {
                    if let Some(id) = self.host_object_id_by_script_name(&unit) {
                        self.host_script_set_unmanned(id);
                    }
                }
                HostScriptUnmannedRequest::Team { team } => {
                    for id in self.host_script_team_member_ids(&team) {
                        self.host_script_set_unmanned(id);
                    }
                }
                HostScriptUnmannedRequest::DeleteAll => {
                    let ids: Vec<ObjectId> = self
                        .objects
                        .values()
                        .filter(|obj| obj.status.disabled_unmanned && !obj.status.destroyed)
                        .map(|obj| obj.id)
                        .collect();
                    for id in ids {
                        self.destroy_object(id);
                    }
                }
            }
        }
    }

    /// C++ `setDisabled(DISABLED_UNMANNED)` + `deselectObject(PLAYERMASK_ALL)` +
    /// `setTeam(Neutral default team)`.
    fn host_script_set_unmanned(&mut self, id: ObjectId) {
        {
            let Some(obj) = self.objects.get_mut(&id) else {
                return;
            };
            if !obj.is_alive() || obj.status.destroyed {
                return;
            }
            obj.apply_kill_pilot_unmanned();
            obj.deselect();
            obj.set_team(Team::Neutral);
        }
        self.selected_objects.retain(|sid| *sid != id);
        for player in self.players.values_mut() {
            player.selected_objects.retain(|sid| *sid != id);
        }
    }

    /// C++ `doObjectRadarCreateEvent` / `doTeamRadarCreateEvent`.
    pub(in super::super) fn apply_host_radar_event_script_requests(&mut self) {
        use crate::game_logic::host_radar::host_create_radar_event;
        use gamelogic::scripting::HostScriptRadarEventRequest;
        for req in gamelogic::scripting::take_host_script_radar_event_requests() {
            let (pos, event_type) = match req {
                HostScriptRadarEventRequest::Object { unit, event_type } => {
                    let Some(id) = self.host_object_id_by_script_name(&unit) else {
                        continue;
                    };
                    let Some(obj) = self.objects.get(&id) else {
                        continue;
                    };
                    (obj.get_position(), event_type)
                }
                HostScriptRadarEventRequest::Team { team, event_type } => {
                    let Some(pos) = self.host_script_estimate_team_position(&team) else {
                        continue;
                    };
                    (pos, event_type)
                }
            };
            host_create_radar_event(pos, Self::host_script_radar_event_type(event_type));
        }
    }

    pub(in super::super) fn host_script_radar_event_type(event_type: i32) -> game_engine::common::system::radar::RadarEventType {
        use game_engine::common::system::radar::RadarEventType;
        match event_type {
            1 => RadarEventType::Construction,
            2 => RadarEventType::Upgrade,
            3 => RadarEventType::UnderAttack,
            4 => RadarEventType::Information,
            5 => RadarEventType::BeaconPulse,
            6 => RadarEventType::Infiltration,
            7 => RadarEventType::BattlePlan,
            8 => RadarEventType::StealthDiscovered,
            9 => RadarEventType::StealthNeutralized,
            10 => RadarEventType::Fake,
            _ => RadarEventType::Invalid,
        }
    }

    /// C++ `Team::getEstimateTeamPosition` — average of live members.
    fn host_script_estimate_team_position(&self, team_name: &str) -> Option<glam::Vec3> {
        let ids = self.host_script_team_member_ids(team_name);
        let mut sum = glam::Vec3::ZERO;
        let mut n = 0u32;
        for id in ids {
            let Some(obj) = self.objects.get(&id) else {
                continue;
            };
            if !obj.is_alive() || obj.status.destroyed {
                continue;
            }
            sum += obj.get_position();
            n = n.saturating_add(1);
        }
        if n == 0 {
            None
        } else {
            Some(sum / n as f32)
        }
    }

    /// C++ `doNamedEnableStealth` / `doTeamEnableStealth`.
    pub(in super::super) fn apply_host_stealth_enabled_script_requests(&mut self) {
        use gamelogic::scripting::HostScriptStealthEnabledRequest;
        for req in gamelogic::scripting::take_host_script_stealth_enabled_requests() {
            match req {
                HostScriptStealthEnabledRequest::Named { unit, enabled } => {
                    if let Some(id) = self.host_object_id_by_script_name(&unit) {
                        self.host_script_set_stealth_enabled(id, enabled);
                    }
                }
                HostScriptStealthEnabledRequest::Team { team, enabled } => {
                    for id in self.host_script_team_member_ids(&team) {
                        self.host_script_set_stealth_enabled(id, enabled);
                    }
                }
            }
        }
    }

    /// C++ `setScriptStatus(OBJECT_STATUS_SCRIPT_UNSTEALTHED, !enabled)`.
    fn host_script_set_stealth_enabled(&mut self, id: ObjectId, enabled: bool) {
        let frame = self.frame;
        let Some(obj) = self.objects.get_mut(&id) else {
            return;
        };
        obj.set_script_unstealthed(!enabled);
        if !enabled {
            obj.apply_stealth_allowed_update(frame, false);
        }
    }


    /// C++ `TheThingFactory->newObject(thing, obj->getTeam())` then
    /// `StickyBombUpdate::initStickyBomb(obj, NULL, &perimeterPos)`.
    fn host_script_plant_boobytrap(&mut self, thing: &str, target_id: ObjectId) {
        use crate::game_logic::host_booby_trap::BOOBY_TRAP_OBJECT;

        let thing = thing.trim();
        if thing.is_empty() {
            return;
        }
        let lower = thing.to_ascii_lowercase();
        // C++ only inits when the new object has StickyBombUpdate.
        if !(lower.contains("boobytrap")
            || lower.contains("sticky")
            || lower.contains("democharge")
            || lower.contains("remotecharge"))
        {
            return;
        }
        let Some(obj) = self.objects.get(&target_id) else {
            return;
        };
        if !obj.is_alive() || obj.status.destroyed {
            return;
        }
        let team = obj.team;
        let owner = obj.owner_player_id;
        let geom = obj.selection_radius.max(8.0);
        let p = obj.get_position();
        let pos = glam::Vec3::new(p.x, p.y + 8.0, p.z);
        let frame = self.frame;

        let charge = if self.templates.contains_key(thing) {
            self.create_object_for_owner_or_team(thing, team, owner, pos)
        } else if thing.eq_ignore_ascii_case(BOOBY_TRAP_OBJECT) {
            self.spawn_booby_trap_special_object(target_id, team, target_id)
        } else {
            None
        };
        let Some(cid) = charge else {
            return;
        };

        let is_booby_kind = thing.to_ascii_lowercase().contains("boobytrap");
        if let Some(o) = self.objects.get_mut(&cid) {
            o.booby_trap_special = true;
            o.booby_trap_attached_to = Some(target_id);
            o.producer_id = Some(target_id);
        }
        if is_booby_kind {
            let _ = self.booby_trap.install(
                target_id,
                target_id,
                team,
                frame,
                geom,
                Some(cid),
            );
            if let Some(target) = self.objects.get_mut(&target_id) {
                target.set_status_booby_trapped(true);
            }
        }
    }

    /// C++ `ScriptActions::doGuardSupplyCenter` live drain.
    fn apply_host_guard_supply_center_script_requests(&mut self) {
        let requests = gamelogic::scripting::take_host_guard_supply_center_requests();
        if requests.is_empty() {
            return;
        }
        let mut ai_mgr = std::mem::take(&mut self.ai_manager);
        for (team_name, min_supplies) in requests {
            let _ = ai_mgr.guard_supply_center_for_team(self, &team_name, min_supplies);
        }
        self.ai_manager = ai_mgr;
    }

    /// C++ SKIRMISH_ATTACK_NEAREST_GROUP_WITH_VALUE /
    /// SKIRMISH_PERFORM_COMMANDBUTTON_ON_MOST_VALUABLE_OBJECT.
    fn apply_host_skirmish_fight_script_requests(&mut self) {
        let attacks = gamelogic::scripting::take_host_skirmish_attack_nearest_group_requests();
        for (team, comparison, value) in attacks {
            let members = self.host_script_team_member_ids(&team);
            if members.is_empty() {
                continue;
            }
            let mut cx = 0.0;
            let mut cz = 0.0;
            let mut n = 0.0;
            for id in &members {
                if let Some(obj) = self.objects.get(id) {
                    let p = obj.get_position();
                    cx += p.x;
                    cz += p.z;
                    n += 1.0;
                }
            }
            if n <= 0.0 {
                continue;
            }
            let origin = glam::Vec3::new(cx / n, 0.0, cz / n);
            let attacker_team = self
                .objects
                .get(&members[0])
                .map(|o| o.team)
                .unwrap_or(crate::game_logic::Team::Neutral);
            let mut dest = origin;
            if matches!(comparison, 3 | 4) {
                let mut best: Option<(f32, glam::Vec3)> = None;
                for obj in self.objects.values() {
                    if !obj.is_alive() || obj.team == attacker_team || obj.team == crate::game_logic::Team::Neutral {
                        continue;
                    }
                    let cost = obj.thing.template.build_cost.supplies as i32;
                    if cost < value {
                        continue;
                    }
                    let pos = obj.get_position();
                    let d = (pos - origin).length_squared();
                    if best.map(|(bd, _)| d < bd).unwrap_or(true) {
                        best = Some((d, pos));
                    }
                }
                if let Some((_, pos)) = best {
                    dest = pos;
                }
            }
            for id in members {
                let _ = self.unit_command_attack_move_to(id, dest);
            }
        }

        let buttons = gamelogic::scripting::take_host_skirmish_command_button_most_valuable_requests();
        for (team, ability, range) in buttons {
            self.host_script_skirmish_command_button_on_most_valuable(&team, &ability, range);
        }
    }

    /// C++ `ScriptActions::doSkirmishCommandButtonOnMostValuable` /
    /// leftover `do_skirmish_perform_command_button_on_most_valuable_object`.
    /// Find the scripted button, pick the most valuable valid target in range
    /// of the group center, then `groupDoCommandButtonAtObject`.
    fn host_script_skirmish_command_button_on_most_valuable(
        &mut self,
        team: &str,
        ability: &str,
        range: f32,
    ) {
        use crate::command_executor::CommandExecutor;
        use gamelogic::object::update::special_power_update::SpecialPowerCommandOption;

        if !Self::leftover_skirmish_command_button_exists(ability) {
            return;
        }
        let members = self.host_script_team_member_ids(team);
        if members.is_empty() {
            return;
        }
        let Some(center) = self.host_script_team_center(&members) else {
            return;
        };
        let pid = self
            .host_object(members[0])
            .and_then(|o| o.owner_player_id)
            .unwrap_or(0);
        let range2 = range.max(0.0) * range.max(0.0);
        let team_set: std::collections::HashSet<_> = members.iter().copied().collect();
        let options = Self::leftover_skirmish_command_button_options(ability);
        let requires_object_target = options.intersects(
            SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_PRISONER,
        );

        let mut best: Option<(i32, crate::game_logic::ObjectId)> = None;
        for obj in self.objects.values() {
            if !obj.is_alive() || obj.status.destroyed || obj.status.effectively_dead {
                continue;
            }
            if obj.status.under_construction {
                continue;
            }
            if team_set.contains(&obj.id) {
                continue;
            }
            let p = obj.get_position();
            let dx = p.x - center.x;
            let dz = p.z - center.z;
            if dx * dx + dz * dz > range2 {
                continue;
            }
            let rel = self.host_script_relationship(pid, obj);
            let relationship_ok = if requires_object_target {
                (options.contains(SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT)
                    && rel == gamelogic::common::Relationship::Enemies)
                    || (options.contains(SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT)
                        && rel == gamelogic::common::Relationship::Neutral)
                    || (options.contains(SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT)
                        && rel == gamelogic::common::Relationship::Allies)
                    || (!options.intersects(
                        SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                            | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                            | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT,
                    ) && rel == gamelogic::common::Relationship::Enemies)
            } else {
                rel == gamelogic::common::Relationship::Enemies
            };
            if !relationship_ok {
                continue;
            }
            let cost = obj.thing.template.build_cost.supplies as i32;
            if best.map(|(c, _)| cost > c).unwrap_or(true) {
                best = Some((cost, obj.id));
            }
        }
        let Some((_, tid)) = best else {
            return;
        };
        let _ = CommandExecutor::new(self, pid).execute_do_command_button(
            &members,
            ability,
            None,
            Some(tid),
        );
    }

    /// C++ `TheControlBar->findCommandButton` / leftover host-queue payload.
    /// Empty INI catalog falls back to the live button mapper.
    fn leftover_skirmish_command_button_exists(ability: &str) -> bool {
        use crate::command_system::command_type_from_button_name;
        if ability.trim().is_empty() {
            return false;
        }
        if let Some(bar) = gamelogic::control_bar::get_control_bar_bridge() {
            if bar.find_command_button_by_name(ability).is_some() {
                return true;
            }
        }
        if let Some(bar) = game_engine::common::ini::ini_command_button::get_control_bar() {
            if bar.find_command_button_resolved(ability).is_some() {
                return true;
            }
            if !bar.get_button_names().is_empty() {
                return false;
            }
        }
        command_type_from_button_name(ability).is_some()
    }

    fn leftover_skirmish_command_button_options(
        ability: &str,
    ) -> gamelogic::object::update::special_power_update::SpecialPowerCommandOption {
        use gamelogic::object::update::special_power_update::SpecialPowerCommandOption;
        if let Some(bar) = gamelogic::control_bar::get_control_bar_bridge() {
            if let Some(btn) = bar.find_command_button_by_name(ability) {
                return SpecialPowerCommandOption::from_bits_truncate(btn.get_options_bits());
            }
        }
        if let Some(bar) = game_engine::common::ini::ini_command_button::get_control_bar() {
            if let Some(btn) = bar.find_command_button_resolved(ability) {
                return SpecialPowerCommandOption::from_bits_truncate(btn.options_bits);
            }
        }
        SpecialPowerCommandOption::from_bits_truncate(0)
    }

    fn host_script_relationship(
        &self,
        viewer: u32,
        candidate: &crate::game_logic::Object,
    ) -> gamelogic::common::Relationship {
        use crate::game_logic::Team;
        use gamelogic::common::Relationship;
        let Some(oid) = candidate.owner_player_id else {
            let vt = self
                .players
                .get(&viewer)
                .map(|p| p.team)
                .unwrap_or(Team::Neutral);
            if candidate.team == Team::Neutral || vt == Team::Neutral {
                return Relationship::Neutral;
            }
            if candidate.team == vt {
                return Relationship::Allies;
            }
            return Relationship::Enemies;
        };
        self.players
            .get(&viewer)
            .and_then(|p| p.map_relationship(oid))
            .unwrap_or_else(|| {
                let vt = self
                    .players
                    .get(&viewer)
                    .map(|p| p.team)
                    .unwrap_or(candidate.team);
                let ot = self
                    .players
                    .get(&oid)
                    .map(|p| p.team)
                    .unwrap_or(candidate.team);
                if vt == ot {
                    Relationship::Allies
                } else if vt == Team::Neutral || ot == Team::Neutral {
                    Relationship::Neutral
                } else {
                    Relationship::Enemies
                }
            })
    }



    fn host_script_coord_to_world(x: f32, y: f32, z: f32) -> glam::Vec3 {
        // Generals Coord3D: (x,y) map plane, z = height.
        glam::Vec3::new(x, z, y)
    }

    fn host_script_create_team(&self, team_name: &str) -> crate::game_logic::Team {
        if let Ok(factory) = gamelogic::team::get_team_factory().lock() {
            if let Some(proto) = factory.find_team_prototype(team_name) {
                let owner = proto.get_owner_name().to_string();
                if !owner.is_empty() {
                    if let Some(pid) = self.host_player_id_for_script_token(&owner) {
                        if let Some(player) = self.players.get(&pid) {
                            return player.team;
                        }
                    }
                }
            }
        }
        Self::resolve_host_team_name(team_name).unwrap_or(crate::game_logic::Team::Neutral)
    }

    fn host_script_create_object(
        &mut self,
        name: Option<&str>,
        thing: &str,
        team_name: &str,
        x: f32,
        y: f32,
        z: f32,
        angle: f32,
    ) -> Option<ObjectId> {
        if let Some(unit_name) = name.filter(|n| !n.is_empty()) {
            if let Some(id) = self.host_object_id_by_script_name(unit_name) {
                if self
                    .objects
                    .get(&id)
                    .is_some_and(|obj| obj.is_alive() && !obj.status.destroyed)
                {
                    return None;
                }
            }
        }
        let team = self.host_script_create_team(team_name);
        let mut pos = Self::host_script_coord_to_world(x, y, z);
        if z == 0.0 {
            if let Some(h) = self.terrain_height_at(glam::Vec3::new(pos.x, 0.0, pos.z)) {
                pos.y = h;
            }
        }
        let id = self.create_object(thing, team, pos)?;
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.set_orientation(angle);
            if !team_name.trim().is_empty() {
                obj.team_instance_name = team_name.to_string();
            }
            if let Some(unit_name) = name.filter(|n| !n.is_empty()) {
                obj.name = unit_name.to_string();
            }
        }
        Some(id)
    }

    fn host_script_create_reinforcement_team(&mut self, team_name: &str, waypoint_name: &str) {
        let Some(dest) = self.host_script_waypoint_position(waypoint_name) else {
            return;
        };
        let mut origin = dest;
        let (start, transport, units) = {
            let Ok(factory) = gamelogic::team::get_team_factory().lock() else {
                return;
            };
            let Some(proto) = factory.find_team_prototype(team_name) else {
                return;
            };
            (
                proto.get_start_reinforce_waypoint().to_string(),
                proto.get_transport_unit_type().to_string(),
                proto
                    .units_info()
                    .iter()
                    .filter(|unit| unit.max_units >= 1 && !unit.unit_thing_name.is_empty())
                    .map(|unit| (unit.unit_thing_name.to_string(), unit.max_units))
                    .collect::<Vec<_>>(),
            )
        };
        if !start.is_empty() {
            if let Some(start_pos) = self.host_script_waypoint_position(&start) {
                origin = start_pos;
            }
        }
        let mut spawned: Vec<ObjectId> = Vec::new();
        if !transport.is_empty() {
            if let Some(id) = self.host_script_create_object(
                None,
                &transport,
                team_name,
                origin.x,
                origin.z,
                origin.y,
                0.0,
            ) {
                spawned.push(id);
            }
        }
        let mut slot = 0i32;
        for (thing, count) in units {
            for _ in 0..count {
                let offset = slot as f32 * 5.0;
                if let Some(id) = self.host_script_create_object(
                    None,
                    &thing,
                    team_name,
                    origin.x + offset,
                    origin.z,
                    origin.y,
                    0.0,
                ) {
                    spawned.push(id);
                }
                slot += 1;
            }
        }
        if (origin - dest).length_squared() > 1.0 {
            for id in spawned {
                let _ = self.unit_command_move_to(id, dest);
            }
        }
    }


    /// C++ `ScriptActions::doTeamHuntWithCommandButton` live drain.
    fn host_script_team_hunt_with_command_button(&mut self, team: &str, button: &str) {
        // C++ findCommandButton + command-type switch before any unit is armed.
        // Leftover `command_button_is_hunt_capable` is the same gate.
        if !Self::leftover_command_button_is_hunt_capable(button) {
            return;
        }
        let ids = self.host_script_hunt_guard_team_member_ids(team);
        let button = if button.is_empty() { None } else { Some(button) };
        for id in ids {
            if self.unit_can_team_hunt_with_command_button(id, button) {
                let _ = self.start_command_button_hunt_named(id, button);
            }
        }
    }

    /// C++ `Object::leaveGroup` before PLAYER_HUNT `aiHunt`.
    /// Live formation_id is the AIGroup/formation membership leftover `group_id` maps to.
    fn host_object_leave_group(&mut self, id: ObjectId) {
        if let Some(unit) = self.objects.get_mut(&id) {
            if unit.formation_id != 0 || unit.formation_offset != glam::Vec2::ZERO {
                unit.set_formation(0, glam::Vec2::ZERO);
            }
        }
        let _ = gamelogic::object::registry::OBJECT_REGISTRY.with_object_mut(id.0, |obj| {
            obj.leave_group();
        });
    }

    /// C++ `ScriptEngine::getTeamNamed` / leftover `TeamFactory::find_team`.
    /// First leftover instance members, else live `team_instance_name` — never faction Team.
    fn host_script_hunt_guard_team_member_ids(&self, team_name: &str) -> Vec<ObjectId> {
        let needle = team_name.trim();
        if needle.is_empty() {
            return Vec::new();
        }
        let leftover_ids: Vec<ObjectId> = gamelogic::team::get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| {
                factory.find_team(needle).and_then(|team| {
                    team.read().ok().map(|tg| tg.get_members().to_vec())
                })
            })
            .unwrap_or_default()
            .into_iter()
            .map(ObjectId)
            .filter(|id| {
                self.objects
                    .get(id)
                    .is_some_and(|o| o.is_alive() && !o.status.destroyed)
            })
            .collect();
        if !leftover_ids.is_empty() {
            return leftover_ids;
        }
        self.host_script_team_census_member_ids(needle)
            .into_iter()
            .map(ObjectId)
            .filter(|id| {
                self.objects
                    .get(&id)
                    .is_some_and(|o| o.is_alive() && !o.status.destroyed)
            })
            .collect()
    }

    /// Leftover `command_button_is_hunt_capable` + C++ findCommandButton NULL → no-op.
    fn leftover_command_button_is_hunt_capable(ability: &str) -> bool {
        if ability.is_empty() {
            return false;
        }
        if let Some(bar) = gamelogic::control_bar::get_control_bar_bridge() {
            if let Some(btn) = bar.find_command_button_by_name(ability) {
                return Self::leftover_command_type_is_hunt_capable(
                    btn.get_command_type(),
                    btn.get_special_power_template().is_some(),
                    btn.get_options_bits(),
                );
            }
        }
        if let Some(bar) = game_engine::common::ini::ini_command_button::get_control_bar() {
            if let Some(btn) = bar.find_command_button_resolved(ability) {
                return Self::leftover_command_type_is_hunt_capable(
                    gamelogic::command_button::map_gui_command_to_command_type(&btn.command),
                    btn.get_special_power_template().is_some(),
                    btn.options_bits,
                );
            }
            if !bar.get_button_names().is_empty() {
                return false;
            }
        }
        false
    }

    /// Leftover `ScriptActionDispatcher::command_button_is_hunt_capable` switch.
    fn leftover_command_type_is_hunt_capable(
        command_type: gamelogic::commands::command::CommandType,
        has_special_power_template: bool,
        options_bits: u32,
    ) -> bool {
        use gamelogic::commands::command::CommandType;
        use gamelogic::object::update::special_power_update::SpecialPowerCommandOption;
        match command_type {
            CommandType::DoSpecialPower => {
                if !has_special_power_template {
                    return false;
                }
                let options = SpecialPowerCommandOption::from_bits_truncate(options_bits);
                options.intersects(
                    SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                        | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                        | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT,
                )
            }
            CommandType::SwitchWeapons
            | CommandType::DoAttackObject
            | CommandType::Enter
            | CommandType::ConvertToCarbomb => true,
            _ => false,
        }
    }

    fn host_script_team_member_ids(&self, team_name: &str) -> Vec<ObjectId> {
        let needle = team_name.trim();
        if needle.is_empty() {
            return Vec::new();
        }
        let faction = Self::resolve_host_team_name(team_name);
        self.objects
            .values()
            .filter(|obj| {
                obj.is_alive()
                    && !obj.status.destroyed
                    && (faction.map(|t| obj.team == t).unwrap_or(false)
                        || (!obj.team_instance_name.is_empty()
                            && obj.team_instance_name.eq_ignore_ascii_case(needle))
                        || obj.team.get_name().eq_ignore_ascii_case(needle))
            })
            .map(|obj| obj.id)
            .collect()
    }

    fn host_script_leftover_waypoint(&self, waypoint_name: &str) -> Option<(u32, glam::Vec3)> {
        let name = gamelogic::common::AsciiString::from(waypoint_name);
        let (wid, loc) = gamelogic::terrain::get_terrain_logic()
            .read()
            .ok()
            .and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&name)
                    .map(|wp| (wp.get_id(), *wp.get_location()))
            })?;
        let mut pos = glam::Vec3::new(loc.x, loc.z, loc.y);
        if let Some(h) = self.terrain_height_at(glam::Vec3::new(pos.x, 0.0, pos.z)) {
            pos.y = h;
        }
        Some((wid, pos))
    }

    fn host_script_waypoint_position(&self, waypoint_name: &str) -> Option<glam::Vec3> {
        self.host_script_leftover_waypoint(waypoint_name)
            .map(|(_, pos)| pos)
    }

    /// C++ `TheTerrainLogic->getClosestWaypointOnPath` then `link[0]` chain.
    fn host_script_waypoint_path_from(
        &self,
        path_label: &str,
        from: glam::Vec3,
    ) -> Option<Vec<glam::Vec3>> {
        let leftover_pos = gamelogic::common::Coord3D::new(from.x, from.z, from.y);
        let terrain = gamelogic::terrain::get_terrain_logic().read().ok()?;
        let start = terrain.get_closest_waypoint_on_path(&leftover_pos, path_label)?;
        let chain = terrain.walk_link0_chain(start, gamelogic::terrain::WAYPOINT_PATH_LIMIT);
        if chain.is_empty() {
            return None;
        }
        Some(
            chain
                .into_iter()
                .map(|wp| {
                    let loc = *wp.get_location();
                    let mut pos = glam::Vec3::new(loc.x, loc.z, loc.y);
                    if let Some(h) = self.terrain_height_at(glam::Vec3::new(pos.x, 0.0, pos.z)) {
                        pos.y = h;
                    }
                    pos
                })
                .collect(),
        )
    }

    /// C++ `aiFollowWaypointPath` / `groupFollowWaypointPath` / Exact / AsTeam.
    fn host_script_issue_follow_waypoint_path(
        &mut self,
        units: &[ObjectId],
        waypoints: &[glam::Vec3],
        exact: bool,
        as_team: bool,
        path_label: &str,
    ) {
        if waypoints.is_empty() {
            return;
        }
        let mut movers: Vec<(ObjectId, glam::Vec3, glam::Vec2)> = Vec::new();
        for &unit_id in units {
            let Some(unit) = self.host_object(unit_id) else {
                continue;
            };
            if !unit.is_alive() || !unit.can_move() {
                continue;
            }
            if unit.is_kind_of(crate::game_logic::KindOf::Immobile)
                || unit.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                continue;
            }
            movers.push((unit_id, unit.get_position(), unit.formation_offset));
        }
        if movers.is_empty() {
            return;
        }
        let (mut cx, mut cz) = (0.0f32, 0.0f32);
        for (_, pos, _) in &movers {
            cx += pos.x;
            cz += pos.z;
        }
        let n = movers.len() as f32;
        cx /= n;
        cz /= n;
        let fid0 = self
            .host_object(movers[0].0)
            .map(|o| o.formation_id)
            .unwrap_or(0);
        let use_formation = as_team
            && fid0 != 0
            && movers.iter().all(|(id, _, _)| {
                self.host_object(*id)
                    .map(|o| o.formation_id == fid0)
                    .unwrap_or(false)
            });
        let last = *waypoints.last().unwrap();
        let labels = leftover_waypoint_path_labels(path_label, last);
        for (unit_id, pos, form_off) in movers {
            let offset = if as_team {
                if use_formation {
                    form_off
                } else {
                    glam::Vec2::new(pos.x - cx, pos.z - cz)
                }
            } else {
                glam::Vec2::ZERO
            };
            let unit_wps: Vec<glam::Vec3> = waypoints
                .iter()
                .map(|wp| glam::Vec3::new(wp.x + offset.x, wp.y, wp.z + offset.y))
                .collect();
            let goal = *unit_wps.last().unwrap();
            let via = &unit_wps[..unit_wps.len().saturating_sub(1)];
            let _ = self.unit_command_waypoint_path_prep(unit_id, as_team);
            let assigned = if exact {
                self.assign_unit_path_exact(unit_id, goal, via)
            } else {
                self.assign_unit_path(unit_id, goal, via)
            };
            if assigned {
                if let Some(unit) = self.host_object_mut(unit_id) {
                    unit.stamp_pending_waypoint_labels(labels.iter().cloned());
                }
            }
        }
    }

    fn host_script_area_center(&self, area_name: &str) -> Option<glam::Vec3> {
        if let Ok(terrain) = gamelogic::terrain::get_terrain_logic().read() {
            if let Some(trigger) = terrain.get_trigger_area_by_name(area_name) {
                let c = trigger.get_center_point();
                let mut pos = glam::Vec3::new(c.x, c.z, c.y);
                if let Some(h) = self.terrain_height_at(glam::Vec3::new(pos.x, 0.0, pos.z)) {
                    pos.y = h;
                }
                return Some(pos);
            }
        }
        for (name, (min_x, min_z, max_x, max_z)) in
            gamelogic::scripting::engine::get_area_tracker().all_area_aabbs()
        {
            if name.eq_ignore_ascii_case(area_name) {
                let mut pos = glam::Vec3::new((min_x + max_x) * 0.5, 0.0, (min_z + max_z) * 0.5);
                if let Some(h) = self.terrain_height_at(pos) {
                    pos.y = h;
                }
                return Some(pos);
            }
        }
        None
    }

    fn host_script_nearest_team_victim(
        &self,
        from: ObjectId,
        victim_team: &str,
    ) -> Option<ObjectId> {
        let origin = self.objects.get(&from)?.get_position();
        self.host_script_team_member_ids(victim_team)
            .into_iter()
            .filter(|&id| id != from)
            .filter_map(|id| {
                self.objects
                    .get(&id)
                    .map(|obj| (id, origin.distance(obj.get_position())))
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| id)
    }

    fn host_script_attack_team(&mut self, unit_id: ObjectId, victim_team: &str) {
        if let Some(team) = Self::resolve_host_team_name(victim_team) {
            if team != crate::game_logic::Team::Neutral {
                if let Some(unit) = self.objects.get_mut(&unit_id) {
                    unit.set_max_shots_to_fire(-1);
                    unit.auto_acquire_when_idle = true;
                    unit.attack_priority_set =
                        Some(format!("AIGroup.AttackTeam.{}", team.get_name()));
                }
            }
        }
        if let Some(vid) = self.host_script_nearest_team_victim(unit_id, victim_team) {
            let _ = self.unit_command_attack(unit_id, vid);
            if let Some(team) = Self::resolve_host_team_name(victim_team) {
                if team != crate::game_logic::Team::Neutral {
                    if let Some(unit) = self.objects.get_mut(&unit_id) {
                        unit.set_max_shots_to_fire(-1);
                        unit.auto_acquire_when_idle = true;
                        unit.attack_priority_set =
                            Some(format!("AIGroup.AttackTeam.{}", team.get_name()));
                    }
                }
            }
        }
    }

    fn host_script_attack_area(&mut self, unit_id: ObjectId, area_name: &str) {
        let tag = format!("AIGroup.AttackArea.poly:{area_name}");
        let center = self.host_script_area_center(area_name);
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            unit.auto_acquire_when_idle = true;
            unit.attack_priority_set = Some(tag.clone());
        }
        let victim = self.find_attack_area_victim(
            unit_id,
            center.unwrap_or(glam::Vec3::ZERO),
            1.0,
            Some(area_name),
        );
        if let Some(vid) = victim {
            let _ = self.unit_command_attack(unit_id, vid);
            if let Some(unit) = self.objects.get_mut(&unit_id) {
                unit.auto_acquire_when_idle = true;
                unit.attack_priority_set = Some(tag);
            }
        } else if let Some(dest) = center {
            let _ = self.unit_command_attack_move_to(unit_id, dest);
            if let Some(unit) = self.objects.get_mut(&unit_id) {
                unit.attack_priority_set = Some(tag);
            }
        }
    }


    pub(in super::super) fn evaluate_and_execute_scripts(&mut self, dt: f32) {
        if !self.scripts_loaded {
            return;
        }

        // Host script path: named-unit/team/area queries hit HOST objects.
        // Crate evaluator sees the name→id map + query snapshot (no crate Objects).
        self.inject_host_named_unit_map_into_crate_tracker();
        self.inject_host_supply_source_queries();

        self.update_script_camera(dt * self.visual_speed_multiplier.max(0.0));

        // Increment script frame counter
        self.mission_script_counter += 1;

        for event in script_events::drain_events() {
            match event {
                ScriptEvent::PlayerDefeated { player_id } => {
                    log::debug!(
                        "📜 Script event: player {} defeated (frame {})",
                        player_id,
                        self.frame
                    );
                    self.partition_manager
                        .reveal_map_for_player_permanently(player_id);
                }
                ScriptEvent::RevealMapForPlayer { player_id } => {
                    log::debug!("📜 Script event: reveal map for player {}", player_id);
                    self.partition_manager.reveal_map_for_player(player_id);
                }
                ScriptEvent::CompletedSpecialPower {
                    player_id,
                    ref special_power_name,
                    creator_id,
                } => {
                    log::debug!(
                        "📜 Script event: completed special power {} player {} creator {}",
                        special_power_name,
                        player_id,
                        creator_id
                    );
                    let _ = gamelogic::scripting::engine::with_script_engine_mut(|engine| {
                        engine.notify_of_completed_special_power(
                            player_id as usize,
                            special_power_name,
                            creator_id,
                        );
                    });
                }

                ScriptEvent::AllianceStateChanged { player_id, state } => {
                    log::debug!(
                        "📜 Script event: alliance state {:?} for player {}",
                        state,
                        player_id
                    );
                }
            }
        }

        // Leftover ScriptingEngine event queue / process_events is leftover-only
        // (hq-8ta4n). Live host conditions/actions walk ScriptEngine::update.
        // C++ GameLogic.cpp:3600 — one TheScriptEngine->UPDATE() per logic frame.
        // Take the engine out of the global RwLock for the duration of update().
        // std::sync::RwLock is not re-entrant: holding write() across update()
        // deadlocks when MUSIC_SET_TRACK / MOVE_CAMERA_TO call
        // get_script_engine().read() (hang after "named cache populated").
        let taken = match gamelogic::scripting::engine::get_script_engine().write() {
            Ok(mut guard) => guard.take(),
            Err(_) => {
                log::error!("ScriptEngine::update failed: lock poisoned");
                None
            }
        };
        if let Some(engine) = taken {
            if let Err(err) = engine.update() {
                log::error!("ScriptEngine::update failed: {err}");
            }
            if let Ok(mut guard) = gamelogic::scripting::engine::get_script_engine().write() {
                *guard = Some(engine);
            }
        }
        self.apply_host_skirmish_script_requests();
        self.apply_host_set_base_construction_speed_requests();
        self.apply_host_set_train_held_requests();
        self.apply_host_money_script_requests();
        self.apply_host_can_build_script_requests();
        self.apply_host_buildable_override_script_requests();
        self.apply_host_rank_script_requests();
        self.apply_host_transfer_script_requests();
        self.apply_host_player_relates_script_requests();
        self.apply_host_team_override_relation_script_requests();

        self.apply_host_loco_set_script_requests();
        self.apply_host_face_script_requests();

        self.apply_host_move_attack_script_requests();
        self.apply_host_hunt_guard_script_requests();
        self.apply_host_idle_script_requests();
        self.apply_host_kill_delete_damage_script_requests();


        self.apply_host_follow_waypoints_script_requests();
        self.apply_host_skirmish_approach_path_script_requests();


        self.apply_host_create_script_requests();
        self.apply_host_boobytrap_script_requests();
        self.apply_host_unmanned_script_requests();
        self.apply_host_radar_event_script_requests();
        self.apply_host_stealth_enabled_script_requests();
        self.apply_host_team_attitude_script_requests();
        self.apply_host_script_visual_status_requests();
        self.apply_host_guard_supply_center_script_requests();
        self.apply_host_guard_variant_script_requests();
        self.apply_host_named_fire_special_script_requests();
        self.apply_host_use_command_button_script_requests();
        self.apply_host_object_sound_script_requests();

        self.apply_host_skirmish_fight_script_requests();




        self.mission_scripts.note_logic_frame(self.frame as u64);

        self.script_broadcasts
            .retain(|msg| self.sim_time_seconds <= msg.expires_at);

        if self
            .cinematic_text
            .as_ref()
            .is_some_and(|(_, expires_at)| self.sim_time_seconds > *expires_at)
        {
            self.cinematic_text = None;
            self.cinematic_font = None;
        }

        if self
            .military_caption
            .as_ref()
            .is_some_and(|(_, expires_at)| self.sim_time_seconds > *expires_at)
        {
            self.military_caption = None;
        }

        for msg in self.mission_scripts.drain_messages() {
            self.script_broadcasts.push(ScriptBroadcast {
                text: msg.clone(),
                expires_at: self.sim_time_seconds + SCRIPT_BROADCAST_DURATION,
            });
            self.new_script_messages.push(msg);
        }

        for sound in self.mission_scripts.drain_sounds() {
            self.play_ui_sound(&sound);
        }

        for sound in self.mission_scripts.drain_sound_events() {
            let translated = translate_audio_event(&sound.sound_name);
            let mut event = AudioEventRequest::new(translated);
            if let Some(pos) = sound.position {
                event = event.with_position(pos);
            }
            self.queue_audio_event(event);
        }

        for camera_target in self.mission_scripts.drain_camera_moves() {
            self.request_camera_focus(camera_target);
        }

        if !self
            .mission_scripts
            .drain_camera_move_to_selection_requests()
            .is_empty()
        {
            // C++ doModCameraMoveToSelection → cameraModFinalMoveTo: path modifier,
            // not a new lookAt. No-op during rotate; no-op if no path/move.
            if self.pending_camera_rotate.is_none() {
                if let Some(center) = self.selected_objects_center_for_local_player() {
                    if let Some(path) = self.script_camera_path.as_mut() {
                        path.camera_mod_final_move_to(center);
                    }
                    if let Some(move_to) = self.script_camera_move_to.as_mut() {
                        move_to.camera_mod_final_move_to(center);
                    }
                    #[cfg(feature = "game_client")]
                    {
                        game_client::display::view::with_tactical_view(|view| {
                            view.camera_mod_final_move_to(
                                &game_client::display::view::Point3::new(
                                    center.x, center.z, center.y,
                                ),
                            );
                        });
                    }
                }
            }
        }

        if !self
            .mission_scripts
            .drain_camera_move_home_requests()
            .is_empty()
        {
            if let Some(home) = self.local_player_camera_home_position() {
                self.camera_follow_target = None;
                self.request_camera_focus(home);
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_follows()
            .into_iter()
            .last()
        {
            if last.object_id == 0 {
                self.camera_follow_target = None;
                self.camera_tether_play = None;
            } else {
                self.script_camera_move_to = None;
                self.script_camera_path = None;
                self.camera_tether_play = None;
                self.camera_follow_target = Some(ObjectId(last.object_id));
                if last.snap_to_unit {
                    if let Some(obj) = self.objects.get(&ObjectId(last.object_id)) {
                        self.request_camera_focus(obj.get_position());
                    }
                }
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_tethers()
            .into_iter()
            .last()
        {
            self.script_camera_move_to = None;
            self.script_camera_path = None;
            self.set_camera_tether_object(ObjectId(last.object_id), last.snap_to_unit, last.play);
        }

        if !self
            .mission_scripts
            .drain_camera_mod_freeze_time_requests()
            .is_empty()
        {
            self.apply_script_camera_mod_freeze_time();
        }



        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_final_speed_multiplier_requests()
            .into_iter()
            .last()
        {
            self.apply_script_camera_mod_final_speed_multiplier(&last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_rolling_average_requests()
            .into_iter()
            .last()
        {
            self.apply_script_camera_mod_rolling_average(&last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_visual_speed_multiplier_requests()
            .into_iter()
            .last()
        {
            self.apply_visual_speed_multiplier(&last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_script_freeze_time_requests()
            .into_iter()
            .last()
        {
            self.script_time_frozen_by_script = last;
        }

        if let Some(last) = self
            .mission_scripts
            .drain_set_fps_limit_requests()
            .into_iter()
            .last()
        {
            self.apply_set_fps_limit(&last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_move_to()
            .into_iter()
            .last()
        {
            self.start_camera_move_to(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_path_moves()
            .into_iter()
            .last()
        {
            self.start_camera_path_move(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_set_default_requests()
            .into_iter()
            .last()
        {
            self.apply_script_camera_default(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_slave_mode_enable_requests()
            .into_iter()
            .last()
        {
            self.pending_camera_slave_mode_enable = Some(last);
            self.pending_camera_slave_mode_disable = false;
        }

        if !self
            .mission_scripts
            .drain_camera_slave_mode_disable_requests()
            .is_empty()
        {
            self.pending_camera_slave_mode_enable = None;
            self.pending_camera_slave_mode_disable = true;
        }

        let screen_shakes = self.mission_scripts.drain_screen_shake_requests();
        if !screen_shakes.is_empty() {
            self.pending_screen_shakes.extend(screen_shakes);
        }

        let camera_shakers = self.mission_scripts.drain_camera_add_shaker_requests();
        if !camera_shakers.is_empty() {
            self.pending_camera_add_shakers.extend(camera_shakers);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_resets()
            .into_iter()
            .last()
        {
            self.camera_follow_target = None;
            // C++ setupWaypointPath ends with m_doingRotateCamera = false.
            // Leftover reset_camera replaces camera_rotate so a prior ROTATE
            // cannot keep peeking into presentation after the reset.
            self.pending_camera_rotate = None;
            self.script_camera_rotate_remaining = 0.0;
            self.pending_camera_zoom_reset = true;
            self.pending_camera_zoom_reset_duration = last.duration_seconds.max(0.0);
            self.pending_camera_zoom_reset_ease_in = last.ease_in_seconds.max(0.0);
            self.pending_camera_zoom_reset_ease_out = last.ease_out_seconds.max(0.0);
            let request = CameraMoveToRequest {
                position: last.position,
                seconds: last.duration_seconds,
                camera_stutter_seconds: 0.0,
                ease_in_seconds: last.ease_in_seconds.max(0.0),
                ease_out_seconds: last.ease_out_seconds.max(0.0),
            };
            self.start_camera_move_to(request);
            if let Some(move_to) = self.script_camera_move_to.as_mut() {
                move_to.set_suppress_travel_look(true);
            }
        }


        if let Some(last) = self
            .mission_scripts
            .drain_camera_zoom_requests()
            .into_iter()
            .last()
        {
            self.begin_script_camera_zoom(last.duration_seconds);
            self.pending_camera_zoom = Some(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_pitch_requests()
            .into_iter()
            .last()
        {
            self.begin_script_camera_pitch(last.duration_seconds);
            self.pending_camera_pitch = Some(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_rotate_requests()
            .into_iter()
            .last()
        {
            // C++ rotateCamera replaces any current animation. FREEZE_ANGLE only
            // pins the in-flight move/path and must not swallow later rotates.
            self.begin_script_camera_rotate(last.duration_seconds);
            self.pending_camera_rotate = Some(last);
        }

        // C++ mods apply to the in-flight animation. Drain MOVE/PATH/RESET/ROTATE
        // first so same-frame ROTATE_CAMERA + CAMERA_MOD_FREEZE_ANGLE pins yaw.
        if !self
            .mission_scripts
            .drain_camera_mod_freeze_angle_requests()
            .is_empty()
        {
            self.apply_script_camera_mod_freeze_angle();
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_final_zoom_requests()
            .into_iter()
            .last()
        {
            #[cfg(feature = "game_client")]
            {
                game_client::display::view::with_tactical_view(|view| {
                    view.camera_mod_final_zoom(last.zoom, last.ease_in, last.ease_out);
                });
            }
            // Leftover/C++ cameraModFinalZoom: idle (no rotate/path/move) is a no-op.
            let remaining = self.script_camera_remaining_seconds();
            if remaining > 0.0 {
                let max_zoom = (320.0 + 300.0) / 320.0;
                self.begin_script_camera_zoom(remaining);
                self.pending_camera_zoom = Some(CameraZoomRequest {
                    zoom: last.zoom * max_zoom,
                    duration_seconds: remaining,
                    ease_in_seconds: (remaining * last.ease_in.clamp(0.0, 1.0)).max(0.0),
                    ease_out_seconds: (remaining * last.ease_out.clamp(0.0, 1.0)).max(0.0),
                });
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_final_pitch_requests()
            .into_iter()
            .last()
        {
            #[cfg(feature = "game_client")]
            {
                game_client::display::view::with_tactical_view(|view| {
                    view.camera_mod_final_pitch(last.pitch, last.ease_in, last.ease_out);
                });
            }
            // Leftover/C++ cameraModFinalPitch: idle (no rotate/path/move) is a no-op.
            let remaining = self.script_camera_remaining_seconds();
            if remaining > 0.0 {
                self.begin_script_camera_pitch(remaining);
                self.pending_camera_pitch = Some(CameraPitchRequest {
                    pitch: last.pitch,
                    duration_seconds: remaining,
                    ease_in_seconds: (remaining * last.ease_in.clamp(0.0, 1.0)).max(0.0),
                    ease_out_seconds: (remaining * last.ease_out.clamp(0.0, 1.0)).max(0.0),
                });
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_setup_requests()
            .into_iter()
            .last()
        {
            self.camera_follow_target = None;
            // C++ moveCameraTo → setupWaypointPath rebuilds m_mcwpInfo and
            // sets m_doingRotateCamera = false. Leftover setup_camera →
            // look_at cancels camera_move / camera_path / camera_rotate.
            self.script_camera_move_to = None;
            self.script_camera_path = None;
            self.script_look_toward_object_id = None;
            self.script_look_toward_hold_seconds = 0.0;
            self.script_camera_rotate_remaining = 0.0;
            self.request_camera_focus(last.position);
            let max_zoom = (320.0 + 300.0) / 320.0;
            self.pending_camera_zoom = Some(CameraZoomRequest {
                zoom: last.zoom * max_zoom,
                duration_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
            self.pending_camera_pitch = Some(CameraPitchRequest {
                pitch: last.pitch,
                duration_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
            self.pending_camera_rotate = None;
            self.pending_camera_look_toward = Some(CameraLookTowardWaypointRequest {
                position: last.look_toward,
                duration_seconds: 0.0,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
                reverse_rotation: false,
            });
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_look_toward_waypoint_requests()
            .into_iter()
            .last()
        {
            // C++ rotateCameraTowardPosition: m_doingMoveCameraOnWaypointPath = false.
            self.script_camera_move_to = None;
            self.script_camera_path = None;
            self.pending_camera_rotate = None;
            self.begin_script_camera_rotate(last.duration_seconds);
            self.pending_camera_look_toward = Some(last);
        }
        if let Some(last) = self
            .mission_scripts
            .drain_camera_look_toward_object_requests()
            .into_iter()
            .last()
        {
            if let Some(position) = self
                .objects
                .get(&ObjectId(last.object_id))
                .map(|obj| obj.get_position())
            {
                // C++ rotateCameraTowardObject: m_doingMoveCameraOnWaypointPath = false.
                self.script_camera_move_to = None;
                self.script_camera_path = None;
                self.pending_camera_rotate = None;
                self.begin_script_camera_rotate(
                    last.duration_seconds + last.hold_seconds.max(0.0),
                );
                self.pending_camera_look_toward = Some(CameraLookTowardWaypointRequest {
                    position,
                    duration_seconds: last.duration_seconds,
                    ease_in_seconds: last.ease_in_seconds,
                    ease_out_seconds: last.ease_out_seconds,
                    reverse_rotation: false,
                });
                self.script_look_toward_object_id = Some(last.object_id);
                self.script_look_toward_hold_seconds = last.hold_seconds.max(0.0);
            } else {
                log::warn!(
                    "Camera look toward object request ignored; object {} not found",
                    last.object_id
                );
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_look_toward_requests()
            .into_iter()
            .last()
        {
            self.apply_script_camera_mod_look_toward(last.position, false);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_mod_final_look_toward_requests()
            .into_iter()
            .last()
        {
            self.apply_script_camera_mod_look_toward(last.position, true);
        }


        if let Some(last) = self
            .mission_scripts
            .drain_letterbox_events()
            .last()
            .copied()
        {
            self.cinematic_letterbox = last;
            // C++ ScriptActions::doLetterBoxMode HideControlBar(TRUE)/ShowControlBar(FALSE).
            #[cfg(feature = "game_client")]
            {
                if last {
                    let _ = game_client::gui::callbacks::control_bar_callbacks::hide_control_bar(
                        true,
                    );
                } else {
                    let _ = game_client::gui::callbacks::control_bar_callbacks::show_control_bar(
                        false,
                    );
                }
            }
        }

        if let Some((text, font, duration_seconds)) = self
            .mission_scripts
            .drain_cinematic_text()
            .into_iter()
            .last()
        {
            let duration = (duration_seconds as f32).max(0.0);
            self.cinematic_text = Some((text, self.sim_time_seconds + duration));
            self.cinematic_font = if font.is_empty() { None } else { Some(font) };
        }

        if let Some(last) = self
            .mission_scripts
            .drain_military_captions()
            .into_iter()
            .last()
        {
            let duration = Self::military_caption_duration_seconds(last.duration_ms);
            self.military_caption = Some((last.text, self.sim_time_seconds + duration));
        }

        if let Some(movie) = self
            .mission_scripts
            .drain_movie_requests()
            .into_iter()
            .last()
        {
            self.pending_movie = Some(movie.clone());
            self.script_broadcasts.push(ScriptBroadcast {
                text: format!("Movie requested: {}", movie),
                expires_at: self.sim_time_seconds + SCRIPT_BROADCAST_DURATION,
            });
        }

        if let Some(movie) = self
            .mission_scripts
            .drain_radar_movie_requests()
            .into_iter()
            .last()
        {
            self.pending_radar_movie = Some(movie);
        }

        let objective_updates = self.mission_scripts.drain_objective_updates();
        if !objective_updates.is_empty() {
            for update in objective_updates {
                let status = if update.completed {
                    ObjectiveStatus::Completed
                } else {
                    ObjectiveStatus::Active
                };

                let updated_existing = self.with_objective_mut(&update.name, |objective| {
                    objective.title = update.name.clone();
                    objective.description = update.description.clone();
                    objective.status = status;
                });

                if !updated_existing {
                    self.mission_objectives.push(ObjectiveDisplay::new(
                        Some(update.name.clone()),
                        update.name.clone(),
                        update.description.clone(),
                        ObjectiveCategory::Primary,
                    ));
                    let idx = self.mission_objectives.len().saturating_sub(1);
                    self.objective_lookup
                        .insert(update.name.to_ascii_lowercase(), idx);
                }
            }
        }

        for effect in self.mission_scripts.drain_effect_requests() {
            self.script_broadcasts.push(ScriptBroadcast {
                text: format!(
                    "Effect '{}' at ({:.0}, {:.0}, {:.0})",
                    effect.effect_type, effect.position.x, effect.position.y, effect.position.z
                ),
                expires_at: self.sim_time_seconds + SCRIPT_BROADCAST_DURATION,
            });
        }

        for radar_event in self.mission_scripts.drain_radar_event_requests() {
            self.queue_script_radar_event(radar_event);
        }

        if let Some(enabled) = self
            .mission_scripts
            .drain_radar_enabled_updates()
            .into_iter()
            .last()
        {
            self.radar_enabled = enabled;
        }

        if let Some(forced) = self
            .mission_scripts
            .drain_radar_forced_updates()
            .into_iter()
            .last()
        {
            self.radar_forced = forced;
        }

        if let Some(visible) = self
            .mission_scripts
            .drain_weather_visibility_updates()
            .into_iter()
            .last()
        {
            self.set_weather_visible(visible);
        }

        let popup_messages = self.mission_scripts.drain_popup_message_requests();
        if !popup_messages.is_empty() {
            // C++ InGameUI owns one popup layout: every new popup replaces the
            // previously visible one.  Keep only the newest presentation
            // residual; MissionScriptHooks itself remains the future-event
            // queue and is already drained above.
            let active_popup = popup_messages.last().cloned();
            #[cfg(feature = "game_client")]
            if let Some(popup) = active_popup.as_ref() {
                // C++ clears/replaces the single InGameUI popup layout. Send
                // only its newest request to GameClient and retain its opaque
                // identity so a delayed ButtonOk/Esc cannot dismiss a later
                // replacement popup in Main.
                game_client::core::script_action_handler::script_popup_message_with_host_generation(
                    &popup.message,
                    popup.x_percent,
                    popup.y_percent,
                    popup.width,
                    popup.pause,
                    popup.pause_music,
                    Some(popup.popup_generation),
                );
            }

            for popup in popup_messages {
                if popup.pause_music {
                    self.pending_music_stop = true;
                }
                self.script_broadcasts.push(ScriptBroadcast {
                    text: popup.message.clone(),
                    expires_at: self.sim_time_seconds + SCRIPT_BROADCAST_DURATION,
                });
                self.new_script_messages.push(popup.message.clone());
            }

            self.pending_popup_messages.clear();
            if let Some(active_popup) = active_popup {
                self.pending_popup_messages.push(active_popup);
            }
        }

        if let Some(last) = self
            .mission_scripts
            .drain_view_guardband_requests()
            .into_iter()
            .last()
        {
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_resize_view_guardband(
                last.x_bias,
                last.y_bias,
            );
            self.pending_view_guardband = Some(last);
        }

        if let Some(last) = self
            .mission_scripts
            .drain_camera_bw_mode_requests()
            .into_iter()
            .last()
        {
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_set_camera_bw_mode(
                last.enabled,
                last.frames,
            );
            self.pending_camera_bw_mode = Some(last);
        }

        if let Some(enabled) = self
            .mission_scripts
            .drain_skybox_enabled_updates()
            .into_iter()
            .last()
        {
            self.script_skybox_enabled = enabled;
            {
                let mut global = game_engine::common::global_data::write();
                global.draw_sky_box = enabled;
            }
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_set_skybox_enabled(enabled);
        }

        for request in self.mission_scripts.drain_camera_motion_blur_requests() {
            #[cfg(feature = "game_client")]
            match &request {
                CameraMotionBlurRequest::Basic { zoom_in, saturate } => {
                    game_client::core::script_action_handler::script_camera_motion_blur(
                        *zoom_in, *saturate,
                    );
                }
                CameraMotionBlurRequest::Jump { position, saturate } => {
                    // C++ doCameraMotionBlurJump: leftover set filter+pos only.
                    // lookAt / request_cam only if leftover filter failed.
                    let passed =
                        game_client::core::script_action_handler::script_camera_motion_blur_jump(
                            position.x, position.z, position.y, *saturate,
                        );
                    if !passed {
                        self.camera_follow_target = None;
                        self.request_camera_focus(*position);
                    }
                }
                CameraMotionBlurRequest::Follow { amount } => {
                    game_client::core::script_action_handler::script_camera_motion_blur_follow(
                        *amount,
                    );
                }
                CameraMotionBlurRequest::EndFollow => {
                    game_client::core::script_action_handler::script_camera_motion_blur_end_follow(
                    );
                }
            }
            #[cfg(not(feature = "game_client"))]
            if let CameraMotionBlurRequest::Jump { position, .. } = &request {
                self.camera_follow_target = None;
                self.request_camera_focus(*position);
            }
            self.pending_camera_motion_blur.push(request);
        }

        for flash in self.mission_scripts.drain_cameo_flash_requests() {
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_cameo_flash(
                &flash.command_button_name,
                flash.flash_count,
            );
            self.script_cameo_flash_count
                .insert(flash.command_button_name, flash.flash_count);
        }

        for mutation in self.mission_scripts.drain_named_timer_mutations() {
            match mutation {
                NamedTimerMutation::Add {
                    name,
                    text,
                    countdown,
                } => {
                    #[cfg(feature = "game_client")]
                    game_client::core::script_action_handler::script_add_named_timer(
                        &name, &text, countdown,
                    );
                    self.script_named_timers.insert(name, (text, countdown));
                }
                NamedTimerMutation::Remove { name } => {
                    #[cfg(feature = "game_client")]
                    game_client::core::script_action_handler::script_remove_named_timer(&name);
                    self.script_named_timers.remove(&name);
                }
            }
        }

        if let Some(show) = self
            .mission_scripts
            .drain_named_timer_display_updates()
            .into_iter()
            .last()
        {
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_show_named_timer_display(show);
            self.script_named_timer_display_shown = show;
        }

        if let Some(enabled) = self
            .mission_scripts
            .drain_superweapon_display_enabled_updates()
            .into_iter()
            .last()
        {
            #[cfg(feature = "game_client")]
            game_client::core::script_action_handler::script_set_superweapon_display_enabled(
                enabled,
            );
            self.script_superweapon_display_enabled = enabled;
        }

        for mutation in self
            .mission_scripts
            .drain_superweapon_object_display_mutations()
        {
            match mutation {
                SuperweaponObjectDisplayMutation::Hide { object_id } => {
                    #[cfg(feature = "game_client")]
                    game_client::core::script_action_handler::script_hide_object_superweapon_display(
                        object_id as gamelogic::common::ObjectID,
                    );
                    self.script_superweapon_hidden_objects
                        .insert(ObjectId(object_id));
                }
                SuperweaponObjectDisplayMutation::Show { object_id } => {
                    #[cfg(feature = "game_client")]
                    game_client::core::script_action_handler::script_show_object_superweapon_display(
                        object_id as gamelogic::common::ObjectID,
                    );
                    self.script_superweapon_hidden_objects
                        .remove(&ObjectId(object_id));
                }
            }
        }

        for mutation in self
            .mission_scripts
            .drain_named_special_power_countdown_mutations()
        {
            let _ = self.script_named_special_power_countdown(
                &mutation.unit_name,
                &mutation.power_name,
                mutation.op,
                mutation.seconds,
            );
        }


        if !self.mission_scripts.drain_music_stop_requests().is_empty() {
            self.pending_music_stop = true;
        }

        #[cfg(feature = "game_client")]
        {
            if let Some(amount) = self
                .mission_scripts
                .drain_oversize_terrain_requests()
                .into_iter()
                .last()
            {
                if let Ok(mut terrain_guard) =
                    game_client::terrain::terrain_visual::get_terrain_visual()
                {
                    if let Some(visual) = terrain_guard.as_mut() {
                        visual.oversize_terrain(amount);
                    }
                }
            }

            if let Some(level) = self
                .mission_scripts
                .drain_border_shroud_levels()
                .into_iter()
                .last()
            {
                if !game_client::core::script_action_handler::set_script_display_border_shroud_level(
                    level,
                ) {
                    log::warn!(
                        "Border shroud level script request not applied: display bridge unavailable"
                    );
                }
            }
        }
    }

    pub(in super::super) fn start_camera_path_move(&mut self, request: CameraPathRequest) {
        self.script_camera_move_to = None;
        // C++ setupWaypointPath: m_doingRotateCamera = false.
        self.pending_camera_rotate = None;
        self.script_camera_rotate_remaining = 0.0;
        if let Some(move_state) =
            ScriptCameraPathMove::new(self.script_camera_focus_estimate, &request)
        {
            let mut move_state = move_state;
            if self.script_camera_freeze_time_armed {
                move_state.set_freeze_time(true);
                self.script_camera_freeze_time_armed = false;
            }
            if self.script_camera_freeze_angle_armed {
                move_state.set_freeze_angle(true);
                self.script_camera_freeze_angle_armed = false;
            }
            if let Some(multiplier) = self.script_camera_pending_final_speed_multiplier.take() {
                move_state.set_final_speed_multiplier(multiplier);
            }
            if let Some(frames) = self.script_camera_pending_rolling_average_frames.take() {
                move_state.set_rolling_average_frames(frames);
            }
            self.mission_scripts.set_camera_movement_finished(false);
            self.script_camera_path = Some(move_state);
        } else {
            self.script_camera_path = None;
            self.mark_script_camera_movement_maybe_finished();
            self.script_broadcasts.push(ScriptBroadcast {
                text: format!("Camera path '{}' not found", request.waypoint),
                expires_at: self.sim_time_seconds + SCRIPT_BROADCAST_DURATION,
            });
        }
    }

    pub(in super::super) fn start_camera_move_to(&mut self, request: CameraMoveToRequest) {
        self.mission_scripts.set_camera_movement_finished(false);
        self.script_camera_path = None;
        // C++ setupWaypointPath: m_doingRotateCamera = false. RESET_CAMERA
        // and MOVE_CAMERA_TO must not leave a stale ROTATE_CAMERA ticking.
        self.pending_camera_rotate = None;
        self.script_camera_rotate_remaining = 0.0;
        let mut move_state = ScriptCameraMoveTo::new(self.script_camera_focus_estimate, &request);
        if self.script_camera_freeze_time_armed {
            move_state.set_freeze_time(true);
            self.script_camera_freeze_time_armed = false;
        }
        if self.script_camera_freeze_angle_armed {
            move_state.set_freeze_angle(true);
            self.script_camera_freeze_angle_armed = false;
        }
        if let Some(multiplier) = self.script_camera_pending_final_speed_multiplier.take() {
            move_state.set_final_speed_multiplier(multiplier);
        }
        self.script_camera_move_to = Some(move_state);
    }

    #[cfg(test)]
    pub fn script_camera_path_active(&self) -> bool {
        self.script_camera_path.is_some()
    }

    #[cfg(test)]
    pub fn script_camera_move_to_target(&self) -> Option<Vec3> {
        self.script_camera_move_to.as_ref().map(|m| m.final_focus())
    }

    fn script_camera_orientation_duration(seconds: f32) -> f32 {
        if seconds > 0.0 {
            seconds
        } else {
            1.0 / 30.0
        }
    }

    pub(in super::super) fn is_script_camera_movement_finished_now(&self) -> bool {
        self.script_camera_move_to.is_none()
            && self.script_camera_path.is_none()
            && !self.script_camera_has_orientation_motion()
    }

    fn script_camera_has_orientation_motion(&self) -> bool {
        self.script_camera_rotate_remaining > 0.0
            || self.script_camera_zoom_remaining > 0.0
            || self.script_camera_pitch_remaining > 0.0
    }

    pub(super) fn clear_script_camera_orientation_remaining(&mut self) {
        self.script_camera_rotate_remaining = 0.0;
        self.script_camera_zoom_remaining = 0.0;
        self.script_camera_pitch_remaining = 0.0;
        self.script_camera_freeze_time = false;
    }

    fn begin_script_camera_rotate(&mut self, duration_seconds: f32) {
        self.script_camera_rotate_remaining =
            Self::script_camera_orientation_duration(duration_seconds);
        self.mission_scripts.set_camera_movement_finished(false);
    }

    fn begin_script_camera_zoom(&mut self, duration_seconds: f32) {
        self.script_camera_zoom_remaining =
            Self::script_camera_orientation_duration(duration_seconds);
        self.mission_scripts.set_camera_movement_finished(false);
    }

    fn begin_script_camera_pitch(&mut self, duration_seconds: f32) {
        self.script_camera_pitch_remaining =
            Self::script_camera_orientation_duration(duration_seconds);
        self.mission_scripts.set_camera_movement_finished(false);
    }

    fn mark_script_camera_movement_maybe_finished(&mut self) {
        if self.is_script_camera_movement_finished_now() {
            self.mission_scripts.set_camera_movement_finished(true);
            self.script_camera_freeze_time = false;
            self.script_camera_freeze_time_armed = false;
        } else {
            self.mission_scripts.set_camera_movement_finished(false);
        }
    }

    fn tick_script_camera_orientation(&mut self, dt: f32) {
        let dt = dt.max(0.0);
        if dt <= 0.0 {
            return;
        }
        let had = self.script_camera_has_orientation_motion();
        if self.script_camera_rotate_remaining > 0.0 {
            self.script_camera_rotate_remaining =
                (self.script_camera_rotate_remaining - dt).max(0.0);
        }
        if self.script_camera_zoom_remaining > 0.0 {
            self.script_camera_zoom_remaining = (self.script_camera_zoom_remaining - dt).max(0.0);
        }
        if self.script_camera_pitch_remaining > 0.0 {
            self.script_camera_pitch_remaining =
                (self.script_camera_pitch_remaining - dt).max(0.0);
        }
        if had && !self.script_camera_has_orientation_motion() {
            self.mark_script_camera_movement_maybe_finished();
        }
    }

    pub(in super::super) fn script_camera_remaining_seconds(&self) -> f32 {
        // C++ cameraModFinalZoom/Pitch: remaining rotate frames first, then path/move.
        if self.script_camera_rotate_remaining > 0.0 {
            return self.script_camera_rotate_remaining;
        }
        if let Some(rotate) = self.pending_camera_rotate.as_ref() {
            if rotate.duration_seconds > 0.0 {
                return rotate.duration_seconds;
            }
        }
        if let Some(move_to) = self.script_camera_move_to.as_ref() {
            return move_to.remaining_time_seconds();
        }
        if let Some(path) = self.script_camera_path.as_ref() {
            return path.remaining_time_seconds();
        }
        0.0
    }

    pub(in super::super) fn is_script_camera_angle_frozen(&self) -> bool {
        self.script_camera_move_to
            .as_ref()
            .map(|move_to| move_to.freeze_angle())
            .unwrap_or(false)
            || self
                .script_camera_path
                .as_ref()
                .map(|path| path.freeze_angle())
                .unwrap_or(false)
    }

    /// C++ `W3DView::setDefaultView`: pitch + max-height scale; angle ignored.
    pub(in super::super) fn apply_script_camera_default(
        &mut self,
        request: CameraSetDefaultRequest,
    ) {
        self.script_default_camera_pitch = request.pitch;
        self.script_default_camera_angle = 0.0;
        self.script_default_camera_max_height = if request.max_height.is_finite() {
            request.max_height
        } else {
            1.0
        };
    }

    pub(in super::super) fn apply_script_camera_mod_freeze_time(&mut self) {
        // C++ cameraModFreezeTime: m_freezeTimeForCameraMovement = true.
        self.script_camera_freeze_time = true;
        let mut applied = false;
        if let Some(move_to) = self.script_camera_move_to.as_mut() {
            move_to.set_freeze_time(true);
            applied = true;
        }
        if let Some(path) = self.script_camera_path.as_mut() {
            path.set_freeze_time(true);
            applied = true;
        }
        if self.script_camera_has_orientation_motion() {
            applied = true;
        }
        if !applied {
            self.script_camera_freeze_time_armed = true;
        }
    }

    pub(in super::super) fn apply_script_camera_mod_freeze_angle(&mut self) {
        #[cfg(feature = "game_client")]
        {
            game_client::display::view::with_tactical_view(|view| {
                view.camera_mod_freeze_angle();
            });
        }
        let mut applied = false;
        if let Some(move_to) = self.script_camera_move_to.as_mut() {
            move_to.set_freeze_angle(true);
            applied = true;
        }
        if let Some(path) = self.script_camera_path.as_mut() {
            path.freeze_angles_to_start();
            applied = true;
        }
        // Leftover freeze_current_angle: pin in-flight rotate start=end=current.
        if let Some(rotate) = self.pending_camera_rotate.as_mut() {
            rotate.rotations = 0.0;
            applied = true;
        } else if self.script_camera_rotate_remaining > 0.0 {
            self.pending_camera_rotate = Some(CameraRotateRequest {
                rotations: 0.0,
                duration_seconds: self.script_camera_rotate_remaining,
                ease_in_seconds: 0.0,
                ease_out_seconds: 0.0,
            });
            applied = true;
        }
        if applied {
            // Pin the in-flight move/path. Do not leave a queued travel look.
            self.pending_camera_look_toward = None;
        }
    }

    /// C++ `cameraModLookToward` / `cameraModFinalLookToward`: rewrite the
    /// active waypoint-path (or simple moveCameraTo) look. No-op if idle.
    pub(in super::super) fn apply_script_camera_mod_look_toward(
        &mut self,
        position: Vec3,
        final_look: bool,
    ) {
        // C++ `cameraModLookToward` / `cameraModFinalLookToward` no-op while rotating.
        if self.pending_camera_rotate.is_some() {
            return;
        }
        let mut applied = false;
        let mut path_final = false;
        if let Some(move_to) = self.script_camera_move_to.as_mut() {
            move_to.set_look_toward(position);
            applied = true;
        }
        if let Some(path) = self.script_camera_path.as_mut() {
            if final_look {
                path.camera_mod_final_look_toward(position);
                path_final = true;
            } else {
                path.camera_mod_look_toward(position);
            }
            applied = true;
        }
        if !applied {
            return;
        }
        self.pending_camera_rotate = None;
        if path_final {
            // Last-segment swing is applied as the path advances. Do not retarget
            // the whole remaining duration (C++ only rewrites last 1-2 waypoints).
            return;
        }
        let remaining = self.script_camera_remaining_seconds();
        self.pending_camera_look_toward = Some(CameraLookTowardWaypointRequest {
            position,
            duration_seconds: remaining,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
            reverse_rotation: false,
        });
    }

    pub(in super::super) fn apply_script_camera_mod_final_speed_multiplier(
        &mut self,
        request: &CameraModFinalSpeedMultiplierRequest,
    ) {
        let multiplier = request.multiplier as f32;
        let mut applied = false;
        if let Some(move_to) = self.script_camera_move_to.as_mut() {
            move_to.set_final_speed_multiplier(multiplier);
            applied = true;
        }
        if let Some(path) = self.script_camera_path.as_mut() {
            path.set_final_speed_multiplier(multiplier);
            applied = true;
        }
        if !applied {
            self.script_camera_pending_final_speed_multiplier = Some(multiplier.max(0.0));
        }
    }

    pub(in super::super) fn apply_script_camera_mod_rolling_average(
        &mut self,
        request: &CameraModRollingAverageRequest,
    ) {
        let frames = request.frames.max(1);
        if let Some(path) = self.script_camera_path.as_mut() {
            path.set_rolling_average_frames(frames);
        } else {
            self.script_camera_pending_rolling_average_frames = Some(frames);
        }
    }

    pub(in super::super) fn apply_visual_speed_multiplier(
        &mut self,
        request: &VisualSpeedMultiplierRequest,
    ) {
        let multiplier = request.multiplier.max(1) as f32;
        if multiplier.is_finite() {
            self.visual_speed_multiplier = multiplier;
        }
    }

    pub(in super::super) fn apply_set_fps_limit(&mut self, request: &SetFpsLimitRequest) {
        self.pending_script_fps_limit = Some(request.fps);
    }
    pub(in super::super) fn update_script_camera(&mut self, dt: f32) {
        self.tick_script_camera_orientation(dt);
        if let Some(object_id) = self.script_look_toward_object_id {
            if let Some(obj) = self.objects.get(&ObjectId(object_id)) {
                if let Some(look) = self.pending_camera_look_toward.as_mut() {
                    look.position = obj.get_position();
                    if look.duration_seconds > 0.0 {
                        look.duration_seconds = (look.duration_seconds - dt).max(0.0);
                    } else if self.script_look_toward_hold_seconds > 0.0 {
                        self.script_look_toward_hold_seconds =
                            (self.script_look_toward_hold_seconds - dt).max(0.0);
                    } else {
                        self.script_look_toward_object_id = None;
                    }
                }
            } else {
                self.script_look_toward_object_id = None;
            }
        }

        let move_step = self.script_camera_move_to.as_mut().map(|move_to| {
            if move_to.is_finished() {
                (true, move_to.final_focus(), false, None, 0.0)
            } else if let Some(focus) = move_to.advance(dt) {
                let look = if let Some(look) = move_to.look_toward() {
                    Some(look)
                } else if move_to.freeze_angle() || move_to.suppress_travel_look() {
                    None
                } else {
                    let dir = move_to.target - move_to.start;
                    Some(Vec3::new(focus.x + dir.x, focus.y, focus.z + dir.z))
                };
                (
                    false,
                    focus,
                    move_to.freeze_angle(),
                    look,
                    move_to.remaining_time_seconds(),
                )
            } else {
                (false, Vec3::ZERO, true, None, 0.0)
            }
        });
        if let Some((finished, focus, _freeze_angle, look, remaining)) = move_step {
            self.mission_scripts.set_camera_movement_finished(false);
            if finished {
                self.request_camera_focus(focus);
                self.script_camera_move_to = None;
                self.mark_script_camera_movement_maybe_finished();
                return;
            }
            if focus != Vec3::ZERO || look.is_some() {
                self.request_camera_focus(focus);
                if let Some(look) = look {
                    self.pending_camera_look_toward = Some(CameraLookTowardWaypointRequest {
                        position: look,
                        duration_seconds: remaining,
                        ease_in_seconds: 0.0,
                        ease_out_seconds: 0.0,
                        reverse_rotation: false,
                    });
                }
            }
            return;
        }

        let path_step = self.script_camera_path.as_mut().map(|path_move| {
            if path_move.is_finished() {
                (true, path_move.final_focus(), None, 0.0)
            } else if let Some(focus) = path_move.advance(dt) {
                let look = if let Some(look) = path_move.frozen_start_look_toward(focus) {
                    Some(look)
                } else if let Some(look) = path_move.look_toward_for_current_segment() {
                    Some(look)
                } else if path_move.freeze_angle() || path_move.suppress_travel_look() {
                    None
                } else {
                    path_move.travel_look_toward()
                };
                (
                    false,
                    focus,
                    look,
                    path_move.remaining_time_seconds().max(0.05),
                )
            } else {
                (false, Vec3::ZERO, None, 0.0)
            }
        });
        let Some((finished, focus, look, remaining)) = path_step else {
            if !self.is_script_camera_movement_finished_now() {
                self.mission_scripts.set_camera_movement_finished(false);
            }
            return;
        };
        self.mission_scripts.set_camera_movement_finished(false);
        if finished {
            self.request_camera_focus(focus);
            self.script_camera_path = None;
            self.mark_script_camera_movement_maybe_finished();
            return;
        }
        if focus != Vec3::ZERO || look.is_some() {
            self.request_camera_focus(focus);
            if let Some(look) = look {
                self.pending_camera_look_toward = Some(CameraLookTowardWaypointRequest {
                    position: look,
                    duration_seconds: remaining,
                    ease_in_seconds: 0.0,
                    ease_out_seconds: 0.0,
                    reverse_rotation: false,
                });
            }
        }
    }


    pub(in super::super) fn military_caption_duration_seconds(duration_ms: i32) -> f32 {
        (duration_ms as f32 / 1000.0).max(0.0)
    }
}

fn panel_flag_is_indestructible(flag: &str) -> bool {
    flag.chars()
        .filter(|c| !c.is_ascii_whitespace() && *c != '_')
        .collect::<String>()
        .eq_ignore_ascii_case("indestructible")
}

