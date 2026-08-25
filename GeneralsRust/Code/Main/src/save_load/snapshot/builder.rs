//! SnapshotBuilder: capture world state from live GameLogic.

use super::*;
use crate::game_logic::*;
use crate::save_load::{SaveLoadError, SaveLoadResult, Xfer, XferData, XferMode};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

/// Snapshot builder for creating world snapshots from current game state
pub struct SnapshotBuilder {
    // Access to game systems for snapshot creation
}

impl Default for SnapshotBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotBuilder {
    pub fn new() -> Self {
        Self {}
    }

    /// Create complete world snapshot from current game state
    pub fn create_world_snapshot(&self, game_logic: &GameLogic) -> SaveLoadResult<WorldSnapshot> {
        log::info!("Creating world snapshot from game state");

        // Snapshot all objects from game state
        let objects = self.snapshot_all_objects(game_logic)?;

        // Snapshot all players
        let mut players = self.snapshot_all_players(game_logic)?;
        super::player_upgrade_persist::stamp_completed_upgrades(&mut players, game_logic);

        // Create the world snapshot with actual game state
        let snapshot = WorldSnapshot {
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
            timestamp: std::time::SystemTime::now(),
            frame_number: game_logic.get_current_frame(),
            random_seed: 0, // Main crate GameLogic doesn't track random seed explicitly

            objects,
            players,
            teams: self.snapshot_all_teams(game_logic)?,
            terrain: self.snapshot_terrain(game_logic)?,
            weather: self.snapshot_weather(game_logic)?,
            resource_manager: self.snapshot_resource_manager(game_logic)?,
            combat_tracker: self.snapshot_combat_tracker(game_logic)?,
            experience_tracker: self.snapshot_experience_tracker(game_logic)?,
            pathfinding_cache: self.snapshot_pathfinding_cache(game_logic)?,
            ai_players: self.snapshot_ai_players(game_logic)?,
            global_ai_state: self.snapshot_global_ai_state(game_logic)?,
            special_power_strikes: self.snapshot_special_power_strikes(game_logic)?,
            combat_particles: self.snapshot_combat_particles(game_logic)?,
            host_upgrades: self.snapshot_host_upgrades(game_logic)?,
            // Direct GameLogic-owned allocator; zero is never a valid next
            // sequence and the getter preserves that invariant for v4 saves.
            next_weapon_discharge_sequence: game_logic
                .weapon_discharge_next_sequence_for_snapshot(),
            // SaveFileManager's logic-only entry point intentionally writes
            // this default. CnCGameEngine captures the renderer-owned DTO and
            // attaches it through the explicit companion-aware save API.
            client_drawables: ClientDrawableWorldSnapshot::default(),
            player_template_bindings: self.snapshot_player_template_bindings(game_logic)?,
            shroud: self.snapshot_shroud_state()?,
            lifecycle_tail: {
                let mut bytes = super::lifecycle_tail::encode_lifecycle_tail(
                    &super::lifecycle_tail::capture_lifecycle_tail(game_logic),
                );
                super::special_power_cooldown_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::battle_plan_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::subdual_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::hotkey_squad_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::booby_trap_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::carpet_bomb_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::production_door_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::dozer_repair_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::rebuild_hole_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::weapon_set_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::ability_hijack_persist::append_to_lifecycle_tail(&mut bytes, game_logic);

                super::ai_team_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::dock_queue_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::module_runtime_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::deliver_payload_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::object_module_xfer_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::auto_deposit_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::supply_drop_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::jet_ai_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::chinook_ai_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::hacker_income_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::warehouse_crippling_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::helix_napalm_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::money_crate_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::gps_scrambler_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::dynamic_shroud_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::angry_mob_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::power_plant_rods_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::cleanup_hazard_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::point_defense_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::projectile_stream_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::transport_exit_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::bridge_behavior_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::object_xfer_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::ai_player_queue_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::inferno_fire_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::firewall_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::neutron_slow_death_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::turret_aim_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::stealth_grant_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::weapon_leech_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::score_keeper_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::garrison_firepoint_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::stealth_detector_persist::append_to_lifecycle_tail(&mut bytes, game_logic);

                bytes
            },
            player_ranks: self.snapshot_player_ranks(game_logic)?,
            object_instance_guards: self.snapshot_object_instance_guards(game_logic),
            overcharge_active: self.snapshot_overcharge_active(game_logic),
            cia_intelligence: game_logic.cia_intelligence().clone(),
            vision_spied: self.snapshot_vision_spied(game_logic),
            builder_tasks: self.snapshot_builder_tasks(game_logic),
            sell_list: self.snapshot_sell_list(game_logic),
            object_persist: self.snapshot_object_persist(game_logic),
            client_drawable_visuals: self.snapshot_client_drawable_visuals(game_logic),
            player_energy: self.snapshot_player_energy(game_logic),
            object_triggers: self.snapshot_object_triggers(game_logic),
            is_scoring_enabled: gamelogic::helpers::TheGameLogic::is_scoring_enabled(),
            limit_superweapons: game_logic.skirmish_rules().limit_superweapons,
            cave_system: game_logic.cave_system_residual().clone(),
            tunnel_network: game_logic.tunnel_network_residual().clone(),
            airfield_parking: self.snapshot_airfield_parking(game_logic),
            persist_v18: super::persist_v18::capture_persist_v18(game_logic),
            object_experience_trackers: self.snapshot_object_experience_trackers(game_logic),
            object_command_sets: self.snapshot_object_command_sets(game_logic),
            object_disguises: self.snapshot_object_disguises(game_logic),
        };

        super::player_team_persist::stamp_from_live(game_logic);

        log::info!(
            "World snapshot complete: {} objects, {} players",
            snapshot.objects.len(),
            snapshot.players.len()
        );

        Ok(snapshot)
    }

    /// Capture the process-global PartitionManager equivalent. The host
    /// transaction keeps this singleton aligned with the active GameLogic;
    /// snapshotting the derived per-player presentation bytes here would lose
    /// the raw C++ looker/shrouder counters and pending undo queue.
    fn snapshot_shroud_state(
        &self,
    ) -> SaveLoadResult<gamelogic::system::shroud_manager::ShroudSnapshot> {
        gamelogic::system::shroud_manager::get_shroud_manager()
            .lock()
            .map(|manager| manager.snapshot_state())
            .map_err(|_| {
                SaveLoadError::Corrupted("ShroudManager lock poisoned while saving".to_string())
            })
    }

