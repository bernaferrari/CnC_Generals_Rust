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
        let players = self.snapshot_all_players(game_logic)?;

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
            lifecycle_tail: super::lifecycle_tail::encode_lifecycle_tail(
                &super::lifecycle_tail::capture_lifecycle_tail(game_logic),
            ),
            player_ranks: self.snapshot_player_ranks(game_logic)?,
        };

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

        // C++ parity order: players/teams before objects, then world systems.
        self.restore_all_players(&snapshot.players, game_logic)?;
        self.restore_player_ranks(snapshot, game_logic)?;
        self.restore_player_template_bindings(snapshot, game_logic)?;
        self.restore_all_teams(&snapshot.teams, game_logic)?;
        self.restore_all_objects(&snapshot.objects, game_logic)?;
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
                    formation_position: None,
                    formation_id: None,
                    group_id: None,
                    waypoints: Vec::new(),
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
        if snapshot.version < WORLD_SNAPSHOT_BINCODE_VERSION {
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
        let (logic_width, logic_height, logic_heights) =
            gamelogic::terrain::get_terrain_logic()
                .read()
                .ok()
                .map(|terrain| {
                    let (w, h) = terrain.logic_height_map_extents();
                    (w.max(0) as u32, h.max(0) as u32, terrain.logic_height_map_bytes().to_vec())
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
}
