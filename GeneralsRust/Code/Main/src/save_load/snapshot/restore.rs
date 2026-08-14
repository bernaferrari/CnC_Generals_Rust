//! SnapshotBuilder: restore live GameLogic from a WorldSnapshot.

use super::*;
use crate::game_logic::*;
use crate::save_load::{SaveLoadError, SaveLoadResult, Xfer, XferData, XferMode};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

impl SnapshotBuilder {
    // Private helper methods for snapshot restoration

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    pub(super) fn restore_all_objects(
        &self,
        objects: &HashMap<ObjectId, ObjectSnapshot>,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        game_logic.clear_all_objects();

        let mut ids: Vec<ObjectId> = objects.keys().cloned().collect();
        ids.sort();

        let mut max_id = 0u32;
        for id in ids {
            let snapshot = objects.get(&id).ok_or_else(|| {
                SaveLoadError::Corrupted(format!("Missing snapshot for object {}", id))
            })?;
            self.restore_object(snapshot, game_logic)?;
            max_id = max_id.max(id.0);
        }

        // Fix up container relationships once all objects exist.
        for snapshot in objects.values() {
            self.restore_object_references(snapshot, game_logic)?;
        }

        game_logic.set_next_object_id_for_restore(ObjectId(max_id.saturating_add(1)));
        // Loaded objects live in the host HashMap `host_authoritative_*` reads
        // when GameWorld is not coupled.
        Ok(())
    }

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    pub(super) fn restore_object(
        &self,
        snapshot: &ObjectSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        // Prefer catalog templates when present. Map-spawned objects often have a
        // matching entry after load_map, but mid-match loads into a fresh GameLogic
        // may not have the full INI catalog — synthesize a minimal template so
        // retail map saves remain restorable (production fail-open for catalog gaps).
        let template = if let Some(t) = game_logic.templates.get(snapshot.template_name.as_str()) {
            t.clone()
        } else {
            let mut t = ThingTemplate::new(&snapshot.template_name);
            t.set_health(snapshot.health.maximum.max(1.0));
            game_logic
                .templates
                .insert(snapshot.template_name.clone(), t.clone());
            log::debug!(
                "Synthesized template '{}' while restoring object {}",
                snapshot.template_name,
                snapshot.id
            );
            t
        };

        let mut object = Object::new(template, snapshot.id, snapshot.team);
        object.name = snapshot.template_name.clone();

        // Geometry / transform
        object.set_position(snapshot.geometry.position);
        object.set_orientation(snapshot.geometry.rotation);
        object.thing.geometry.bounds_min = snapshot.geometry.bounds_min;
        object.thing.geometry.bounds_max = snapshot.geometry.bounds_max;
        object.thing.geometry.radius = snapshot.geometry.radius;
        object.position = snapshot.geometry.position;

        // Core gameplay state
        self.restore_object_status(&snapshot.status, &mut object);
        object.selected = snapshot.status.selected;
        object.health = snapshot.health.clone();
        object.movement = snapshot.movement.clone();
        object.experience = snapshot.experience.clone();

        // Concrete WeaponSet layout: [0]=primary, [1]=secondary,
        // [2]=tertiary.  Zero-value pads preserve a later slot's identity.
        let (primary, secondary, tertiary) = Self::restore_object_weapons(&snapshot.weapons);
        object.weapon = primary;
        object.secondary_weapon = secondary;
        object.tertiary_weapon = tertiary;
        // v7 carries the C++ client-only SuspendFXFrame in a parallel object
        // tail.  Older streams intentionally leave the zero sentinel, while
        // a short/misaligned tail never manufactures a frame for a slot that
        // was not restored as a concrete Weapon.
        for slot in 0..3u8 {
            if let Some(&suspend_fx_frame) = snapshot.weapon_suspend_fx_frames.get(slot as usize) {
                object.restore_weapon_suspend_fx_frame_for_slot(slot, suspend_fx_frame);
            }
        }
        // v4 stores only the mutable C++ Weapon cursor. Authored cadence and
        // draw topology are rebuilt by Object; a saved multi-barrel cursor is
        // staged losslessly until that topology is validated, rather than
        // being normalized against the temporary one-barrel default.
        for (slot, state) in snapshot.weapon_barrel_states.iter().enumerate() {
            if object.weapon_slot(slot as u8).is_some() {
                object.restore_weapon_barrel_runtime_for_slot(
                    slot as u8,
                    state.current_barrel,
                    state.shots_left_on_barrel,
                );
            }
        }
        // Restore only the normalized accepted-discharge marker. A zero
        // sequence or malformed slot fails closed inside Object, so stale AI
        // fire-intent data can never become a post-load visual replay cue.
        object.restore_weapon_discharge_marker(
            snapshot.last_weapon_discharge_sequence,
            snapshot.last_weapon_discharge_slot,
            snapshot.last_weapon_discharge_barrel,
            snapshot.last_weapon_discharge_frame,
        );
        // Old saves can contain an active slot whose backing weapon did not
        // exist in that snapshot.  Do not let it fall through to primary.
        if object.weapon_slot(object.active_weapon_slot).is_none() {
            object.active_weapon_slot = object.first_available_weapon_slot().unwrap_or(0);
        }
        // Persisted lock state is only valid when it names a real restored
        // WeaponSet slot.  Unknown/old slot ordinals fail closed.
        if snapshot.status.weapon_lock_type != WeaponLockType::NotLocked
            && object
                .weapon_slot(snapshot.status.weapon_lock_slot)
                .is_some()
        {
            object.weapon_lock_type = snapshot.status.weapon_lock_type;
            object.weapon_lock_slot = snapshot.status.weapon_lock_slot;
            object.active_weapon_slot = snapshot.status.weapon_lock_slot;
        } else {
            object.weapon_lock_type = WeaponLockType::NotLocked;
            object.weapon_lock_slot = object.active_weapon_slot;
        }
        object.occupants = snapshot.contained_objects.clone();
        object.hacker_disable_channel = snapshot.hacker_disable_channel;

        if let Some(runtime) = &snapshot.collector_runtime {
            object.owner_player_id = runtime.owner_player_id;
            object.producer_id = runtime.producer_id;
            object.preferred_dock_id = runtime.preferred_dock_id;
            // Do not route snapshot restoration through `set_target`: the
            // live order setter intentionally moves a target-less object to
            // Idle, while the independently serialized AIState may be
            // SpecialAbility, Gathering, or ReturningResources.
            object.target = runtime.target;
            if runtime.target.is_none() {
                object.target_location = None;
            }
            object.supply_center_spawn_behavior_fired = runtime.supply_center_spawn_behavior_fired;
            object.supply_truck_state = runtime.supply_truck_state;
            object.supply_truck_force_pending = runtime.supply_truck_force_pending;
            object.supply_truck_next_dock_action_frame =
                runtime.supply_truck_next_dock_action_frame;
            object.stored_resources.supplies = runtime.stored_supply_boxes;
        }

        self.restore_object_type_data(&snapshot.object_type, &mut object)?;
        self.restore_object_modules(&snapshot.modules, &mut object, game_logic)?;

        // The generic pending-ability map is runtime-only.  Preserve the
        // source-side state now, but defer rebuilding that map until every
        // object has been inserted below.  Object snapshots are restored in
        // ID order, so validating a target here would incorrectly erase a
        // valid channel whose target happens to be restored later.
        if let Some(channel) = object.hacker_disable_channel {
            if object.thing.template.hacker_disable_building.is_some() && object.is_alive() {
                object.set_ai_state(AIState::SpecialAbility);
                object.set_order_target(Some(channel.target_id));
            } else {
                // A malformed/current save must not revive a guessed hacker
                // channel.  Clear it before the object becomes live.
                object.hacker_disable_channel = None;
                object.set_status_using_ability(false);
                object.set_target(None);
            }
        }

        game_logic.objects.insert(snapshot.id, object);
        Ok(())
    }