    /// Restore game state from world snapshot
    pub fn restore_from_snapshot(
        &self,
        snapshot: &WorldSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        log::info!(
            "Restoring world from snapshot: {} objects, {} players",
            snapshot.objects.len(),
            snapshot.players.len()
        );

        // Restore frame number
        game_logic.set_current_frame(snapshot.frame_number);

        // C++ Object.cpp:4218-4246 writes trigger slots after pose. Restore
        // HOST_TRIGGER_WORLD (including m_iPos) before recreate/set_position
        // so units already inside do not emit a fresh ENTERED_AREA edge.
        self.restore_object_triggers(snapshot);

        // C++ parity order: players/teams before objects, then world systems.
        self.restore_all_players(&snapshot.players, game_logic)?;
        super::player_team_persist::apply_pending(game_logic);
        self.restore_player_ranks(snapshot, game_logic)?;
        self.restore_player_energy(snapshot, game_logic)?;
        self.restore_player_template_bindings(snapshot, game_logic)?;
        self.restore_all_teams(&snapshot.teams, game_logic)?;
        self.restore_all_objects(&snapshot.objects, game_logic)?;
        self.restore_object_instance_guards(snapshot, game_logic)?;
        self.restore_overcharge_active(snapshot, game_logic)?;
        self.restore_object_experience_trackers(snapshot, game_logic)?;
        self.restore_object_command_sets(snapshot, game_logic)?;
        self.restore_object_disguises(snapshot, game_logic)?;

        self.restore_cia_vision_builder_sell(snapshot, game_logic)?;
        self.restore_object_persist(snapshot, game_logic)?;
        self.restore_client_drawable_visuals(snapshot, game_logic);

        self.restore_terrain(&snapshot.terrain, game_logic)?;
        self.restore_pathfinding_cache(&snapshot.pathfinding_cache, game_logic)?;
        self.restore_weather(&snapshot.weather, game_logic)?;
        self.restore_resource_manager(&snapshot.resource_manager, game_logic)?;
        self.restore_combat_tracker(&snapshot.combat_tracker, game_logic)?;
        self.restore_experience_tracker(&snapshot.experience_tracker, game_logic)?;
        self.restore_global_ai_state(&snapshot.global_ai_state, game_logic)?;
        self.restore_ai_players(&snapshot.ai_players, game_logic)?;
        self.restore_special_power_strikes(&snapshot.special_power_strikes, game_logic)?;
        self.restore_combat_particles(&snapshot.combat_particles, game_logic)?;
        self.restore_host_upgrades(&snapshot.host_upgrades, game_logic)?;
        super::player_upgrade_persist::apply_completed_upgrades(snapshot, game_logic);
        // The v4 counter is the next unused logical accepted-discharge ID.
        // The runtime setter clamps legacy/malformed zero to one and clears
        // transient presentation events so a load cannot replay pre-save FX.
        game_logic.restore_weapon_discharge_next_sequence(snapshot.next_weapon_discharge_sequence);

        // C++ `GameState.cpp:661,683` saveLock around load. Manager xfer
        // unlocks internally to recreate saved modules.
        save_lock_live_w3d_ghosts(true)?;
        if let Some(ghost_bytes) = take_loaded_w3d_ghost_xfer() {
            restore_w3d_ghost_manager_from_xfer_bytes(&ghost_bytes)?;
        }
        save_lock_live_w3d_ghosts(false)?;
        if let Some(client_bytes) = take_loaded_game_client_xfer() {
            restore_game_client_from_xfer_bytes(&client_bytes)?;
        }
        restore_objectless_from_client_drawables(&snapshot.client_drawables);
        if let Some(particle_bytes) = take_loaded_particle_system_xfer() {
            restore_particle_system_from_xfer_bytes(&particle_bytes)?;
        }
        if let Some(terrain_visual_bytes) = take_loaded_terrain_visual_xfer() {
            restore_terrain_visual_from_xfer_bytes(&terrain_visual_bytes)?;
        }

        // Map loading initializes a fresh shroud grid and may reveal
        // staging-map objects. Replace that singleton only when this save
        // actually carries the v6 shroud tail, after all object/team restore
        // callbacks have finished mutating candidate state.
        if snapshot.shroud.grid.is_some()
            || !snapshot.shroud.pending_undo_shroud_reveals.is_empty()
            || !snapshot.shroud.pending_full_reveal_players.is_empty()
            || !snapshot.shroud.pending_permanent_reveal_players.is_empty()
        {
            gamelogic::system::shroud_manager::get_shroud_manager()
                .lock()
                .map_err(|_| {
                    SaveLoadError::Corrupted(
                        "ShroudManager lock poisoned while restoring".to_string(),
                    )
                })?
                .replace_state(&snapshot.shroud)
                .map_err(SaveLoadError::Corrupted)?;
        }

        let tail = super::lifecycle_tail::decode_lifecycle_tail(&snapshot.lifecycle_tail)?;
        super::lifecycle_tail::apply_lifecycle_tail_to_host(&tail, game_logic)?;
        super::special_power_cooldown_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::battle_plan_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::subdual_persist::apply_from_lifecycle_tail(&snapshot.lifecycle_tail, game_logic)?;
        super::booby_trap_persist::apply_from_lifecycle_tail(&snapshot.lifecycle_tail, game_logic)?;
        super::carpet_bomb_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::production_door_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::dozer_repair_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::rebuild_hole_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::weapon_set_persist::apply_from_lifecycle_tail(&snapshot.lifecycle_tail, game_logic)?;
        super::ai_team_persist::apply_from_lifecycle_tail(&snapshot.lifecycle_tail, game_logic)?;
        super::ability_hijack_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::dock_queue_persist::apply_from_lifecycle_tail(&snapshot.lifecycle_tail, game_logic)?;
        super::module_runtime_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::deliver_payload_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::object_module_xfer_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::auto_deposit_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::supply_drop_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::jet_ai_persist::apply_from_lifecycle_tail(&snapshot.lifecycle_tail, game_logic)?;
        super::chinook_ai_persist::apply_from_lifecycle_tail(&snapshot.lifecycle_tail, game_logic)?;
        super::hacker_income_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::warehouse_crippling_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::helix_napalm_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::money_crate_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::gps_scrambler_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::dynamic_shroud_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::angry_mob_persist::apply_from_lifecycle_tail(&snapshot.lifecycle_tail, game_logic)?;
        super::power_plant_rods_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::cleanup_hazard_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::point_defense_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::projectile_stream_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::transport_exit_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::bridge_behavior_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::object_xfer_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::ai_player_queue_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::inferno_fire_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::firewall_persist::apply_from_lifecycle_tail(&snapshot.lifecycle_tail, game_logic)?;
        super::neutron_slow_death_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::turret_aim_persist::apply_from_lifecycle_tail(&snapshot.lifecycle_tail, game_logic)?;
        super::stealth_grant_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::weapon_leech_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::score_keeper_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;

        self.sync_all_garrisoned_units_from_occupants(game_logic);
        self.restore_game_logic_persist_tail(snapshot, game_logic);
        if snapshot.version >= WORLD_SNAPSHOT_DIRECT_XFER_V18_TAIL_VERSION {
            super::persist_v18::restore_persist_v18(&snapshot.persist_v18, game_logic);
        }
        super::hotkey_squad_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::garrison_firepoint_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::stealth_detector_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;

        log::info!("World restoration complete");
        Ok(())
    }

    fn snapshot_all_objects(
        &self,
        game_logic: &GameLogic,
    ) -> SaveLoadResult<HashMap<ObjectId, ObjectSnapshot>> {
        let mut objects = HashMap::new();

        for (id, object) in game_logic.host_objects() {
            match self.snapshot_object(game_logic, object) {
                Ok(snapshot) => {
                    objects.insert(*id, snapshot);
                }
                Err(e) => {
                    log::warn!("Failed to snapshot object {:?}: {}", id, e);
                }
            }
        }

        Ok(objects)
    }

    fn snapshot_object(
        &self,
        game_logic: &GameLogic,
        object: &Object,
    ) -> SaveLoadResult<ObjectSnapshot> {
        // Get player_id from team (simplified mapping)
        let player_id = match object.team {
            Team::USA => 0,
            Team::China => 1,
            Team::GLA => 2,
            Team::Neutral => 3,
        };

        // Snapshot the object's state
        let status = self.snapshot_object_status(game_logic, object);
        let object_type = self.snapshot_object_type(game_logic, object)?;
        let weapon_discharge_marker = object.weapon_discharge_marker();

        let weapons = Self::snapshot_object_weapons(object);
        let weapon_suspend_fx_frames = weapons
            .iter()
            .map(|weapon| weapon.suspend_fx_frame)
            .collect();
        let temporary_weapon_runtime = object
            .temporary_weapon_runtime
            .has_behavior_modules()
            .then(|| object.temporary_weapon_runtime.clone());

        Ok(ObjectSnapshot {
            id: object.id,
            template_name: object.template_name.clone(),
            team: object.team,
            player_id,
            geometry: GeometryInfo {
                position: object.get_position(),
                rotation: object.thing.geometry.rotation,
                bounds_min: object.thing.geometry.bounds_min,
                bounds_max: object.thing.geometry.bounds_max,
                radius: object.thing.geometry.radius,
            },
            status,
            // C++ Object::xfer uses getBodyModule()->getHealth() (the live
            // body store). When GameWorld is coupled, that is GW HP — not the
            // mid-frame HashMap field that can lag writeback.
            health: {
                let mut h = object.health.clone();
                if let Some(hp) = game_logic.host_authoritative_health(object.id) {
                    h.current = hp;
                }
                h
            },
            movement: object.movement.clone(),
            experience: object.experience.clone(),
            // Concrete C++ WeaponSet layout: PRIMARY=0, SECONDARY=1,
            // TERTIARY=2.  Missing earlier slots are represented by an
            // explicit zero-value pad so later slot identity survives load.
            weapons,
            contained_objects: object.occupants.clone(),
            container_object: object.contained_by,
            modules: self.snapshot_object_modules(object)?,
            object_type,
            hacker_disable_channel: object.hacker_disable_channel,
            weapon_barrel_states: std::array::from_fn(|slot| {
                object
                    .weapon_barrel_cursor_for_snapshot(slot as u8)
                    .map(
                        |(current_barrel, shots_left_on_barrel)| WeaponBarrelStateSnapshot {
                            current_barrel,
                            shots_left_on_barrel,
                        },
                    )
                    .unwrap_or_default()
            }),
            // The accepted-discharge marker is logical state, deliberately
            // independent of the older AI fire-intent presentation residual.
            // It names the exact pre-advance barrel a renderer may baseline.
            last_weapon_discharge_sequence: weapon_discharge_marker.sequence,
            last_weapon_discharge_slot: weapon_discharge_marker.weapon_slot,
            last_weapon_discharge_barrel: weapon_discharge_marker.fired_barrel,
            last_weapon_discharge_frame: weapon_discharge_marker.logic_frame,
            collector_runtime: Some(CollectorRuntimeSnapshot {
                owner_player_id: object.owner_player_id,
                producer_id: object.producer_id,
                preferred_dock_id: object.preferred_dock_id,
                target: object.target,
                supply_center_spawn_behavior_fired: object.supply_center_spawn_behavior_fired,
                supply_truck_state: object.supply_truck_state,
                supply_truck_force_pending: object.supply_truck_force_pending,
                supply_truck_next_dock_action_frame: object.supply_truck_next_dock_action_frame,
                stored_supply_boxes: object.stored_resources.supplies,
            }),
            weapon_suspend_fx_frames,
            temporary_weapon_runtime,
            weapon_bonus_frenzy: object.weapon_bonus_frenzy,
            weapon_bonus_frenzy_level: object.weapon_bonus_frenzy_level,
            weapon_bonus_frenzy_until_frame: object.weapon_bonus_frenzy_until_frame,
        })
    }

    /// Capture all concrete WeaponSet slots into the snapshot `weapons` vec.
    ///
    /// Index 0 = primary, 1 = secondary, 2 = tertiary. Runtime state such as
    /// `last_fire_time` / ammo must survive so combat does not desync after load.
    pub(crate) fn snapshot_object_weapons(object: &Object) -> Vec<Weapon> {
        let slots = [
            object.weapon.as_ref(),
            object.secondary_weapon.as_ref(),
            object.tertiary_weapon.as_ref(),
        ];
        let Some(last_present) = slots.iter().rposition(Option::is_some) else {
            return Vec::new();
        };

        let mut weapons = Vec::with_capacity(last_present + 1);
        for slot in slots.into_iter().take(last_present + 1) {
            weapons.push(slot.cloned().unwrap_or_else(Self::empty_weapon_slot_pad));
        }
        weapons
    }

    /// Snapshot-only marker for a missing lower WeaponSet slot.
    fn empty_weapon_slot_pad() -> Weapon {
        Weapon {
            damage: 0.0,
            range: 0.0,
            min_range: 0.0,
            reload_time: 0.0,
            last_fire_time: 0.0,
            ammo: None,
            clip_size: 0,
            clip_reload_time: 0.0,
            can_target_air: false,
            can_target_ground: false,
            projectile_speed: 0.0,
            pre_attack_delay: 0.0,
            splash_radius: 0.0,
            suspend_fx_frame: 0,
            reloading_clip: false,
            last_bonus_rof: 0.0,
        }
    }

