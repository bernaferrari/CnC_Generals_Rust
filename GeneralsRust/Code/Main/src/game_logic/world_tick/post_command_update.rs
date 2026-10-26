//! Source post-command update phases; one kernel for an ordinary or resumed tick.
//! GameLogic.cpp3672–3803. Extraction alone changes no clear dispatch behavior.
use super::*;

impl GameLogic {
    pub(super) fn update_simulation_after_commands(
        &mut self,
        dt: f32,
        mut ui: crate::game_logic::ScriptPresentationEffects<'_>,
        object_ids: Vec<ObjectId>,
        source_now: u32,
        after_reset: bool,
    ) -> SimulationStepOutcome {
        // -----------------------------------------------------------------------
        // Phase 6: Object Updates -- Sleepy heap (C++ GameLogic.cpp:3699-3740)
        // -----------------------------------------------------------------------
        // C++ ticks UpdateModules via m_sleepyUpdates (wake_frame, phase).
        // These remaining system obligations share the indexed heap mechanics.
        // Keep the original entry installed through its callback, then finish
        // it before inspecting the next due entry. Per-object module admission
        // and projectile phase dispatch remain tracked in hq-om90a.
        // C++ clamps registration wake frames to 1, but compares them with
        // the actual current frame. Clamping the tick repeats frame 1.
        let now = source_now;
        while let Some(token) = self.host_sleepy.next_due(now) {
            match token.kind {
                super::super::HostSleepyKind::Construction => {
                    self.update_construction_with_effects(&object_ids, dt, &mut ui);
                }
                super::super::HostSleepyKind::SellList => {
                    self.update_sell_list_with_effects(&mut ui);
                }
                super::super::HostSleepyKind::DozerBoredRepair => {
                    self.update_dozer_bored_repair();
                }
                super::super::HostSleepyKind::RebuildHoles => {
                    self.update_rebuild_holes_with_effects(&mut ui);
                }
                super::super::HostSleepyKind::Movement => {
                    // C++ AIUpdate updates the active state before
                    // AIUpdate::doLocomotor. Panic continuation must therefore
                    // inspect the path completed by the preceding locomotor
                    // pass, then install the next waypoint before this pass.
                    self.begin_host_waypoint_ai_observations(&object_ids);
                    if let Err(error) =
                        self.tick_host_panic_states_with_effects(&object_ids, &mut ui)
                    {
                        return SimulationStepOutcome::Failed(error);
                    }
                    self.tick_host_guard_states_with_effects(&object_ids, &mut ui);
                    if let Err(error) = self.update_movement_with_effects(&object_ids, dt, &mut ui)
                    {
                        return SimulationStepOutcome::Failed(error);
                    }
                }
                super::super::HostSleepyKind::SpecialPowers => {
                    update_special_powers();
                }
            }
            self.host_sleepy.finish(token, now, 1);
        }

        // Host superweapon residual: complete queued DaisyCutter / A10 / Scud /
        // ParticleCannon / NuclearMissile / AnthraxBomb / SpectreGunship /
        // CarpetBomb / ArtilleryBarrage / CruiseMissile strikes (area / line /
        // multi-shell / loft-MOAB damage + nuke radiation / anthrax toxin /
        // spectre orbit). Fail-closed vs full OCL aircraft / NeutronMissileUpdate
        // / PoisonField stack / B52 DeliverPayload / door loft path.
        // Wave 827: host-sole unless movement authority live: host system residuals sole-tick after GW writeback.
        {
            if let Err(error) = self.update_special_power_strikes_with_effects(&mut ui) {
                return SimulationStepOutcome::Failed(error);
            }
        }
        // Gate: no radiation field spawned this frame -> the residual is a
        // pure no-op (its own pending build early-returns empty). Skip the
        // registry joins entirely.
        if !self
            .special_power_strikes
            .radiation_spawned_this_frame()
            .is_empty()
        {
            self.spawn_nuke_radiation_field_objects_for_new_fields_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::NUKE_RADIATION_FIELDS);
        }
        // Wave 802: host-sole unless movement authority live: field-object lifetime is owned by
        // GW tick_status_timer_expirations + expire logs.
        {
            self.update_nuke_radiation_field_objects_with_effects(&mut ui);
        }
        // Gate: mirror of the radiation gate above, for toxin fields.
        if !self
            .special_power_strikes
            .toxin_spawned_this_frame()
            .is_empty()
        {
            self.spawn_anthrax_toxin_field_objects_for_new_fields_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::ANTHRAX_TOXIN_FIELDS);
        }
        // Wave 802: host-sole unless movement authority live: field-object lifetime is owned by
        // GW tick_status_timer_expirations + expire logs.
        {
            self.update_anthrax_toxin_field_objects_with_effects(&mut ui);
        }

        // Host America Paradrop residual: spawn infantry after approach delay.
        // Fail-closed vs full OCL cargo plane / parachute payload path.
        // Wave 796: host-sole unless movement authority live: Paradrop cargo flight is owned by
        // GW tick_status_timer_expirations + drop/ground logs.
        {
            self.update_paradrop_cargo_planes_with_effects(&mut ui);
        }
        // Gate: zero paradrop missions -> clear_frame_events/plan_due_drops
        // are provable no-ops (events are pushed only alongside a mission
        // insert; the missions map is insert-only). Skip the pass.
        if self.host_paradrops.mission_count() > 0 {
            self.update_paradrops_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::PARADROPS);
        }

        // Host DeliverPayload cargo residual: DropDelay-staggered spawn of payload
        // units at location (Supply Drop Zone crates). Fail-closed vs full aircraft.
        // Gate: zero deliver-payload missions -> frame-event clears, the
        // cargo-flight tick, and the spawn plan are provable no-ops (all
        // side queues are populated only alongside a mission insert).
        if self.host_deliver_payloads.mission_count() > 0 {
            self.update_deliver_payloads_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::DELIVER_PAYLOADS);
        }

        // Host MoneyCrateCollide residual: unit + BuildingPickup cash collect.
        // Wave 826: host-sole unless movement authority live: combat/field residuals sole-tick after GW writeback.
        {
            self.update_money_crate_collides_with_effects(&mut ui);
        }
        // Wave 817: host-sole unless movement authority live: crate lifetime owned by GW expire + logs.
        {
            if let Err(error) = self.update_crate_deletion_updates_with_effects(&mut ui) {
                return SimulationStepOutcome::Failed(error);
            }
        }

        // Host GLA Rebel Ambush residual: leftover OCLSpecialPower CreateObject
        // (science UpgradeOCL) plus FadeIn/DiesOnBadLand after the fire-frame spawn.
        // Gate: zero ambush missions -> fade-clear drain and spawn plan are
        // provable no-ops (pending_fade_clears is pushed only after a live
        // ambush spawn, which requires a mission).
        if self.host_ambushes.mission_count() > 0 {
            self.update_ambushes_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::AMBUSHES);
        }

        // Host USA Leaflet Drop residual: disable enemy infantry/vehicles after Delay.
        // Fail-closed vs full OCL B52 / LeafletContainer / LeafletFX particle path.
        // Wave 795: host-sole unless movement authority live: Leaflet B52 flight is owned by
        // GW tick_status_timer_expirations + drop/ground logs.
        {
            self.update_leaflet_b52_flights_with_effects(&mut ui);
        }
        // Gate: zero leaflet missions -> the per-plan world radius scan never
        // runs (plan_due_impacts includes Completed missions for walk-in
        // re-pulse, so mission_count — any phase — is the only safe test).
        if self.host_leaflet_drops.mission_count() > 0 {
            self.update_leaflet_drops();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::LEAFLET_DROPS);
        }

        // Host GLA Sneak Attack residual: spawn tunnel + shockwave after Lifetime delay.
        // Fail-closed vs full OCL Start animation / TunnelContain path.
        // Gate: zero sneak-attack missions -> shockwave drain and spawn plan
        // are provable no-ops (pending_shockwaves is pushed only alongside a
        // mission insert).
        if self.host_sneak_attacks.mission_count() > 0 {
            self.update_sneak_attacks_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::SNEAK_ATTACKS);
        }

        // Host mine / demo-trap residual: proximity trigger + timed detonation.
        // Fail-closed vs full MinefieldBehavior / DemoTrapUpdate modules.
        // Wave 826: host-sole unless movement authority live: combat/field residuals sole-tick after GW writeback.
        {
            self.update_mines_and_demo_traps_with_effects(&mut ui);
        }

        // Host USA Ambulance AutoHeal residual: heal ally infantry in radius.
        // Fail-closed vs full AutoHealBehavior particle / world-anim pulse FX.
        self.update_ambulance_auto_heal_with_effects(dt, &mut ui);
        self.update_default_auto_heal_with_effects(&mut ui);
        // First TransportContain update creates InitialPayload before rider
        // regeneration. This Result must reach the native tick's caller.
        if let Err(error) = self.update_initial_transport_payloads_with_effects(&mut ui) {
            return SimulationStepOutcome::Failed(error);
        }
        // C++ TransportContain::update HealthRegen%PerSec on embarked riders.
        self.update_transport_health_regen(dt);

        // Host China Propaganda / Speaker Tower residual: heal + ENTHUSIASTIC buff.
        // Fail-closed vs full PropagandaTowerBehavior sole-benefactor / PulseFX matrix.
        self.update_propaganda_tower_pulse_with_effects(dt, &mut ui);
        self.update_overcharge_drain_with_effects(dt, &mut ui);
        // Wave 810: host-sole unless movement authority live: rods completion owned by GW expire + logs.
        {
            self.update_power_plant_rods();
        }

        // Host China vehicle HordeUpdate residual. Vehicles run every frame for
        // decal size (UPDATE_SLEEP_NONE) but membership is gated on UpdateRate.
        // Wave 812: host-sole unless movement authority live: horde status owned by GW expire + logs.
        {
            self.update_battlemaster_horde_status();
        }

        // Host China infantry HordeUpdate residual. Infantry UPDATE_SLEEP(UpdateRate).
        // Wave 813: host-sole unless movement authority live: horde status owned by GW expire + logs.

        {
            self.update_china_infantry_horde_status();
        }

        // Host China ECM Tank / jammer residual: jam enemy weapons in radius.
        // Fail-closed vs full subdual damage accumulate / laser stream / missile scatter.
        self.update_ecm_jam_field_with_effects(&mut ui);
        self.update_ecm_missile_jam();

        // Host America Microwave Tank residual: DISABLE_SUBDUED on cooked structures.
        // Fail-closed vs full subdual accumulate/heal / laser stream / emitter field.
        // Wave 825: host-sole unless movement authority live: zone/field damage sole-ticks after GW writeback.
        {
            self.update_microwave_disable_with_effects(&mut ui);
        }
        // Wave 825: host-sole unless movement authority live: zone/field damage sole-ticks after GW writeback.
        {
            self.update_microwave_emitter_field_with_effects(&mut ui);
        }

        // Host PointDefenseLaser residual: Paladin / Avenger / King Raptor intercept missiles.
        // Fail-closed vs full PointDefenseLaserUpdate velocity prediction / laser FX.
        // Wave 826: host-sole unless movement authority live: combat/field residuals sole-tick after GW writeback.
        {
            self.update_point_defense_intercept_with_effects(&mut ui);
        }
        // Wave 806: host-sole unless movement authority live: lifetime owned by GW expire + logs.
        {
            self.update_point_defense_laser_beam_objects_with_effects(&mut ui);
        }
        // Wave 806: host-sole unless movement authority live: lifetime owned by GW expire + logs.
        {
            self.update_weapon_laser_beam_objects_with_effects(&mut ui);
        }
        // Wave 804: host-sole unless movement authority live: residual owned by GW expire + logs.
        {
            self.update_comanche_rocket_pod_projectiles_with_effects(&mut ui);
        }
        self.update_stealth_jet_missile_projectiles_with_effects(&mut ui);
        // Wave 800: host-sole unless movement authority live: cannon shell flight is owned by
        // GW tick_status_timer_expirations + impact logs.
        {
            self.update_scud_launcher_missile_projectiles_with_effects(&mut ui);
        }
        self.update_tomahawk_missile_projectiles_with_effects(&mut ui);
        self.update_rocket_buggy_missile_projectiles_with_effects(&mut ui);
        // Wave 800: host-sole unless movement authority live: cannon shell flight is owned by
        // GW tick_status_timer_expirations + impact logs.
        {
            self.update_neutron_cannon_shell_projectiles_with_effects(&mut ui);
        }
        // Gate (latched counter): see the technical_rpg gate below.
        if self.rpg_trooper_missiles_spawned > 0 {
            self.update_rpg_trooper_missile_projectiles_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::RPG_TROOPER_MISSILES);
        }
        // Gate (latched counter): see the technical_rpg gate below.
        if self.tank_hunter_missiles_spawned > 0 {
            self.update_tank_hunter_missile_projectiles_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::TANK_HUNTER_MISSILES);
        }
        // Gate (latched counter): see the technical_rpg gate below.
        if self.missile_defender_missiles_spawned > 0 {
            self.update_missile_defender_missile_projectiles_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::MISSILE_DEFENDER_MISSILES);
        }
        // Gate (latched counter): see the technical_rpg gate below.
        if self.scorpion_shells_spawned > 0 {
            self.update_scorpion_shell_projectiles_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::SCORPION_SHELLS);
        }
        // Wave 805: host-sole unless movement authority live: residual owned by GW expire + logs.
        {
            self.update_scorpion_missile_projectiles_with_effects(&mut ui);
        }
        // Wave 800: host-sole unless movement authority live: cannon shell flight is owned by
        // GW tick_status_timer_expirations + impact logs.
        {
            self.update_nuke_cannon_shell_projectiles_with_effects(&mut ui);
        }
        self.update_usa_tank_shell_projectiles_with_effects(&mut ui);
        // Gate (latched counter): see the technical_rpg gate below.
        if self.battlemaster_shells_spawned > 0 {
            self.update_battlemaster_shell_projectiles_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::BATTLEMASTER_SHELLS);
        }
        // Gate (latched counter): see the technical_rpg gate below.
        if self.overlord_shells_spawned > 0 {
            self.update_overlord_shell_projectiles_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::OVERLORD_SHELLS);
        }
        // Wave 803: host-sole unless movement authority live: residual owned by GW expire + logs.
        {
            self.update_inferno_shell_projectiles_with_effects(&mut ui);
        }
        // Gate (latched counter): see the technical_rpg gate below.
        if self.marauder_shells_spawned > 0 {
            self.update_marauder_shell_projectiles_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::MARAUDER_SHELLS);
        }
        // Gate (latched counter): see the technical_rpg gate below.
        if self.fire_base_shells_spawned > 0 {
            self.update_fire_base_shell_projectiles_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::FIRE_BASE_SHELLS);
        }
        // Gate (latched counter): see the technical_rpg gate below.
        if self.raptor_missiles_spawned > 0 {
            self.update_raptor_missile_projectiles_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::RAPTOR_MISSILES);
        }
        // Gate (latched counter): see the technical_rpg gate below.
        if self.mig_missiles_spawned > 0 {
            self.update_mig_missile_projectiles_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::MIG_MISSILES);
        }
        // Wave 804: host-sole unless movement authority live: residual owned by GW expire + logs.
        {
            self.update_flashbang_grenade_projectiles_with_effects(&mut ui);
        }
        // Gate (latched counter): see the technical_rpg gate below —
        // counter==0 proves the flag never got set, scan matches nothing.
        if self.humvee_tow_missiles_spawned > 0 {
            self.update_humvee_tow_missile_projectiles_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::HUMVEE_TOW_MISSILES);
        }
        // Gate (latched counter): see the technical_rpg gate below.
        if self.dragon_flame_missiles_spawned > 0 {
            self.update_dragon_flame_projectiles_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::DRAGON_FLAME_MISSILES);
        }
        // Wave 798: host-sole unless movement authority live: ToxinStream projectile flight is owned by
        // GW tick_status_timer_expirations + impact/stream logs.
        {
            self.update_toxin_stream_projectiles_with_effects(&mut ui);
        }
        // Gate (latched counter): this projectile type exists only after its
        // first spawn — the spawn fn sets the object flag and bumps the
        // counter together, and the flag is never persisted, so counter==0
        // proves the whole-world scan below matches nothing.
        if self.technical_rpg_missiles_spawned > 0 {
            self.update_technical_rpg_missile_projectiles_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::TECHNICAL_RPG_MISSILES);
        }
        // Gate (latched counter): see the technical_rpg gate above.
        if self.technical_cannon_shells_spawned > 0 {
            self.update_technical_cannon_shell_projectiles_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::TECHNICAL_CANNON_SHELLS);
        }
        // Gate: no queued cleanup order -> take_orders/restore_orders are
        // provable no-ops and the per-order hazard scan never runs.
        if !self.cleanup_areas.orders().is_empty() {
            self.update_cleanup_area_orders_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::CLEANUP_AREA_ORDERS);
        }
        // Gate (latched counter): see the technical_rpg gate above.
        if self.cleanup_stream_missiles_spawned > 0 {
            self.update_cleanup_stream_projectiles_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::CLEANUP_STREAM_MISSILES);
        }
        self.update_missile_defender_laser_beam_objects_with_effects(&mut ui);

        // Host China EMP Pulse residual: DISABLED_EMP timers tick on objects in AI pass.
        // Activation is event-driven via DoSpecialPower (no continuous field).

        // Host China Frenzy ("Rage") residual: FRENZY weapon-bonus timers tick in AI pass.
        // Activation is event-driven via DoSpecialPower (no continuous generator field).

        // Host RadarScan residual: expire temporary FOW reveals (undo lookers).
        // RadarVanPing DeletionUpdate residual + FOW undo.
        // Wave 809: host-sole unless movement authority live: lifetime owned by GW expire + logs.
        {
            if let Err(error) = self.update_radar_van_pings_with_effects(&mut ui) {
                return SimulationStepOutcome::Failed(error);
            }
        }
        self.update_radar_scans_with_effects(&mut ui);

        // Host SpySatellite residual: expire temporary FOW reveals (undo lookers).
        // Fail-closed vs full OCL SpySatellitePing / DynamicShroudClearingRangeUpdate.
        // Wave 803: host-sole unless movement authority live: residual owned by GW expire + logs.
        {
            if let Err(error) = self.update_spy_satellite_pings_with_effects(&mut ui) {
                return SimulationStepOutcome::Failed(error);
            }
        }
        self.update_spy_satellites_with_effects(&mut ui);
        crate::command_executor::tick_live_beacon_client_updates_with_effects(self, &mut ui);

        // Host CIA Intelligence residual: expire vision-spied marks + FOW undos.
        // Fail-closed vs full SpyVisionUpdate setUnitsVisionSpied module path.
        self.update_satellite_hack_spy_vision();
        self.update_cia_intelligence();

        // Host China FireWall residual: tick fire damage along wall segments.
        // FireWallSegment object DeletionUpdate residual + zone damage ticks.
        // Wave 826: host-sole unless movement authority live: combat/field residuals sole-tick after GW writeback.
        {
            if let Err(error) = self.update_firewall_segment_objects_with_effects(&mut ui) {
                return SimulationStepOutcome::Failed(error);
            }
        }
        // Wave 825: host-sole unless movement authority live: zone/field damage sole-ticks after GW writeback.
        {
            self.update_firewalls_with_effects(&mut ui);
        }

        // Host China Inferno Cannon residual: tick FireFieldSmall DoT at impact zones.
        // Fail-closed vs full InfernoTankShell projectile / OCL_FireFieldSmall spawn.
        // Wave 825: host-sole unless movement authority live: zone/field damage sole-ticks after GW writeback.
        {
            self.update_inferno_fire_zones_with_effects(&mut ui);
        }
        // Wave 802: host-sole unless movement authority live: field-object lifetime is owned by
        // GW tick_status_timer_expirations + expire logs.
        {
            self.update_inferno_fire_field_objects_with_effects(&mut ui);
        }

        // Host China Helix NapalmBomb residual: tick FirestormSmall DoT at drop zones.
        // Fail-closed vs full SpecialObject NapalmBomb fall / expand animation.
        // Gate: no active firestorm zone -> advance_geometry, the whole-world
        // position snapshot, and plan_due_ticks are provable no-ops.
        if !self.helix_napalm.active_zones().is_empty() {
            self.update_helix_napalm_firestorms_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::HELIX_NAPALM_FIRESTORMS);
        }
        // Wave 804: host-sole unless movement authority live: residual owned by GW expire + logs.
        {
            self.update_helix_napalm_bomb_projectiles_with_effects(&mut ui);
        }

        // Host GLA Bomb Truck BioBomb residual: tick MediumPoisonField DoT.
        // Fail-closed vs full FireWeaponWhenDead exclusive effect matrix.
        // Gate: no active poison zone -> the object-position snapshot and
        // plan_due_ticks are provable no-ops (registry iterates only zones).
        if !self.bomb_truck_detonate.active_poison_zones().is_empty() {
            self.update_bomb_truck_poison_zones_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::BOMB_TRUCK_POISON_ZONES);
        }
        // Nuclear Tanks SmallRadiationField residual ticks.
        // Wave 825: host-sole unless movement authority live: zone/field damage sole-ticks after GW writeback.
        {
            self.update_nuclear_tanks_radiation_zones_with_effects(&mut ui);
        }

        // Host GLA SCUD toxin residual: tick MediumPoisonField DoT at impact zones.
        // Fail-closed vs full OCL_PoisonFieldMedium object spawn / particle bones.
        // Wave 825: host-sole unless movement authority live: zone/field damage sole-ticks after GW writeback.
        {
            self.update_scud_poison_zones_with_effects(&mut ui);
        }
        self.update_tensile_formations_with_effects(&mut ui);
        // Wave 820: host-sole unless movement authority live: fire-spread owned by GW expire + logs.
        {
            self.update_fire_spread_with_effects(&mut ui);
        }
        // Wave 780: BaseRegenerateUpdate is host-sole (C++ has one store; the
        // GW dual-peel heal tick is retired with wave780). Host skips only
        // while damage authority is live and the shadow session last-writes HP.
        {
            self.update_base_regenerate_with_effects(&mut ui);
        }
        self.update_supply_warehouse_crippling();

        // Wave 781: host-sole unless movement authority live: EnemyNearUpdate is owned by
        // GW tick_status_timer_expirations + writeback.
        {
            self.update_enemy_near();
        }
        // Wave 784: host-sole unless movement authority live: AnimationSteeringUpdate is owned by
        // GW tick_status_timer_expirations + writeback.
        {
            self.update_animation_steering();
        }
        // Wave 783: host-sole unless movement authority live: FloatUpdate is owned by
        // GW tick_status_timer_expirations + transform writeback.
        {
            self.update_float_update_with_effects(&mut ui);
        }
        // Wave 782: host-sole unless movement authority live: ProneUpdate is owned by
        // GW tick_status_timer_expirations + writeback.
        {
            self.update_prone_update();
        }
        // Wave 785: host-sole unless movement authority live: RadiusDecalUpdate is owned by
        // GW tick_status_timer_expirations + writeback.
        {
            self.update_radius_decal_update();
        }
        // Wave 786: host-sole unless movement authority live: CheckpointUpdate is owned by
        // GW tick_status_timer_expirations + writeback.
        {
            self.update_checkpoint_update();
        }
        // Wave 787: host-sole unless movement authority live: SmartBombTargetHomingUpdate is owned by
        // GW tick_status_timer_expirations + transform writeback.
        {
            self.update_smart_bomb_target_homing_with_effects(&mut ui);
        }
        self.update_spectre_gunship_flights_with_effects(&mut ui);
        self.update_fuel_air_gas_slow_death_with_effects(&mut ui);
        self.update_neutron_missile_flights_with_effects(&mut ui);
        self.update_scud_storm_missile_flights_with_effects(&mut ui);
        // Wave 794: host-sole unless movement authority live: CarpetBomb flight is owned by
        // GW tick_status_timer_expirations + drop/detonate logs.
        {
            self.update_carpet_bomb_flights_with_effects(&mut ui);
        }
        // Wave 793: host-sole unless movement authority live: ArtilleryBarrage flight is owned by
        // GW tick_status_timer_expirations + drop/detonate logs.
        {
            self.update_artillery_barrage_flights_with_effects(&mut ui);
        }
        // Wave 792: host-sole unless movement authority live: A10 strike flight is owned by
        // GW tick_status_timer_expirations + drop/detonate logs.
        {
            self.update_a10_strike_flights_with_effects(&mut ui);
        }
        // Wave 788: host-sole unless movement authority live: DaisyCutter/MOAB flight is owned by
        // GW tick_status_timer_expirations + drop/detonate logs.
        {
            self.update_daisy_cutter_flights_with_effects(&mut ui);
        }
        // Wave 789: host-sole unless movement authority live: AnthraxBomb flight is owned by
        // GW tick_status_timer_expirations + drop/detonate logs.
        {
            if let Err(error) = self.update_anthrax_bomb_flights_with_effects(&mut ui) {
                return SimulationStepOutcome::Failed(error);
            }
        }
        // Wave 790: host-sole unless movement authority live: ClusterMines flight is owned by
        // GW tick_status_timer_expirations + drop/detonate logs.
        {
            if let Err(error) = self.update_cluster_mines_flights_with_effects(&mut ui) {
                return SimulationStepOutcome::Failed(error);
            }
        }
        // Wave 791: host-sole unless movement authority live: EMP Pulse flight + spheroid is owned by
        // GW tick_status_timer_expirations + drop/detonate/expire logs.
        {
            if let Err(error) = self.update_emp_pulse_spheroids_with_effects(&mut ui) {
                return SimulationStepOutcome::Failed(error);
            }
            if let Err(error) = self.update_emp_pulse_flights_with_effects(&mut ui) {
                return SimulationStepOutcome::Failed(error);
            }
        }
        // Gate: no frenzy marker spawns or due deletes -> the take-drain is a
        // state-identical no-op (both-empty mem::takes leave both empty).
        if !(self.frenzies.pending_marker_deletes.is_empty()
            && self.frenzies.markers_this_frame.is_empty())
        {
            if let Err(error) = self.update_frenzy_invisible_markers_with_effects(&mut ui) {
                return SimulationStepOutcome::Failed(error);
            }
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::FRENZY_MARKERS);
        }
        if let Err(error) = self.update_gps_scrambler_grow_with_effects(&mut ui) {
            return SimulationStepOutcome::Failed(error);
        }
        // Gate: no growing spy-drone activation -> the fn early-returns
        // before any world work; skip the activation scan entirely.
        if self.spy_drones.activations_mut().iter().any(|a| a.growing) {
            self.update_spy_drone_grow();
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::SPY_DRONE_GROW);
        }
        // Gate: zero active nuke-cannon radiation zones -> the unconditional
        // whole-world object-position snapshot, plan_due_ticks, and
        // prune_expired are provable no-ops on the empty registry.
        if self.nuke_cannon_residual.active_count() > 0 {
            self.update_nuke_cannon_radiation_zones_with_effects(&mut ui);
        } else {
            #[cfg(test)]
            residual_gate_seam::note(&residual_gate_seam::NUKE_CANNON_ZONES);
        }
        self.tick_fire_ocl_after_weapon_cooldown();
        // Wave 825: host-sole unless movement authority live: zone/field damage sole-ticks after GW writeback.
        {
            self.update_toxin_tractor_poison_zones_with_effects(&mut ui);
        }

        // Host America Aurora dive bomb residual: delayed area damage at target.
        // AuroraBombLocomotor flight residual + FuelAir gas OCL path.
        // Wave 826: host-sole unless movement authority live: combat/field residuals sole-tick after GW writeback.
        {
            self.update_aurora_bombs_with_effects(&mut ui);
        }
        // Wave 797: host-sole unless movement authority live: AuroraBomb projectile dive is owned by
        // GW tick_status_timer_expirations + destroy logs.
        {
            self.update_aurora_bomb_projectiles_with_effects(&mut ui);
        }

        // Host GLA Angry Mob residual: aggregate fire on nearby enemies + expand.
        // Fail-closed vs full SpawnBehavior member objects / MobMemberSlavedUpdate.
        self.update_angry_mobs_with_effects(&mut ui);
        // Wave 799: host-sole unless movement authority live: AngryMob projectile flight is owned by
        // GW tick_status_timer_expirations + impact logs.
        {
            self.update_angry_mob_projectiles_with_effects(&mut ui);
        }

        // Host RailroadBehavior residual: locomotives follow track waypoints,
        // wait at stations, hitch carriages. Crate RailroadGuideAIUpdate is
        // gated empty (OBJECT_REGISTRY) so this is the live path.
        // C++ RailroadBehavior::update (RailroadGuideAIUpdate.cpp:652-832).
        self.update_railroads_with_effects(&mut ui);
        // C++ RailedTransportAIUpdate::update — park at nearest End, open dock
        // when the current End is reached. Crate leftover is dual-world gated.
        self.update_railed_transports();

        // Host stealth residual: detector scans + DETECTED expiry.
        // Fail-closed vs full StealthUpdate/StealthDetectorUpdate modules
        // (no IR FX, kindof filters, or disguise).
        self.update_stealth_and_detection_with_effects(&mut ui);

        // Stinger HiveStructureBody SpawnReplaceDelay residual slave respawns.
        // Wave 814: host-sole unless movement authority live: hive respawn owned by GW expire + logs.
        {
            self.update_stinger_hive_respawns_with_effects(&mut ui);
        }

        // -----------------------------------------------------------------------
        // Phase 7: Combat Resolution (within object updates)
        // -----------------------------------------------------------------------
        // Weapon fire and damage application as part of the object update pass.
        self.update_combat_with_effects(&object_ids, dt, &mut ui);
        self.mirror_overlord_addon_damage_after_combat_with_effects(&mut ui);
        self.flush_pending_garrison_really_damaged_ejects_with_effects(&mut ui);
        if let Err(error) = self.flush_subdual_passenger_orders_with_effects(&mut ui) {
            return SimulationStepOutcome::Failed(error);
        }
        // Nested AttackStateMachine residual (privateAttackObject enter path).
        let frame = self.frame;
        let t = frame as f32 * LOGIC_FRAME_TIMESTEP;
        if let Err(error) =
            self.tick_nested_attack_machines_with_effects(&object_ids, t, frame, &mut ui)
        {
            return SimulationStepOutcome::Failed(error);
        }
        self.tick_all_turret_state_machines_with_effects(&object_ids, t, frame, &mut ui);
        self.sync_attack_priority_from_script_engine();
        self.tick_mood_auto_acquire(&object_ids);
        self.tick_attack_team_persist(&object_ids);
        self.tick_attack_area_persist(&object_ids);
        if let Err(error) = self.tick_out_of_ammo_jet_damage_with_effects(&mut ui) {
            return SimulationStepOutcome::Failed(error);
        }
        if let Err(error) = self.tick_airfield_parking_heal_with_effects(&mut ui) {
            return SimulationStepOutcome::Failed(error);
        }
        self.tick_airfield_runway_clear();
        self.tick_shock_stun_all_with_effects(&mut ui);
        // C++ PartitionManager collide residual (broadphase fail-closed O(n²)).
        self.integrate_owned_physics_motion_with_effects(&mut ui);

        // Admit accepted shots from this session's CombatSystem before flight.
        // The same owner applies ordered impact damage and body callbacks.

        {
            let objects = &self.objects;
            let projectile_terrain = crate::game_logic::game_logic::FlightTerrainView {
                grid: &self.pathfinding_system.grid,
                logical: Some(self.world_services.terrain()),
                terrain: self.terrain.as_ref(),
                samples: self.pathfinding_height_samples.as_ref(),
            };
            crate::game_logic::combat::drain_pending_projectiles_with_terrain(
                &mut self.combat_system,
                objects.read(),
                self.frame,
                Some(&projectile_terrain),
                &mut self.next_object_id,
            );
        }
        self.apply_owned_projectileless_delayed_damage_with_effects(&mut ui);

        // C++ Weapon::fireWeaponTemplate runs FireOCL at shot acceptance,
        // independently of whether the projectile later finds a target.
        self.execute_pending_weapon_fire_ocls_with_effects(&mut ui);
        // Apply countermeasure diversion within the receiving projectile
        // operation, before committing damage; never repair health afterward.
        let projectile_hits = self.update_owned_projectile_impacts_with_effects(dt, &mut ui);
        for victim in self.combat_system.take_pending_under_attack() {
            let _ = self.try_under_attack_from_damage(victim);
        }
        for victim in self.combat_system.take_pending_on_die() {
            let killer = self.objects.get(&victim).and_then(|o| {
                o.last_damage_source
                    .and_then(|src| self.objects.get(&src).map(|s| s.team))
            });
            if let Err(error) =
                self.mark_object_for_destruction_with_effects(victim, killer, &mut ui)
            {
                return SimulationStepOutcome::Failed(error);
            }
        }
        // Wave 470: countermeasure flare spawn/object residual stays host-owned
        // even when GameWorld sole-integrates projectile flight.
        self.flush_countermeasure_flare_spawns_with_effects(&mut ui);
        // Wave 806: host-sole unless movement authority live: lifetime owned by GW expire + logs.
        {
            self.update_countermeasure_flare_objects_with_effects(&mut ui);
        }
        self.drain_historic_bonus_firestorms();

        if !projectile_hits.is_empty() {
            // Presentation/audio residual: WeaponFire already queued on shot;
            // hit SFX is fail-closed via presentation audio events when present.
            let _ = projectile_hits;
        }
        // C++ Weapon.ini ProjectileDetonationFX/OCL residual at real impact
        // (not fire-time), for either host or GameWorld flight ownership.
        self.flush_projectile_impact_fx_with_effects(&mut ui);
        // MissileAI exhaust attaches at IGNITION to this native manager;
        // its receiving lifecycle resolves actual Object/projectile poses.
        {
            let frame = self.frame;
            let snaps = self.combat_system.projectiles_snapshot();
            let mut exhausts: Vec<_> = snaps
                .into_iter()
                .map(|p| {
                    (
                        p.id,
                        p.shooter_id,
                        p.position,
                        p.live_exhaust_name().to_string(),
                    )
                })
                .collect();
            // HashMap-backed projectile storage has no stable iteration order;
            // stable creation order keeps host particle identities deterministic.
            exhausts.sort_by_key(|(projectile_id, _, _, _)| *projectile_id);
            self.combat_particles
                .sync_projectile_exhausts(frame, &exhausts, &mut ui);
            self.sync_live_state_particles(&mut ui);
            // C++ ProjectileStreamUpdate residual: track projectile positions for stream draw.
            // Re-adding every live shot each frame must not flip-flop targets (that
            // would punch a hole every particle). Lock the stream to the newest
            // projectile's victim so a retarget inserts one INVALID hole.
            let mut stream_feeds: Vec<(
                crate::game_logic::ObjectId,
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
                    let shooter = self.objects.get(&p.shooter_id);
                    let firing_name = if !p.historic_weapon_key.is_empty() {
                        Some(p.historic_weapon_key.as_str())
                    } else {
                        shooter.and_then(|o| o.authored_weapon_name_for_slot(o.last_fire_slot))
                    };
                    // C++ Weapon::newProjectileFired never searches another
                    // WeaponSet slot. Accepted shots retain their own name even
                    // after the shooter's current selection changes or it dies.
                    let sname = match &p.stream_binding {
                        Some(binding) => binding.name.clone(),
                        None => firing_name
                            .map(crate::game_logic::weapon_bootstrap::host_projectile_stream_name_for_weapon_name)
                            .unwrap_or_default(),
                    };
                    if sname.is_empty() {
                        return None;
                    }
                    Some((
                        p.shooter_id,
                        p.id,
                        sname,
                        p.position,
                        p.target_id,
                        p.target_position,
                    ))
                })
                .collect();
            stream_feeds.sort_by_key(|(shooter, pid, _, _, _, _)| (*shooter, *pid));
            let mut idx = 0;
            while idx < stream_feeds.len() {
                let shooter_id = stream_feeds[idx].0;
                let end = stream_feeds[idx..]
                    .iter()
                    .position(|(sid, _, _, _, _, _)| *sid != shooter_id)
                    .map(|rel| idx + rel)
                    .unwrap_or(stream_feeds.len());
                let newest_target = stream_feeds[end - 1].4;
                let newest_pos = stream_feeds[end - 1].5;
                let sname = stream_feeds[idx].2.clone();
                for feed in &stream_feeds[idx..end] {
                    self.projectile_streams.add_projectile(
                        shooter_id,
                        &sname,
                        feed.3,
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
        self.check_building_damage_states_with_effects(&object_ids, &mut ui);

        // Object AI is part of the object/module pass, before the system AI
        // queue drain (C++ GameLogic.cpp:3699-3743). The existing owned host
        // executes these module families directly.
        if let Err(error) = self.update_ai_with_effects(&object_ids, dt, &mut ui) {
            return SimulationStepOutcome::Failed(error);
        }

        // Phase 8: C++ AI::update — pathfinding, then this match's players.
        if let Err(error) = self.update_match_ai_with_effects(&mut ui) {
            return SimulationStepOutcome::Failed(error);
        }
        if let Some(error) = self.team_creation_error() {
            return SimulationStepOutcome::Failed(error);
        }

        // Phase 8b: Apply commands queued by AI this frame (C++ CommandList drain
        // after TheAI->UPDATE). Ensures same-frame set_target/move logs reach
        // shadow/presentation without a second engine-side process_commands.
        if after_reset {
            self.process_queued_commands_without_recorder(&mut ui);
        } else {
            self.process_commands_with_effects(&mut ui);
        }

        // -----------------------------------------------------------------------
        // Phase 9: Production / Build Assistant (C++ line 3748)
        // -----------------------------------------------------------------------
        // C++: TheBuildAssistant->UPDATE();
        // Production queues update after AI so build orders issued by AI this
        // frame can be immediately reflected.
        // Drain prior-frame upgrade presentation events before research advances.
        self.host_upgrades.clear_frame_events();
        if let Err(error) = self.update_production_with_effects(dt, &mut ui) {
            return SimulationStepOutcome::Failed(error);
        }
        // Wave 827: host-sole unless movement authority live: host system residuals sole-tick after GW writeback.
        {
            self.update_player_upgrades_with_effects(&mut ui);
        }
        // Skip crate BuildAssistant::update on the live tick. Its update() only
        // walks an in-memory mock sell-list and never resolves host objects
        // (Common/build_assistant.rs: "Mock object lookup - in real
        // implementation this would find the object"). Host sell/production
        // is update_production above. Keep the BuildAssistant type for C++
        // parity / tests — do not delete it.

        // -----------------------------------------------------------------------
        // Phase 10: Player Resources
        // -----------------------------------------------------------------------
        self.update_player_resources(dt);
        // GLA Black Market residual cash (AutoDepositUpdate discrete deposits + floating text).
        // Fail-closed: not full InGameUI GPU / InitialCaptureBonus (retail 0).
        // Wave 821: host-sole unless movement authority live: AutoDeposit owned by GW expire + logs.
        {
            self.update_black_market_deposits();
        }
        // Tech Oil Derrick residual cash (AutoDeposit + SupplyLines boost + floating text).
        // Fail-closed: not full InGameUI GPU / STEALTHED local display gate.
        // Wave 821: host-sole unless movement authority live: AutoDeposit owned by GW expire + logs.
        {
            self.update_oil_derrick_deposits();
        }
        // China Hacker / Internet Center residual cash (HackInternetAIUpdate).
        // Fail-closed: not full unpack/pack state machine / variation factor.
        // Wave 822: host-sole unless movement authority live: Hacker income owned by GW expire + logs.
        {
            if let Err(error) = self.update_hacker_income_with_effects(&mut ui) {
                return SimulationStepOutcome::Failed(error);
            }
        }
        // America Supply Drop Zone residual: OCL schedules cargo DeliverPayload
        // residual; cash credits after approach delay + crate spawn.
        // Fail-closed: not full CreateAtEdge cargo plane / parachute fall path.
        // Wave 826: host-sole unless movement authority live: combat/field residuals sole-tick after GW writeback.
        {
            self.update_supply_drop_zone_drops();
        }
        // USA Battle Drone residual master repair: 12-unit weld SM, not instant 48-range.
        // Fail-closed: not full arm pack/unpack weld FX / RepairMinAltitude matrix.
        // CommandCenter / RadarVan radar-online residual (Player::hasRadar).
        // Fail-closed: not full RadarUpgrade grant / power-brownout disable-proof path.
        // Radar belongs to each controlling Player. Recompute at the ordinary
        // host phase; a presentation/comparison shadow cannot grant radar.
        self.update_player_radar_with_effects(&mut ui);
        self.drain_masked_object_selection();
        self.update_power_disabled_state();

        // -----------------------------------------------------------------------
        // Phase 11: Damage/Physics Resolution
        // -----------------------------------------------------------------------
        // Deferred damage and collision resolution after all objects have moved.
        // (Covered above in update_combat; kept as a documentation marker.)

        // -----------------------------------------------------------------------
        // Phase 12: Partition Manager Update (C++ line 3753)
        // -----------------------------------------------------------------------
        // C++: ThePartitionManager->UPDATE();
        // Spatial partition updated AFTER all objects moved and BEFORE death
        // cleanup so spatial queries during cleanup use correct positions.
        self.update_owned_partition_with_effects(&mut ui);

        // -----------------------------------------------------------------------
        // Phase 13: Death/Cleanup (C++ line 3762)
        // -----------------------------------------------------------------------
        // C++: processDestroyList();
        // Same-frame cascades: iterator re-evaluates end() so objects queued
        // during deletion are processed this logic frame (GameLogic.cpp:2449-2510).
        self.fire_temporary_weapons_for_pending_deaths_with_effects(&mut ui);
        if let Err(error) = self.process_destroy_list_with_effects_checked(&mut ui) {
            return SimulationStepOutcome::Failed(error);
        }
        // C++ BridgeBehavior/Tower onDamage/onHealing/onDie + scaffold rise tick.
        // Residual drain is same-frame after object combat/AI/repair and destroy.
        if let Err(error) = self.sync_host_bridge_rubble_and_scaffolds_with_effects(&mut ui) {
            return SimulationStepOutcome::Failed(error);
        }

        // -----------------------------------------------------------------------
        // Phase 14: Weapon Store Update (C++ line 3767)
        // -----------------------------------------------------------------------
        // C++: TheWeaponStore->UPDATE();
        // Process delayed weapon damage that is now ready.
        self.apply_owned_projectileless_delayed_damage_with_effects(&mut ui);
        match with_weapon_store_mut(|store| store.take_due_delayed_damage()) {
            Ok(due) => {
                for info in due {
                    if let Err(e) = gamelogic::weapon::WeaponStore::apply_delayed_damage(info) {
                        log::warn!("Delayed weapon damage failed: {}", e);
                    }
                }
            }
            Err(e) => {
                let err_str = e.to_string();
                if !err_str.contains("not initialized") {
                    log::warn!("Weapon store update failed: {}", e);
                }
            }
        }

        // -----------------------------------------------------------------------
        // Phase 14b: Locomotor Store Update (C++ line 3768)
        // -----------------------------------------------------------------------
        // C++: TheLocomotorStore->UPDATE();
        // The Rust locomotor store is a template registry without per-frame
        // update logic yet, but we keep the call site for C++ parity.
        // (Will become a real call once the locomotor store gains an update method.)

        // -----------------------------------------------------------------------
        // Phase 15: Victory Conditions (C++ line 3769)
        // -----------------------------------------------------------------------
        // C++: TheVictoryConditions->UPDATE();
        // This logic phase owns evaluation; presentation and probes only observe.
        let _ = self.evaluate_victory_condition_with_effects(&mut ui);

        // -----------------------------------------------------------------------
        // Phase 16: Disabled Status Check (C++ lines 3783-3792)
        // -----------------------------------------------------------------------
        // C++: for( Object *obj = m_objList; obj; obj = obj->getNextObject() )
        // C++:   if( obj->isDisabled() ) obj->checkDisabledStatus();
        self.expire_owned_disabled_statuses_with_effects(&mut ui);
        self.check_bridge_disabled_statuses();

        // -----------------------------------------------------------------------
        // Phase 17: Vision/Shroud Update
        // -----------------------------------------------------------------------
        // The gamelogic crate's ShroudManager only sees objects registered in the
        // gamelogic OBJECT_REGISTRY.  Main-crate objects live in a separate
        // HashMap, so we feed their vision ranges directly into the shroud grid
        // here so fog-of-war actually works for the playable game.
        // C++ parity: TheVision (shroud reveal) runs every logic frame on the
        // DEFAULT retail path — vision is never gated off.  The GameWorld shadow
        // has no shroud/looker channel of its own (crush-vision is a value-only
        // writeback of vision_range / shroud_clearing_range / crushed flags, it
        // never stamps ShroudManager or partition lookers), so skipping this
        // pass under coupled dual-tick left the shroud grid and
        // player_visible_objects unfed on the live path.  Stamping is
        // idempotent per looker (unchanged position/range/mask short-circuits
        // in restamp_host_partition_look), so the Wave 827 post-writeback
        // sole-tick re-run (tick_host_systems_residuals_sole) cannot
        // double-reveal, and a crush-vision writeback only shifts the value the
        // next frame's look pass reads (one-frame latency, no ordering hazard).
        // Wave 827: host-sole unless movement authority live: host system residuals sole-tick after GW writeback.
        self.update_main_crate_vision();
        // Presentation FOW consumers fail open only until this first completed
        // vision pass of the session (boot window); afterwards they derive from
        // real membership (fow_rendering::shroud_runtime_active).

        // C++ Radar.cpp overlay queries live Object pose/stealth each update.
        self.host_radar_sync_live_objects_with_effects(&mut ui);

        // -----------------------------------------------------------------------
        // Phase 18: Team Events Flush
        // -----------------------------------------------------------------------
        // Handled by the gamelogic crate's update_pipeline.

        // -----------------------------------------------------------------------
        // Post-phase: EVA voice announcements
        // -----------------------------------------------------------------------

        // -----------------------------------------------------------------------
        // Post-phase: Audio events
        // -----------------------------------------------------------------------

        // C++ GameLogic.cpp:3795-3803: the full update completed, so the step
        // loop owns the m_frame++ / sim-time advancement.
        SimulationStepOutcome::Advanced
    }
}