    /// Decode snapshot weapons vec into concrete primary / secondary / tertiary slots.
    ///
    /// Fail-closed residual layout:
    /// - empty → neither
    /// - [primary] → primary only (legacy saves)
    /// - [primary, secondary] → legacy two-slot save
    /// - [pad, secondary, tertiary] → missing primary with later slots intact
    pub(crate) fn restore_object_weapons(
        weapons: &[Weapon],
    ) -> (Option<Weapon>, Option<Weapon>, Option<Weapon>) {
        let slot = |index: usize| {
            weapons
                .get(index)
                .filter(|weapon| !Self::is_empty_weapon_slot_pad(weapon))
                .cloned()
        };
        (slot(0), slot(1), slot(2))
    }

    fn is_empty_weapon_slot_pad(weapon: &Weapon) -> bool {
        weapon.damage <= 0.0
            && weapon.range <= 0.0
            && weapon.min_range <= 0.0
            && weapon.reload_time <= 0.0
            && weapon.last_fire_time <= 0.0
            && weapon.ammo.is_none()
            && weapon.clip_size == 0
            && weapon.clip_reload_time <= 0.0
            && !weapon.can_target_air
            && !weapon.can_target_ground
            && weapon.projectile_speed <= 0.0
            && weapon.pre_attack_delay <= 0.0
            && weapon.splash_radius <= 0.0
    }