    fn snapshot_object_status(
        &self,
        game_logic: &GameLogic,
        object: &Object,
    ) -> ObjectStatusSnapshot {
        ObjectStatusSnapshot {
            ai_state: object.ai_state.clone(),
            destroyed: object.status.destroyed,
            under_construction: game_logic
                .host_authoritative_construction(object.id)
                .map(|(_, uc)| uc)
                .unwrap_or(object.status.under_construction),
            selected: object.selected,
            moving: object.status.moving,
            attacking: object.status.attacking,
            airborne_target: object.status.airborne_target,
            stealthed: object.status.stealthed,
            detected: object.status.detected,
            garrisoned: matches!(object.ai_state, AIState::Garrisoned),
            being_repaired: matches!(object.ai_state, AIState::SeekingRepair),
            on_fire: false,
            poisoned: false,
            radar_jammed: false,
            disabled_underpowered: object.status.disabled_underpowered,
            disabled_unmanned: object.status.disabled_unmanned,
            disabled_hacked: object.status.disabled_hacked,
            disabled_hacked_until_frame: object.status.disabled_hacked_until_frame,
            disabled_emp: object.status.disabled_emp,
            disabled_emp_until_frame: object.status.disabled_emp_until_frame,
            weapons_jammed: object.status.weapons_jammed,
            disabled_subdued: object.status.disabled_subdued,
            is_carbomb: object.status.is_carbomb,
            hijacked: object.status.hijacked,
            special_power_ready: object.special_power_ready,
            special_power_cooldown: object.special_power_cooldown,
            special_power_cooldown_remaining: object.special_power_cooldown_remaining,
            active_weapon_slot: object.active_weapon_slot,
            weapon_lock_type: object.weapon_lock_type,
            weapon_lock_slot: object.weapon_lock_slot,
            camo_stealth_look: object.camo_stealth_look,
            detection_expires_frame: object.detection_expires_frame,
            stealth_allowed_frame: object.stealth_allowed_frame,
            unselectable: object.status.unselectable,
            deployed: object.status.deployed,
            disabled_script_disabled: object.status.disabled_script_disabled,
            disabled_script_underpowered: object.status.disabled_script_underpowered,
            script_unsellable: object.script_unsellable,
            script_unstealthed: object.script_unstealthed,
            disabled_paralyzed: object.status.disabled_paralyzed,
            disabled_paralyzed_until_frame: object.status.disabled_paralyzed_until_frame,
            spy_vision_disabled_until_frame: object.status.spy_vision_disabled_until_frame,
            spy_vision_reset_timers: object.status.spy_vision_reset_timers,
            spy_vision_hack_two_wake_frame: object.status.spy_vision_hack_two_wake_frame,
            parachuting: object.status.parachuting,
            parachute_open: object.status.parachute_open,
            parachute_start_height: object.status.parachute_start_height,
            parachute_pitch: object.status.parachute_pitch,
            parachute_roll: object.status.parachute_roll,
            parachute_pitch_rate: object.status.parachute_pitch_rate,
            parachute_roll_rate: object.status.parachute_roll_rate,
            parachute_landing_override: object.status.parachute_landing_override,
            parachute_landing_override_set: object.status.parachute_landing_override_set,
            faerie_fire: object.status.faerie_fire,
            faerie_fire_until_frame: object.faerie_fire_until_frame,
            disabled_held: object.status.disabled_held,
        }
    }

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    fn snapshot_object_modules(
        &self,
        object: &Object,
    ) -> SaveLoadResult<HashMap<String, ModuleSnapshot>> {
        let mut modules = HashMap::new();

        if let Some(building_data) = &object.building_data {
            let production_queue = building_data
                .production_queue
                .iter()
                .map(|item| ProductionQueueEntry {
                    template_name: item.template_name.clone(),
                    progress: item.progress,
                    construction_frames: item.construction_frames,
                    cost: item.cost.supplies,
                    quantity_total: item.quantity_total.max(1),
                    quantity_produced: item.quantity_produced,
                    is_upgrade: item.is_upgrade(),
                })
                .collect();

            modules.insert(
                "Production".to_string(),
                ModuleSnapshot::Production(ProductionModuleSnapshot {
                    production_queue,
                    is_producing: !building_data.production_queue.is_empty(),
                    production_progress: building_data.get_production_progress().unwrap_or(0.0),
                    rally_point: building_data.rally_point,
                    exit_delay_remaining: building_data.exit_delay_remaining,
                    exit_delay_remaining_frames: building_data.exit_delay_remaining_frames,
                    exit_burst_remaining: building_data.exit_burst_remaining,
                    queue_exit_state_initialized: building_data.queue_exit_state_initialized,
                }),
            );
        }

        if !object.applied_upgrades.is_empty() {
            let active_upgrades =
                Self::sorted_unique_strings(object.applied_upgrades.iter().cloned());
            modules.insert(
                "Upgrade".to_string(),
                ModuleSnapshot::Upgrade(UpgradeModuleSnapshot {
                    active_upgrades,
                    upgrade_progress: HashMap::new(),
                }),
            );
        }

        // C++ Object::xfer writes TransportContain / StealthUpdate /
        // StealthDetectorUpdate as template modules. Live host identity is
        // residual flags; persist the transport hull so load can reinstall
        // even if the template catalog is missing.
        if object.is_listening_outpost_style_container() {
            modules.insert(
                "Contain".to_string(),
                ModuleSnapshot::Contain(ContainModuleSnapshot {
                    contained_objects: object.occupants.clone(),
                    max_capacity: object.max_transport,
                    contain_type: "ListeningOutpost".to_string(),
                    exit_positions: Vec::new(),
                }),
            );
        }

        Ok(modules)
    }

    fn snapshot_object_type(
        &self,
        game_logic: &GameLogic,
        object: &Object,
    ) -> SaveLoadResult<ObjectTypeSnapshot> {
        // Determine object type from the object's type field
        match object.object_type {
            ObjectType::Infantry | ObjectType::Vehicle | ObjectType::Aircraft => {
                Ok(ObjectTypeSnapshot::Unit(UnitSnapshot {
                    unit_type: format!("{:?}", object.object_type),
                    formation_position: (object.formation_id != 0).then_some(glam::Vec3::new(
                        object.formation_offset.x,
                        object.formation_offset.y,
                        0.0,
                    )),
                    formation_id: (object.formation_id != 0).then_some(object.formation_id),
                    group_id: None,
                    waypoints: remaining_unit_waypoints(object),
                }))
            }
            ObjectType::Building => Ok(ObjectTypeSnapshot::Building(BuildingSnapshot {
                building_type: object.template_name.clone(),
                construction_progress: game_logic
                    .host_authoritative_construction(object.id)
                    .map(|(pct, _)| pct)
                    .unwrap_or(object.construction_percent),
                power_provided: object.power_provided,
                power_required: object.power_consumed,
                is_powered: object.power_provided >= object.power_consumed,
                connected_buildings: Vec::new(),
            })),
            ObjectType::Projectile => Ok(ObjectTypeSnapshot::Projectile(ProjectileSnapshot {
                projectile_type: object.template_name.clone(),
                source_object: object.id,
                target_object: object.target,
                target_position: object.target_location.unwrap_or(object.get_position()),
                flight_time: 0.0,
                max_flight_time: 1.0,
            })),
            ObjectType::Supply | ObjectType::Neutral => {
                Ok(ObjectTypeSnapshot::Resource(ResourceSnapshot {
                    resource_type: object.template_name.clone(),
                    amount: object.stored_resources.supplies,
                    depletion_rate: 0.0,
                    is_infinite: false,
                }))
            }
        }
    }

    fn snapshot_all_players(&self, game_logic: &GameLogic) -> SaveLoadResult<Vec<PlayerSnapshot>> {
        let mut players = Vec::new();
        let mut player_ids: Vec<u32> = game_logic.get_players().keys().copied().collect();
        player_ids.sort_unstable();

        for player_id in player_ids {
            let Some(player) = game_logic.get_player(player_id) else {
                continue;
            };
            let tech_tree = self.snapshot_tech_tree(player, game_logic)?;
            let snapshot = PlayerSnapshot {
                id: player.id,
                name: player.name.clone(),
                team: player.team,
                is_human: player.is_local,
                is_active: player.is_alive,
                resources: player.resources,
                population: PopulationInfo {
                    current: self.snapshot_population_used(game_logic, player.team),
                    maximum: 100,
                },
                tech_tree: tech_tree.clone(),
                upgrades: tech_tree.unlocked_upgrades.clone(),
                build_queue: self.snapshot_player_build_queue(game_logic, player.team),
                research_queue: Self::sorted_unique_strings(player.queued_upgrades.iter().cloned()),
                statistics: self.snapshot_player_statistics(player),
            };
            players.push(snapshot);
        }

        Ok(players)
    }

    fn snapshot_tech_tree(
        &self,
        player: &Player,
        game_logic: &GameLogic,
    ) -> SaveLoadResult<TechTreeSnapshot> {
        let mut unlocked_units = HashSet::new();
        let mut unlocked_buildings = HashSet::new();

        for object in game_logic.host_objects().values() {
            if object.team != player.team || !object.is_alive() {
                continue;
            }
            match object.object_type {
                ObjectType::Infantry | ObjectType::Vehicle | ObjectType::Aircraft => {
                    unlocked_units.insert(object.template_name.clone());
                }
                ObjectType::Building => {
                    unlocked_buildings.insert(object.template_name.clone());
                }
                _ => {}
            }
        }

        let unlocked_upgrades =
            Self::sorted_unique_strings(player.unlocked_sciences.iter().cloned());
        let mut research_progress = HashMap::new();
        for upgrade_name in Self::sorted_unique_strings(player.queued_upgrades.iter().cloned()) {
            research_progress.insert(upgrade_name, 0.0);
        }

        Ok(TechTreeSnapshot {
            unlocked_units: Self::sorted_unique_strings(unlocked_units),
            unlocked_buildings: Self::sorted_unique_strings(unlocked_buildings),
            unlocked_upgrades,
            research_progress,
        })
    }

