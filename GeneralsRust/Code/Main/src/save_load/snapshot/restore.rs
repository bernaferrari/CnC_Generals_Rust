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

        // Occupants exist now. Re-apply ArmedRidersUpgradeMyWeaponSet so the
        // Listening Outpost dummy primary + PLAYER_UPGRADE survive load
        // (C++ TransportContain keeps that flag from the live rider list).
        let outpost_ids: Vec<ObjectId> = game_logic
            .host_objects()
            .iter()
            .filter(|(_, object)| object.is_listening_outpost_style_container())
            .map(|(id, _)| *id)
            .collect();
        for id in outpost_ids {
            game_logic.refresh_battle_bus_armed_riders_weapon_set(id);
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

        let mut object = Object::new_with_logic_frame(
            template,
            snapshot.id,
            snapshot.team,
            game_logic.get_current_frame() as u32,
        );
        // C++ Object::xfer (`Object.cpp:4068`) persists `m_name` independently
        // of the template. Constructor leaves it empty; the v11 world tail
        // writes the instance name after every object exists.
        object.name.clear();

        // C++ Object ctor instantiates template modules (TransportContain,
        // StealthUpdate, StealthDetectorUpdate) before those modules xfer
        // runtime (`Object.cpp` behaviors + `StealthDetectorUpdate.cpp:64-72`).
        // Live host identity is spawn-only `install_listening_outpost_transport`;
        // restore must reinstall before overlaying saved status/weapons so
        // stealth delay, dummy weapon, and occupants survive.
        if crate::game_logic::host_listening_outpost::is_listening_outpost_template(
            &snapshot.template_name,
        ) {
            object.install_listening_outpost_transport();
        }

        // Geometry / transform
        object.set_position(snapshot.geometry.position);
        object.set_orientation(snapshot.geometry.rotation);
        object.thing.geometry.bounds_min = snapshot.geometry.bounds_min;
        object.thing.geometry.bounds_max = snapshot.geometry.bounds_max;
        object.thing.geometry.radius = snapshot.geometry.radius;
        object.position = snapshot.geometry.position;
        // C++ Object ctor instantiates ChinookAIUpdate / TransportContain
        // before those modules xfer. Spawn-only install must run after pose
        // so default AI original_pos matches the saved hull, and before
        // weapon restore so Combat Chinook dummy-weapon wipe cannot clobber
        // the saved WeaponSet. Persist overlay then writes mid-drop state.
        if crate::game_logic::host_combat_chinook::is_combat_chinook_template(
            &snapshot.template_name,
        ) {
            object.install_combat_chinook_transport();
        } else if crate::game_logic::host_combat_chinook::is_regular_chinook_template(
            &snapshot.template_name,
        ) {
            object.install_chinook_transport();
        }

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
        // v8 carries the source-keyed temporary behavior ownership tail. A
        // missing legacy tail must not manufacture runtime state from a
        // template name; it remains an explicitly inactive bundle.
        if let Some(runtime) = &snapshot.temporary_weapon_runtime {
            if !runtime.matches_thing_template(&object.thing.template) {
                return Err(SaveLoadError::Corrupted(format!(
                    "Temporary Weapon runtime source mismatch for object {}",
                    snapshot.id
                )));
            }
            object.temporary_weapon_runtime = runtime.clone();
        } else {
            object.temporary_weapon_runtime =
                crate::game_logic::host_temporary_weapon_behavior::TemporaryWeaponRuntimeBundle::default();
        }
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
            // C++ SupplyWarehouseDockUpdate::loadPostProcess
            // updateDrawableSupplyStatus(startingBoxes, boxesStored).
            object.set_stored_supplies(runtime.stored_supply_boxes);
        }

        // C++ TempWeaponBonusHelper::xfer (`TempWeaponBonusHelper.cpp:112-113`)
        // writes m_currentBonus + m_frameToRemove. Absent/default tails stay
        // inactive (fail-closed).
        object.weapon_bonus_frenzy = snapshot.weapon_bonus_frenzy;
        object.weapon_bonus_frenzy_level = snapshot.weapon_bonus_frenzy_level;
        object.weapon_bonus_frenzy_until_frame = snapshot.weapon_bonus_frenzy_until_frame;

        self.restore_object_type_data(&snapshot.object_type, &mut object)?;
        self.restore_object_modules(&snapshot.modules, &mut object, game_logic)?;
        Self::rebuild_garrisoned_units_from_occupants(&mut object);

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
        object.status.unselectable = status.unselectable;
        object.status.deployed = status.deployed;
        object.status.disabled_script_disabled = status.disabled_script_disabled;
        object.status.disabled_script_underpowered = status.disabled_script_underpowered;
        object.script_unsellable = status.script_unsellable;
        object.script_unstealthed = status.script_unstealthed;
        // Wave 79 Drawable residual: restore StealthLook ordinal.
        object.camo_stealth_look = status.camo_stealth_look;
        // C++ StealthUpdate::xfer (`StealthUpdate.cpp:1127-1130`) persists
        // m_stealthAllowedFrame + m_detectionExpiresFrame. Without the expiry
        // frame, host update_stealth_and_detection requires >0 and DETECTED
        // never clears after load.
        object.detection_expires_frame = status.detection_expires_frame;
        object.stealth_allowed_frame = status.stealth_allowed_frame;
        object.status.disabled_paralyzed = status.disabled_paralyzed;
        object.status.disabled_paralyzed_until_frame = status.disabled_paralyzed_until_frame;
        object.status.spy_vision_disabled_until_frame = status.spy_vision_disabled_until_frame;
        object.status.spy_vision_reset_timers = status.spy_vision_reset_timers;
        object.status.spy_vision_hack_two_wake_frame = status.spy_vision_hack_two_wake_frame;
        object.status.parachuting = status.parachuting;
        object.status.parachute_open = status.parachute_open;
        object.status.parachute_start_height = status.parachute_start_height;
        object.status.parachute_pitch = status.parachute_pitch;
        object.status.parachute_roll = status.parachute_roll;
        object.status.parachute_pitch_rate = status.parachute_pitch_rate;
        object.status.parachute_roll_rate = status.parachute_roll_rate;
        object.status.parachute_landing_override = status.parachute_landing_override;
        object.status.parachute_landing_override_set = status.parachute_landing_override_set;
        object.status.faerie_fire = status.faerie_fire;
        object.faerie_fire_until_frame = status.faerie_fire_until_frame;
        object.status.disabled_held = status.disabled_held;

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
                            // C++ ProductionUpdate::update recomputes the integer
                            // threshold from UpgradeTemplate::calcTimeToBuild
                            // (m_buildTime * LOGICFRAMES_PER_SECOND). Upgrade.ini
                            // lives in UpgradeCenter, not the ThingTemplate catalog,
                            // so a catalog miss must not fall back to 30s.
                            let total_time = if entry.is_upgrade {
                                upgrade_production_restore_time_secs(&entry.template_name)
                            } else {
                                template.map(|t| t.build_time.max(0.1)).unwrap_or(30.0_f32)
                            };
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
                ModuleSnapshot::Contain(snapshot) => {
                    // Leftover TransportContain xfer already matches C++
                    // (`OpenContain.cpp:1574+`). Live host restore rebuilds a
                    // bare Object; reinstall identity without wiping restored
                    // weapons / stealth delay (those overlay in restore_object).
                    if snapshot.contain_type == "ListeningOutpost"
                        && !object.is_listening_outpost_style_container()
                    {
                        object.is_listening_outpost_transport = true;
                        object.passengers_allowed_to_fire = true;
                        object.armed_riders_upgrade_weapon_set = true;
                        object.is_detector = true;
                        object.detection_range = crate::game_logic::host_listening_outpost::LISTENING_OUTPOST_DETECTION_RANGE;
                        object.innate_stealth = true;
                        object.stealth_breaks_on_move = true;
                        object.stealth_breaks_on_attack = false;
                        if object.stealth_delay_frames == 0 {
                            object.stealth_delay_frames = crate::game_logic::host_listening_outpost::LISTENING_OUTPOST_STEALTH_DELAY_FRAMES;
                        }
                        object.thing.template.add_kind_of(KindOf::Attackable);
                        object.record_host_detector();
                        object.record_host_contain_capacity();
                        object.record_host_stealth_flags();
                    }
                    if snapshot.max_capacity > 0 {
                        object.max_transport = snapshot.max_capacity;
                        object.record_host_contain_capacity();
                    }
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
            ObjectTypeSnapshot::Unit(unit_snapshot) => {
                object.object_type = if object.is_kind_of(KindOf::Infantry) {
                    ObjectType::Infantry
                } else if object.is_kind_of(KindOf::Aircraft) {
                    ObjectType::Aircraft
                } else {
                    ObjectType::Vehicle
                };
                let formation_id = unit_snapshot.formation_id.unwrap_or(0);
                let offset = unit_snapshot
                    .formation_position
                    .map(|pos| glam::Vec2::new(pos.x, pos.y))
                    .unwrap_or(glam::Vec2::ZERO);
                object.set_formation(formation_id, offset);
                if object.movement.path.is_empty() && !unit_snapshot.waypoints.is_empty() {
                    object.movement.path = unit_snapshot.waypoints.clone();
                    object.movement.current_path_index = 0;
                    if object.movement.target_position.is_none() {
                        object.movement.target_position = unit_snapshot.waypoints.first().copied();
                    }
                }
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
                object.set_stored_supplies(resource_snapshot.amount);
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
            if game_logic.host_object(container_id).is_some() {
                if let Some(container) = game_logic.host_object_mut(container_id) {
                    if !container.occupants.contains(&snapshot.id) {
                        container.occupants.push(snapshot.id);
                    }
                    Self::rebuild_garrisoned_units_from_occupants(container);
                }
                if let Some(occupant) = game_logic.host_object_mut(snapshot.id) {
                    occupant.contained_by = Some(container_id);
                }
            } else {
                log::warn!(
                    "contain fixup orphan container={} occupant={}",
                    container_id.0,
                    snapshot.id.0
                );
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

            // C++ Player.cpp:2513-2539 addScience never sets m_cashBountyPercent.
            // Palace CashBountyPower modules re-apply after objects restore.
            let cash_bounty_percent = 0.0_f32;

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
                is_observer: false,
                did_preorder: false,
                statistics,
                power_sabotaged_till_frame: 0,
                color_rgb: (200, 200, 200),
                color_night_rgb: (200, 200, 200),

                start_position: -1,
                alliance_team: -1,
                cash_bounty_percent,
                // Recomputed from owned CommandCenter / RadarVan on next
                // update_player_radar residual pass (fail-closed restore).
                radar_count: 0,
                radar_disabled: false,
                disable_proof_radar_count: 0,
                logical_retaliation_mode_enabled: false,
                // Pre-v10 fallback. C++ Player::xfer (Player.cpp:4268-4275)
                // persists these; v10 `player_ranks` overwrites them after
                // this constructor when the world tail is present.
                rank_level: 1,
                skill_points: 0,
                science_purchase_points: 0,
                skill_points_modifier: 1.0,
                special_powers_used: 0,
                can_build_units: true,
                can_build_base: true,
                units_should_hunt: false,
                list_in_score_screen: true,

                kind_of_production_cost_changes: Vec::new(),
                shared_special_power_cooldowns: std::collections::HashMap::new(),
                // C++ Player::xfer m_upgradesCompleted. Stamp writes the name
                // list onto snap.upgrades; apply also re-adds after host registry.
                completed_upgrades: snap
                    .upgrades
                    .iter()
                    .filter(|name| !name.trim().is_empty())
                    .cloned()
                    .collect(),
                resource_supply_centers: Vec::new(),
                resource_supply_warehouses: Vec::new(),
                map_side: crate::game_logic::PlayerMapSideState::default(),
                team_relations: std::collections::HashMap::new(),
                team_instance_team_relations: std::collections::HashMap::new(),
                team_instance_player_relations: std::collections::HashMap::new(),
                sciences_disabled: std::collections::HashSet::new(),
                sciences_hidden: std::collections::HashSet::new(),
                attacked_by: [false; crate::game_logic::Player::MAX_ATTACKED_BY_PLAYERS],
                attacked_frame: 0,
            });
        }
        crate::save_load::apply_pending_player_team_chunks(game_logic);
        Ok(())
    }

    pub(super) fn restore_all_teams(
        &self,
        teams: &[TeamSnapshot],
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        // Leftover Team::xfer latches (created/active/see_enemy/destroy_threshold
        // / generic-script flags). Must not call set_active(created=true).
        let _ = teams;
        crate::save_load::apply_pending_player_team_chunks(game_logic);
        Ok(())
    }

    pub(super) fn restore_terrain(
        &self,
        terrain_snapshot: &TerrainSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        if terrain_snapshot.logic_width > 0
            && terrain_snapshot.logic_height > 0
            && !terrain_snapshot.logic_heights.is_empty()
        {
            if let Ok(mut terrain) = gamelogic::terrain::get_terrain_logic().write() {
                terrain.restore_logic_height_map(
                    terrain_snapshot.logic_width as i32,
                    terrain_snapshot.logic_height as i32,
                    &terrain_snapshot.logic_heights,
                );
            }
        }

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
                resource_obj.set_stored_supplies(depot.amount);
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
        super::player_upgrade_persist::apply_from_live_registry(game_logic);
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

    /// C++ `OpenContain::xfer` (`OpenContain.cpp:1590`) persist the contain
    /// list. Host HUD / capacity use `BuildingData.garrisoned_units`, which
    /// is a live mirror of `Object.occupants` and must be rebuilt on load.
    pub(super) fn rebuild_garrisoned_units_from_occupants(object: &mut Object) {
        if object.building_data.is_none() {
            if object.object_type != ObjectType::Building {
                return;
            }
            let building_type = BuildingType::from_template_name(&object.template_name);
            object.building_data = Some(BuildingData::new(building_type));
        }
        if let Some(building) = object.building_data.as_mut() {
            building.garrisoned_units = object.occupants.clone();
        }
    }

    /// C++ `Object::xfer` (`Object.cpp:4068`) `m_name` and
    /// `AIUpdateInterface::xfer` (`AIUpdate.cpp:5015-5019`) guard anchors.
    pub(super) fn restore_object_instance_guards(
        &self,
        snapshot: &WorldSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        if snapshot.version < WORLD_SNAPSHOT_DIRECT_XFER_V11_TAIL_VERSION {
            return Ok(());
        }
        let mut seen = HashSet::new();
        for entry in &snapshot.object_instance_guards {
            if !seen.insert(entry.object_id) {
                return Err(SaveLoadError::Corrupted(format!(
                    "Duplicate ObjectInstanceGuard snapshot for object {}",
                    entry.object_id
                )));
            }
            let Some(object) = game_logic.host_object_mut(entry.object_id) else {
                log::warn!(
                    "ObjectInstanceGuard snapshot references missing object {}",
                    entry.object_id
                );
                continue;
            };
            object.name = entry.instance_name.clone();
            // Direct field writes: `set_guard_position` / `set_guard_target`
            // would overwrite independently restored AIState.
            object.guard_position = entry.guard_position;
            object.guard_target = entry.guard_target;
            object.guard_radius = entry.guard_radius;
            object.guard_mode = entry.guard_mode;
        }
        Ok(())
    }

    pub(super) fn sync_all_garrisoned_units_from_occupants(&self, game_logic: &mut GameLogic) {
        let ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
        for id in ids {
            if let Some(object) = game_logic.host_object_mut(id) {
                Self::rebuild_garrisoned_units_from_occupants(object);
            }
        }
    }
}

/// C++ `UpgradeTemplate::calcTimeToBuild` source seconds for a restored
/// PRODUCTION_UPGRADE entry. Prefer leftover UpgradeCenter BuildTime (the
/// same store `CommandExecutor::resolve_upgrade_build_time_secs` uses), then
/// the retail Upgrade.ini residual so CamoNetting/Camouflage stay 5s/60s
/// even when the center has not been populated yet.
fn upgrade_production_restore_time_secs(upgrade_name: &str) -> f32 {
    let parsed_secs = gamelogic::upgrade::center::with_upgrade_center(|center| {
        center
            .find_upgrade(upgrade_name)
            .map(|template| template.get_build_time())
    });
    let fallback_secs = crate::game_logic::host_upgrades::HostUpgradeKind::from_name(upgrade_name)
        .retail_build_time_secs();
    parsed_secs
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
        .unwrap_or(fallback_secs)
        .max(1.0 / 30.0)
}
