//! World capture and restoration commit order.
use super::*;

impl SnapshotBuilder {
    pub fn new() -> Self {
        Self {}
    }

    /// Create complete world snapshot from current game state
    pub fn create_world_snapshot(&self, game_logic: &GameLogic) -> SaveLoadResult<WorldSnapshot> {
        #[cfg(feature = "game_client")]
        {
            self.create_world_snapshot_impl(game_logic, None)
        }
        #[cfg(not(feature = "game_client"))]
        {
            self.create_world_snapshot_impl(game_logic)
        }
    }

    #[cfg(feature = "game_client")]
    pub fn create_world_snapshot_with_client(
        &self,
        game_logic: &GameLogic,
        client: &game_client::core::game_client::GameClient,
    ) -> SaveLoadResult<WorldSnapshot> {
        self.create_world_snapshot_impl(game_logic, Some(client))
    }

    fn create_world_snapshot_impl(
        &self,
        game_logic: &GameLogic,
        #[cfg(feature = "game_client")] client: Option<&game_client::core::game_client::GameClient>,
    ) -> SaveLoadResult<WorldSnapshot> {
        // The runtime destruction queue is not part of Xfer. Capture only
        // after end-of-frame physical removal, rather than restoring an
        // immortal DESTROYED object whose deletion request was discarded.
        if game_logic.has_pending_object_removals() {
            return Err(SaveLoadError::Serialization(
                "Cannot snapshot before pending object removals finish".to_string(),
            ));
        }

        log::info!("Creating world snapshot from game state");

        // Snapshot all objects from game state
        let objects = self.snapshot_all_objects(game_logic)?;

        // Snapshot all players
        let mut players = self.snapshot_all_players(game_logic)?;
        super::super::player_upgrade_persist::stamp_completed_upgrades(&mut players, game_logic);

        // Create the world snapshot with actual game state
        let snapshot = WorldSnapshot {
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
            timestamp: std::time::SystemTime::now(),
            frame_number: game_logic.get_current_frame(),
            // Last observed base-seed broadcast for the driving instance
            // (broadcast channel for recorder/skirmish/save reseeds).
            random_seed: game_logic.logic_base_seed as u64,

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
            shroud: self.snapshot_shroud_state(game_logic)?,
            lifecycle_tail: {
                let mut bytes = super::super::lifecycle_tail::encode_lifecycle_tail(
                    &super::super::lifecycle_tail::capture_lifecycle_tail(game_logic),
                );
                super::super::special_power_cooldown_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::battle_plan_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::super::subdual_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::super::hotkey_squad_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::booby_trap_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::super::carpet_bomb_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::super::production_door_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::dozer_repair_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::rebuild_hole_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::weapon_set_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::super::ability_hijack_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );

                super::super::ai_team_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::super::dock_queue_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::super::module_runtime_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::deliver_payload_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::object_module_xfer_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::auto_deposit_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::supply_drop_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::super::jet_ai_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::super::chinook_ai_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::super::hacker_income_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::warehouse_crippling_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::helix_napalm_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::money_crate_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::super::gps_scrambler_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::dynamic_shroud_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::angry_mob_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::super::power_plant_rods_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::cleanup_hazard_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::point_defense_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::projectile_stream_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::transport_exit_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::bridge_behavior_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::object_xfer_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::super::ai_player_queue_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::inferno_fire_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::firewall_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::super::neutron_slow_death_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::turret_aim_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::super::stealth_grant_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::weapon_leech_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::score_keeper_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::garrison_firepoint_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::stealth_detector_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );

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
            persist_v18: {
                #[cfg(feature = "game_client")]
                {
                    if let Some(client) = client {
                        super::super::persist_v18::capture_persist_v18_with_client(
                            game_logic, client,
                        )
                    } else {
                        super::super::persist_v18::capture_persist_v18(game_logic)
                    }
                }
                #[cfg(not(feature = "game_client"))]
                {
                    super::super::persist_v18::capture_persist_v18(game_logic)
                }
            },
            object_experience_trackers: self.snapshot_object_experience_trackers(game_logic),
            object_command_sets: self.snapshot_object_command_sets(game_logic),
            object_disguises: self.snapshot_object_disguises(game_logic),
            // Driving instance's Rust RNG continuation. Restoring it after
            // reconstruction preserves the source world's future draws.
            logic_rng_seed_words: game_logic.logic_random.seed_words(),
            // C++ GameStateMap::xfer moves the exact counter early in load.
            next_object_id: game_logic.next_object_id_for_snapshot().0,
            pending_combat: game_logic.combat_system.pending_combat_snapshot(),
        };