    /// C++ `Player::xfer` (`Player.cpp:4268-4275`) persists `m_rankLevel`,
    /// `m_skillPoints`, and `m_sciencePurchasePoints`. Host `PlayerSnapshot`
    /// historically omitted them, so load reset rank 1 / 0 / 0. Keep the
    /// values as a world tail rather than mutating nested PlayerSnapshot.
    fn snapshot_player_ranks(
        &self,
        game_logic: &GameLogic,
    ) -> SaveLoadResult<Vec<PlayerRankSnapshot>> {
        let mut ranks = Vec::new();
        let mut player_ids: Vec<u32> = game_logic.get_players().keys().copied().collect();
        player_ids.sort_unstable();
        for player_id in player_ids {
            let Some(player) = game_logic.get_player(player_id) else {
                continue;
            };
            ranks.push(PlayerRankSnapshot {
                player_id,
                rank_level: player.rank_level,
                skill_points: player.skill_points,
                science_purchase_points: player.science_purchase_points,
            });
        }
        Ok(ranks)
    }

    /// C++ `Energy::xfer` v3 persists `m_powerSabotagedTillFrame`.
    fn snapshot_player_energy(&self, game_logic: &GameLogic) -> Vec<PlayerEnergySnapshot> {
        let mut energy = Vec::new();
        let mut player_ids: Vec<u32> = game_logic.get_players().keys().copied().collect();
        player_ids.sort_unstable();
        for player_id in player_ids {
            let Some(player) = game_logic.get_player(player_id) else {
                continue;
            };
            energy.push(PlayerEnergySnapshot {
                player_id,
                power_sabotaged_till_frame: player.power_sabotaged_till_frame,
            });
        }
        energy
    }

    /// C++ `Object::xfer` (`Object.cpp:4068`) and `AIUpdateInterface::xfer`
    /// (`AIUpdate.cpp:5015-5019`). World tail so nested object records stay
    /// aligned with v1-v10 streams.
    fn snapshot_object_instance_guards(
        &self,
        game_logic: &GameLogic,
    ) -> Vec<ObjectInstanceGuardSnapshot> {
        let mut ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
        ids.sort();
        let mut entries = Vec::with_capacity(ids.len());
        for id in ids {
            let Some(object) = game_logic.host_object(id) else {
                continue;
            };
            entries.push(ObjectInstanceGuardSnapshot {
                object_id: id,
                instance_name: object.name.clone(),
                guard_position: object.guard_position,
                guard_target: object.guard_target,
                guard_radius: object.guard_radius,
                guard_mode: object.guard_mode,
            });
        }
        entries
    }

    /// C++ OverchargeBehavior::xfer m_overchargeActive + loadPostProcess.
    fn snapshot_overcharge_active(&self, game_logic: &GameLogic) -> Vec<ObjectOverchargeSnapshot> {
        let mut ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
        ids.sort();
        let mut entries = Vec::new();
        for id in ids {
            let Some(object) = game_logic.host_object(id) else {
                continue;
            };
            if object.overcharge_enabled {
                entries.push(ObjectOverchargeSnapshot {
                    object_id: id,
                    overcharge_enabled: true,
                });
            }
        }
        entries
    }

    fn restore_overcharge_active(
        &self,
        snapshot: &WorldSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        if snapshot.version < WORLD_SNAPSHOT_DIRECT_XFER_V12_TAIL_VERSION {
            return Ok(());
        }
        let mut seen = HashSet::new();
        for entry in &snapshot.overcharge_active {
            if !seen.insert(entry.object_id) {
                return Err(SaveLoadError::Corrupted(format!(
                    "Duplicate Overcharge snapshot for object {}",
                    entry.object_id
                )));
            }
            let Some(object) = game_logic.host_object_mut(entry.object_id) else {
                log::warn!(
                    "Overcharge snapshot references missing object {}",
                    entry.object_id
                );
                continue;
            };
            // C++ loadPostProcess re-fires addPowerBonus when the flag is
            // true because Energy production was reconstructed from base
            // only. Live persists power_provided (already includes the
            // bonus), so only the module flag is restored.
            object.set_overcharge_enabled(entry.overcharge_enabled);
        }
        Ok(())
    }

    /// C++ `Object::xfer` `m_visionSpiedMask` (`Object.cpp:4126-4130`).
    fn snapshot_vision_spied(&self, game_logic: &GameLogic) -> Vec<ObjectVisionSpiedSnapshot> {
        let mut ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
        ids.sort();
        let mut entries = Vec::new();
        for id in ids {
            let Some(object) = game_logic.host_object(id) else {
                continue;
            };
            if object.vision_spied_mask != 0 {
                entries.push(ObjectVisionSpiedSnapshot {
                    object_id: id,
                    vision_spied_mask: object.vision_spied_mask,
                });
            }
        }
        entries
    }

    /// C++ `Object::xfer` `m_builderID` + Dozer BUILD task.
    fn snapshot_builder_tasks(&self, game_logic: &GameLogic) -> Vec<ObjectBuilderTaskSnapshot> {
        let mut ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
        ids.sort();
        let mut entries = Vec::new();
        for id in ids {
            let Some(object) = game_logic.host_object(id) else {
                continue;
            };
            if object.builder_id.is_none() && object.dozer_task_build_target.is_none() {
                continue;
            }
            entries.push(ObjectBuilderTaskSnapshot {
                object_id: id,
                builder_id: object.builder_id,
                dozer_task_build_target: object.dozer_task_build_target,
                dozer_task_build_order_frame: object.dozer_task_build_order_frame,
            });
        }
        entries
    }

    /// C++ `BuildAssistant::xferTheSellList` (id + sell frame).
    fn snapshot_sell_list(&self, game_logic: &GameLogic) -> Vec<SellListEntrySnapshot> {
        game_logic
            .sell_list_for_snapshot()
            .into_iter()
            .map(|(object_id, sell_frame)| SellListEntrySnapshot {
                object_id,
                sell_frame,
            })
            .collect()
    }

    /// C++ `Object::xfer` sole-heal / contain-frame / original team / formation.
    fn snapshot_object_persist(&self, game_logic: &GameLogic) -> Vec<ObjectPersistTailSnapshot> {
        let mut ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
        ids.sort();
        let mut entries = Vec::new();
        for id in ids {
            let Some(object) = game_logic.host_object(id) else {
                continue;
            };
            let contained_by_frame = game_logic.contained_by_frame_for_snapshot(id);
            let original_team = object
                .building_data
                .as_ref()
                .and_then(|building| building.original_team);
            let has_heal = object.sole_healing_benefactor.is_some()
                || object.sole_healing_benefactor_expiration_frame != 0;
            let has_formation = object.formation_id != 0;
            let has_decal = object.terrain_decal_type != 8 || object.terrain_decal_size != 0.0;
            let has_opacity = (object.camo_friendly_opacity - 1.0).abs() > f32::EPSILON;
            if !has_heal
                && contained_by_frame.is_none()
                && original_team.is_none()
                && !has_formation
                && !has_decal
                && !has_opacity
            {
                continue;
            }
            entries.push(ObjectPersistTailSnapshot {
                object_id: id,
                sole_healing_benefactor: object.sole_healing_benefactor,
                sole_healing_benefactor_expiration_frame: object
                    .sole_healing_benefactor_expiration_frame,
                contained_by_frame,
                original_team,
                formation_id: object.formation_id,
                formation_offset: [object.formation_offset.x, object.formation_offset.y],
                stealth_opacity: object.camo_friendly_opacity,
                terrain_decal_type: object.terrain_decal_type,
                terrain_decal_size: object.terrain_decal_size,
            });
        }
        entries
    }

    /// C++ `Object::xfer` trigger-area slots (`Object.cpp:4218-4246`).
    fn snapshot_object_triggers(
        &self,
        game_logic: &GameLogic,
    ) -> Vec<ObjectTriggerPersistSnapshot> {
        let mut entries: Vec<ObjectTriggerPersistSnapshot> =
            gamelogic::scripting::capture_host_object_trigger_persists()
                .into_iter()
                .map(|entry| ObjectTriggerPersistSnapshot {
                    object_id: ObjectId(entry.object_id),
                    i_x: entry.i_x,
                    i_y: entry.i_y,
                    entered_or_exited_frame: entry.entered_or_exited_frame,
                    slots: entry
                        .slots
                        .into_iter()
                        .map(|slot| ObjectTriggerSlotSnapshot {
                            trigger_id: slot.trigger_id,
                            trigger_name: slot.trigger_name,
                            is_inside: slot.is_inside,
                            entered: slot.entered,
                            exited: slot.exited,
                        })
                        .collect(),
                })
                .collect();
        let mut seen: HashSet<ObjectId> = entries.iter().map(|entry| entry.object_id).collect();
        let mut ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
        ids.sort();
        for id in ids {
            if !seen.insert(id) {
                continue;
            }
            let Some(object) = game_logic.host_object(id) else {
                continue;
            };
            let position = object.get_position();
            entries.push(ObjectTriggerPersistSnapshot {
                object_id: id,
                i_x: position.x as i32,
                i_y: position.z as i32,
                entered_or_exited_frame: 0,
                slots: Vec::new(),
            });
        }
        entries.sort_by_key(|entry| entry.object_id);
        entries
    }

    fn restore_object_triggers(&self, snapshot: &WorldSnapshot) {
        if snapshot.version < WORLD_SNAPSHOT_DIRECT_XFER_V16_TAIL_VERSION {
            return;
        }
        let entries: Vec<gamelogic::scripting::HostObjectTriggerPersist> = snapshot
            .object_triggers
            .iter()
            .map(|entry| gamelogic::scripting::HostObjectTriggerPersist {
                object_id: entry.object_id.0,
                i_x: entry.i_x,
                i_y: entry.i_y,
                entered_or_exited_frame: entry.entered_or_exited_frame,
                slots: entry
                    .slots
                    .iter()
                    .map(|slot| gamelogic::scripting::HostTriggerSlotPersist {
                        trigger_id: slot.trigger_id,
                        trigger_name: slot.trigger_name.clone(),
                        is_inside: slot.is_inside,
                        entered: slot.entered,
                        exited: slot.exited,
                    })
                    .collect(),
            })
            .collect();
        gamelogic::scripting::restore_host_object_trigger_persists(&entries);
    }

