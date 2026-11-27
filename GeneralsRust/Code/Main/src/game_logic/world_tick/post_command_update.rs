//! Operations on the live match used by the simulation's frame schedule.
use super::*;
use crate::game_logic::ScriptPresentationEffects;
use generals_sim::GameLogicResult;
use generals_sim::frame::{CommandPhase, FrameObjectRoster, FrameWorld, LogicFrame};
use generals_sim::recurring::{RecurringUpdate, RecurringUpdates};

pub(super) struct LiveFrame<'world, 'effects> {
    pub(super) logic: &'world mut GameLogic,
    pub(super) effects: ScriptPresentationEffects<'effects>,
}

impl FrameWorld for LiveFrame<'_, '_> {
    type CommandBoundary = CommandPhaseYield;

    fn run_early_scripts(&mut self, frame: LogicFrame) -> GameLogicResult<()> {
        let world = &mut self.logic;
        world
            .pathfinding_system
            .note_logic_frame(u64::from(frame.number));
        world.refresh_pathfind_ally_masks();
        world
            .evaluate_and_execute_scripts_with_effects(frame.step_seconds, self.effects.reborrow());
        world.team_creation_error().map_or(Ok(()), Err)
    }

    fn time_is_frozen(&self) -> bool {
        self.logic
            .simulation_frozen_with_script_engine(None)
            .expect("the receiving ScriptEngine is restored after early scripting")
    }

    fn has_admitted_clear(&mut self) -> bool {
        self.logic.replay_pending.receiving_clear_command_pending()
    }

    fn continue_from_debug_freeze(&mut self) {
        self.logic
            .force_script_debug_continue(None)
            .expect("the receiving ScriptEngine owns debug continue");
    }

    fn update_terrain(&mut self) -> GameLogicResult<()> {
        self.logic
            .update_owned_water_with_effects(&mut self.effects)?;
        self.logic.refresh_owned_bridge_pathfinder_states();
        let _ = self.logic.refresh_surface_cells_and_water_edge_damage(0.0);
        Ok(())
    }

    fn collect_object_ids(&self) -> Vec<ObjectId> {
        self.logic.objects.registered_keys().copied().collect()
    }

    fn dispatch_commands(
        &mut self,
        mode: CommandPhase,
        frame: LogicFrame,
        roster: FrameObjectRoster,
    ) -> Result<FrameObjectRoster, CommandPhaseYield> {
        match mode {
            CommandPhase::Standalone => self.logic.process_commands_with_effects(&mut self.effects),
            CommandPhase::Native => {
                if let Some(batch) = self.logic.admit_native_command_batch() {
                    return self.logic.dispatch_native_command_batch(
                        batch,
                        &mut self.effects,
                        frame,
                        roster,
                    );
                }
                self.logic
                    .process_queued_commands_without_recorder(&mut self.effects);
            }
        }
        Ok(roster)
    }

    fn recurring_updates(&mut self) -> &mut RecurringUpdates {
        &mut self.logic.recurring_updates
    }

    fn run_recurring_update(
        &mut self,
        update: RecurringUpdate,
        object_ids: &[ObjectId],
        dt: f32,
    ) -> GameLogicResult<()> {
        self.logic
            .run_frame_recurring_update(update, object_ids, dt, &mut self.effects)
    }

    fn update_objects(&mut self, object_ids: &[ObjectId], dt: f32) -> GameLogicResult<()> {
        self.logic
            .run_frame_object_updates(object_ids, dt, &mut self.effects)
    }

    fn update_match_ai(&mut self) -> GameLogicResult<()> {
        self.logic.update_match_ai_with_effects(&mut self.effects)?;
        self.logic.team_creation_error().map_or(Ok(()), Err)
    }

    fn dispatch_ai_orders(&mut self, after_reset: bool) {
        // A reset drains only the retained world queue. Its new Recorder has
        // not admitted another batch while this frame is still in progress.
        if after_reset {
            self.logic
                .process_queued_commands_without_recorder(&mut self.effects);
        } else {
            self.logic.process_commands_with_effects(&mut self.effects);
        }
    }

    fn update_production(&mut self, dt: f32) -> GameLogicResult<()> {
        self.logic.run_frame_production(dt, &mut self.effects)
    }

    fn update_economy(&mut self, dt: f32) -> GameLogicResult<()> {
        self.logic.run_frame_economy(dt, &mut self.effects)
    }

