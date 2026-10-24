//! World capture and restoration commit order.
use super::*;

impl SnapshotBuilder {
    pub fn new() -> Self {
        Self {}
    }

    /// Admit only the checked saved logical grid into a detached receiver.
    /// This supplies no authored map or water-render configuration.
    pub(crate) fn prepare_logical_terrain_for_restore(
        &self,
        snapshot: &WorldSnapshot,
        candidate: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        validate_direct_world_snapshot_version(snapshot.version)?;
        // Native input owns squads; validate its detached wire before world mutation.
        let _ = super::super::hotkey_squad_persist::read_control_groups(&snapshot.lifecycle_tail)?;
        if candidate.object_lifecycle_callback_depth != 0
            || candidate.host_objects().registered_values().any(|object| {
                object.kill_score_phase == crate::game_logic::kill_credit::KillScorePhase::Running
            })
        {
            return Err(SaveLoadError::Corrupted(
                "restore cannot replace an active Object lifecycle callback phase".into(),
            ));
        }
        self.restore_terrain(&snapshot.terrain, candidate)
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
        if game_logic.has_unregistered_host_objects() {
            return Err(SaveLoadError::Serialization(
                "Cannot snapshot an unfinished Object constructor".into(),
            ));
        }
        // NDIE v2 captures the ordered removal queue after synchronous death
        // callbacks return. A callback in progress has no resumable Rust stack,
        // and a failed native retirement is terminal for this receiving owner.
        if game_logic.host_objects().registered_values().any(|object| {
            object.kill_score_phase == crate::game_logic::kill_credit::KillScorePhase::Running
        }) {
            return Err(SaveLoadError::Serialization(
                "Cannot snapshot in-flight scoreTheKill".into(),
            ));
        }
        if game_logic.object_lifecycle_callback_depth != 0 {
            return Err(SaveLoadError::Serialization(
                "Cannot snapshot an active Object lifecycle callback phase".into(),
            ));
        }
        if let Some(error) = game_logic.object_retirement_error() {
            return Err(SaveLoadError::Serialization(format!(
                "Cannot snapshot a failed Object retirement: {error}"
            )));
        }

        log::info!("Creating world snapshot from game state");

        // Snapshot all objects from game state
        let objects = self.snapshot_all_objects(game_logic)?;

        // Snapshot all players
        let mut players = self.snapshot_all_players(game_logic)?;
        super::super::player_upgrade_persist::stamp_completed_upgrades(&mut players, game_logic);

        // Create the world snapshot with actual game state
        let mut snapshot = WorldSnapshot {
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
            w3d_ghost_companion: Some(capture_w3d_ghost_xfer_bytes(&game_logic.w3d_ghosts)?),
            // Production save attaches this after GameClient::xfer, preserving
            // C++ named-block phase order. Pure logic snapshots have none.
            particle_system_companion: None,
            player_template_bindings: self.snapshot_player_template_bindings(game_logic)?,
            shroud: self.snapshot_shroud_state(game_logic)?,
            lifecycle_tail: {
                let mut bytes = super::super::lifecycle_tail::encode_lifecycle_tail(
                    &super::super::lifecycle_tail::capture_lifecycle_tail(game_logic)?,
                );
                super::super::camera_continuation::append(&mut bytes, game_logic)?;
                super::super::combat_projectile_persist::append(&mut bytes, game_logic)?;
                super::super::partition_continuation_persist::append(&mut bytes, game_logic)?;
                super::super::geometry_shape_persist::append(&mut bytes, game_logic)?;
                super::super::body_kind_persist::append(&mut bytes, game_logic)?;
                super::super::native_lifecycle_persist::append(&mut bytes, game_logic)?;
                super::super::original_disabled_persist::append(&mut bytes, game_logic)?;
                super::super::repair_dock_persist::append(&mut bytes, game_logic)?;
                super::super::object_order_persist::append(&mut bytes, game_logic)?;
                super::super::native_die_persist::append(&mut bytes, game_logic)?;
                super::super::special_power_cooldown_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::battle_plan_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
                super::super::subdual_persist::append_to_lifecycle_tail(&mut bytes, game_logic);
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

                super::super::named_unit_history_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::ai_player_queue_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );

                // A10 registry and immutable Body rules are framed before the APRO
                // / particle / CPAD footer chain; actor payload schemas stay unchanged.
                super::super::a10_flight_persist::append_to_lifecycle_tail(&mut bytes, game_logic)?;
                super::super::body_rules_persist::append(&mut bytes, game_logic);
                // Framed completion footer is last; its parser never scans
                // arbitrary object/template payload bytes for a marker.
                super::super::player_academy_persist::append_to_lifecycle_tail(
                    &mut bytes, game_logic,
                );
                super::super::combat_particle_attachment_persist::append(&mut bytes, game_logic)?;
                super::super::construction_completion_persist::append_to_lifecycle_tail(
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
            is_scoring_enabled: game_logic.is_scoring_enabled(),
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

        let chunks = super::super::player_team_chunks::stamp_from_live(game_logic)?;
        super::super::player_team_chunks::validate_roster(&snapshot, &chunks)?;
        super::super::player_team_chunks::bind_chunks_to_world(&mut snapshot, &chunks)?;

        if let Some(initial) = &game_logic.initial_skirmish_admission {
            super::super::initial_skirmish::append(&mut snapshot.lifecycle_tail, initial)?;
            // SINI requires at least29, never downgrade a newer capability.
            snapshot.version = snapshot.version.max(29);
        }

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
            .world_services
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
        // Native input owns squads; validate its detached wire before world mutation.
        let _ = super::super::hotkey_squad_persist::read_control_groups(&snapshot.lifecycle_tail)?;
        // Validate the original ScriptEngine continuation before mutating the
        // receiving world, including callers that deserialize the typed body directly.
        let mut receiving_persist = snapshot.persist_v18.clone();
        let encoded_script_tail =
            super::super::player_team_chunks::script_engine_tail_from_world(snapshot)?;
        if receiving_persist.script_engine_tail.is_none() {
            receiving_persist.script_engine_tail = encoded_script_tail;
        }
        self.validate_object_restore_admission(&snapshot.objects, game_logic)?;
        if game_logic.object_lifecycle_callback_depth != 0
            || game_logic.host_objects().registered_values().any(|object| {
                object.kill_score_phase == crate::game_logic::kill_credit::KillScorePhase::Running
            })
        {
            return Err(SaveLoadError::Corrupted(
                "restore cannot replace an active Object lifecycle callback phase".into(),
            ));
        }
        game_logic
            .world_services
            .shroud()
            .lock()
            .map_err(|_| SaveLoadError::Corrupted("receiving shroud owner is poisoned".into()))?
            .validate_snapshot_geometry(&snapshot.shroud)
            .map_err(SaveLoadError::Corrupted)?;
        self.validate_receiving_object_templates(&snapshot.objects, game_logic)?;
        let initial_skirmish = super::super::initial_skirmish::read_for_restore(snapshot)?;
        if let Some(initial) = &initial_skirmish {
            initial
                .validate_definitions()
                .map_err(crate::save_load::SaveLoadError::Corrupted)?;
        }
        let camera_continuation =
            super::super::camera_continuation::decode(&snapshot.lifecycle_tail)?;
        if let Some(camera) = camera_continuation.as_ref() {
            game_logic
                .validate_camera_continuation_admission(&camera.logic)
                .map_err(SaveLoadError::Corrupted)?;
        }
        if !game_logic.combat_particles().can_clear() {
            return Err(SaveLoadError::Corrupted(
                "snapshot replacement requires the actual live receiving particle manager".into(),
            ));
        }
        let particle_attachments =
            super::super::combat_particle_attachment_persist::decode_and_validate(snapshot)?;
        let projectile_continuation = super::super::combat_projectile_persist::decode(
            &snapshot.lifecycle_tail,
            snapshot.version >= 28,
        )?;
        let partition_continuation = super::super::partition_continuation_persist::decode(
            projectile_continuation.legacy_tail.as_ref(),
        )?;
        super::super::partition_continuation_persist::validate(
            partition_continuation.state.as_ref(),
            &snapshot.objects,
            &snapshot.shroud,
        )?;
        let geometry_shapes = super::super::geometry_shape_persist::decode(
            partition_continuation.legacy_tail.as_ref(),
        )?;
        super::super::geometry_shape_persist::validate(
            geometry_shapes.records.as_deref(),
            &snapshot.objects,
        )?;
        let body_definitions =
            super::super::body_kind_persist::decode(geometry_shapes.legacy_tail.as_ref())?;
        let body_kinds = super::super::body_kind_persist::validate(
            body_definitions.records.as_deref(),
            &snapshot.objects,
            game_logic,
        )?;
        let native_lifecycle =
            super::super::native_lifecycle_persist::decode(body_definitions.legacy_tail.as_ref())?;
        let native_lifecycle_runtimes = super::super::native_lifecycle_persist::validate(
            native_lifecycle.records.as_deref(),
            &snapshot.objects,
            game_logic,
            native_lifecycle.legacy_tail.as_ref(),
        )?;
        let original_disabled =
            super::super::original_disabled_persist::decode(native_lifecycle.legacy_tail.as_ref())?;
        super::super::original_disabled_persist::validate(
            original_disabled.records.as_deref(),
            &snapshot.objects,
        )?;
        let repair_dock =
            super::super::repair_dock_persist::decode(original_disabled.legacy_tail.as_ref())?;
        super::super::repair_dock_persist::validate(
            repair_dock.records.as_deref(),
            &snapshot.objects,
            game_logic,
        )?;
        let object_order =
            super::super::object_order_persist::decode(repair_dock.legacy_tail.as_ref())?;
        super::super::object_order_persist::validate(
            object_order.saved_order.as_deref(),
            &snapshot.objects,
        )?;
        let native_die =
            super::super::native_die_persist::decode(object_order.legacy_tail.as_ref())?;
        let native_die_runtimes = super::super::native_die_persist::validate(
            &native_die,
            snapshot,
            game_logic,
            &body_kinds,
        )?;
        let lifecycle_tail = native_die.legacy_tail.as_ref();
        super::super::subdual_persist::preflight_from_lifecycle_tail(lifecycle_tail, snapshot)?;

        let body_rules_candidate =
            super::super::body_rules_persist::prepare(lifecycle_tail, snapshot.version)?;
        let lifecycle_for_a10 =
            super::super::lifecycle_tail::decode_lifecycle_tail(lifecycle_tail)?;
        let a10_delivery_candidate = super::super::a10_flight_persist::decode_and_validate(
            lifecycle_tail,
            &snapshot.objects,
            &snapshot.special_power_strikes.strikes,
            &lifecycle_for_a10,
            game_logic,
        )?;
        // The candidate already owns the incoming authored graph. TerrainSnapshot
        // does not replace Waypoints. Validate incoming IDs before retiring actors.
        let complete_object_records: std::collections::HashSet<_> =
            snapshot.objects.keys().copied().collect();
        let ai_team_candidate = {
            let terrain_owner = game_logic.world_services.terrain();
            let terrain = terrain_owner.read().map_err(|_| {
                SaveLoadError::Corrupted("TMAI receiving Terrain unavailable".into())
            })?;
            super::super::ai_team_persist::prepare_from_lifecycle_tail(
                lifecycle_tail,
                &complete_object_records,
                &terrain,
            )?
        };
        // Reject malformed companion bytes before this API mutates its receiver.
        // Production map loading then restores this candidate world atomically.
        let ghosts = snapshot
            .w3d_ghost_companion
            .as_deref()
            .map(decode_w3d_ghost_xfer_bytes)
            .transpose()?
            .unwrap_or_default();
        let player_team_chunks = super::super::player_team_chunks::chunks_from_world(snapshot)?;
        super::super::player_team_chunks::validate_roster(snapshot, &player_team_chunks)?;
        super::super::player_team_chunks::validate_definitions(game_logic, &player_team_chunks)?;
        // Retire outgoing callbacks while their original frame/players/teams
        // still exist. Later object restoration may only reset an empty roster;
        // it must not run old destructors against newly restored owners.
        // Pending old-world starts are not incoming snapshot output. Isolate
        // them BEFORE destruction: retirement itself emits required stops and
        // other immediate callback output against the outgoing identities/poses.
        let pending_audio_before_retirement = std::mem::take(&mut game_logic.queued_audio_events);
        if let Err(error) =
            game_logic.destroy_all_objects_immediate_with_effects_checked(&mut Default::default())
        {
            // A failed retirement retains its exact owner. Preserve both the
            // previous pending output and the partial retirement output, without
            // replaying callbacks or pretending the receiver was replaced.
            let retirement_audio = std::mem::replace(
                &mut game_logic.queued_audio_events,
                pending_audio_before_retirement,
            );
            game_logic.queued_audio_events.extend(retirement_audio);
            return Err(SaveLoadError::Corrupted(format!(
                "outgoing Object retirement: {error}"
            )));
        }
        // Successful retirement keeps its newly emitted output. Only the
        // superseded pending old-world output is discarded here.
        drop(pending_audio_before_retirement);
        // This direct API mutates the receiver below and may subsequently fail.
        // Its transient result must not describe the previous world after a
        // partial restore. Production loading isolates this work in a candidate.
        if let Some(value) = native_lifecycle.hulk_max_lifetime_override {
            game_logic.native_hulk_max_lifetime_override = value;
        }
        game_logic.initial_skirmish_admission = initial_skirmish;
        game_logic.clear_victory_observation();
        // These are transient UI births, not snapshot state. Clear for every
        // replacement, including legacy saves lacking a camera continuation.
        // Production prepares this direct mutating restore on a detached candidate.
        game_logic.session_view.world_animations.clear();
        game_logic.health_events.clear();
        game_logic.ownership_events.clear();
        game_logic.economy_events.clear();
        game_logic.attack_events.clear();
        game_logic.production_events.clear();
        // Replacement starts a new runtime output identity. Detached production
        // candidates own this state, so rejection cannot consume live receipts.
        game_logic.movement_events.reset();
        game_logic.construction_events.reset();
        game_logic.eva_announcements.reset();
        log::info!(
            "Restoring world from snapshot: {} objects, {} players",
            snapshot.objects.len(),
            snapshot.players.len()
        );

        // Restore frame number
        game_logic.set_current_frame(snapshot.frame_number);

        if let Some(policy) = body_rules_candidate {
            game_logic.install_validated_body_damage_policy(policy);
        }

        // Lighting was validated with every other capsule before retirement.
        // Commit the prepared rules before Players and Objects are rebuilt.
        if let Some(rules) = a10_delivery_candidate.lighting.clone() {
            game_logic.install_validated_lighting_rules(rules);
        }

        // C++ Object.cpp:4218-4246 writes trigger slots after pose. Restore
        // HOST_TRIGGER_WORLD (including m_iPos) before recreate/set_position
        // so units already inside do not emit a fresh ENTERED_AREA edge.
        self.restore_object_triggers(snapshot, game_logic);

        // C++ parity order: players/teams before objects, then world systems.
        self.restore_all_players(&snapshot.players, game_logic)?;
        super::super::player_team_chunks::apply_before_objects(game_logic, &player_team_chunks)?;
        self.restore_player_ranks(snapshot, game_logic)?;
        self.restore_player_energy(snapshot, game_logic)?;
        self.restore_player_template_bindings(snapshot, game_logic)?;
        self.restore_all_teams(&snapshot.teams, game_logic)?;
        self.restore_all_objects_with_geometry(
            &snapshot.objects,
            snapshot.next_object_id,
            &body_kinds,
            &native_lifecycle_runtimes,
            object_order.saved_order.as_deref(),
            &super::super::geometry_shape_persist::shapes(geometry_shapes.records.as_deref()),
            game_logic,
        )?;
        super::super::native_lifecycle_persist::apply_transport_payload_latches(
            native_lifecycle.records.as_deref(),
            game_logic,
        );
        super::super::original_disabled_persist::apply(
            original_disabled.records.as_deref(),
            game_logic,
        );
        super::super::repair_dock_persist::apply(repair_dock.records.as_deref(), game_logic);
        super::super::native_die_persist::apply(native_die_runtimes, game_logic);
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
        // Accepted world versions (23 through 29) explicitly serialize the AI
        // roster. An empty roster must not run skirmish constructors, which
        // would invent controllers and overwrite saved player build flags.
        if !snapshot.ai_players.is_empty() {
            self.restore_global_ai_state(&snapshot.global_ai_state, game_logic)?;
        }
        self.restore_ai_players(&snapshot.ai_players, game_logic)?;
        self.restore_special_power_strikes(&snapshot.special_power_strikes, game_logic)?;
        self.restore_combat_particles(
            &snapshot.combat_particles,
            &particle_attachments,
            game_logic,
        )?;
        self.restore_host_upgrades(&snapshot.host_upgrades, game_logic)?;
        super::super::player_upgrade_persist::apply_completed_upgrades(snapshot, game_logic);
        // The v4 counter is the next unused logical accepted-discharge ID.
        // The runtime setter clamps legacy/malformed zero to one and clears
        // transient presentation events so a load cannot replay pre-save FX.
        game_logic.restore_weapon_discharge_next_sequence(snapshot.next_weapon_discharge_sequence);

        // C++ `GameState.cpp:661,683` saveLock around load. Manager xfer
        // unlocks internally to recreate saved modules.
        // The already decoded manager is private to this load candidate.
        // Commit at the original GhostObject phase; no shared pending slot.
        game_logic.w3d_ghosts = ghosts;
        game_logic.restore_host_fog_ghost_links();
        // CHUNK_GameClient belongs to the presentation client. The host keeps
        // its bytes staged until the candidate GameLogic commits; applying
        // them here would mutate the still-playable client's drawables.
        let visual_world = gamelogic::helpers::ClientVisualHandle::new(std::sync::Arc::clone(
            &game_logic.world_services,
        ));
        restore_objectless_from_client_drawables(&visual_world, &snapshot.client_drawables);
        if let Some(particle_bytes) = snapshot.particle_system_companion.as_deref() {
            if let Some(owner) = restore_particle_system_from_xfer_bytes(particle_bytes)? {
                let manager_guard = game_client::effects::get_particle_system_manager()
                    .map_err(|error| SaveLoadError::Corrupted(error.to_string()))?;
                let manager = manager_guard.as_ref().ok_or_else(|| {
                    SaveLoadError::Corrupted(
                        "paired particle companion has no receiving manager".into(),
                    )
                })?;
                game_logic
                    .combat_particles_mut()
                    .adopt_loaded_client_systems(manager, &owner)
                    .map_err(SaveLoadError::Corrupted)?;
            }
        }
        // Direct WorldSnapshot restore has no archive companion. Full saves
        // explicitly prepare/admit their paired TerrainVisual in the caller.

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
                .restore_receiving_partition_grid(&snapshot.shroud, snapshot.frame_number as u32)
                .map_err(SaveLoadError::Corrupted)?;
            let local_id = game_logic
                .get_players()
                .values()
                .find(|player| player.is_local)
                .map(|player| player.id);
            if let Some(local_id) = local_id {
                let services = game_logic.world_services.clone();
                if let Ok(mut shroud) = services.shroud().lock() {
                    shroud.refresh_radar_shroud_for_player_into(
                        local_id,
                        snapshot.frame_number as u32,
                        game_logic.radar_mut(),
                    );
                }
            }
        }

        game_logic
            .restore_host_fog_ghost_shroud_history()
            .map_err(SaveLoadError::Corrupted)?;

        let tail = super::super::lifecycle_tail::decode_lifecycle_tail(lifecycle_tail)?;
        super::super::lifecycle_tail::apply_lifecycle_tail_to_host(&tail, game_logic)?;
        let mut restored_a10_registry = a10_delivery_candidate.registry;
        for id in &a10_delivery_candidate.live_transport_bindings {
            let actor = game_logic
                .objects
                .get(id)
                .expect("A10 preflight validated every live transport object record");
            assert!(
                restored_a10_registry
                    .restore_transport_lifetime(*id, &actor.visual_draw_identity_token,)
            );
        }
        game_logic.a10_strike_flight_reg = restored_a10_registry;
        game_logic
            .special_power_strikes_mut()
            .reconcile_a10_delivery_ownership(&a10_delivery_candidate.live_strike_ids);
        super::super::special_power_cooldown_persist::apply_from_lifecycle_tail(
            lifecycle_tail,
            game_logic,
        )?;
        super::super::battle_plan_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::subdual_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::booby_trap_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::carpet_bomb_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::production_door_persist::apply_from_lifecycle_tail(
            lifecycle_tail,
            game_logic,
        )?;
        super::super::dozer_repair_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::rebuild_hole_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::weapon_set_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::ai_team_persist::apply_prepared(
            ai_team_candidate,
            game_logic,
            Some(&complete_object_records),
        );
        super::super::ability_hijack_persist::apply_from_lifecycle_tail(
            lifecycle_tail,
            game_logic,
        )?;
        super::super::dock_queue_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::module_runtime_persist::apply_from_lifecycle_tail(
            lifecycle_tail,
            game_logic,
        )?;
        super::super::deliver_payload_persist::apply_from_lifecycle_tail(
            lifecycle_tail,
            game_logic,
        )?;
        super::super::object_module_xfer_persist::apply_from_lifecycle_tail(
            lifecycle_tail,
            game_logic,
        )?;
        super::super::auto_deposit_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::supply_drop_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::jet_ai_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::chinook_ai_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::hacker_income_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::warehouse_crippling_persist::apply_from_lifecycle_tail(
            lifecycle_tail,
            game_logic,
        )?;
        super::super::helix_napalm_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::money_crate_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::gps_scrambler_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::dynamic_shroud_persist::apply_from_lifecycle_tail(
            lifecycle_tail,
            game_logic,
        )?;
        super::super::angry_mob_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::power_plant_rods_persist::apply_from_lifecycle_tail(
            lifecycle_tail,
            game_logic,
        )?;
        super::super::cleanup_hazard_persist::apply_from_lifecycle_tail(
            lifecycle_tail,
            game_logic,
        )?;
        super::super::point_defense_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::projectile_stream_persist::apply_from_lifecycle_tail(
            lifecycle_tail,
            game_logic,
        )?;
        super::super::transport_exit_persist::apply_from_lifecycle_tail(
            lifecycle_tail,
            game_logic,
        )?;
        super::super::bridge_behavior_persist::apply_from_lifecycle_tail(
            lifecycle_tail,
            game_logic,
        )?;
        // C++ has ONE TurretAI::xfer per module; here TRAI (15-field subset,
        // resets all objects first) must run BEFORE OXOB (47 fields incl. all
        // 15 turret fields, captured for every object). OXOB is the final
        // turret authority — running TRAI after it zeroed OXOB-restored
        // idle-scan/hold/substate state for objects outside TRAI's capture
        // predicate. TRAI stays ahead only for legacy tails without OXOB.
        super::super::turret_aim_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::player_academy_persist::apply_from_lifecycle_tail(
            lifecycle_tail,
            game_logic,
        )?;
        super::super::construction_completion_persist::apply_from_lifecycle_tail(
            lifecycle_tail,
            game_logic,
        )?;
        super::super::object_xfer_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        // OXOB's active Guard deadline must agree with the immutable nested
        // continuation; neither record may silently repair a conflicting save.
        super::super::guard_state_persist::validate_after_lifecycle_tail(snapshot, game_logic)?;
        super::super::named_unit_history_persist::apply_from_lifecycle_tail(
            lifecycle_tail,
            game_logic,
        )?;
        super::super::ai_player_queue_persist::apply_from_lifecycle_tail(
            lifecycle_tail,
            game_logic,
        )?;
        super::super::inferno_fire_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::firewall_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::neutron_slow_death_persist::apply_from_lifecycle_tail(
            lifecycle_tail,
            game_logic,
        )?;
        super::super::stealth_grant_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::weapon_leech_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;
        super::super::score_keeper_persist::apply_from_lifecycle_tail(lifecycle_tail, game_logic)?;

        self.sync_all_garrisoned_units_from_occupants(game_logic);
        self.restore_game_logic_persist_tail(snapshot, game_logic);
        super::super::persist_v18::restore_persist_v18_with_scale_encoding(
            &receiving_persist,
            game_logic,
            snapshot.version < 30,
        )?;
        // Historical native saves may carry a stale global adapter in v18.
        // NLFC already owns the receiving native value and takes precedence.
        if let Some(value) = native_lifecycle.hulk_max_lifetime_override {
            game_logic.native_hulk_max_lifetime_override = value;
        }
        // CPP AIUpdate::xfer resolves the saved attack-info name on LOAD.
        // The receiving interpreter tail is now restored; rebind object names
        // and definitions before the first AI/script observer can select a target.
        game_logic.sync_attack_priority_from_script_engine();
        super::super::garrison_firepoint_persist::apply_from_lifecycle_tail(
            lifecycle_tail,
            game_logic,
        )?;
        super::super::stealth_detector_persist::apply_from_lifecycle_tail(
            lifecycle_tail,
            game_logic,
        )?;

        game_logic
            .combat_system
            .restore_pending_combat(&snapshot.pending_combat);
        game_logic
            .combat_system
            .restore_live_projectile_arena(projectile_continuation.arena);

        // Energy::xfer deliberately omits derived production/consumption.
        // Rebuild only after all object/status tails (including disabled_held)
        // are transferred, before the first script/construction observer.
        game_logic.restore_player_power();
        game_logic.restore_owned_bridge_bindings();

        // Recreating objects may consume random values; install the saved
        // continuation only after every restore operation succeeds. This is
        // instance state, including for callers of the direct builder API.
        // Older payloads omit this field and use the all-zero sentinel.
        // The existing random_seed field is the selected seed, not a
        // process broadcast generation. Restore only this candidate owner.
        game_logic.set_logic_random_seed(snapshot.random_seed as u32);
        if snapshot.logic_rng_seed_words != [0; 6] {
            game_logic
                .logic_random
                .set_seed_words(snapshot.logic_rng_seed_words);
        }

        super::super::player_team_chunks::apply_members(game_logic, &player_team_chunks)?;
        // Saved destinations and paths are canonical state, not fresh movement
        // cues. Discard only this receiver's transient restore observations.
        game_logic.movement_events.clear();
        // Player's saved counts are canonical. Reconstruct only the derived
        // lifecycle admission map before a resumed capture/delete can observe it.
        game_logic.rebuild_host_radar_provider_admission();
        // All saved status/HP overlays are installed. Keep the exact restored
        // Body enum and any positive-HP AI-dead flag; zero HP is always dead.
        // Pure model reconstruction must not replay damage or death callbacks.
        for object in game_logic.host_objects_mut().registered_values_mut() {
            object.restore_body_observation(Some(object.body_damage_state));
        }
        // CPP TurretAI::loadPostProcess361–365 derives the initial team
        // from the loaded target rather than saving a stale team surrogate.
        game_logic.resolve_owned_turret_targets_after_load()?;
        if let Some(camera) = camera_continuation {
            game_logic
                .admit_camera_continuation(camera.logic)
                .map_err(SaveLoadError::Corrupted)?;
        }
        // Restore exact stale COI/first-dirty history only after the receiving
        // shroud grid and every actor tail are installed. This does not flush
        // pending dirtiness or replay contact callbacks during load.
        if let Some(partition) = partition_continuation.state {
            game_logic
                .restore_partition_continuation(partition)
                .map_err(SaveLoadError::Corrupted)?;
        } else {
            game_logic.reattach_legacy_partition_after_restore();
        }
        log::info!("World restoration complete");
        Ok(())
    }
}