    pub(super) fn restore_object_status(&self, status: &ObjectStatusSnapshot, object: &mut Object) {
        object.status.destroyed = status.destroyed;
        object.status.under_construction = status.under_construction;
        object.status.moving = status.moving;
        object.status.attacking = status.attacking;
        object.status.airborne_target = status.airborne_target;
        object.status.stealthed = status.stealthed;
        object.status.detected = status.detected;
        object.status.selected = status.selected;
        object.status.disabled_underpowered = status.disabled_underpowered;
        object.status.disabled_unmanned = status.disabled_unmanned;
        object.status.disabled_hacked = status.disabled_hacked;
        object.status.disabled_hacked_until_frame = status.disabled_hacked_until_frame;
        object.status.disabled_emp = status.disabled_emp;
        object.status.disabled_emp_until_frame = status.disabled_emp_until_frame;
        object.status.weapons_jammed = status.weapons_jammed;
        object.status.disabled_subdued = status.disabled_subdued;
        object.status.is_carbomb = status.is_carbomb;
        object.status.hijacked = status.hijacked;
        // Wave 79 Drawable residual: restore StealthLook ordinal.
        object.camo_stealth_look = status.camo_stealth_look;

        object.ai_state = if status.destroyed {
            AIState::Idle
        } else if status.ai_state == AIState::Idle && status.garrisoned {
            AIState::Garrisoned
        } else if status.ai_state == AIState::Idle && status.being_repaired {
            AIState::SeekingRepair
        } else if status.ai_state == AIState::Idle && status.attacking {
            AIState::Attacking
        } else if status.ai_state == AIState::Idle && status.moving {
            AIState::Moving
        } else {
            status.ai_state.clone()
        };
        object.special_power_ready = status.special_power_ready;
        object.special_power_cooldown = status.special_power_cooldown;
        object.special_power_cooldown_remaining = status.special_power_cooldown_remaining;
        // Per-power map not in older snaps — seed from aggregate residual when cooling down.
        object.special_power_cooldowns.clear();
        if object.special_power_cooldown_remaining > 0.0 {
            // Unknown power key residual: keep aggregate only until next consume.
        }
        object.active_weapon_slot = status.active_weapon_slot;

        // Not represented in `ObjectStatus` in `Code/Main/src/game_logic/mod.rs`.
        let _ = status.on_fire;
        let _ = status.poisoned;
        let _ = status.radar_jammed;
    }