    fn update_partition(&mut self) -> GameLogicResult<()> {
        self.logic
            .update_owned_partition_with_effects(&mut self.effects)
            .map(|_| ())
    }

    fn retire_objects(&mut self) -> GameLogicResult<()> {
        self.logic.run_frame_retirements(&mut self.effects)
    }

    fn update_weapons(&mut self) -> GameLogicResult<()> {
        self.logic
            .apply_owned_projectileless_delayed_damage_with_effects(&mut self.effects)
    }

    fn evaluate_victory(&mut self) -> GameLogicResult<()> {
        self.logic
            .evaluate_victory_condition_with_effects(&mut self.effects)
            .map(|_| ())
    }

    fn expire_disabled_statuses(&mut self) {
        self.logic
            .expire_owned_disabled_statuses_with_effects(&mut self.effects);
        self.logic.check_bridge_disabled_statuses();
    }

    fn update_visibility(&mut self) {
        self.logic.update_main_crate_vision();
        self.logic
            .host_radar_sync_live_objects_with_effects(&mut self.effects);
    }
}

impl GameLogic {
    fn run_frame_recurring_update(
        &mut self,
        update: RecurringUpdate,
        object_ids: &[ObjectId],
        dt: f32,
        ui: &mut ScriptPresentationEffects<'_>,
    ) -> GameLogicResult<()> {
        match update {
            RecurringUpdate::Construction => {
                self.update_construction_with_effects(object_ids, dt, ui);
            }
            RecurringUpdate::SellList => {
                self.update_sell_list_with_effects(ui);
            }
            RecurringUpdate::DozerBoredRepair => {
                self.update_dozer_bored_repair();
            }
            RecurringUpdate::RebuildHoles => {
                self.update_rebuild_holes_with_effects(ui);
            }
            RecurringUpdate::Movement => {
                // C++ AIUpdate updates the active state before
                // AIUpdate::doLocomotor. Panic continuation must therefore
                // inspect the path completed by the preceding locomotor
                // pass, then install the next waypoint before this pass.
                // Wander state entry happens in the script action itself;
                // its periodic work belongs to the simulation, after the
                // freeze gate, not to each unrelated script action.
                self.tick_host_wander_in_place();
                self.begin_host_waypoint_ai_observations(object_ids);
                self.tick_host_panic_states_with_effects(object_ids, ui)?;
                self.tick_host_guard_states_with_effects(object_ids, ui)?;
                self.tick_host_pre_locomotor_movement_states(object_ids, ui)?;
                self.update_movement_with_effects(object_ids, dt, ui)?;
            }
            RecurringUpdate::SpecialPowers => {
                update_special_powers();
            }
        }
        Ok(())
    }