    fn restore_object_persist(
        &self,
        snapshot: &WorldSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        if snapshot.version < WORLD_SNAPSHOT_DIRECT_XFER_V14_TAIL_VERSION {
            return Ok(());
        }

        let mut seen = HashSet::new();
        let mut contain_frames = Vec::new();
        let mut max_formation_id = 0u32;
        for entry in &snapshot.object_persist {
            if !seen.insert(entry.object_id) {
                return Err(SaveLoadError::Corrupted(format!(
                    "Duplicate object persist snapshot for object {}",
                    entry.object_id
                )));
            }
            if let Some(frame) = entry.contained_by_frame {
                contain_frames.push((entry.object_id, frame));
            }
            let Some(object) = game_logic.host_object_mut(entry.object_id) else {
                log::warn!(
                    "Object persist snapshot references missing object {}",
                    entry.object_id
                );
                continue;
            };
            object.sole_healing_benefactor = entry.sole_healing_benefactor;
            object.sole_healing_benefactor_expiration_frame =
                entry.sole_healing_benefactor_expiration_frame;
            object.set_formation(
                entry.formation_id,
                glam::Vec2::new(entry.formation_offset[0], entry.formation_offset[1]),
            );
            max_formation_id = max_formation_id.max(entry.formation_id);
            object.camo_friendly_opacity = entry.stealth_opacity;
            object.terrain_decal_type = entry.terrain_decal_type;
            object.terrain_decal_size = entry.terrain_decal_size;
            if let Some(team) = entry.original_team {
                if object.building_data.is_none() {
                    object.building_data = Some(crate::game_logic::buildings::BuildingData::new(
                        crate::game_logic::buildings::BuildingType::Bunker,
                    ));
                }
                if let Some(building) = object.building_data.as_mut() {
                    building.original_team = Some(team);
                }
            }
        }
        game_logic.restore_contained_by_frames(&contain_frames);
        if max_formation_id != 0 {
            game_logic.set_next_formation_id_for_restore(max_formation_id.saturating_add(1));
        }
        Ok(())
    }

    /// C++ `ExperienceTracker::xfer` sink + scalar (ExperienceTracker.cpp:239-243).
    fn snapshot_object_experience_trackers(
        &self,
        game_logic: &GameLogic,
    ) -> Vec<ObjectExperienceTrackerSnapshot> {
        let mut ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
        ids.sort();
        let mut entries = Vec::new();
        for id in ids {
            let Some(object) = game_logic.host_object(id) else {
                continue;
            };
            let scalar = if object.experience_scalar.is_finite() {
                object.experience_scalar
            } else {
                1.0
            };
            if object.experience_sink.is_none() && (scalar - 1.0).abs() <= f32::EPSILON {
                continue;
            }
            entries.push(ObjectExperienceTrackerSnapshot {
                object_id: id,
                experience_sink: object.experience_sink,
                experience_scalar: scalar,
            });
        }
        entries
    }

    fn restore_object_experience_trackers(
        &self,
        snapshot: &WorldSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        if snapshot.version < WORLD_SNAPSHOT_DIRECT_XFER_V18_TAIL_VERSION
            && snapshot.object_experience_trackers.is_empty()
        {
            return Ok(());
        }
        let mut seen = HashSet::new();
        for entry in &snapshot.object_experience_trackers {
            if !seen.insert(entry.object_id) {
                return Err(SaveLoadError::Corrupted(format!(
                    "Duplicate experience tracker snapshot for object {}",
                    entry.object_id
                )));
            }
            let Some(object) = game_logic.host_object_mut(entry.object_id) else {
                log::warn!(
                    "Experience tracker snapshot references missing object {}",
                    entry.object_id
                );
                continue;
            };
            object.set_experience_sink(entry.experience_sink);
            object.set_experience_scalar(entry.experience_scalar);
        }
        Ok(())
    }

    /// C++ `Object::xfer` `m_commandSetStringOverride` (`Object.cpp:4403`).
    fn snapshot_object_command_sets(
        &self,
        game_logic: &GameLogic,
    ) -> Vec<ObjectCommandSetSnapshot> {
        let mut ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
        ids.sort();
        let mut entries = Vec::new();
        for id in ids {
            let Some(object) = game_logic.host_object(id) else {
                continue;
            };
            let Some(command_set) = object.command_set_override.as_ref() else {
                continue;
            };
            if command_set.is_empty() {
                continue;
            }
            entries.push(ObjectCommandSetSnapshot {
                object_id: id,
                command_set_override: command_set.clone(),
            });
        }
        entries
    }

    fn restore_object_command_sets(
        &self,
        snapshot: &WorldSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        if snapshot.version < WORLD_SNAPSHOT_DIRECT_XFER_V19_TAIL_VERSION
            && snapshot.object_command_sets.is_empty()
        {
            return Ok(());
        }
        let mut seen = HashSet::new();
        for entry in &snapshot.object_command_sets {
            if !seen.insert(entry.object_id) {
                return Err(SaveLoadError::Corrupted(format!(
                    "Duplicate command set snapshot for object {}",
                    entry.object_id
                )));
            }
            let Some(object) = game_logic.host_object_mut(entry.object_id) else {
                log::warn!(
                    "Command set snapshot references missing object {}",
                    entry.object_id
                );
                continue;
            };
            let override_name = if entry.command_set_override.is_empty() {
                None
            } else {
                Some(entry.command_set_override.clone())
            };
            object.set_command_set_override(override_name);
        }
        Ok(())
    }

    /// C++ `StealthUpdate::xfer` disguise identity + transition
    /// (`StealthUpdate.cpp:1141-1177`).
    fn snapshot_object_disguises(&self, game_logic: &GameLogic) -> Vec<ObjectDisguiseSnapshot> {
        let mut ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
        ids.sort();
        let mut entries = Vec::new();
        for id in ids {
            let Some(object) = game_logic.host_object(id) else {
                continue;
            };
            if !object.status.disguised
                && object.disguise_as_template.is_none()
                && object.disguise_pending_template.is_none()
                && object.status.disguise_transition_frames == 0
            {
                continue;
            }
            entries.push(ObjectDisguiseSnapshot {
                object_id: id,
                disguise_as_template: object.disguise_as_template.clone().unwrap_or_default(),
                disguise_as_team: disguise_team_to_u8(object.disguise_as_team),
                disguise_pending_template: object
                    .disguise_pending_template
                    .clone()
                    .unwrap_or_default(),
                disguise_pending_team: disguise_team_to_u8(object.disguise_pending_team),
                disguised: object.status.disguised,
                disguise_transition_frames: object.status.disguise_transition_frames,
                disguise_transitioning_to: object.status.disguise_transitioning_to,
                disguise_halfpoint_reached: object.status.disguise_halfpoint_reached,
            });
        }
        entries
    }

    fn restore_object_disguises(
        &self,
        snapshot: &WorldSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        if snapshot.version < WORLD_SNAPSHOT_DIRECT_XFER_V20_TAIL_VERSION
            && snapshot.object_disguises.is_empty()
        {
            return Ok(());
        }
        let mut seen = HashSet::new();
        for entry in &snapshot.object_disguises {
            if !seen.insert(entry.object_id) {
                return Err(SaveLoadError::Corrupted(format!(
                    "Duplicate disguise snapshot for object {}",
                    entry.object_id
                )));
            }
            let Some(object) = game_logic.host_object_mut(entry.object_id) else {
                log::warn!(
                    "Disguise snapshot references missing object {}",
                    entry.object_id
                );
                continue;
            };
            object.restore_disguise_from_save(
                if entry.disguise_as_template.is_empty() {
                    None
                } else {
                    Some(entry.disguise_as_template.clone())
                },
                disguise_team_from_u8(entry.disguise_as_team),
                if entry.disguise_pending_template.is_empty() {
                    None
                } else {
                    Some(entry.disguise_pending_template.clone())
                },
                disguise_team_from_u8(entry.disguise_pending_team),
                entry.disguised,
                entry.disguise_transition_frames,
                entry.disguise_transitioning_to,
                entry.disguise_halfpoint_reached,
            );
        }
        Ok(())
    }

    fn snapshot_client_drawable_visuals(
        &self,
        game_logic: &GameLogic,
    ) -> Vec<ClientDrawableVisualSnapshot> {
        let mut ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
        ids.sort();
        let mut entries = Vec::new();
        for id in ids {
            let Some(object) = game_logic.host_object(id) else {
                continue;
            };
            let hidden_by_stealth = object.camo_stealth_look == 5;
            let stealth_opacity = object.camo_friendly_opacity;
            let hidden = object.drawable_hidden;
            let loco_pitch = object.drawable_loco_pitch;
            let loco_roll = object.drawable_loco_roll;
            let expiration_date = object.drawable_expiration_date;
            if !hidden_by_stealth
                && !hidden
                && (stealth_opacity - 1.0).abs() <= f32::EPSILON
                && object.terrain_decal_type == 8
                && loco_pitch == 0.0
                && loco_roll == 0.0
                && expiration_date == 0
            {
                continue;
            }
            entries.push(ClientDrawableVisualSnapshot {
                object_id: id.0,
                draw_module_index: 0,
                hidden,
                hidden_by_stealth,
                stealth_opacity,
                effective_opacity: object.drawable_explicit_opacity * stealth_opacity,
                loco_pitch,
                loco_roll,
                expiration_date,
                terrain_decal: object.terrain_decal_type,
            });
        }
        entries
    }