    pub(super) fn restore_object_modules(
        &self,
        modules: &HashMap<String, ModuleSnapshot>,
        object: &mut Object,
        game_logic: &GameLogic,
    ) -> SaveLoadResult<()> {
        for module_snapshot in modules.values() {
            match module_snapshot {
                ModuleSnapshot::Production(snapshot) => {
                    if object.building_data.is_none() {
                        let building_type = BuildingType::from_template_name(&object.template_name);
                        object.building_data = Some(BuildingData::new(building_type));
                    }

                    if let Some(building_data) = object.building_data.as_mut() {
                        building_data.rally_point = snapshot.rally_point;
                        building_data.exit_delay_remaining = snapshot.exit_delay_remaining.max(0.0);
                        building_data.exit_delay_remaining_frames =
                            snapshot.exit_delay_remaining_frames;
                        building_data.exit_burst_remaining = snapshot.exit_burst_remaining;
                        building_data.queue_exit_state_initialized =
                            snapshot.queue_exit_state_initialized;
                        building_data.production_queue.clear();

                        for (index, entry) in snapshot.production_queue.iter().enumerate() {
                            let template = game_logic.templates.get(&entry.template_name);
                            let total_time =
                                template.map(|t| t.build_time.max(0.1)).unwrap_or(30.0_f32);
                            let template_power = template.map(|t| t.build_cost.power).unwrap_or(0);

                            let mut progress = entry.progress.max(0.0);
                            if index == 0 && progress <= 0.0 && snapshot.production_progress > 0.0 {
                                progress =
                                    snapshot.production_progress.clamp(0.0, 1.0) * total_time;
                            }
                            progress = progress.min(total_time);

                            building_data.production_queue.push(ProductionItem {
                                template_name: entry.template_name.clone(),
                                progress,
                                total_time,
                                construction_frames: entry.construction_frames,
                                cost: Resources {
                                    supplies: entry.cost,
                                    power: template_power,
                                },
                                quantity_total: entry.quantity_total.max(1),
                                quantity_produced: entry
                                    .quantity_produced
                                    .min(entry.quantity_total.max(1)),
                                kind: if entry.is_upgrade {
                                    crate::game_logic::buildings::ProductionKind::Upgrade
                                } else {
                                    crate::game_logic::buildings::ProductionKind::Unit
                                },
                            });
                        }
                    }
                }
                ModuleSnapshot::Upgrade(snapshot) => {
                    object.applied_upgrades = snapshot
                        .active_upgrades
                        .iter()
                        .filter(|name| !name.trim().is_empty())
                        .cloned()
                        .collect();
                }
                _ => {}
            }
        }

        Ok(())
    }

    pub(super) fn restore_object_type_data(
        &self,
        object_type: &ObjectTypeSnapshot,
        object: &mut Object,
    ) -> SaveLoadResult<()> {
        match object_type {
            ObjectTypeSnapshot::Unit(_unit_snapshot) => {
                object.object_type = if object.is_kind_of(KindOf::Infantry) {
                    ObjectType::Infantry
                } else if object.is_kind_of(KindOf::Aircraft) {
                    ObjectType::Aircraft
                } else {
                    ObjectType::Vehicle
                };
                // Unit formation/waypoints aren't represented in `Code/Main` yet.
            }
            ObjectTypeSnapshot::Building(building_snapshot) => {
                object.object_type = ObjectType::Building;
                object.construction_percent = building_snapshot.construction_progress;
                object.power_provided = building_snapshot.power_provided;
                object.power_consumed = building_snapshot.power_required;
            }
            ObjectTypeSnapshot::Projectile(projectile_snapshot) => {
                object.object_type = ObjectType::Projectile;
                object.target = projectile_snapshot.target_object;
                object.target_location = Some(projectile_snapshot.target_position);
            }
            ObjectTypeSnapshot::Resource(resource_snapshot) => {
                object.object_type = if object.is_kind_of(KindOf::Resource)
                    || object.is_kind_of(KindOf::Harvestable)
                {
                    ObjectType::Supply
                } else {
                    ObjectType::Neutral
                };
                object.stored_resources.supplies = resource_snapshot.amount;
            }
        }

        Ok(())
    }

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    pub(super) fn restore_object_references(
        &self,
        snapshot: &ObjectSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        if let Some(container_id) = snapshot.container_object {
            if let Some(container) = game_logic.host_object_mut(container_id) {
                if !container.occupants.contains(&snapshot.id) {
                    container.occupants.push(snapshot.id);
                }
            }
        }

        // Rebuild the runtime-only pending command only after all object
        // snapshots exist.  A source-backed HDB channel may survive save/load
        // while approaching, unpacking, preparing, or packing; an absent or
        // dead target instead fails closed and cannot leave `IS_USING_ABILITY`
        // stuck on the restored source.
        if let Some(channel) = snapshot.hacker_disable_channel {
            let source_live = game_logic.host_object(snapshot.id).is_some_and(|source| {
                source.hacker_disable_channel == Some(channel)
                    && source.thing.template.hacker_disable_building.is_some()
                    && source.is_alive()
            });
            let target_live = game_logic
                .host_object(channel.target_id)
                .is_some_and(|target| target.is_alive());
            if source_live && target_live {
                game_logic.queue_pending_special_ability(
                    snapshot.id,
                    PendingSpecialAbility::HackerDisableBuilding {
                        target_id: channel.target_id,
                    },
                );
            } else if let Some(source) = game_logic.host_object_mut(snapshot.id) {
                source.hacker_disable_channel = None;
                source.set_status_using_ability(false);
                source.set_target(None);
                if source.ai_state == AIState::SpecialAbility {
                    source.set_ai_state(AIState::Idle);
                }
            }
        }
        Ok(())
    }