    fn run_frame_object_updates(
        &mut self,
        object_ids: &[ObjectId],
        dt: f32,
        ui: &mut ScriptPresentationEffects<'_>,
    ) -> GameLogicResult<()> {
        // Host superweapon residual: complete queued DaisyCutter / A10 / Scud /
        // ParticleCannon / NuclearMissile / AnthraxBomb / SpectreGunship /
        // CarpetBomb / ArtilleryBarrage / CruiseMissile strikes (area / line /
        // multi-shell / loft-MOAB damage + nuke radiation / anthrax toxin /
        // spectre orbit). Fail-closed vs full OCL aircraft / NeutronMissileUpdate
        // / PoisonField stack / B52 DeliverPayload / door loft path.
        {
            self.update_special_power_strikes_with_effects(ui)?;
        }
        // Resolve new field objects only when a strike spawned radiation.
        if !self
            .special_power_strikes
            .radiation_spawned_this_frame()
            .is_empty()
        {
            self.spawn_nuke_radiation_field_objects_for_new_fields_with_effects(ui);
        }
        {
            self.update_nuke_radiation_field_objects_with_effects(ui)?;
        }
        // Gate: mirror of the radiation gate above, for toxin fields.
        if !self
            .special_power_strikes
            .toxin_spawned_this_frame()
            .is_empty()
        {
            self.spawn_anthrax_toxin_field_objects_for_new_fields_with_effects(ui);
        }
        {
            self.update_anthrax_toxin_field_objects_with_effects(ui)?;
        }

        // Host America Paradrop residual: spawn infantry after approach delay.
        // Fail-closed vs full OCL cargo plane / parachute payload path.
        {
            self.update_paradrop_cargo_planes_with_effects(ui);
        }
        // Gate: zero paradrop missions -> clear_frame_events/plan_due_drops
        // are provable no-ops (events are pushed only alongside a mission
        // insert; the missions map is insert-only). Skip the pass.
        if self.host_paradrops.mission_count() > 0 {
            self.update_paradrops_with_effects(ui);
        }

        // Host DeliverPayload cargo residual: DropDelay-staggered spawn of payload
        // units at location (Supply Drop Zone crates). Fail-closed vs full aircraft.
        // Gate: zero deliver-payload missions -> frame-event clears, the
        // cargo-flight tick, and the spawn plan are provable no-ops (all
        // side queues are populated only alongside a mission insert).
        if self.host_deliver_payloads.mission_count() > 0 {
            self.update_deliver_payloads_with_effects(ui);
        }

        // Host MoneyCrateCollide residual: unit + BuildingPickup cash collect.
        {
            self.update_money_crate_collides_with_effects(ui);
        }

        // Host GLA Rebel Ambush residual: leftover OCLSpecialPower CreateObject
        // (science UpgradeOCL) plus FadeIn/DiesOnBadLand after the fire-frame spawn.
        // Gate: zero ambush missions -> fade-clear drain and spawn plan are
        // provable no-ops (pending_fade_clears is pushed only after a live
        // ambush spawn, which requires a mission).
        if self.host_ambushes.mission_count() > 0 {
            self.update_ambushes_with_effects(ui);
        }

        // Host USA Leaflet Drop residual: disable enemy infantry/vehicles after Delay.
        // Fail-closed vs full OCL B52 / LeafletContainer / LeafletFX particle path.
        {
            self.update_leaflet_b52_flights_with_effects(ui);
        }
        // Gate: zero leaflet missions -> the per-plan world radius scan never
        // runs (plan_due_impacts includes Completed missions for walk-in
        // re-pulse, so mission_count — any phase — is the only safe test).
        if self.host_leaflet_drops.mission_count() > 0 {
            self.update_leaflet_drops();
        }

        // Host GLA Sneak Attack residual: spawn tunnel + shockwave after Lifetime delay.
        // Fail-closed vs full OCL Start animation / TunnelContain path.
        // Gate: zero sneak-attack missions -> shockwave drain and spawn plan
        // are provable no-ops (pending_shockwaves is pushed only alongside a
        // mission insert).
        if self.host_sneak_attacks.mission_count() > 0 {
            self.update_sneak_attacks_with_effects(ui)?;
        }

        // Host mine / demo-trap residual: proximity trigger + timed detonation.
        // Fail-closed vs full MinefieldBehavior / DemoTrapUpdate modules.
        {
            self.update_mines_and_demo_traps_with_effects(ui)?;
        }

        // Host USA Ambulance AutoHeal residual: heal ally infantry in radius.
        // Fail-closed vs full AutoHealBehavior particle / world-anim pulse FX.
        self.update_ambulance_auto_heal_with_effects(dt, ui)?;
        self.update_default_auto_heal_with_effects(ui)?;
        // First TransportContain update creates InitialPayload before rider
        // regeneration. This Result must reach the native tick's caller.
        self.update_initial_transport_payloads_with_effects(ui)?;
        // C++ TransportContain::update HealthRegen%PerSec on embarked riders.
        self.update_transport_health_regen_with_effects(dt, ui)?;

        // Host China Propaganda / Speaker Tower residual: heal + ENTHUSIASTIC buff.
        // Fail-closed vs full PropagandaTowerBehavior sole-benefactor / PulseFX matrix.
        self.update_propaganda_tower_pulse_with_effects(dt, ui)?;
        self.update_overcharge_drain_with_effects(dt, ui)?;
        {
            self.update_power_plant_rods();
        }

        // C++ HordeUpdate::update for every authored HordeUpdate module.
        // Vehicles run every frame for the decal (UPDATE_SLEEP_NONE) with
        // membership gated on UpdateRate; infantry UPDATE_SLEEP(UpdateRate).
        self.update_horde_modules();

        // Authored primary shots use normal combat; the missile jammer is independent.
        self.update_ecm_missile_jam();

        // Host America Microwave Tank residual: DISABLE_SUBDUED on cooked structures.
        // Fail-closed vs full subdual accumulate/heal / laser stream / emitter field.
        {
            self.update_microwave_disable_with_effects(ui)?;
        }
        {
            self.update_microwave_emitter_field_with_effects(ui)?;
        }

        // Host PointDefenseLaser residual: Paladin / Avenger / King Raptor intercept missiles.
        // Fail-closed vs full PointDefenseLaserUpdate velocity prediction / laser FX.
        {
            self.update_point_defense_intercept_with_effects(ui)?;
        }
        {
            self.update_point_defense_laser_beam_objects_with_effects(ui);
        }
        {
            self.update_weapon_laser_beam_objects_with_effects(ui);
        }
        self.update_tomahawk_missile_projectiles_with_effects(ui)?;
        // Gate (latched counter): see the technical_rpg gate below.
        if self.fire_base_shells_spawned > 0 {
            self.update_fire_base_shell_projectiles_with_effects(ui)?;
        }
        // Gate (latched counter): see the technical_rpg gate above.
        if self.technical_cannon_shells_spawned > 0 {
            self.update_technical_cannon_shell_projectiles_with_effects(ui)?;
        }
        // Gate: no queued cleanup order -> take_orders/restore_orders are
        // provable no-ops and the per-order hazard scan never runs.
        if !self.cleanup_areas.orders().is_empty() {
            self.update_cleanup_area_orders_with_effects(ui)?;
        }
        // Gate (latched counter): see the technical_rpg gate above.
        if self.cleanup_stream_missiles_spawned > 0 {
            self.update_cleanup_stream_projectiles_with_effects(ui)?;
        }
        self.update_missile_defender_laser_beam_objects_with_effects(ui);

        // Host China EMP Pulse residual: DISABLED_EMP timers tick on objects in AI pass.
        // Activation is event-driven via DoSpecialPower (no continuous field).

        // Host China Frenzy ("Rage") residual: FRENZY weapon-bonus timers tick in AI pass.
        // Activation is event-driven via DoSpecialPower (no continuous generator field).

        // Host RadarScan residual: expire temporary FOW reveals (undo lookers).
        // RadarVanPing DeletionUpdate residual + FOW undo.
        {
            self.update_radar_van_pings_with_effects(ui)?;
        }
        self.update_radar_scans_with_effects(ui);

        // Host SpySatellite residual: expire temporary FOW reveals (undo lookers).
        // Fail-closed vs full OCL SpySatellitePing / DynamicShroudClearingRangeUpdate.
        {
            self.update_spy_satellite_pings_with_effects(ui)?;
        }
        self.update_spy_satellites_with_effects(ui);
        crate::command_executor::tick_live_beacon_client_updates_with_effects(self, ui);

        // Host CIA Intelligence residual: expire vision-spied marks + FOW undos.
        // Fail-closed vs full SpyVisionUpdate setUnitsVisionSpied module path.
        self.update_satellite_hack_spy_vision();
        self.update_cia_intelligence();

        // Host China FireWall residual: tick fire damage along wall segments.
        // FireWallSegment object DeletionUpdate residual + zone damage ticks.
        {
            self.update_firewall_segment_objects_with_effects(ui)?;
        }
        {
            self.update_firewalls_with_effects(ui)?;
        }

        // Host China Inferno Cannon residual: tick FireFieldSmall DoT at impact zones.
        // Fail-closed vs full InfernoTankShell projectile / OCL_FireFieldSmall spawn.
        {
            self.update_inferno_fire_zones_with_effects(ui)?;
        }
        {
            self.update_inferno_fire_field_objects_with_effects(ui)?;
        }

        // Host China Helix NapalmBomb residual: tick FirestormSmall DoT at drop zones.
        // Fail-closed vs full SpecialObject NapalmBomb fall / expand animation.
        // Gate: no active firestorm zone -> advance_geometry, the whole-world
        // position snapshot, and plan_due_ticks are provable no-ops.
        if !self.helix_napalm.active_zones().is_empty() {
            self.update_helix_napalm_firestorms_with_effects(ui)?;
        }
        {
            self.update_helix_napalm_bomb_projectiles_with_effects(ui);
        }

        // Nuclear Tanks SmallRadiationField residual ticks.
        {
            self.update_nuclear_tanks_radiation_zones_with_effects(ui)?;
        }

        // Only fixture tests populate this registry. Live impacts create authored field objects.
        #[cfg(test)]
        {
            self.update_scud_poison_zones_with_effects(ui)?;
        }
        self.update_tensile_formations_with_effects(ui);
        {
            self.update_fire_spread_with_effects(ui)?;
        }
        {
            self.update_base_regenerate_with_effects(ui)?;
        }
        self.update_supply_warehouse_crippling_with_effects(ui)?;

        {
            self.update_enemy_near();
        }
        {
            self.update_animation_steering();
        }
        {
            self.update_float_update_with_effects(ui)?;
        }
        {
            self.update_prone_update();
        }
        {
            self.update_radius_decal_update();
        }
        {
            self.update_checkpoint_update();
        }
        {
            self.update_smart_bomb_target_homing_with_effects(ui);
        }
        self.update_spectre_gunship_flights_with_effects(ui);
        self.update_fuel_air_gas_slow_death_with_effects(ui)?;
        self.update_neutron_missile_flights_with_effects(ui)?;
        self.update_scud_storm_missile_flights_with_effects(ui)?;
        {
            self.update_carpet_bomb_flights_with_effects(ui)?;
        }
        // The artillery carrier releases its payload into the shared projectile system.
        {
            self.update_artillery_barrage_flights_with_effects(ui)?;
        }
        {
            self.update_a10_strike_flights_with_effects(ui)?;
        }
        {
            self.update_daisy_cutter_flights_with_effects(ui)?;
        }
        {
            self.update_anthrax_bomb_flights_with_effects(ui)?;
        }
        {
            self.update_cluster_mines_flights_with_effects(ui)?;
        }
        {
            self.update_emp_pulse_spheroids_with_effects(ui)?;
            self.update_emp_pulse_flights_with_effects(ui)?;
        }
        // Gate: no frenzy marker spawns or due deletes -> the take-drain is a
        // state-identical no-op (both-empty mem::takes leave both empty).
        if !(self.frenzies.pending_marker_deletes.is_empty()
            && self.frenzies.markers_this_frame.is_empty())
        {
            self.update_frenzy_invisible_markers_with_effects(ui)?;
        }
        self.update_gps_scrambler_grow_with_effects(ui)?;
        // Gate: no growing spy-drone activation -> the fn early-returns
        // before any world work; skip the activation scan entirely.
        if self.spy_drones.activations_mut().iter().any(|a| a.growing) {
            self.update_spy_drone_grow();
        }
        // Gate: zero active nuke-cannon radiation zones -> the unconditional
        // whole-world object-position snapshot, plan_due_ticks, and
        // prune_expired are provable no-ops on the empty registry.
        if self.nuke_cannon_residual.active_count() > 0 {
            self.update_nuke_cannon_radiation_zones_with_effects(ui)?;
        }
        {
            self.update_toxin_tractor_poison_zones_with_effects(ui)?;
        }

        // Host America Aurora dive bomb residual: delayed area damage at target.
        // AuroraBombLocomotor flight residual + FuelAir gas OCL path.
        {
            self.update_aurora_bombs_with_effects(ui)?;
        }
        {
            self.update_aurora_bomb_projectiles_with_effects(ui);
        }

        // Host GLA Angry Mob residual: aggregate fire on nearby enemies + expand.
        // Fail-closed vs full SpawnBehavior member objects / MobMemberSlavedUpdate.
        self.update_angry_mobs_with_effects(ui)?;
        {
            self.update_angry_mob_projectiles_with_effects(ui)?;
        }

        // C++ RailroadBehavior::update (RailroadGuideAIUpdate.cpp:652–832).
        self.update_railroads_with_effects(ui);
        // C++ RailedTransportAIUpdate::update parks at the nearest End and
        // opens the dock on arrival.
        self.update_railed_transports();
        for &id in object_ids {
            if let Some(config) = self.railed_dock_config(id) {
                self.railed_dock_update(id, config, ui)
                    .map_err(generals_sim::GameLogicError::Configuration)?;
            }
        }

        // Host stealth residual: detector scans + DETECTED expiry.
        // Fail-closed vs full StealthUpdate/StealthDetectorUpdate modules
        // (no IR FX, kindof filters, or disguise).
        self.update_stealth_and_detection_with_effects(ui)?;

        // QueueProductionExitUpdate and SpawnBehavior are independent ordered
        // object modules. Capture births once so a replacement object with the
        // same ID, or a child created during spawning, is not ticked twice.
        let queue_epoch = self.visual_world_epoch;
        let queue_exit_updates: Vec<_> = self
            .objects
            .registered()
            .filter_map(|(&id, object)| {
                let metadata = object.get_template().production_exit_metadata?;
                metadata.is_queue().then(|| {
                    (
                        id,
                        object.visual_object_generation,
                        metadata,
                        object.get_template().queue_exit_precedes_spawn_behavior(),
                    )
                })
            })
            .collect();
        for (id, birth, metadata, before_spawn) in &queue_exit_updates {
            if *before_spawn
                && self.visual_world_epoch == queue_epoch
                && let Some(object) = self.objects.get_mut(id)
                && object.visual_object_generation == *birth
            {
                object.tick_queue_exit(metadata);
            }
        }
        self.update_spawn_behavior_with_effects(ui)?;
        for (id, birth, metadata, before_spawn) in &queue_exit_updates {
            if !*before_spawn
                && self.visual_world_epoch == queue_epoch
                && let Some(object) = self.objects.get_mut(id)
                && object.visual_object_generation == *birth
            {
                object.tick_queue_exit(metadata);
            }
        }

        // -----------------------------------------------------------------------
        // Phase 7: Combat Resolution (within object updates)
        // -----------------------------------------------------------------------
        // Weapon fire and damage application as part of the object update pass.
        self.update_combat_with_effects(object_ids, dt, ui)?;
        self.mirror_overlord_addon_damage_after_combat_with_effects(ui);
        self.flush_pending_garrison_really_damaged_ejects_with_effects(ui)?;
        self.flush_subdual_passenger_orders_with_effects(ui)?;
        // Nested AttackStateMachine residual (privateAttackObject enter path).
        let frame = self.frame;
        let t = frame as f32 * LOGIC_FRAME_TIMESTEP;
        self.tick_nested_attack_machines_with_effects(object_ids, t, frame, ui)?;
        self.tick_all_turret_state_machines_with_effects(object_ids, t, frame, ui)?;
        self.sync_attack_priority_from_script_engine();
        self.tick_mood_auto_acquire_with_effects(object_ids, ui)?;
        self.tick_attack_team_persist(object_ids);
        self.tick_attack_area_persist(object_ids);
        self.tick_out_of_ammo_jet_damage_with_effects(ui)?;
        self.tick_airfield_parking_heal_with_effects(ui)?;
        self.tick_airfield_runway_clear();
        // C++ PartitionManager collide residual (broadphase fail-closed O(n²)).
        // Physics motion runs first: its sole Euler step handles the ground
        // slam and admits the C++ bounce force; the shock pass then applies
        // stun bookkeeping on the post-contact state.
        self.integrate_owned_physics_motion_with_effects(ui)?;
        self.tick_shock_stun_all_with_effects(ui)?;

        // Admit accepted shots from this session's CombatSystem before flight.
        // The same owner applies ordered impact damage and body callbacks.

        self.materialize_owned_queued_projectiles_with_effects(ui)?;
        self.update_projectile_weapon_bonus_upgrades(None);
        self.apply_owned_projectileless_delayed_damage_with_effects(ui)?;

        // C++ Weapon::fireWeaponTemplate runs FireOCL at shot acceptance,
        // independently of whether the projectile later finds a target.
        self.execute_pending_weapon_fire_ocls_with_effects(ui)?;
        // Apply countermeasure diversion within the receiving projectile
        // operation, before committing damage; never repair health afterward.
        let projectile_hits = self.update_owned_projectile_impacts_with_effects(dt, ui)?;
        for victim in self.combat_system.take_pending_under_attack() {
            let _ = self.try_under_attack_from_damage(victim);
        }
        for victim in self.combat_system.take_pending_on_die() {
            let killer = self.objects.get(&victim).and_then(|o| {
                o.last_damage_source
                    .and_then(|src| self.objects.get(&src).map(|s| s.team))
            });
            self.mark_object_for_destruction_with_effects(victim, killer, ui)?;
        }
        self.flush_countermeasure_flare_spawns_with_effects(ui);
        {
            self.update_countermeasure_flare_objects_with_effects(ui);
        }
        self.drain_historic_bonus_firestorms();

        if !projectile_hits.is_empty() {
            // Presentation/audio residual: WeaponFire already queued on shot;
            // hit SFX is fail-closed via presentation audio events when present.
            let _ = projectile_hits;
        }
        // Apply Weapon.ini ProjectileDetonationFX and OCLs at impact.
        self.flush_projectile_impact_fx_with_effects(ui)?;
        // MissileAI exhaust attaches at IGNITION to this native manager;
        // its receiving lifecycle resolves actual Object/projectile poses.
        {
            let frame = self.frame;
            let snaps = self.combat_system.projectiles_snapshot();
            let mut exhausts: Vec<_> = snaps
                .into_iter()
                .filter_map(|p| {
                    let position = if p.missile_motion.is_some() {
                        let birth = p.projectile_actor_birth?;
                        if !self.spawn_identity_matches(p.id, birth.0, birth.1) {
                            return None;
                        }
                        self.objects.get(&p.id)?.get_position()
                    } else {
                        p.position
                    };
                    Some((
                        p.id,
                        p.shooter_id,
                        position,
                        p.live_exhaust_name().to_string(),
                    ))
                })
                .collect();
            // HashMap-backed projectile storage has no stable iteration order;
            // stable creation order keeps host particle identities deterministic.
            exhausts.sort_by_key(|(projectile_id, _, _, _)| *projectile_id);
            self.combat_particles
                .sync_projectile_exhausts(frame, &exhausts, ui);
            self.sync_live_state_particles(ui);
            // C++ ProjectileStreamUpdate residual: track projectile positions for stream draw.
            // Re-adding every live shot each frame must not flip-flop targets (that
            // would punch a hole every particle). Lock the stream to the newest
            // projectile's victim so a retarget inserts one INVALID hole.
            // C++ Weapon::m_projectileStreamID is per Weapon instance. Group by
            // (shooter, slot) so concurrent PRIMARY/SECONDARY ribbons stay distinct.
            let mut stream_feeds: Vec<(
                crate::game_logic::ObjectId,
                u8,
                crate::game_logic::ObjectId,
                String,
                glam::Vec3,
                Option<crate::game_logic::ObjectId>,
                glam::Vec3,
            )> = self
                .combat_system
                .projectiles_snapshot()
                .into_iter()
                .filter_map(|p| {
                    let position = if p.missile_motion.is_some() {
                        let birth = p.projectile_actor_birth?;
                        if !self.spawn_identity_matches(p.id, birth.0, birth.1) {
                            return None;
                        }
                        self.objects.get(&p.id)?.get_position()
                    } else {
                        p.position
                    };
                    let shooter = self.objects.get(&p.shooter_id);
                    let firing_name = if !p.historic_weapon_key.is_empty() {
                        Some(p.historic_weapon_key.as_str())
                    } else {
                        shooter.and_then(|o| o.authored_weapon_name_for_slot(o.last_fire_slot))
                    };
                    // C++ Weapon::newProjectileFired never searches another
                    // WeaponSet slot. Accepted shots retain their own name even
                    // after the shooter's current selection changes or it dies.
                    let (slot, sname) = match &p.stream_binding {
                        Some(binding) => (binding.slot, binding.name.clone()),
                        None => (
                            shooter.map(|o| o.last_fire_slot).unwrap_or(0),
                            firing_name
                                .map(crate::game_logic::weapon_bootstrap::host_projectile_stream_name_for_weapon_name)
                                .unwrap_or_default(),
                        ),
                    };
                    if sname.is_empty() {
                        return None;
                    }
                    Some((
                        p.shooter_id,
                        slot,
                        p.id,
                        sname,
                        position,
                        p.target_id,
                        p.target_position,
                    ))
                })
                .collect();
            stream_feeds.sort_by_key(|(shooter, slot, pid, _, _, _, _)| (*shooter, *slot, *pid));
            let mut idx = 0;
            while idx < stream_feeds.len() {
                let shooter_id = stream_feeds[idx].0;
                let slot = stream_feeds[idx].1;
                let end = stream_feeds[idx..]
                    .iter()
                    .position(|(sid, sl, _, _, _, _, _)| *sid != shooter_id || *sl != slot)
                    .map(|rel| idx + rel)
                    .unwrap_or(stream_feeds.len());
                let newest_target = stream_feeds[end - 1].5;
                let newest_pos = stream_feeds[end - 1].6;
                let sname = stream_feeds[idx].3.clone();
                for feed in &stream_feeds[idx..end] {
                    self.projectile_streams.add_projectile(
                        shooter_id,
                        slot,
                        &sname,
                        feed.4,
                        newest_target,
                        Some(newest_pos),
                        frame,
                    );
                }
                if let Some(owner) = self.objects.get(&shooter_id) {
                    if owner.is_kind_of(crate::game_logic::KindOf::Vehicle) {
                        let geom = &(*owner.geometry_info());
                        self.projectile_streams.apply_vehicle_roof_skim(
                            shooter_id,
                            slot,
                            owner.get_position(),
                            geom.major_radius,
                            geom.max_height_above_position(),
                        );
                    }
                }
                idx = end;
            }
            self.projectile_streams.cull_idle(frame, 45);
        }

        // -----------------------------------------------------------------------
        // Phase 7b: Building Body Damage State Checks (C++ BodyModule update)
        // -----------------------------------------------------------------------
        // C++ parity (GarrisonContain::onBodyDamageStateChange): when a garrisoned
        // building drops to ReallyDamaged health (<= 30%), all occupants are
        // force-ejected. This runs after combat so the health state is current.
        self.check_building_damage_states_with_effects(object_ids, ui)?;

        // Object AI is part of the object/module pass, before the system AI
        // queue drain (C++ GameLogic.cpp:3699-3743). The existing owned host
        // executes these module families directly.
        self.update_ai_with_effects(object_ids, dt, ui)?;
        Ok(())
    }