        super::super::player_team_persist::stamp_from_live(game_logic);

        log::info!(
            "World snapshot complete: {} objects, {} players",
            snapshot.objects.len(),
            snapshot.players.len()
        );

        Ok(snapshot)
    }

    /// Capture the driving world's PartitionManager equivalent;
    /// snapshotting the derived per-player presentation bytes here would lose
    /// the raw C++ looker/shrouder counters and pending undo queue.
    fn snapshot_shroud_state(
        &self,
        game_logic: &GameLogic,
    ) -> SaveLoadResult<gamelogic::system::shroud_manager::ShroudSnapshot> {
        game_logic
            .engine_stores
            .shroud()
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
        validate_direct_world_snapshot_version(snapshot.version)?;
        // This direct API mutates the receiver below and may subsequently fail.
        // Its transient result must not describe the previous world after a
        // partial restore. Production loading isolates this work in a candidate.
        game_logic.clear_victory_observation();
        game_logic.health_events.clear();
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
        self.restore_object_triggers(snapshot, game_logic);

        // C++ parity order: players/teams before objects, then world systems.
        self.restore_all_players(&snapshot.players, game_logic)?;
        super::super::player_team_persist::apply_pending(game_logic);
        self.restore_player_ranks(snapshot, game_logic)?;
        self.restore_player_energy(snapshot, game_logic)?;
        self.restore_player_template_bindings(snapshot, game_logic)?;
        self.restore_all_teams(&snapshot.teams, game_logic)?;
        self.restore_all_objects(&snapshot.objects, snapshot.next_object_id, game_logic)?;
        self.restore_object_instance_guards(snapshot, game_logic)?;
        self.restore_overcharge_active(snapshot, game_logic)?;
        self.restore_object_experience_trackers(snapshot, game_logic)?;
        self.restore_object_command_sets(snapshot, game_logic)?;
        self.restore_object_disguises(snapshot, game_logic)?;

        self.restore_cia_vision_builder_sell(snapshot, game_logic)?;
        self.restore_object_persist(snapshot, game_logic)?;
        self.restore_client_drawable_visuals(snapshot, game_logic);

        self.restore_terrain(&snapshot.terrain, game_logic)?;
        // PathfindingCacheSnapshot is retained on the wire, but complete
        // ObjectSnapshot movement records are authoritative and must not be
        // reconstructed from a shared position-keyed cache.
        self.restore_weather(&snapshot.weather, game_logic)?;
        self.restore_resource_manager(&snapshot.resource_manager, game_logic)?;
        self.restore_combat_tracker(&snapshot.combat_tracker, game_logic)?;
        // Both accepted world versions (23/24) explicitly serialize the AI
        // roster. An empty roster must not run skirmish constructors, which
        // would invent controllers and overwrite saved player build flags.
        if !snapshot.ai_players.is_empty() {
            self.restore_global_ai_state(&snapshot.global_ai_state, game_logic)?;
        }
        self.restore_ai_players(&snapshot.ai_players, game_logic)?;
        self.restore_special_power_strikes(&snapshot.special_power_strikes, game_logic)?;
        self.restore_combat_particles(&snapshot.combat_particles, game_logic)?;
        self.restore_host_upgrades(&snapshot.host_upgrades, game_logic)?;
        super::super::player_upgrade_persist::apply_completed_upgrades(snapshot, game_logic);
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
        // CHUNK_GameClient belongs to the presentation client. The host keeps
        // its bytes staged until the candidate GameLogic commits; applying
        // them here would mutate the still-playable client's drawables.
        let visual_world = gamelogic::helpers::ClientVisualHandle::new(std::sync::Arc::clone(
            &game_logic.engine_stores,
        ));
        restore_objectless_from_client_drawables(&visual_world, &snapshot.client_drawables);
        if let Some(particle_bytes) = take_loaded_particle_system_xfer() {
            restore_particle_system_from_xfer_bytes(&particle_bytes)?;
        }
        if let Some(terrain_visual_bytes) = take_loaded_terrain_visual_xfer() {
            restore_terrain_visual_from_xfer_bytes(&terrain_visual_bytes)?;
        }

        // Map loading initializes a fresh shroud grid and may reveal
        // staging-map objects. Replace that world's manager only when this save
        // actually carries the v6 shroud tail, after all object/team restore
        // callbacks have finished mutating candidate state.
        if snapshot.shroud.grid.is_some()
            || !snapshot.shroud.pending_undo_shroud_reveals.is_empty()
            || !snapshot.shroud.pending_full_reveal_players.is_empty()
            || !snapshot.shroud.pending_permanent_reveal_players.is_empty()
        {
            game_logic
                .engine_stores
                .shroud()
                .lock()
                .map_err(|_| {
                    SaveLoadError::Corrupted(
                        "ShroudManager lock poisoned while restoring".to_string(),
                    )
                })?
                .replace_state(&snapshot.shroud, snapshot.frame_number as u32)
                .map_err(SaveLoadError::Corrupted)?;
        }

        let tail = super::super::lifecycle_tail::decode_lifecycle_tail(&snapshot.lifecycle_tail)?;
        super::super::lifecycle_tail::apply_lifecycle_tail_to_host(&tail, game_logic)?;
        super::super::special_power_cooldown_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::battle_plan_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::subdual_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::booby_trap_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::carpet_bomb_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::production_door_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::dozer_repair_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::rebuild_hole_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::weapon_set_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        let complete_object_records: std::collections::HashSet<_> =
            snapshot.objects.keys().copied().collect();
        super::super::ai_team_persist::apply_from_lifecycle_tail_with_object_records(
            &snapshot.lifecycle_tail,
            game_logic,
            Some(&complete_object_records),
        )?;
        super::super::ability_hijack_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::dock_queue_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::module_runtime_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::deliver_payload_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::object_module_xfer_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::auto_deposit_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::supply_drop_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::jet_ai_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::chinook_ai_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::hacker_income_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::warehouse_crippling_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::helix_napalm_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::money_crate_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::gps_scrambler_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::dynamic_shroud_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::angry_mob_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::power_plant_rods_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::cleanup_hazard_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::point_defense_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::projectile_stream_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::transport_exit_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::bridge_behavior_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        // C++ has ONE TurretAI::xfer per module; here TRAI (15-field subset,
        // resets all objects first) must run BEFORE OXOB (47 fields incl. all
        // 15 turret fields, captured for every object). OXOB is the final
        // turret authority — running TRAI after it zeroed OXOB-restored
        // idle-scan/hold/substate state for objects outside TRAI's capture
        // predicate. TRAI stays ahead only for legacy tails without OXOB.
        super::super::turret_aim_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::object_xfer_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::ai_player_queue_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::inferno_fire_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::firewall_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::neutron_slow_death_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::stealth_grant_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::weapon_leech_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::score_keeper_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;

        self.sync_all_garrisoned_units_from_occupants(game_logic);
        self.restore_game_logic_persist_tail(snapshot, game_logic);
        super::super::persist_v18::restore_persist_v18(&snapshot.persist_v18, game_logic);
        super::super::hotkey_squad_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::garrison_firepoint_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;
        super::super::stealth_detector_persist::apply_from_lifecycle_tail(
            &snapshot.lifecycle_tail,
            game_logic,
        )?;

        game_logic
            .combat_system
            .restore_pending_combat(&snapshot.pending_combat);

        // Energy::xfer deliberately omits derived production/consumption.
        // Rebuild only after all object/status tails (including disabled_held)
        // are transferred, before the first script/construction observer.
        game_logic.restore_player_power();

        // Recreating objects may consume random values; install the saved
        // continuation only after every restore operation succeeds. This is
        // instance state, including for callers of the direct builder API.
        // Older payloads omit this field and use the all-zero sentinel.
        if snapshot.logic_rng_seed_words != [0; 6] {
            game_logic
                .logic_random
                .set_seed_words(snapshot.logic_rng_seed_words);
            // This tracks the broadcast, not the actual stream's seed.
            game_logic.logic_base_seed =
                game_engine::common::random_value::get_game_logic_random_seed();
        }

        log::info!("World restoration complete");
        Ok(())
    }
}