    pub(super) fn restore_all_players(
        &self,
        players: &[PlayerSnapshot],
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        game_logic.clear_all_players();
        for snap in players {
            let statistics = PlayerStatistics {
                units_built: snap.statistics.units_built,
                units_lost: snap.statistics.units_lost,
                structures_built: snap.statistics.buildings_built,
                structures_lost: snap.statistics.buildings_lost,
                resources_collected: snap.statistics.resources_gathered,
                ..PlayerStatistics::default()
            };

            let mut unlocked_sciences: std::collections::HashSet<String> =
                snap.tech_tree.unlocked_upgrades.iter().cloned().collect();
            unlocked_sciences.extend(snap.upgrades.iter().cloned());

            let mut queued_upgrades: HashSet<String> = snap
                .research_queue
                .iter()
                .filter(|name| !name.trim().is_empty())
                .cloned()
                .collect();
            queued_upgrades.extend(
                snap.tech_tree
                    .research_progress
                    .keys()
                    .filter(|name| !name.trim().is_empty())
                    .cloned(),
            );

            // Cash bounty residual: re-derive percent from unlocked sciences.
            let mut cash_bounty_percent = 0.0_f32;
            for sci in &unlocked_sciences {
                if let Some(pct) =
                    crate::game_logic::host_cash_bounty::cash_bounty_percent_for_science(sci)
                {
                    if pct > cash_bounty_percent {
                        cash_bounty_percent = pct;
                    }
                }
            }

            game_logic.add_player(Player {
                id: snap.id,
                team: snap.team,
                name: snap.name.clone(),
                resources: snap.resources,
                pending_supply_delta: 0,
                power_available: snap.resources.power,
                power_produced: 0,
                power_consumed: 0,
                // C++ Energy production is rebuilt after load.  In
                // particular, an active OverchargeBehavior replays its bonus
                // for the object's current controller in loadPostProcess.
                captured_overcharge_power_delta: 0,
                income_accumulator: 0.0,
                selected_objects: Vec::new(),
                unlocked_sciences,
                queued_upgrades,
                is_local: snap.is_human,
                is_alive: snap.is_active,
                did_preorder: false,
                statistics,
                power_sabotaged_till_frame: 0,
                color_rgb: (200, 200, 200),
                start_position: -1,
                alliance_team: -1,
                cash_bounty_percent,
                // Recomputed from owned CommandCenter / RadarVan on next
                // update_player_radar residual pass (fail-closed restore).
                radar_count: 0,
                radar_disabled: false,
                logical_retaliation_mode_enabled: false,
                rank_level: 1,
                skill_points: 0,
                science_purchase_points: 0,
                kind_of_production_cost_changes: Vec::new(),
                shared_special_power_cooldowns: std::collections::HashMap::new(),
            });
        }

        Ok(())
    }

    pub(super) fn restore_all_teams(
        &self,
        teams: &[TeamSnapshot],
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        // Teams are derived from players/objects in `Code/Main`; no separate state to restore yet.
        let _ = teams;
        let _ = game_logic;

        Ok(())
    }

    pub(super) fn restore_terrain(
        &self,
        terrain_snapshot: &TerrainSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        if terrain_snapshot.width == 0 || terrain_snapshot.height == 0 {
            return Ok(());
        }

        let expected_len =
            match (terrain_snapshot.width as usize).checked_mul(terrain_snapshot.height as usize) {
                Some(len) if len > 0 => len,
                _ => {
                    log::warn!(
                        "Skipping terrain restore due to invalid grid dimensions ({}x{})",
                        terrain_snapshot.width,
                        terrain_snapshot.height
                    );
                    return Ok(());
                }
            };

        if !terrain_snapshot.height_map.is_empty() {
            if terrain_snapshot.height_map.len() != expected_len {
                log::warn!(
                    "Skipping terrain height restore due to invalid snapshot payload ({}x{}, {} samples, expected {})",
                    terrain_snapshot.width,
                    terrain_snapshot.height,
                    terrain_snapshot.height_map.len(),
                    expected_len
                );
            } else if !game_logic.restore_terrain_heights_from_grid(
                terrain_snapshot.width,
                terrain_snapshot.height,
                &terrain_snapshot.height_map,
            ) {
                log::warn!(
                    "Skipping terrain height restore due to backend rejection ({}x{}, {} samples)",
                    terrain_snapshot.width,
                    terrain_snapshot.height,
                    terrain_snapshot.height_map.len()
                );
            }
        }

        if !terrain_snapshot.passability_map.is_empty() {
            if terrain_snapshot.passability_map.len() != expected_len {
                log::warn!(
                    "Skipping terrain passability restore due to invalid snapshot payload ({}x{}, {} cells, expected {})",
                    terrain_snapshot.width,
                    terrain_snapshot.height,
                    terrain_snapshot.passability_map.len(),
                    expected_len
                );
                return Ok(());
            }

            if !game_logic.restore_pathfinding_passability(
                terrain_snapshot.width,
                terrain_snapshot.height,
                &terrain_snapshot.passability_map,
            ) {
                log::warn!(
                    "Skipping terrain passability restore due to grid mismatch (snapshot {}x{}, map grid differs)",
                    terrain_snapshot.width,
                    terrain_snapshot.height
                );
            }
        }

        Ok(())
    }