    fn run_frame_production(
        &mut self,
        dt: f32,
        ui: &mut ScriptPresentationEffects<'_>,
    ) -> GameLogicResult<()> {
        // Drain prior-frame upgrade presentation events before research advances.
        self.host_upgrades.clear_frame_events();
        self.update_production_with_effects(dt, ui)?;
        {
            self.update_player_upgrades_with_effects(ui);
        }
        Ok(())
    }

    fn run_frame_economy(
        &mut self,
        dt: f32,
        ui: &mut ScriptPresentationEffects<'_>,
    ) -> GameLogicResult<()> {
        self.update_player_resources(dt);
        // GLA Black Market residual cash (AutoDepositUpdate discrete deposits + floating text).
        // Fail-closed: not full InGameUI GPU / InitialCaptureBonus (retail 0).
        {
            self.update_black_market_deposits();
        }
        // Tech Oil Derrick residual cash (AutoDeposit + SupplyLines boost + floating text).
        // Fail-closed: not full InGameUI GPU / STEALTHED local display gate.
        {
            self.update_oil_derrick_deposits();
        }
        // China Hacker / Internet Center residual cash (HackInternetAIUpdate).
        // Fail-closed: not full unpack/pack state machine / variation factor.
        {
            self.update_hacker_income_with_effects(ui)?;
        }
        // America Supply Drop Zone residual: OCL schedules cargo DeliverPayload
        // residual; cash credits after approach delay + crate spawn.
        // Fail-closed: not full CreateAtEdge cargo plane / parachute fall path.
        {
            self.update_supply_drop_zone_drops();
        }
        // USA Battle Drone residual master repair: 12-unit weld SM, not instant 48-range.
        // Fail-closed: not full arm pack/unpack weld FX / RepairMinAltitude matrix.
        // CommandCenter / RadarVan radar-online residual (Player::hasRadar).
        // Fail-closed: not full RadarUpgrade grant / power-brownout disable-proof path.
        // Recompute radar for each controlling Player after economy updates.
        self.update_player_radar_with_effects(ui);
        self.drain_masked_object_selection();
        self.update_power_disabled_state();
        Ok(())
    }

    fn run_frame_retirements(
        &mut self,
        ui: &mut ScriptPresentationEffects<'_>,
    ) -> GameLogicResult<()> {
        // Same-frame cascades: iterator re-evaluates end() so objects queued
        // during deletion are processed this logic frame (GameLogic.cpp:2449-2510).
        self.fire_temporary_weapons_for_pending_deaths_with_effects(ui)?;
        self.process_destroy_list_with_effects_checked(ui)?;
        // C++ BridgeBehavior/Tower onDamage/onHealing/onDie + scaffold rise tick.
        // Residual drain is same-frame after object combat/AI/repair and destroy.
        self.sync_host_bridge_rubble_and_scaffolds_with_effects(ui)?;
        Ok(())
    }
}