    fn restore_client_drawable_visuals(
        &self,
        snapshot: &WorldSnapshot,
        game_logic: &mut GameLogic,
    ) {
        if snapshot.version < WORLD_SNAPSHOT_DIRECT_XFER_V14_TAIL_VERSION {
            return;
        }
        for entry in &snapshot.client_drawable_visuals {
            let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
                continue;
            };
            object.drawable_hidden = entry.hidden;
            if entry.hidden_by_stealth {
                object.camo_stealth_look = 5;
            }
            object.camo_friendly_opacity = entry.stealth_opacity;
            object.drawable_loco_pitch = entry.loco_pitch;
            object.drawable_loco_roll = entry.loco_roll;
            object.drawable_expiration_date = entry.expiration_date;
            object.terrain_decal_type = entry.terrain_decal;
        }
    }

    fn restore_cia_vision_builder_sell(
        &self,
        snapshot: &WorldSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        if snapshot.version < WORLD_SNAPSHOT_DIRECT_XFER_V13_TAIL_VERSION {
            return Ok(());
        }

        let mut seen_vision = HashSet::new();
        for entry in &snapshot.vision_spied {
            if !seen_vision.insert(entry.object_id) {
                return Err(SaveLoadError::Corrupted(format!(
                    "Duplicate vision-spied snapshot for object {}",
                    entry.object_id
                )));
            }
            let Some(object) = game_logic.host_object_mut(entry.object_id) else {
                log::warn!(
                    "Vision-spied snapshot references missing object {}",
                    entry.object_id
                );
                continue;
            };
            object.vision_spied_mask = entry.vision_spied_mask;
            object.record_host_vision_camo();
        }

        let mut seen_builder = HashSet::new();
        for entry in &snapshot.builder_tasks {
            if !seen_builder.insert(entry.object_id) {
                return Err(SaveLoadError::Corrupted(format!(
                    "Duplicate builder-task snapshot for object {}",
                    entry.object_id
                )));
            }
            let Some(object) = game_logic.host_object_mut(entry.object_id) else {
                log::warn!(
                    "Builder-task snapshot references missing object {}",
                    entry.object_id
                );
                continue;
            };
            object.builder_id = entry.builder_id;
            object.dozer_task_build_target = entry.dozer_task_build_target;
            object.dozer_task_build_order_frame = entry.dozer_task_build_order_frame;
        }

        game_logic.restore_cia_intelligence(snapshot.cia_intelligence.clone());
        let sell_entries: Vec<(ObjectId, u32)> = snapshot
            .sell_list
            .iter()
            .map(|entry| (entry.object_id, entry.sell_frame))
            .collect();
        game_logic.restore_sell_list_from_snapshot(&sell_entries);
        Ok(())
    }

    fn restore_player_ranks(
        &self,
        snapshot: &WorldSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        if snapshot.version < WORLD_SNAPSHOT_DIRECT_XFER_V10_TAIL_VERSION {
            return Ok(());
        }
        let mut seen_players = HashSet::new();
        for rank in &snapshot.player_ranks {
            if !seen_players.insert(rank.player_id) {
                return Err(SaveLoadError::Corrupted(format!(
                    "Duplicate PlayerRank snapshot for player {}",
                    rank.player_id
                )));
            }
            let Some(player) = game_logic.get_player_mut(rank.player_id) else {
                return Err(SaveLoadError::Corrupted(format!(
                    "PlayerRank snapshot references missing player {}",
                    rank.player_id
                )));
            };
            player.rank_level = rank.rank_level.max(1);
            player.skill_points = rank.skill_points;
            player.science_purchase_points = rank.science_purchase_points.max(0);
        }
        Ok(())
    }

    fn restore_player_energy(
        &self,
        snapshot: &WorldSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        if snapshot.version < WORLD_SNAPSHOT_DIRECT_XFER_V15_TAIL_VERSION {
            return Ok(());
        }
        let mut seen_players = HashSet::new();
        for energy in &snapshot.player_energy {
            if !seen_players.insert(energy.player_id) {
                return Err(SaveLoadError::Corrupted(format!(
                    "Duplicate PlayerEnergy snapshot for player {}",
                    energy.player_id
                )));
            }
            let Some(player) = game_logic.get_player_mut(energy.player_id) else {
                return Err(SaveLoadError::Corrupted(format!(
                    "PlayerEnergy snapshot references missing player {}",
                    energy.player_id
                )));
            };
            player.power_sabotaged_till_frame = energy.power_sabotaged_till_frame;
        }
        Ok(())
    }

    /// Capture canonical, indexed identities as a world tail instead of
    /// changing the historical positional `PlayerSnapshot` layout.  A stored
    /// name without its exact Common-store index would allow a reordered data
    /// set to load a plausible but incorrect General.
    fn snapshot_player_template_bindings(
        &self,
        game_logic: &GameLogic,
    ) -> SaveLoadResult<Vec<PlayerTemplateBindingSnapshot>> {
        let mut bindings = Vec::new();
        for (player_id, identity) in game_logic.player_template_identities_for_snapshot() {
            let template = identity.resolve().ok_or_else(|| {
                SaveLoadError::Corrupted(format!(
                    "Player {} has an invalid PlayerTemplate identity while saving",
                    player_id
                ))
            })?;
            game_engine::common::ini::ensure_player_templates_loaded();
            let store = game_engine::common::rts::player_template::get_player_template_store();
            let template_index = identity.template_index.or_else(|| {
                store
                    .find_template_index(template.get_name())
                    .map(|index| index as i32)
            });
            let Some(template_index) = template_index else {
                return Err(SaveLoadError::Corrupted(format!(
                    "Player {} PlayerTemplate '{}' has no store index while saving",
                    player_id,
                    template.get_name()
                )));
            };
            let indexed = store.get_nth_player_template_signed(template_index);
            if indexed.map(|candidate| candidate.get_name()) != Some(template.get_name()) {
                return Err(SaveLoadError::Corrupted(format!(
                    "Player {} PlayerTemplate '{}' no longer matches store index {} while saving",
                    player_id,
                    template.get_name(),
                    template_index
                )));
            }
            bindings.push(PlayerTemplateBindingSnapshot {
                player_id,
                template_name: template.get_name().to_string(),
                template_index,
            });
        }
        bindings.sort_by_key(|binding| binding.player_id);
        Ok(bindings)
    }

    /// Validate every v5 identity before installing any of them.  This keeps
    /// malformed or stale snapshot data from leaving a half-bound session.
    fn restore_player_template_bindings(
        &self,
        snapshot: &WorldSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        if snapshot.version < WORLD_SNAPSHOT_DIRECT_XFER_V5_TAIL_VERSION {
            return Ok(());
        }

        let mut identities = Vec::with_capacity(snapshot.player_template_bindings.len());
        let mut seen_players = HashSet::new();
        for binding in &snapshot.player_template_bindings {
            if !seen_players.insert(binding.player_id) {
                return Err(SaveLoadError::Corrupted(format!(
                    "Duplicate PlayerTemplate binding for player {}",
                    binding.player_id
                )));
            }
            let identity = PlayerTemplateIdentity::from_exact_indexed_name(
                &binding.template_name,
                binding.template_index,
            )
            .ok_or_else(|| {
                SaveLoadError::Corrupted(format!(
                    "Player {} has stale PlayerTemplate name/index ('{}', {})",
                    binding.player_id, binding.template_name, binding.template_index
                ))
            })?;
            let Some(player) = game_logic.get_player(binding.player_id) else {
                return Err(SaveLoadError::Corrupted(format!(
                    "PlayerTemplate binding references missing player {}",
                    binding.player_id
                )));
            };
            if identity.base_team() != Some(player.team) {
                return Err(SaveLoadError::Corrupted(format!(
                    "PlayerTemplate binding for player {} does not match restored team",
                    binding.player_id
                )));
            }
            identities.push((binding.player_id, identity));
        }

        for (player_id, identity) in identities {
            if !game_logic.install_restored_player_template_identity(player_id, identity) {
                return Err(SaveLoadError::Corrupted(format!(
                    "Could not install PlayerTemplate binding for player {}",
                    player_id
                )));
            }
        }
        Ok(())
    }

    fn snapshot_population_used(&self, game_logic: &GameLogic, team: Team) -> u32 {
        game_logic
            .host_objects()
            .values()
            .filter(|object| object.team == team && object.is_alive() && object.is_mobile())
            .count() as u32
    }

    fn snapshot_player_build_queue(&self, game_logic: &GameLogic, team: Team) -> Vec<String> {
        let mut object_ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
        object_ids.sort_by_key(|id| id.0);

        let mut build_queue = Vec::new();
        for object_id in object_ids {
            let Some(object) = game_logic.host_object(object_id) else {
                continue;
            };
            if object.team != team {
                continue;
            }
            let Some(building_data) = &object.building_data else {
                continue;
            };
            for item in &building_data.production_queue {
                build_queue.push(item.template_name.clone());
            }
        }

        build_queue
    }

    fn snapshot_player_statistics(&self, player: &Player) -> PlayerStatisticsSnapshot {
        PlayerStatisticsSnapshot {
            units_built: player.statistics.units_built,
            units_lost: player.statistics.units_lost,
            buildings_built: player.statistics.structures_built,
            buildings_lost: player.statistics.structures_lost,
            damage_dealt: 0.0, // Would need combat tracking
            damage_received: 0.0,
            resources_gathered: player.statistics.resources_collected,
            experience_gained: 0.0,
        }
    }

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    fn snapshot_all_teams(&self, game_logic: &GameLogic) -> SaveLoadResult<Vec<TeamSnapshot>> {
        // Teams are derived from players/objects in the current `Code/Main` model.
        // Mirror C++ behavior by snapshotting per-team membership (and leaving alliance state empty
        // until the diplomacy system is implemented).
        let mut by_team: HashMap<Team, Vec<u32>> = HashMap::new();

        for (&player_id, player) in game_logic.get_players().iter() {
            by_team.entry(player.team).or_default().push(player_id);
        }

        let team_order = [Team::USA, Team::China, Team::GLA, Team::Neutral];
        let mut snapshots = Vec::new();
        for team in team_order {
            let Some(players) = by_team.get(&team) else {
                continue;
            };
            let is_defeated = players
                .iter()
                .filter_map(|pid| game_logic.get_player(*pid))
                .all(|p| !p.is_alive);

            snapshots.push(TeamSnapshot {
                team,
                players: players.clone(),
                allied_teams: Vec::new(),
                is_defeated,
                shared_vision: false,
                shared_control: false,
            });
        }

        Ok(snapshots)
    }

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    fn snapshot_terrain(&self, _game_logic: &GameLogic) -> SaveLoadResult<TerrainSnapshot> {
        let (width, height, passability_map) = _game_logic.snapshot_pathfinding_passability();
        let height_map = _game_logic
            .snapshot_terrain_heights_for_path_grid()
            .unwrap_or_default();
        let (logic_width, logic_height, logic_heights) = gamelogic::terrain::get_terrain_logic()
            .read()
            .ok()
            .map(|terrain| {
                let (w, h) = terrain.logic_height_map_extents();
                (
                    w.max(0) as u32,
                    h.max(0) as u32,
                    terrain.logic_height_map_bytes().to_vec(),
                )
            })
            .unwrap_or((0, 0, Vec::new()));
        Ok(TerrainSnapshot {
            width,
            height,
            height_map,
            texture_map: Vec::new(),
            passability_map,
            modifications: Vec::new(),
            logic_width,
            logic_height,
            logic_heights,
        })
    }

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    fn snapshot_weather(&self, _game_logic: &GameLogic) -> SaveLoadResult<WeatherSnapshot> {
        let weather = _game_logic.weather_state();
        Ok(WeatherSnapshot {
            current_weather: weather.current_weather.clone(),
            weather_intensity: weather.intensity,
            weather_duration: weather.duration_remaining,
            next_weather_change: weather.next_change_time,
            visible: weather.visible,
        })
    }

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    fn snapshot_resource_manager(
        &self,
        _game_logic: &GameLogic,
    ) -> SaveLoadResult<ResourceManagerSnapshot> {
        let mut resource_ids: Vec<ObjectId> = _game_logic
            .host_objects()
            .iter()
            .filter_map(|(id, object)| Self::is_resource_source_object(object).then_some(*id))
            .collect();
        resource_ids.sort();

        let mut supply_deposits = Vec::new();
        for resource_id in resource_ids {
            let Some(resource) = _game_logic.host_object(resource_id) else {
                continue;
            };

            let harvesters = _game_logic
                .host_objects()
                .iter()
                .filter_map(|(id, object)| {
                    (object.target == Some(resource_id)
                        && (object.ai_state == AIState::Gathering || object.is_worker()))
                    .then_some(*id)
                })
                .collect();

            supply_deposits.push(SupplyDepositSnapshot {
                position: resource.get_position(),
                amount: resource.stored_resources.supplies,
                depletion_rate: 0.0,
                harvesters,
            });
        }

        Ok(ResourceManagerSnapshot {
            supply_deposits,
            resource_zones: Vec::new(),
        })
    }

    fn snapshot_pathfinding_cache(
        &self,
        game_logic: &GameLogic,
    ) -> SaveLoadResult<PathfindingCacheSnapshot> {
        let mut cached_paths: HashMap<(SerializableVec3, SerializableVec3), Vec<SerializableVec3>> =
            HashMap::new();
        let mut cache_timestamps: HashMap<(SerializableVec3, SerializableVec3), f32> =
            HashMap::new();

        let now = game_logic.get_current_frame() as f32 / 30.0;
        for object in game_logic.host_objects().values() {
            if object.movement.path.len() < 2 {
                continue;
            }
            let Some(target_position) = object
                .movement
                .target_position
                .or_else(|| object.movement.path.last().copied())
            else {
                continue;
            };

            let key = (
                SerializableVec3::from(object.get_position()),
                SerializableVec3::from(target_position),
            );

            let path: Vec<SerializableVec3> = object
                .movement
                .path
                .iter()
                .copied()
                .map(SerializableVec3::from)
                .collect();
            if path.len() < 2 {
                continue;
            }

            let should_replace = match cached_paths.get(&key) {
                Some(existing) => path.len() > existing.len(),
                None => true,
            };
            if should_replace {
                cached_paths.insert(key, path);
                cache_timestamps.insert(key, now);
            }
        }

        Ok(PathfindingCacheSnapshot {
            cached_paths,
            cache_timestamps,
        })
    }

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    fn snapshot_combat_tracker(
        &self,
        _game_logic: &GameLogic,
    ) -> SaveLoadResult<CombatTrackerSnapshot> {
        let sim_time = _game_logic.get_current_frame() as f32 / 30.0;

        let mut active_combats = Vec::new();
        for (&attacker_id, attacker) in _game_logic.host_objects() {
            if !attacker.is_alive() {
                continue;
            }
            let Some(target_id) = attacker.target else {
                continue;
            };
            let Some(target) = _game_logic.host_object(target_id) else {
                continue;
            };
            if !target.is_alive() {
                continue;
            }
            if !attacker.status.attacking
                && !matches!(
                    attacker.ai_state,
                    AIState::Attacking | AIState::AttackMoving | AIState::GuardingObject
                )
            {
                continue;
            }

            active_combats.push(ActiveCombatSnapshot {
                attacker: attacker_id,
                target: target_id,
                start_time: sim_time,
                damage_dealt: attacker.weapon.as_ref().map(|w| w.damage).unwrap_or(0.0),
            });
        }

        let mut recent_deaths = Vec::new();
        for (&object_id, object) in _game_logic.host_objects() {
            if !object.status.destroyed {
                continue;
            }
            recent_deaths.push(DeathEventSnapshot {
                object_id,
                killer_id: None,
                death_time: sim_time,
                death_position: object.get_position(),
            });
        }

        Ok(CombatTrackerSnapshot {
            active_combats,
            recent_deaths,
        })
    }

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    fn snapshot_experience_tracker(
        &self,
        _game_logic: &GameLogic,
    ) -> SaveLoadResult<ExperienceTrackerSnapshot> {
        let sim_time = _game_logic.get_current_frame() as f32 / 30.0;
        let mut experience_events = Vec::new();
        let mut veterancy_bonuses = HashMap::new();

        for (&object_id, object) in _game_logic.host_objects() {
            if object.experience.current <= 0.0 && object.experience.level == VeterancyLevel::Rookie
            {
                continue;
            }

            experience_events.push(ExperienceEventSnapshot {
                object_id,
                experience_gained: object.experience.current,
                source: "snapshot_state".to_string(),
                timestamp: sim_time,
            });
            veterancy_bonuses.insert(
                object_id,
                Self::veterancy_bonuses_for_level(object.experience.level),
            );
        }

        Ok(ExperienceTrackerSnapshot {
            experience_events,
            veterancy_bonuses,
        })
    }

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    fn snapshot_global_ai_state(
        &self,
        _game_logic: &GameLogic,
    ) -> SaveLoadResult<GlobalAIStateSnapshot> {
        let difficulty = _game_logic.get_difficulty();

        let mut global_timers = HashMap::new();
        global_timers.insert(
            "sim_time_seconds".to_string(),
            _game_logic.get_current_frame() as f32 / 30.0,
        );
        global_timers.insert(
            "logic_frame".to_string(),
            _game_logic.get_current_frame() as f32,
        );

        let mut global_flags = HashMap::new();
        global_flags.insert("battle_active".to_string(), _game_logic.is_in_battle());

        Ok(GlobalAIStateSnapshot {
            global_timers,
            global_flags,
            difficulty_modifiers: DifficultyModifiers {
                ai_resource_bonus: difficulty.get_resource_bonus(),
                ai_damage_bonus: difficulty.get_aggression_factor(),
                ai_health_bonus: match difficulty {
                    crate::ai::AIDifficulty::Easy => 0.9,
                    crate::ai::AIDifficulty::Medium => 1.0,
                    crate::ai::AIDifficulty::Hard => 1.2,
                    crate::ai::AIDifficulty::Brutal => 1.4,
                },
                ai_build_speed_bonus: 1.0 / difficulty.get_build_delay_modifier(),
            },
        })
    }

    /// Capture the host skirmish-AI rows that own offline build/attack
    /// decisions.  This intentionally excludes transient pathfinder caches;
    /// those are rebuilt from the restored objects and terrain.
    fn snapshot_ai_players(&self, game_logic: &GameLogic) -> SaveLoadResult<Vec<AIPlayerSnapshot>> {
        Ok(game_logic.snapshot_host_ai_players_for_save())
    }

    /// Capture pending/completed host superweapon strikes so mid-flight loads
    /// still impact after remaining delay frames elapse.
    fn snapshot_special_power_strikes(
        &self,
        game_logic: &GameLogic,
    ) -> SaveLoadResult<SpecialPowerStrikeRegistrySnapshot> {
        let reg = game_logic.special_power_strikes();
        Ok(SpecialPowerStrikeRegistrySnapshot {
            next_id: reg.next_id(),
            strikes: reg.strikes_snapshot(),
            next_radiation_id: reg.next_radiation_id(),
            radiation_fields: reg.radiation_fields().to_vec(),
            radiation_fields_spawned_total: reg.radiation_fields_spawned_total(),
            radiation_objects_spawned: reg.radiation_objects_spawned(),
            radiation_damage_applications_total: reg.radiation_damage_applications_total(),
            next_toxin_id: reg.next_toxin_id(),
            toxin_fields: reg.toxin_fields().to_vec(),
            toxin_fields_spawned_total: reg.toxin_fields_spawned_total(),
            toxin_objects_spawned: reg.toxin_objects_spawned(),
            toxin_damage_applications_total: reg.toxin_damage_applications_total(),
            next_orbit_id: reg.next_orbit_id(),
            orbit_fields: reg.orbit_fields().to_vec(),
            orbit_fields_spawned_total: reg.orbit_fields_spawned_total(),
            orbit_damage_applications_total: reg.orbit_damage_applications_total(),
            next_beam_id: reg.next_beam_id(),
            beam_fields: reg.beam_fields().to_vec(),
            beam_fields_spawned_total: reg.beam_fields_spawned_total(),
            beam_objects_spawned: reg.beam_objects_spawned(),
            beam_damage_applications_total: reg.beam_damage_applications_total(),
            next_remnant_id: reg.next_remnant_id(),
            remnant_fields: reg.remnant_fields().to_vec(),
            remnant_fields_spawned_total: reg.remnant_fields_spawned_total(),
            remnant_objects_spawned: reg.remnant_objects_spawned(),
            remnant_damage_applications_total: reg.remnant_damage_applications_total(),
        })
    }

    /// Capture host combat particle registry residual (not full GPU particles).
    fn snapshot_combat_particles(
        &self,
        game_logic: &GameLogic,
    ) -> SaveLoadResult<CombatParticleRegistrySnapshot> {
        let reg = game_logic.combat_particles();
        Ok(CombatParticleRegistrySnapshot {
            next_id: reg.next_id(),
            systems: reg.systems_snapshot(),
        })
    }

    /// Capture pending/completed host upgrade research so mid-flight loads
    /// still complete with unlocks after restore.
    fn snapshot_host_upgrades(
        &self,
        game_logic: &GameLogic,
    ) -> SaveLoadResult<HostUpgradeRegistrySnapshot> {
        let reg = game_logic.host_upgrades();
        Ok(HostUpgradeRegistrySnapshot {
            next_id: reg.next_id(),
            entries: reg.entries_snapshot(),
        })
    }

    pub(super) fn is_resource_source_object(object: &Object) -> bool {
        object.object_type == ObjectType::Supply
            || object.is_kind_of(KindOf::Resource)
            || object.is_kind_of(KindOf::Harvestable)
            || object.template_name.to_ascii_lowercase().contains("supply")
    }

    pub(super) fn veterancy_bonuses_for_level(level: VeterancyLevel) -> VeterancyBonuses {
        match level {
            VeterancyLevel::Rookie => VeterancyBonuses {
                health_bonus: 1.0,
                damage_bonus: 1.0,
                accuracy_bonus: 1.0,
                range_bonus: 1.0,
            },
            VeterancyLevel::Veteran => VeterancyBonuses {
                health_bonus: 1.25,
                damage_bonus: 1.25,
                accuracy_bonus: 1.05,
                range_bonus: 1.0,
            },
            VeterancyLevel::Elite => VeterancyBonuses {
                health_bonus: 1.5,
                damage_bonus: 1.5,
                accuracy_bonus: 1.1,
                range_bonus: 1.05,
            },
            VeterancyLevel::Heroic => VeterancyBonuses {
                health_bonus: 2.0,
                damage_bonus: 2.0,
                accuracy_bonus: 1.2,
                range_bonus: 1.1,
            },
        }
    }

    pub(super) fn veterancy_level_from_bonus(health_bonus: f32) -> (VeterancyLevel, f32) {
        if health_bonus >= 1.9 {
            (VeterancyLevel::Heroic, 300.0)
        } else if health_bonus >= 1.45 {
            (VeterancyLevel::Elite, 150.0)
        } else if health_bonus >= 1.2 {
            (VeterancyLevel::Veteran, 60.0)
        } else {
            (VeterancyLevel::Rookie, 0.0)
        }
    }

    pub(super) fn difficulty_from_modifiers(
        modifiers: &DifficultyModifiers,
    ) -> crate::ai::AIDifficulty {
        let score = (modifiers.ai_resource_bonus
            + modifiers.ai_damage_bonus
            + modifiers.ai_health_bonus
            + modifiers.ai_build_speed_bonus)
            / 4.0;

        if score < 0.95 {
            crate::ai::AIDifficulty::Easy
        } else if score < 1.15 {
            crate::ai::AIDifficulty::Medium
        } else if score < 1.35 {
            crate::ai::AIDifficulty::Hard
        } else {
            crate::ai::AIDifficulty::Brutal
        }
    }

    pub(super) fn sorted_unique_strings<I>(iter: I) -> Vec<String>
    where
        I: IntoIterator<Item = String>,
    {
        let mut values: Vec<String> = iter
            .into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        values.sort();
        values
    }

    fn snapshot_airfield_parking(&self, game_logic: &GameLogic) -> AirfieldParkingWorldSnapshot {
        let mut jet_stalls: Vec<AirfieldJetStallSnapshot> = game_logic
            .host_objects()
            .iter()
            .filter_map(|(id, object)| {
                object
                    .airfield_parking_space_index
                    .map(|space_index| AirfieldJetStallSnapshot {
                        object_id: *id,
                        space_index: Some(space_index),
                    })
            })
            .collect();
        jet_stalls.sort_by_key(|stall| stall.object_id.0);
        AirfieldParkingWorldSnapshot {
            fields: game_logic
                .snapshot_airfield_parking_spaces()
                .into_iter()
                .map(|(airfield_id, spaces)| AirfieldParkingFieldSnapshot {
                    airfield_id,
                    spaces: spaces
                        .into_iter()
                        .map(
                            |(object_id, reserved_for_exit)| AirfieldParkingSpaceSnapshot {
                                object_id,
                                reserved_for_exit,
                            },
                        )
                        .collect(),
                })
                .collect(),
            runways: game_logic
                .snapshot_runway_reservations()
                .into_iter()
                .map(|(airfield_id, occupants)| AirfieldRunwaySnapshot {
                    airfield_id,
                    occupants,
                })
                .collect(),
            next_in_line: game_logic
                .snapshot_airfield_runway_next_in_line()
                .into_iter()
                .map(|(airfield_id, occupants)| AirfieldRunwaySnapshot {
                    airfield_id,
                    occupants,
                })
                .collect(),
            was_in_line: game_logic
                .snapshot_airfield_runway_was_in_line()
                .into_iter()
                .map(
                    |(airfield_id, was_in_line)| AirfieldRunwayWasInLineSnapshot {
                        airfield_id,
                        was_in_line,
                    },
                )
                .collect(),
            jet_stalls,
            flight_decks: game_logic
                .snapshot_flight_deck_occupancy()
                .into_iter()
                .map(
                    |(
                        carrier_id,
                        got_info,
                        spaces,
                        runways,
                        designated_target,
                        designated_command,
                        pending_replacement,
                    )| {
                        FlightDeckPersistSnapshot {
                            carrier_id,
                            spaces: spaces
                                .into_iter()
                                .map(|(object_id, runway)| FlightDeckSpaceSnapshot {
                                    object_id,
                                    runway,
                                })
                                .collect(),
                            runways: runways
                                .into_iter()
                                .map(
                                    |(in_use_takeoff, in_use_landing)| FlightDeckRunwaySnapshot {
                                        in_use_takeoff,
                                        in_use_landing,
                                    },
                                )
                                .collect(),
                            got_info,
                            designated_target,
                            designated_command,
                            pending_replacement,
                        }
                    },
                )
                .collect(),
        }
    }

    fn restore_game_logic_persist_tail(
        &self,
        snapshot: &WorldSnapshot,
        game_logic: &mut GameLogic,
    ) {
        gamelogic::helpers::TheGameLogic::set_scoring_enabled(snapshot.is_scoring_enabled);
        game_logic.set_limit_superweapons(snapshot.limit_superweapons);
        if let Ok(mut leftover) = gamelogic::system::game_logic::get_game_logic().lock() {
            leftover.set_superweapon_restriction(if snapshot.limit_superweapons { 1 } else { 0 });
        }
        game_logic.restore_cave_system(snapshot.cave_system.clone());
        game_logic.restore_tunnel_network(snapshot.tunnel_network.clone());
        game_logic.restore_airfield_parking_spaces(
            snapshot
                .airfield_parking
                .fields
                .iter()
                .map(|field| {
                    (
                        field.airfield_id,
                        field
                            .spaces
                            .iter()
                            .map(|space| (space.object_id, space.reserved_for_exit))
                            .collect(),
                    )
                })
                .collect(),
        );
        game_logic.restore_runway_reservations(
            snapshot
                .airfield_parking
                .runways
                .iter()
                .map(|runway| (runway.airfield_id, runway.occupants.clone()))
                .collect(),
        );
        game_logic.restore_airfield_runway_next_in_line(
            snapshot
                .airfield_parking
                .next_in_line
                .iter()
                .map(|runway| (runway.airfield_id, runway.occupants.clone()))
                .collect(),
        );
        game_logic.restore_airfield_runway_was_in_line(
            snapshot
                .airfield_parking
                .was_in_line
                .iter()
                .map(|runway| (runway.airfield_id, runway.was_in_line.clone()))
                .collect(),
        );
        for stall in &snapshot.airfield_parking.jet_stalls {
            if let Some(object) = game_logic.host_object_mut(stall.object_id) {
                object.airfield_parking_space_index = stall.space_index;
            }
        }
        game_logic.restore_flight_deck_occupancy(
            snapshot
                .airfield_parking
                .flight_decks
                .iter()
                .map(|deck| {
                    (
                        deck.carrier_id,
                        deck.got_info,
                        deck.spaces
                            .iter()
                            .map(|space| (space.object_id, space.runway))
                            .collect(),
                        deck.runways
                            .iter()
                            .map(|runway| (runway.in_use_takeoff, runway.in_use_landing))
                            .collect(),
                        deck.designated_target,
                        deck.designated_command,
                        deck.pending_replacement,
                    )
                })
                .collect(),
        );
    }
}

fn disguise_team_to_u8(team: Option<Team>) -> u8 {
    match team {
        Some(Team::USA) => 0,
        Some(Team::China) => 1,
        Some(Team::GLA) => 2,
        Some(Team::Neutral) => 3,
        None => 255,
    }
}

fn disguise_team_from_u8(value: u8) -> Option<Team> {
    match value {
        0 => Some(Team::USA),
        1 => Some(Team::China),
        2 => Some(Team::GLA),
        3 => Some(Team::Neutral),
        _ => None,
    }
}

fn remaining_unit_waypoints(object: &Object) -> Vec<Vec3> {
    let idx = object
        .movement
        .current_path_index
        .min(object.movement.path.len());
    object.movement.path[idx..].to_vec()
}