    pub(super) fn restore_weather(
        &self,
        weather_snapshot: &WeatherSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        game_logic.set_weather_state(
            weather_snapshot.current_weather.clone(),
            weather_snapshot.weather_intensity,
            weather_snapshot.weather_duration,
            weather_snapshot.next_weather_change,
        );
        game_logic.set_weather_visible(weather_snapshot.visible);

        Ok(())
    }

    pub(super) fn restore_resource_manager(
        &self,
        resource_mgr_snapshot: &ResourceManagerSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        let mut resource_ids: Vec<ObjectId> = game_logic
            .host_objects()
            .iter()
            .filter_map(|(id, object)| Self::is_resource_source_object(object).then_some(*id))
            .collect();
        resource_ids.sort();

        let mut used = std::collections::HashSet::new();
        for depot in &resource_mgr_snapshot.supply_deposits {
            let mut best: Option<(ObjectId, f32)> = None;
            for resource_id in &resource_ids {
                if used.contains(resource_id) {
                    continue;
                }
                let Some(object) = game_logic.host_object(*resource_id) else {
                    continue;
                };
                let dist_sq = object.get_position().distance_squared(depot.position);
                match best {
                    Some((_, best_dist)) if dist_sq >= best_dist => {}
                    _ => best = Some((*resource_id, dist_sq)),
                }
            }

            let Some((resource_id, _)) = best else {
                log::warn!(
                    "No resource object available while restoring supply depot at {:?}",
                    depot.position
                );
                continue;
            };

            used.insert(resource_id);

            {
                let Some(resource_obj) = game_logic.host_object_mut(resource_id) else {
                    continue;
                };
                resource_obj.set_position(depot.position);
                resource_obj.position = depot.position;
                resource_obj.stored_resources.supplies = depot.amount;
                if resource_obj.object_type != ObjectType::Supply
                    && (resource_obj.is_kind_of(KindOf::Resource)
                        || resource_obj.is_kind_of(KindOf::Harvestable))
                {
                    resource_obj.object_type = ObjectType::Supply;
                }
            }

            for harvester_id in &depot.harvesters {
                if let Some(harvester) = game_logic.host_object_mut(*harvester_id) {
                    harvester.target = Some(resource_id);
                    if matches!(harvester.ai_state, AIState::Idle | AIState::Moving) {
                        harvester.ai_state = AIState::Gathering;
                    }
                }
            }
        }

        Ok(())
    }

    pub(super) fn restore_pathfinding_cache(
        &self,
        cache_snapshot: &PathfindingCacheSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        if cache_snapshot.cached_paths.is_empty() {
            return Ok(());
        }

        for object in game_logic.objects.values_mut() {
            if !object.movement.path.is_empty() {
                continue;
            }
            let Some(target_position) = object.movement.target_position else {
                continue;
            };

            let key = (
                SerializableVec3::from(object.get_position()),
                SerializableVec3::from(target_position),
            );
            let Some(cached_path) = cache_snapshot.cached_paths.get(&key) else {
                continue;
            };
            let restored_path: Vec<Vec3> = cached_path.iter().copied().map(Vec3::from).collect();
            if restored_path.len() < 2 {
                continue;
            }
            object.movement.path = restored_path;
            object.movement.current_path_index = 0;
            object.status.moving = true;
            if matches!(object.ai_state, AIState::Idle) {
                object.ai_state = AIState::Moving;
            }
        }

        Ok(())
    }

    pub(super) fn restore_combat_tracker(
        &self,
        combat_tracker_snapshot: &CombatTrackerSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        for combat in &combat_tracker_snapshot.active_combats {
            if game_logic.host_object(combat.attacker).is_none()
                || game_logic.host_object(combat.target).is_none()
            {
                continue;
            }

            if let Some(attacker) = game_logic.host_object_mut(combat.attacker) {
                attacker.target = Some(combat.target);
                attacker.status.attacking = true;
                if matches!(attacker.ai_state, AIState::Idle | AIState::Moving) {
                    attacker.ai_state = AIState::Attacking;
                }
            }
        }

        let sim_time = game_logic.get_current_frame() as f32 / 30.0;
        for death in &combat_tracker_snapshot.recent_deaths {
            if death.death_time > sim_time {
                continue;
            }
            if let Some(object) = game_logic.host_object_mut(death.object_id) {
                object.status.destroyed = true;
                object.health.current = 0.0;
                object.ai_state = AIState::Idle;
                object.target = None;
            }
        }

        Ok(())
    }

    pub(super) fn restore_special_power_strikes(
        &self,
        snapshot: &SpecialPowerStrikeRegistrySnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        game_logic
            .special_power_strikes_mut()
            .restore_from_snapshot_with_residuals(
                snapshot.next_id,
                snapshot.strikes.clone(),
                snapshot.next_radiation_id,
                snapshot.radiation_fields.clone(),
                snapshot.radiation_fields_spawned_total,
                snapshot.radiation_objects_spawned,
                snapshot.radiation_damage_applications_total,
                snapshot.next_toxin_id,
                snapshot.toxin_fields.clone(),
                snapshot.toxin_fields_spawned_total,
                snapshot.toxin_objects_spawned,
                snapshot.toxin_damage_applications_total,
                snapshot.next_orbit_id,
                snapshot.orbit_fields.clone(),
                snapshot.orbit_fields_spawned_total,
                snapshot.orbit_damage_applications_total,
                snapshot.next_beam_id,
                snapshot.beam_fields.clone(),
                snapshot.beam_fields_spawned_total,
                snapshot.beam_objects_spawned,
                snapshot.beam_damage_applications_total,
                snapshot.next_remnant_id,
                snapshot.remnant_fields.clone(),
                snapshot.remnant_fields_spawned_total,
                snapshot.remnant_objects_spawned,
                snapshot.remnant_damage_applications_total,
            );
        Ok(())
    }

    pub(super) fn restore_combat_particles(
        &self,
        snapshot: &CombatParticleRegistrySnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        game_logic
            .combat_particles_mut()
            .restore_from_snapshot(snapshot.next_id, snapshot.systems.clone());
        Ok(())
    }

    pub(super) fn restore_host_upgrades(
        &self,
        snapshot: &HostUpgradeRegistrySnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        game_logic
            .host_upgrades_mut()
            .restore_from_snapshot(snapshot.next_id, snapshot.entries.clone());
        Ok(())
    }

    pub(super) fn restore_experience_tracker(
        &self,
        exp_tracker_snapshot: &ExperienceTrackerSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        for event in &exp_tracker_snapshot.experience_events {
            if event.experience_gained <= 0.0 {
                continue;
            }
            if let Some(object) = game_logic.host_object_mut(event.object_id) {
                object.gain_experience(event.experience_gained.max(0.0));
            }
        }

        for (object_id, bonuses) in &exp_tracker_snapshot.veterancy_bonuses {
            let Some(object) = game_logic.host_object_mut(*object_id) else {
                continue;
            };

            let (_, min_experience) = Self::veterancy_level_from_bonus(bonuses.health_bonus);
            if object.experience.current < min_experience {
                object.experience.current = min_experience;
                object.gain_experience(0.0);
            }
        }

        Ok(())
    }

    /// Restore the serialized host skirmish-AI rows after players/objects are
    /// available.  Empty rows are an older-save compatibility case: retain the
    /// existing global-AI fallback instead of manufacturing per-player state.
    pub(super) fn restore_ai_players(
        &self,
        ai_players_snapshot: &[AIPlayerSnapshot],
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        if !ai_players_snapshot.is_empty() {
            game_logic.restore_host_ai_players_from_save(ai_players_snapshot);
        }
        Ok(())
    }

    // Historical full-AI restore sketch retained below for the unported
    // strategic/tactical planner objects.  The live host manager now restores
    // its bounded registered-player state above.
    // fn restore_ai_players(
    //     &self,
    //     ai_players_snapshot: &[AIPlayerSnapshot],
    //     game_logic: &mut GameLogic,
    // ) -> SaveLoadResult<()> {
    //     for ai_snapshot in ai_players_snapshot {
    //         let ai_player = game_logic.get_ai_player_mut(ai_snapshot.player_id)?;
    //
    //         ai_player.set_difficulty(&ai_snapshot.difficulty);
    //         ai_player.set_personality(&ai_snapshot.personality);
    //         ai_player.set_current_strategy(&ai_snapshot.current_strategy);
    //
    //         // Restore AI state components
    //         self.restore_ai_strategic_state(&ai_snapshot.strategic_state, ai_player)?;
    //         self.restore_ai_tactical_state(&ai_snapshot.tactical_state, ai_player)?;
    //         self.restore_ai_economic_state(&ai_snapshot.economic_state, ai_player)?;
    //     }
    //
    //     Ok(())
    // }

    // AI player strategic state restoration is disabled until AI state is wired into save/load.
    // fn restore_ai_strategic_state(
    //     &self,
    //     strategic_snapshot: &AIStrategicStateSnapshot,
    //     ai_player: &mut AIPlayer,
    // ) -> SaveLoadResult<()> {
    //     let strategic = ai_player.get_strategic_state_mut();
    //
    //     strategic.set_current_phase(&strategic_snapshot.current_phase);
    //
    //     for objective in &strategic_snapshot.objectives {
    //         strategic.add_objective(objective.clone());
    //     }
    //
    //     strategic.set_threat_assessment(strategic_snapshot.threat_assessment.clone());
    //
    //     Ok(())
    // }

    // AI player tactical state restoration is disabled until AI state is wired into save/load.
    // fn restore_ai_tactical_state(
    //     &self,
    //     tactical_snapshot: &AITacticalStateSnapshot,
    //     ai_player: &mut AIPlayer,
    // ) -> SaveLoadResult<()> {
    //     let tactical = ai_player.get_tactical_state_mut();
    //
    //     for group_snapshot in &tactical_snapshot.unit_groups {
    //         tactical.create_unit_group(
    //             group_snapshot.group_id,
    //             group_snapshot.units.clone(),
    //             &group_snapshot.role,
    //         );
    //     }
    //
    //     for attack_snapshot in &tactical_snapshot.active_attacks {
    //         tactical.register_attack(
    //             attack_snapshot.attack_id,
    //             attack_snapshot.target_position,
    //             attack_snapshot.assigned_groups.clone(),
    //         );
    //     }
    //
    //     Ok(())
    // }

    // AI player economic state restoration is disabled until AI state is wired into save/load.
    // fn restore_ai_economic_state(
    //     &self,
    //     economic_snapshot: &AIEconomicStateSnapshot,
    //     ai_player: &mut AIPlayer,
    // ) -> SaveLoadResult<()> {
    //     let economic = ai_player.get_economic_state_mut();
    //
    //     for priority in &economic_snapshot.build_priorities {
    //         economic.set_build_priority(priority.clone());
    //     }
    //
    //     economic.set_economic_focus(&economic_snapshot.economic_focus);
    //     economic.set_resource_allocation(economic_snapshot.resource_allocation.clone());
    //
    //     Ok(())
    // }

    pub(super) fn restore_global_ai_state(
        &self,
        global_ai_snapshot: &GlobalAIStateSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        let inferred_difficulty =
            Self::difficulty_from_modifiers(&global_ai_snapshot.difficulty_modifiers);

        let local_player_id = game_logic
            .get_players()
            .iter()
            .find_map(|(id, player)| player.is_local.then_some(*id))
            .unwrap_or(u32::MAX);
        game_logic.setup_skirmish_ai(local_player_id);

        let ai_player_ids: Vec<u32> = game_logic
            .get_players()
            .iter()
            .filter_map(|(id, player)| (!player.is_local).then_some(*id))
            .collect();

        for player_id in ai_player_ids {
            game_logic.set_ai_difficulty(player_id, inferred_difficulty);
        }

        Ok(())
    }
}
