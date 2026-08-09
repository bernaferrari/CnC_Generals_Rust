//! Additional `impl GameLogic` methods. Child of `game_logic.rs`.
#![allow(unused_imports, non_snake_case)]
use super::*;

impl GameLogic {
    /// Main update loop with delta time
    pub fn update_with_dt(&mut self, dt: f32) -> SimTimingSnapshot {
        self.step_simulation(dt, None);
        self.sim_timing_snapshot()
    }

    pub fn update_with_timing(&mut self, timing: &FrameTiming) -> SimTimingSnapshot {
        self.step_simulation(timing.delta_seconds(), Some(timing.total_seconds()));
        self.sim_timing_snapshot()
    }

    /// Headless/host residual: bound fixed-step catch-up so a stalled GPU present
    /// cannot dump dozens of logic frames into one drive_frame (UI freeze residual).
    pub fn update_with_dt_budget(&mut self, dt: f32, max_fixed_steps: usize) -> SimTimingSnapshot {
        self.step_simulation_with_budget(dt, None, Some(max_fixed_steps.max(1)));
        self.sim_timing_snapshot()
    }

    pub fn update_with_timing_budget(
        &mut self,
        timing: &FrameTiming,
        max_fixed_steps: usize,
    ) -> SimTimingSnapshot {
        self.step_simulation_with_budget(
            timing.delta_seconds(),
            Some(timing.total_seconds()),
            Some(max_fixed_steps.max(1)),
        );
        self.sim_timing_snapshot()
    }

    /// Wave 923: single host logic-tick boundary (dt/timing + optional budget).
    #[inline]
    pub fn tick_logic_frame(
        &mut self,
        dt: f32,
        timing: Option<&FrameTiming>,
        budget: Option<usize>,
    ) -> SimTimingSnapshot {
        match (budget, timing) {
            (Some(b), Some(t)) => self.update_with_timing_budget(t, b),
            (Some(b), None) => self.update_with_dt_budget(dt, b),
            (None, Some(t)) => self.update_with_timing(t),
            (None, None) => self.update_with_dt(dt),
        }
    }

    /// Menu/shell update path that bounds fixed-step catch-up work per frame.
    /// This prevents multi-second UI stalls after startup while still advancing shell scripts.
    pub fn update_shell_with_budget(
        &mut self,
        dt: f32,
        max_fixed_steps: usize,
    ) -> SimTimingSnapshot {
        self.step_simulation_with_budget(dt, None, Some(max_fixed_steps.max(1)));
        self.sim_timing_snapshot()
    }

    pub(super) fn step_simulation(&mut self, delta_time: f32, absolute_time: Option<f32>) {
        self.step_simulation_with_budget(delta_time, absolute_time, None);
    }

    pub(super) fn step_simulation_with_budget(
        &mut self,
        delta_time: f32,
        absolute_time: Option<f32>,
        max_fixed_steps: Option<usize>,
    ) {
        if self.is_paused {
            return;
        }

        self.accumulated_time += delta_time;

        const FIXED_TIMESTEP: f32 = LOGIC_FRAME_TIMESTEP;

        let mut steps_run = 0usize;
        let mut budget_hit = false;
        while self.accumulated_time >= FIXED_TIMESTEP {
            if let Some(step_budget) = max_fixed_steps {
                if steps_run >= step_budget {
                    budget_hit = true;
                    break;
                }
            }
            self.update_simulation(FIXED_TIMESTEP);
            self.accumulated_time -= FIXED_TIMESTEP;
            self.frame += 1;
            self.sim_time_seconds += FIXED_TIMESTEP;
            steps_run += 1;
        }

        if let Some(total_seconds) = absolute_time {
            self.sim_time_seconds = total_seconds.max(self.sim_time_seconds);
        }

        self.last_fixed_step_diagnostics = FixedStepDiagnostics {
            steps_run,
            budget_hit,
            accumulated_time_seconds: self.accumulated_time,
        };

        self.process_destroy_list();
    }

    /// Execute one simulation step.
    ///
    /// Phase ordering follows C++ GameLogic::update() (GameLogic.cpp lines 3548-3803)
    /// as documented in gamelogic::system::game_logic::GameLogic::update():
    ///
    /// ```text
    /// Line 3595: setFrame / sync to GameClient       [frame setup]
    /// Line 3600: TheScriptEngine->UPDATE()            [early scripting]
    /// Line 3603: freezeTime check
    /// Line 3622: TheTerrainLogic->UPDATE()            [terrain/bridges]
    /// Line 3669: processCommandList                   [command processing]
    /// Line 3672: ALLOW_NONSLEEPY_UPDATES loop         [normal modules]
    /// Line 3697: sleepy updates loop                  [sleepy modules]
    /// Line 3743: TheAI->UPDATE()                      [AI]
    /// Line 3748: TheBuildAssistant->UPDATE()          [production]
    /// Line 3753: ThePartitionManager->UPDATE()        [spatial]
    /// Line 3762: processDestroyList()                 [death/cleanup]
    /// Line 3765: TheCommandList->reset()
    /// Line 3767: TheWeaponStore->UPDATE()             [weapons]
    /// Line 3768: TheLocomotorStore->UPDATE()          [locomotors]
    /// Line 3769: TheVictoryConditions->UPDATE()       [victory]
    /// Line 3783: disabled status check                [re-enable]
    /// Line 3799: m_frame++                            [increment]
    /// ```
    pub(super) fn update_simulation(&mut self, dt: f32) {
        // Pathfinding dynamic obstacles rebuild once per host logic frame.
        self.pathfinding_system.note_logic_frame(self.frame as u64);
        // -----------------------------------------------------------------------
        // Phase 1: Early Scripting (C++ line 3600)
        // -----------------------------------------------------------------------
        // C++: TheScriptEngine->UPDATE();
        // Scripts run BEFORE everything else so they can react to the previous
        // frame's state and issue commands for this frame.
        self.evaluate_and_execute_scripts(dt);

        // -----------------------------------------------------------------------
        // Phase 2: Time Freeze Check (C++ lines 3603-3617)
        // -----------------------------------------------------------------------
        // C++: if (freezeTime) { ... return; }
        // When time is frozen, only scripts evaluated above are allowed to run.
        if self.is_time_frozen_for_simulation() {
            return;
        }

        // -----------------------------------------------------------------------
        // Phase 3: Terrain Update (C++ line 3622)
        // -----------------------------------------------------------------------
        // C++: TheTerrainLogic->UPDATE();
        // Terrain (bridges, dynamic water, trigger areas) updates BEFORE objects
        // so bridge state changes from scripts are reflected during the object pass.
        if let Ok(mut terrain) = gamelogic::terrain::get_terrain_logic().write() {
            terrain.update();
        }
        // C++ TerrainLogic water-rise damage residual (edge dry→wet under units).
        // Full setWaterHeight scripts call apply_water_rise_damage directly with
        // their damageAmount; this catches map/dynamic water transitions.
        let _ = self.refresh_surface_cells_and_water_edge_damage(25.0);

        // -----------------------------------------------------------------------
        // Phase 4: Pre-Update / Collect object IDs
        // -----------------------------------------------------------------------
        let object_ids: Vec<ObjectId> = self.objects.keys().copied().collect();

        // -----------------------------------------------------------------------
        // Phase 5: Command Processing (C++ line 3669)
        // -----------------------------------------------------------------------
        // C++: processCommandList( TheCommandList );
        // Process queued player commands BEFORE object updates so movement/attack
        // orders are in effect when objects run their updates.
        self.process_commands();

        // -----------------------------------------------------------------------
        // Phase 6: Object Updates -- Normal + Sleepy Modules (C++ lines 3672-3738)
        // -----------------------------------------------------------------------
        // C++: ALLOW_NONSLEEPY_UPDATES loop, then sleepy updates loop.
        // These include construction, movement, and the simplified per-object AI
        // decision logic. Stealth modules also live in the sleepy update queue.
        self.update_construction(&object_ids, dt);
        // C++ BuildAssistant::update sell list residual (multi-frame sell).
        // Wave 827: under coupled shadow, host system residuals sole-tick after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_sell_list();
        }
        // C++ DozerPrimaryIdleState bored auto-repair residual.
        // Wave 819: under coupled shadow, idle timer owned by GW; host applies acquire via logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_dozer_bored_repair();
        }
        // C++ RebuildHoleBehavior update residual.
        self.update_rebuild_holes();
        self.update_movement(&object_ids, dt);

        // Special power cooldown/timer updates
        update_special_powers();

        // Host superweapon residual: complete queued DaisyCutter / A10 / Scud /
        // ParticleCannon / NuclearMissile / AnthraxBomb / SpectreGunship /
        // CarpetBomb / ArtilleryBarrage / CruiseMissile strikes (area / line /
        // multi-shell / loft-MOAB damage + nuke radiation / anthrax toxin /
        // spectre orbit). Fail-closed vs full OCL aircraft / NeutronMissileUpdate
        // / PoisonField stack / B52 DeliverPayload / door loft path.
        // Wave 827: under coupled shadow, host system residuals sole-tick after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_special_power_strikes();
        }
        self.spawn_nuke_radiation_field_objects_for_new_fields();
        // Wave 802: under coupled shadow, field-object lifetime is owned by
        // GW tick_status_timer_expirations + expire logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_nuke_radiation_field_objects();
        }
        self.spawn_anthrax_toxin_field_objects_for_new_fields();
        // Wave 802: under coupled shadow, field-object lifetime is owned by
        // GW tick_status_timer_expirations + expire logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_anthrax_toxin_field_objects();
        }

        // Host America Paradrop residual: spawn infantry after approach delay.
        // Fail-closed vs full OCL cargo plane / parachute payload path.
        // Wave 796: under coupled shadow, Paradrop cargo flight is owned by
        // GW tick_status_timer_expirations + drop/ground logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_paradrop_cargo_planes();
        }
        self.update_paradrops();

        // Host DeliverPayload cargo residual: DropDelay-staggered spawn of payload
        // units at location (Supply Drop Zone crates). Fail-closed vs full aircraft.
        self.update_deliver_payloads();

        // Host MoneyCrateCollide residual: unit + BuildingPickup cash collect.
        // Wave 826: under coupled shadow, combat/field residuals sole-tick after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_money_crate_collides();
        }
        // Wave 817: under coupled shadow, crate lifetime owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_crate_deletion_updates();
        }

        // Host GLA Rebel Ambush residual: spawn infantry near target after fade delay.
        // Fail-closed vs full OCL CreateObject / science upgrade tiers.
        self.update_ambushes();

        // Host USA Leaflet Drop residual: disable enemy infantry/vehicles after Delay.
        // Fail-closed vs full OCL B52 / LeafletContainer / LeafletFX particle path.
        // Wave 795: under coupled shadow, Leaflet B52 flight is owned by
        // GW tick_status_timer_expirations + drop/ground logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_leaflet_b52_flights();
        }
        self.update_leaflet_drops();

        // Host GLA Sneak Attack residual: spawn tunnel + shockwave after Lifetime delay.
        // Fail-closed vs full OCL Start animation / multi-shockwave / TunnelContain path.
        self.update_sneak_attacks();

        // Host mine / demo-trap residual: proximity trigger + timed detonation.
        // Fail-closed vs full MinefieldBehavior / DemoTrapUpdate modules.
        // Wave 826: under coupled shadow, combat/field residuals sole-tick after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_mines_and_demo_traps();
        }

        // Host USA Ambulance AutoHeal residual: heal ally infantry in radius.
        // Fail-closed vs full AutoHealBehavior sole-benefactor / vehicle matrix.
        self.update_ambulance_auto_heal(dt);

        // Host China Propaganda / Speaker Tower residual: heal + ENTHUSIASTIC buff.
        // Fail-closed vs full PropagandaTowerBehavior sole-benefactor / PulseFX matrix.
        self.update_propaganda_tower_pulse(dt);
        self.update_overcharge_drain(dt);
        // Wave 810: under coupled shadow, rods completion owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_power_plant_rods();
        }

        // Host China Battlemaster HordeUpdate residual (ExactMatch allies Radius 75 / Count 5).
        // Fail-closed vs full RubOffRadius honorary / terrain-decal flag matrix.
        // Retail UpdateRate 1000ms — residual rechecks each frame when battlemasters exist.
        // Wave 812: under coupled shadow, horde status owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_battlemaster_horde_status();
        }

        // Host China infantry HordeUpdate residual (Red Guard / Tank Hunter, Radius 30 / Count 5).
        // Fail-closed vs full RubOffRadius honorary / terrain-decal flag matrix.
        // Wave 813: under coupled shadow, horde status owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_china_infantry_horde_status();
        }

        // Host China ECM Tank / jammer residual: jam enemy weapons in radius.
        // Fail-closed vs full subdual damage accumulate / laser stream / missile scatter.
        self.update_ecm_jam_field();
        self.update_ecm_missile_jam();

        // Host America Microwave Tank residual: DISABLE_SUBDUED on cooked structures.
        // Fail-closed vs full subdual accumulate/heal / laser stream / emitter field.
        // Wave 825: under coupled shadow, zone/field damage sole-ticks after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_microwave_disable();
        }
        // Wave 825: under coupled shadow, zone/field damage sole-ticks after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_microwave_emitter_field();
        }

        // Host PointDefenseLaser residual: Paladin / Avenger / King Raptor intercept missiles.
        // Fail-closed vs full PointDefenseLaserUpdate velocity prediction / laser FX.
        // Wave 826: under coupled shadow, combat/field residuals sole-tick after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_point_defense_intercept();
        }
        // Wave 806: under coupled shadow, lifetime owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_point_defense_laser_beam_objects();
        }
        // Wave 806: under coupled shadow, lifetime owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_weapon_laser_beam_objects();
        }
        // Wave 804: under coupled shadow, residual owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_comanche_rocket_pod_projectiles();
        }
        self.update_stealth_jet_missile_projectiles();
        // Wave 800: under coupled shadow, cannon shell flight is owned by
        // GW tick_status_timer_expirations + impact logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_scud_launcher_missile_projectiles();
        }
        self.update_tomahawk_missile_projectiles();
        self.update_rocket_buggy_missile_projectiles();
        // Wave 800: under coupled shadow, cannon shell flight is owned by
        // GW tick_status_timer_expirations + impact logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_neutron_cannon_shell_projectiles();
        }
        self.update_rpg_trooper_missile_projectiles();
        self.update_tank_hunter_missile_projectiles();
        self.update_missile_defender_missile_projectiles();
        self.update_scorpion_shell_projectiles();
        // Wave 805: under coupled shadow, residual owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_scorpion_missile_projectiles();
        }
        // Wave 800: under coupled shadow, cannon shell flight is owned by
        // GW tick_status_timer_expirations + impact logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_nuke_cannon_shell_projectiles();
        }
        self.update_usa_tank_shell_projectiles();
        self.update_battlemaster_shell_projectiles();
        self.update_overlord_shell_projectiles();
        // Wave 803: under coupled shadow, residual owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_inferno_shell_projectiles();
        }
        self.update_marauder_shell_projectiles();
        self.update_fire_base_shell_projectiles();
        self.update_raptor_missile_projectiles();
        self.update_mig_missile_projectiles();
        // Wave 804: under coupled shadow, residual owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_flashbang_grenade_projectiles();
        }
        self.update_humvee_tow_missile_projectiles();
        self.update_dragon_flame_projectiles();
        // Wave 798: under coupled shadow, ToxinStream projectile flight is owned by
        // GW tick_status_timer_expirations + impact/stream logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_toxin_stream_projectiles();
        }
        self.update_technical_rpg_missile_projectiles();
        self.update_technical_cannon_shell_projectiles();
        self.update_cleanup_stream_projectiles();
        self.update_missile_defender_laser_beam_objects();

        // Host China EMP Pulse residual: DISABLED_EMP timers tick on objects in AI pass.
        // Activation is event-driven via DoSpecialPower (no continuous field).

        // Host China Frenzy ("Rage") residual: FRENZY weapon-bonus timers tick in AI pass.
        // Activation is event-driven via DoSpecialPower (no continuous generator field).

        // Host RadarScan residual: expire temporary FOW reveals (undo lookers).
        // RadarVanPing DeletionUpdate residual + FOW undo.
        // Wave 809: under coupled shadow, lifetime owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_radar_van_pings();
        }
        self.update_radar_scans();

        // Host SpySatellite residual: expire temporary FOW reveals (undo lookers).
        // Fail-closed vs full OCL SpySatellitePing / DynamicShroudClearingRangeUpdate.
        // Wave 803: under coupled shadow, residual owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_spy_satellite_pings();
        }
        self.update_spy_satellites();

        // Host CIA Intelligence residual: expire vision-spied marks + FOW undos.
        // Fail-closed vs full SpyVisionUpdate setUnitsVisionSpied module path.
        self.update_cia_intelligence();

        // Host China FireWall residual: tick fire damage along wall segments.
        // FireWallSegment object DeletionUpdate residual + zone damage ticks.
        // Wave 826: under coupled shadow, combat/field residuals sole-tick after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_firewall_segment_objects();
        }
        // Wave 825: under coupled shadow, zone/field damage sole-ticks after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_firewalls();
        }

        // Host China Inferno Cannon residual: tick FireFieldSmall DoT at impact zones.
        // Fail-closed vs full InfernoTankShell projectile / OCL_FireFieldSmall spawn.
        // Wave 825: under coupled shadow, zone/field damage sole-ticks after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_inferno_fire_zones();
        }
        // Wave 802: under coupled shadow, field-object lifetime is owned by
        // GW tick_status_timer_expirations + expire logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_inferno_fire_field_objects();
        }

        // Host China Helix NapalmBomb residual: tick FirestormSmall DoT at drop zones.
        // Fail-closed vs full SpecialObject NapalmBomb fall / expand animation.
        self.update_helix_napalm_firestorms();
        // Wave 804: under coupled shadow, residual owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_helix_napalm_bomb_projectiles();
        }

        // Host GLA Bomb Truck BioBomb residual: tick MediumPoisonField DoT.
        // Fail-closed vs full FireWeaponWhenDead exclusive effect matrix.
        self.update_bomb_truck_poison_zones();
        // Nuclear Tanks SmallRadiationField residual ticks.
        // Wave 825: under coupled shadow, zone/field damage sole-ticks after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_nuclear_tanks_radiation_zones();
        }

        // Host GLA SCUD toxin residual: tick MediumPoisonField DoT at impact zones.
        // Fail-closed vs full OCL_PoisonFieldMedium object spawn / particle bones.
        // Wave 825: under coupled shadow, zone/field damage sole-ticks after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_scud_poison_zones();
        }
        self.update_tensile_formations();
        // Wave 820: under coupled shadow, fire-spread owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_fire_spread();
        }
        // Wave 780: under coupled/damage-auth, BaseRegenerateUpdate is owned by
        // GW tick_status_timer_expirations + host_heal_log drain.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && (crate::gameworld_shadow::shadow_coupled_tick_active()
                || crate::gameworld_shadow::gameworld_damage_authority_live()))
        {
            self.update_base_regenerate();
        }
        // Wave 781: under coupled shadow, EnemyNearUpdate is owned by
        // GW tick_status_timer_expirations + writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_enemy_near();
        }
        // Wave 784: under coupled shadow, AnimationSteeringUpdate is owned by
        // GW tick_status_timer_expirations + writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_animation_steering();
        }
        // Wave 783: under coupled shadow, FloatUpdate is owned by
        // GW tick_status_timer_expirations + transform writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_float_update();
        }
        // Wave 782: under coupled shadow, ProneUpdate is owned by
        // GW tick_status_timer_expirations + writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_prone_update();
        }
        // Wave 785: under coupled shadow, RadiusDecalUpdate is owned by
        // GW tick_status_timer_expirations + writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_radius_decal_update();
        }
        // Wave 786: under coupled shadow, CheckpointUpdate is owned by
        // GW tick_status_timer_expirations + writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_checkpoint_update();
        }
        // Wave 787: under coupled shadow, SmartBombTargetHomingUpdate is owned by
        // GW tick_status_timer_expirations + transform writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_smart_bomb_target_homing();
        }
        self.update_fuel_air_gas_slow_death();
        self.update_neutron_missile_flights();
        self.update_scud_storm_missile_flights();
        // Wave 794: under coupled shadow, CarpetBomb flight is owned by
        // GW tick_status_timer_expirations + drop/detonate logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_carpet_bomb_flights();
        }
        // Wave 793: under coupled shadow, ArtilleryBarrage flight is owned by
        // GW tick_status_timer_expirations + drop/detonate logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_artillery_barrage_flights();
        }
        // Wave 792: under coupled shadow, A10 strike flight is owned by
        // GW tick_status_timer_expirations + drop/detonate logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_a10_strike_flights();
        }
        // Wave 788: under coupled shadow, DaisyCutter/MOAB flight is owned by
        // GW tick_status_timer_expirations + drop/detonate logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_daisy_cutter_flights();
        }
        // Wave 789: under coupled shadow, AnthraxBomb flight is owned by
        // GW tick_status_timer_expirations + drop/detonate logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_anthrax_bomb_flights();
        }
        // Wave 790: under coupled shadow, ClusterMines flight is owned by
        // GW tick_status_timer_expirations + drop/detonate logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_cluster_mines_flights();
        }
        // Wave 791: under coupled shadow, EMP Pulse flight + spheroid is owned by
        // GW tick_status_timer_expirations + drop/detonate/expire logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_emp_pulse_spheroids();
            self.update_emp_pulse_flights();
        }
        self.update_frenzy_invisible_markers();
        self.update_gps_scrambler_grow();
        self.update_spy_drone_grow();
        self.update_nuke_cannon_radiation_zones();
        self.tick_fire_ocl_after_weapon_cooldown();
        // Wave 825: under coupled shadow, zone/field damage sole-ticks after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_toxin_tractor_poison_zones();
        }

        // Host America Aurora dive bomb residual: delayed area damage at target.
        // AuroraBombLocomotor flight residual + FuelAir gas OCL path.
        // Wave 826: under coupled shadow, combat/field residuals sole-tick after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_aurora_bombs();
        }
        // Wave 797: under coupled shadow, AuroraBomb projectile dive is owned by
        // GW tick_status_timer_expirations + destroy logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_aurora_bomb_projectiles();
        }

        // Host GLA Angry Mob residual: aggregate fire on nearby enemies + expand.
        // Fail-closed vs full SpawnBehavior member objects / MobMemberSlavedUpdate.
        self.update_angry_mobs();
        // Wave 799: under coupled shadow, AngryMob projectile flight is owned by
        // GW tick_status_timer_expirations + impact logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_angry_mob_projectiles();
        }

        // Host stealth residual: detector scans + DETECTED expiry.
        // Fail-closed vs full StealthUpdate/StealthDetectorUpdate modules
        // (no IR FX, kindof filters, or disguise).
        self.update_stealth_and_detection();

        // Stinger HiveStructureBody SpawnReplaceDelay residual slave respawns.
        // Wave 814: under coupled shadow, hive respawn owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_stinger_hive_respawns();
        }

        // -----------------------------------------------------------------------
        // Phase 7: Combat Resolution (within object updates)
        // -----------------------------------------------------------------------
        // Weapon fire and damage application as part of the object update pass.
        self.update_combat(&object_ids, dt);
        // Nested AttackStateMachine residual (privateAttackObject enter path).
        let frame = self.frame;
        let t = frame as f32 * LOGIC_FRAME_TIMESTEP;
        self.tick_nested_attack_machines(&object_ids, t, frame);
        self.tick_all_turret_state_machines(&object_ids, t, frame);
        self.sync_attack_priority_from_script_engine();
        self.tick_mood_auto_acquire(&object_ids);
        self.tick_out_of_ammo_jet_damage();
        self.tick_airfield_parking_heal();
        self.tick_airfield_runway_clear();
        self.tick_shock_stun_all();
        // C++ PartitionManager collide residual (broadphase fail-closed O(n²)).
        self.tick_physics_collisions_all();

        // Projectiles: drain global fire queue into host CombatSystem and step.
        // Sole ownership — engine must not maintain a second mid-frame CombatSystem.
        {
            let objects = &self.objects;
            // drain needs &HashMap - self.objects is that
            crate::game_logic::combat::drain_pending_projectiles(&mut self.combat_system, objects);
            crate::game_logic::host_historic_bonus::set_logic_frame(self.frame);
        }
        // Countermeasures residual: mark aircraft upgrade flags for combat pass.
        // CombatSystem cannot see player upgrades; diversion is applied after
        // update_projectiles via a dedicated pre-damage filter is not available,
        // so we run a second pass helper that inspects combat's last-hit log.
        // Instead, process diversion inside apply_projectile_countermeasures_pass
        // after projectiles update (combat still applies damage; we heal-back
        // diverted hits is wrong). Prefer combat-level hook:
        // Wave 882: projectile sole-integrate under GW authority (host skips dual advance).
        let projectile_hits = if crate::gameworld_shadow::gameworld_projectile_authority_live() {
            // Snapshot pre-step flight state for GameWorld integrate authority.
            crate::game_logic::host_projectile_log::record_snapshot(
                self.combat_system.projectiles_snapshot(),
            );
            // Defer integrate+hits to shadow_session (GW step + writeback + hits).
            Vec::new()
        } else {
            let hits = self.combat_system.update_projectiles_with_countermeasures(
                dt,
                &mut self.objects,
                Some(&mut self.countermeasures),
                self.frame,
            );
            crate::game_logic::host_projectile_log::record_snapshot(
                self.combat_system.projectiles_snapshot(),
            );
            hits
        };
        // Wave 470: countermeasure flare spawn/object residual stays host-owned
        // even when GameWorld sole-integrates projectile flight.
        self.flush_countermeasure_flare_spawns();
        // Wave 806: under coupled shadow, lifetime owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_countermeasure_flare_objects();
        }
        self.drain_historic_bonus_firestorms();

        if !projectile_hits.is_empty() {
            // Presentation/audio residual: WeaponFire already queued on shot;
            // hit SFX is fail-closed via presentation audio events when present.
            let _ = projectile_hits;
        }
        // C++ Weapon.ini ProjectileDetonationFX residual at real impact
        // (not fire-time). Fail-closed vs full FXList doFXPos / OCL spawn.
        let impact_fx = self.combat_system.take_impact_fx();
        for impact in impact_fx {
            if impact.detonation_fx_name.is_empty() {
                continue;
            }
            let _ = self.combat_particles.spawn_weapon_fire_fx_named_ocl(
                impact.position,
                Some(impact.position),
                self.frame,
                impact.shooter_id,
                impact.target_id,
                "",
                &impact.detonation_fx_name,
                "",
                &impact.detonation_ocl_name,
            );
        }
        // C++ ProjectileExhaust residual: stamp in-flight trail PSys names on host
        // combat particles (position follows projectile snapshot). Fail-closed vs
        // full client attach-to-drawable exhaust matrix.
        {
            let frame = self.frame;
            let exhausts: Vec<_> = self
                .combat_system
                .projectiles_snapshot()
                .into_iter()
                .filter(|p| !p.exhaust_name.is_empty())
                .map(|p| (p.position, p.shooter_id, p.id, p.exhaust_name.clone()))
                .collect();
            for (pos, shooter, pid, name) in exhausts {
                let _ = self.combat_particles.spawn_projectile_exhaust(
                    pos,
                    frame,
                    shooter,
                    Some(pid),
                    &name,
                );
            }
            // C++ ProjectileStreamUpdate residual: track projectile positions for stream draw.
            for p in self.combat_system.projectiles_snapshot() {
                let sname = if p.exhaust_name.is_empty() {
                    // Prefer weapon peel from shooter primary when no exhaust name.
                    self.objects
                        .get(&p.shooter_id)
                        .and_then(|o| o.thing.template.primary_weapon_name.as_deref())
                        .map(crate::game_logic::weapon_bootstrap::host_projectile_stream_name_for_weapon_name)
                        .unwrap_or_default()
                } else {
                    // Exhaust-bearing projectiles still consult stream peel.
                    self.objects
                        .get(&p.shooter_id)
                        .and_then(|o| o.thing.template.primary_weapon_name.as_deref())
                        .map(crate::game_logic::weapon_bootstrap::host_projectile_stream_name_for_weapon_name)
                        .unwrap_or_default()
                };
                if sname.is_empty() {
                    continue;
                }
                self.projectile_streams.add_projectile(
                    p.shooter_id,
                    &sname,
                    p.position,
                    p.target_id,
                    Some(p.target_position),
                    frame,
                );
            }
            self.projectile_streams.cull_idle(frame, 45);
        }

        // -----------------------------------------------------------------------
        // Phase 7b: Building Body Damage State Checks (C++ BodyModule update)
        // -----------------------------------------------------------------------
        // C++ parity (GarrisonContain::onBodyDamageStateChange): when a garrisoned
        // building drops to ReallyDamaged health (<= 30%), all occupants are
        // force-ejected. This runs after combat so the health state is current.
        self.check_building_damage_states(&object_ids);

        // -----------------------------------------------------------------------
        // Phase 8: AI Update (C++ line 3743)
        // -----------------------------------------------------------------------
        // C++: TheAI->UPDATE();
        // AI runs AFTER object updates so AI decisions are based on the latest
        // world state (objects have moved, combat resolved). This ordering is
        // critical: objects update first, then AI observes new positions and
        // issues commands for the next frame.
        {
            // 1. Update the legacy THE_AI singleton (pathfinder queue, groups).
            if let Ok(mut ai) = THE_AI.write() {
                if let Err(e) = ai.update(self.frame) {
                    log::warn!("THE_AI update failed at frame {}: {:?}", self.frame, e);
                }
            }

            // 2. Update the AiIntegrationManager (per-player AIPlayer / SkirmishPlayer
            //    updates including economy, construction, military decisions).
            if let Some(result) = with_ai_integration_mut(|mgr| mgr.update_ai_players_only()) {
                if let Err(e) = result {
                    log::warn!(
                        "AiIntegrationManager update failed at frame {}: {:?}",
                        self.frame,
                        e
                    );
                }
            }
        }

        // Main crate simplified per-object AI decisions (scan for enemies, retreat, etc.)
        self.update_ai(&object_ids, dt);

        // Host skirmish AI players (AIManager / AIPlayer) — residual production path
        // for Medium+ opponents registered via apply_skirmish_config / add_ai_opponent.
        // Borrow-split: take manager out, update against &mut self, put back.
        // NOTE: dual-tick gate vs gamelogic IntegratedAiPlayer deferred — the
        // integration manager is a process-global singleton and test isolation
        // would skip host AI when leftover players remain from other tests.
        {
            let sim_time = self.sim_time_seconds;
            let mut ai_mgr = std::mem::take(&mut self.ai_manager);
            ai_mgr.update(self, sim_time);
            self.ai_manager = ai_mgr;
        }

        // Phase 8b: Apply commands queued by AI this frame (C++ CommandList drain
        // after TheAI->UPDATE). Ensures same-frame set_target/move logs reach
        // shadow/presentation without a second engine-side process_commands.
        self.process_commands();

        // -----------------------------------------------------------------------
        // Phase 9: Production / Build Assistant (C++ line 3748)
        // -----------------------------------------------------------------------
        // C++: TheBuildAssistant->UPDATE();
        // Production queues update after AI so build orders issued by AI this
        // frame can be immediately reflected.
        // Drain prior-frame upgrade presentation events before research advances.
        self.host_upgrades.clear_frame_events();
        self.update_production(dt);
        // Wave 827: under coupled shadow, host system residuals sole-tick after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_player_upgrades();
        }
        if let Some(mut build_assistant) = get_build_assistant() {
            build_assistant.update(self.frame);
        }

        // -----------------------------------------------------------------------
        // Phase 10: Player Resources
        // -----------------------------------------------------------------------
        self.update_player_resources(dt);
        // GLA Black Market residual cash (AutoDepositUpdate discrete deposits + floating text).
        // Fail-closed: not full InGameUI GPU / InitialCaptureBonus (retail 0).
        // Wave 821: under coupled shadow, AutoDeposit owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_black_market_deposits();
        }
        // Tech Oil Derrick residual cash (AutoDeposit + SupplyLines boost + floating text).
        // Fail-closed: not full InGameUI GPU / STEALTHED local display gate.
        // Wave 821: under coupled shadow, AutoDeposit owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_oil_derrick_deposits();
        }
        // China Hacker / Internet Center residual cash (HackInternetAIUpdate).
        // Fail-closed: not full unpack/pack state machine / variation factor.
        // Wave 822: under coupled shadow, Hacker income owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_hacker_income();
        }
        // America Supply Drop Zone residual: OCL schedules cargo DeliverPayload
        // residual; cash credits after approach delay + crate spawn.
        // Fail-closed: not full CreateAtEdge cargo plane / parachute fall path.
        // Wave 826: under coupled shadow, combat/field residuals sole-tick after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_supply_drop_zone_drops();
        }
        // USA Battle Drone residual master repair (RepairRatePerSecond when HP < 60%).
        // Fail-closed: not full arm pack/unpack weld FX / RepairMinAltitude matrix.
        self.update_battle_drone_repair_residual(dt);
        // CommandCenter / RadarVan radar-online residual (Player::hasRadar).
        // Fail-closed: not full RadarUpgrade grant / power-brownout disable-proof path.
        // Wave 818: under coupled shadow, radar_count owned by GW tick + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_player_radar();
        }
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
        // Note: The gamelogic crate's full update_pipeline also runs its own
        // partition manager update (tick_gamelogic_crate in cnc_game_engine.rs).

        // -----------------------------------------------------------------------
        // Phase 13: Death/Cleanup (C++ line 3762)
        // -----------------------------------------------------------------------
        // C++: processDestroyList();
        // Destroyed objects removed from world. Note: the actual destroy list
        // processing happens in step_simulation_with_budget() after this method
        // returns, so objects marked for destruction this frame are cleaned up
        // after the frame is complete.

        // -----------------------------------------------------------------------
        // Phase 14: Weapon Store Update (C++ line 3767)
        // -----------------------------------------------------------------------
        // C++: TheWeaponStore->UPDATE();
        // Process delayed weapon damage that is now ready.
        if let Err(e) = with_weapon_store_mut(|store| store.update()) {
            // "not initialized" is expected before map load; skip silently
            let err_str = e.to_string();
            if !err_str.contains("not initialized") {
                log::warn!("Weapon store update failed: {}", e);
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
        // Handled by the gamelogic crate's update_pipeline (tick_gamelogic_crate).

        // -----------------------------------------------------------------------
        // Phase 16: Disabled Status Check (C++ lines 3783-3792)
        // -----------------------------------------------------------------------
        // C++: for( Object *obj = m_objList; obj; obj = obj->getNextObject() )
        // C++:   if( obj->isDisabled() ) obj->checkDisabledStatus();
        self.check_bridge_disabled_statuses();

        // -----------------------------------------------------------------------
        // Phase 17: Vision/Shroud Update
        // -----------------------------------------------------------------------
        // The gamelogic crate's ShroudManager only sees objects registered in the
        // gamelogic OBJECT_REGISTRY.  Main-crate objects live in a separate
        // HashMap, so we feed their vision ranges directly into the shroud grid
        // here so fog-of-war actually works for the playable game.
        // Wave 827: under coupled shadow, host system residuals sole-tick after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_main_crate_vision();
        }

        // -----------------------------------------------------------------------
        // Phase 18: Team Events Flush
        // -----------------------------------------------------------------------
        // Handled by the gamelogic crate's update_pipeline.

        // -----------------------------------------------------------------------
        // Post-phase: EVA voice announcements
        // -----------------------------------------------------------------------
        self.process_eva_events();

        // -----------------------------------------------------------------------
        // Post-phase: Audio events
        // -----------------------------------------------------------------------
        self.process_audio_events();

        // Host-only ticks: settle deferred economy spends (DAMAGE/HP already
        // fail-open onto host when not in a coupled shadow writeback frame).
        if !crate::gameworld_shadow::shadow_coupled_tick_active() {
            crate::gameworld_shadow::materialize_host_economy_pending(self);
        }
    }

    /// Update construction progress.
    /// C++ parity: buildings only progress when a worker/dozer is nearby.
    /// Multiple dozers stack their build rate (C++ BuildAssistant).
    pub(super) fn update_construction(&mut self, object_ids: &[ObjectId], dt: f32) {
        const BUILDER_RANGE: f32 = 30.0; // Max distance for a dozer to contribute.

        // C++ parity: calcTimeToBuild applies the same power penalty to dozer
        // construction as to production queue speed.
        let team_power_factor = self.compute_team_power_factors();

        // Pre-scan all dozer positions/teams so we don't borrow-conflict.
        let dozer_info: Vec<(Vec3, Team)> = self
            .objects
            .values()
            .filter(|obj| obj.is_alive() && obj.can_construct())
            .map(|obj| (obj.get_position(), obj.team))
            .collect();

        let mut completed_superweapon_detects: Vec<(Team, String)> = Vec::new();
        let mut completed_structures: Vec<ObjectId> = Vec::new();
        let mut ready_superweapons: Vec<(ObjectId, Team, String)> = Vec::new();
        let mut radar_extend_done: Vec<ObjectId> = Vec::new();
        // Wave 617: under sole-tick, GameWorld writeback records ready structures;
        // host applies completion after writeback same frame (Wave 715; not mid-update drain).
        let construction_sole = crate::gameworld_shadow::gameworld_construction_sole_tick_enabled();
        // Empty mid-update ready set: sole completes only via post-writeback helper (Wave 715).
        // Non-sole completes via projected percent (may_complete=true).
        let ready_structures: std::collections::HashSet<ObjectId> =
            std::collections::HashSet::new();
        for &id in object_ids {
            if let Some(obj) = self.objects.get_mut(&id) {
                if obj.status.under_construction {
                    let build_pos = obj.get_position();
                    let build_team = obj.team;
                    // True nearby dozer count (0 allowed) for model-condition residual.
                    let nearby_dozers = dozer_info
                        .iter()
                        .filter(|(pos, t)| {
                            *t == build_team && pos.distance(build_pos) <= BUILDER_RANGE
                        })
                        .count();
                    let dozer_count = nearby_dozers.max(1); // At least 1 so AI-built structures still progress.
                    let actively_built = nearby_dozers > 0;
                    obj.set_under_construction_model_conditions(actively_built);
                    self.construction_model_condition_updates =
                        self.construction_model_condition_updates.saturating_add(1);

                    let power_factor = team_power_factor.get(&build_team).copied().unwrap_or(1.0);
                    let base_rate = 1.0 / obj.thing.template.build_time.max(0.01);
                    let effective_rate = base_rate * dozer_count as f32 * power_factor;
                    // Under CONSTRUCTION_AUTHORITY + shadow, GameWorld sole-ticks percent
                    // using effective_rate; host only completes when writeback hits 1.0
                    // (Wave 617: readiness gated by host_construction_ready_log).
                    // Prior freeze without rate residual stalled builds — rate is logged.
                    let sole = crate::gameworld_shadow::gameworld_construction_sole_tick_enabled();
                    let projected = if sole {
                        // Last writeback percent (GW sole-ticks); host does not advance.
                        obj.construction_percent
                    } else {
                        (obj.construction_percent + effective_rate * dt).min(1.0)
                    };
                    if !sole {
                        obj.construction_percent = projected;
                        crate::game_logic::host_construction_progress_log::record(
                            id,
                            projected,
                            obj.status.under_construction,
                            effective_rate,
                        );
                    } else {
                        // Wave 478: publish dozer/power rate only — no percent stomp.
                        crate::game_logic::host_construction_progress_log::record_rate_only(
                            id,
                            obj.status.under_construction,
                            effective_rate,
                        );
                    }

                    // Wave 617/713: under sole-tick, only complete ready-log IDs.
                    // Empty ready log ⇒ no host percent-complete scan (GW readiness authority).
                    let may_complete = if construction_sole {
                        ready_structures.contains(&id)
                    } else {
                        true
                    };
                    if may_complete && projected >= 1.0 {
                        obj.construction_percent = 1.0;
                        obj.set_status_under_construction(false);
                        obj.clear_under_construction_model_conditions();
                        let full_hp = obj.health.maximum;
                        if crate::gameworld_shadow::gameworld_damage_authority_live() {
                            // HP last-writer via heal channel + writeback.
                            crate::game_logic::host_heal_log::record(id, full_hp);
                        } else {
                            obj.health.current = full_hp;
                            crate::game_logic::host_heal_log::record(id, obj.health.current);
                        }
                        crate::game_logic::host_construction_progress_log::record(
                            id, 1.0, false, 0.0,
                        );
                        crate::game_logic::host_construction_log::record(
                            id,
                            obj.template_name.clone(),
                        );
                        // C++ onStructureConstructionComplete SuperweaponDetected residual.
                        completed_superweapon_detects.push((obj.team, obj.template_name.clone()));
                        completed_structures.push(id);
                    } else {
                        let build_hp = obj.health.maximum * (0.1 + 0.9 * projected);
                        if crate::gameworld_shadow::gameworld_damage_authority_live() {
                            crate::game_logic::host_heal_log::record(id, build_hp);
                        } else {
                            obj.health.current = build_hp;
                            crate::game_logic::host_heal_log::record(id, obj.health.current);
                        }
                    }
                }
                if obj.tick_timers(dt) {
                    let team = obj.team;
                    let name = obj.template_name.clone();
                    // Defer EVA until after borrow ends.
                    ready_superweapons.push((id, team, name));
                }
                // Wave 744: under coupled GameWorld shadow, radar-extend complete
                // is owned by writeback + host_apply_radar_extend_ready_completions.
                // Host must not dual-complete via tick_radar_extend mid-frame.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    if obj.tick_radar_extend(self.frame) {
                        radar_extend_done.push(id);
                    }
                }
                // Wave 743: under production sole-tick, GameWorld owns door phase
                // advance + writeback; host must not dual-tick door residual.
                if !crate::gameworld_shadow::gameworld_production_sole_tick_enabled() {
                    let _ = obj.tick_production_door(self.frame);
                }
                // Wave 626: under construction sole-tick, GW ready-log owns clear
                // residual; host tick still advances non-sole path.
                if !crate::gameworld_shadow::gameworld_construction_sole_tick_enabled() {
                    if obj.tick_construction_complete_clear(self.frame) {
                        self.construction_complete_clears =
                            self.construction_complete_clears.saturating_add(1);
                    }
                }
            }
        }
        // C++ Player sharedNSync timers advance with the logic frame.
        self.tick_shared_special_power_timers(dt);

        for (id, team, name) in ready_superweapons {
            self.try_eva_superweapon_ready(id, team, &name);
        }
        // Wave 618: under sole-tick, GameWorld writeback records SP ready flips;
        // Wave 717: host EVA drain runs after writeback same frame (not mid-update).

        for _id in radar_extend_done {
            self.radar_extend_completes = self.radar_extend_completes.saturating_add(1);
        }

        for (team, name) in completed_superweapon_detects {
            self.try_eva_superweapon_detected(team, &name);
        }

        // C++ parity: when a structure finishes construction, release any dozers
        // that were constructing it — set them to Idle.
        for &completed_id in &completed_structures {
            // C++ SupplyCenterCreate::onBuildComplete residual.
            self.on_supply_center_build_complete(completed_id);
            for obj in self.objects.values_mut() {
                if obj.ai_state == AIState::Constructing
                    && obj.target == Some(completed_id)
                    && obj.is_alive()
                {
                    let oid = obj.id;
                    obj.set_target(None);
                    obj.stop_moving();
                    // Collect for decision-aware Idle after borrow ends.
                    // (set below via second pass if needed — apply inline with free log)
                    obj.set_ai_state(AIState::Idle);
                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                        crate::game_logic::host_ai_decision_log::record_set_state(oid, 0);
                    }
                }
            }
            if let Some(team) = self.objects.get(&completed_id).map(|o| o.team) {
                self.record_structure_completion(team);
            }
            // C++ onStructureConstructionComplete feedback residual.
            self.notify_structure_construction_complete(completed_id);
            // C++ RadarUpgrade/RadarUpdate extendRadar residual on radar providers.
            self.maybe_start_radar_extend(completed_id);
            // Constructed footprint is a static path/LOS obstacle.
            self.block_structure_object_path(completed_id);
        }
        // C++ ACTIVELY_CONSTRUCTING residual for dozers/factories.
        // Wave 815: under coupled shadow, model bit owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_actively_constructing_model_conditions();
        }
    }

    /// Wave 715: after GW construction writeback records ready structures, host
    /// applies completion side effects in the same coupled tick (not next frame).

    /// Wave 717: after GW special-power writeback records ready flips, host
    /// applies EVA superweapon-ready residual in the same coupled tick.
    pub(crate) fn host_apply_special_power_ready_after_writeback(&mut self) {
        if !crate::gameworld_shadow::gameworld_special_power_sole_tick_enabled() {
            return;
        }
        for ev in crate::game_logic::host_special_power_ready_log::drain() {
            if let Some(obj) = self.objects.get(&ev.object) {
                if obj.is_alive() {
                    let team = obj.team;
                    let name = obj.template_name.clone();
                    self.try_eva_superweapon_ready(ev.object, team, &name);
                }
            }
        }
    }

    pub(crate) fn host_apply_construction_completions_after_ready_writeback(&mut self) {
        if !crate::gameworld_shadow::gameworld_construction_sole_tick_enabled() {
            return;
        }
        let ready: Vec<ObjectId> = crate::game_logic::host_construction_ready_log::drain()
            .into_iter()
            .map(|ev| ev.structure)
            .collect();
        if ready.is_empty() {
            return;
        }
        let mut completed_superweapon_detects: Vec<(Team, String)> = Vec::new();
        let mut completed_structures: Vec<ObjectId> = Vec::new();
        for id in ready {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            // Writeback may already have percent=1.0 while under_construction remains set.
            if !(obj.status.under_construction || obj.construction_percent + 1e-6 >= 1.0) {
                continue;
            }
            obj.construction_percent = 1.0;
            obj.set_status_under_construction(false);
            obj.clear_under_construction_model_conditions();
            let full_hp = obj.health.maximum;
            if crate::gameworld_shadow::gameworld_damage_authority_live() {
                crate::game_logic::host_heal_log::record(id, full_hp);
            } else {
                obj.health.current = full_hp;
                crate::game_logic::host_heal_log::record(id, obj.health.current);
            }
            crate::game_logic::host_construction_progress_log::record(id, 1.0, false, 0.0);
            crate::game_logic::host_construction_log::record(id, obj.template_name.clone());
            completed_superweapon_detects.push((obj.team, obj.template_name.clone()));
            completed_structures.push(id);
        }
        for (team, name) in completed_superweapon_detects {
            self.try_eva_superweapon_detected(team, &name);
        }
        for &completed_id in &completed_structures {
            self.on_supply_center_build_complete(completed_id);
            for obj in self.objects.values_mut() {
                if obj.ai_state == AIState::Constructing
                    && obj.target == Some(completed_id)
                    && obj.is_alive()
                {
                    let oid = obj.id;
                    obj.set_target(None);
                    obj.stop_moving();
                    obj.set_ai_state(AIState::Idle);
                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                        crate::game_logic::host_ai_decision_log::record_set_state(oid, 0);
                    }
                }
            }
            if let Some(team) = self.objects.get(&completed_id).map(|o| o.team) {
                self.record_structure_completion(team);
            }
            self.notify_structure_construction_complete(completed_id);
            self.maybe_start_radar_extend(completed_id);
            self.block_structure_object_path(completed_id);
        }
        if !completed_structures.is_empty() {
            // Wave 828: under coupled shadow, ACTIVELY_CONSTRUCTING bit owned by GW expire.
            if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                && crate::gameworld_shadow::shadow_coupled_tick_active())
            {
                self.update_actively_constructing_model_conditions();
            }
        }
    }

    pub(super) fn update_production(&mut self, dt: f32) {
        // Wave 613: production complete collect + apply via host helpers.
        // Under PRODUCTION_AUTHORITY sole-tick, GameWorld advances queue progress
        // and writeback finishes heads; host try_complete + spawn runs after
        // shadow writeback same frame (Wave 714) so ready-log is not a frame late.
        // Wave 875: sole-tick early-return honesty — no host dual-advance.
        if crate::gameworld_shadow::gameworld_production_sole_tick_enabled() {
            return;
        }
        let (upgrade_completions, unit_completions) = self.host_collect_production_completions(dt);
        // Wave 595/608: host production complete/spawn apply residual via host helpers.
        self.apply_upgrade_production_completions(upgrade_completions);
        self.apply_unit_production_completions(unit_completions);
    }

    /// Wave 714: after GW production writeback records ready producers, host
    /// try_completes + spawns in the same coupled tick (not next frame).
    pub(crate) fn host_apply_production_completions_after_ready_writeback(&mut self, dt: f32) {
        if !crate::gameworld_shadow::gameworld_production_sole_tick_enabled() {
            return;
        }
        let (upgrade_completions, unit_completions) = self.host_collect_production_completions(dt);
        self.apply_upgrade_production_completions(upgrade_completions);
        self.apply_unit_production_completions(unit_completions);
    }

    /// Wave 613: host production completion collection residual.
    ///
    /// Sole-tick path: GameWorld sole-ticks progress/exit delay; host
    /// `try_complete_production` only when writeback finished the head.
    /// Non-sole path: host still advances production via building.update_production.
    pub(crate) fn host_collect_production_completions(
        &mut self,
        dt: f32,
    ) -> (
        Vec<(Team, String, ObjectId)>,
        Vec<(Team, String, Vec3, Option<Vec3>, ObjectId)>,
    ) {
        // Wave 613: host production complete collect residual.

        // C++ parity: pre-compute per-team power factor so we don't borrow
        // self.players while self.objects is mutably borrowed.
        // Formula matches ThingTemplate::calcTimeToBuild():
        //   energy_ratio = produced / max(consumed, produced) clamped to [0,1]
        //   energy_short = (1.0 - ratio) * penalty_modifier
        //   rate = max(1.0 - energy_short, 0.5)
        //   if ratio < 1.0: rate = min(rate, 0.8)
        let team_power_factor = self.compute_team_power_factors();

        use crate::game_logic::buildings::ProductionKind;
        // Unit completions: (team, template, spawn_pos, rally, producer_id)
        let mut unit_completions: Vec<(Team, String, Vec3, Option<Vec3>, ObjectId)> = Vec::new();
        // Upgrade completions: (team, upgrade_name, producer_id)
        let mut upgrade_completions: Vec<(Team, String, ObjectId)> = Vec::new();

        // Wave 614: under sole-tick, GameWorld writeback records ready producers;
        // host only try_completes those IDs (GW decides readiness).
        let sole = crate::gameworld_shadow::gameworld_production_sole_tick_enabled();
        // Wave 735: keep full ready events (template + GW spawn pose/rally), not
        // producer IDs alone — host sole-tick applies GW pose authority on spawn.
        let ready_by_producer: std::collections::HashMap<
            ObjectId,
            crate::game_logic::host_production_ready_log::HostProductionReadyEvent,
        > = if sole {
            crate::game_logic::host_production_ready_log::drain()
                .into_iter()
                .map(|ev| (ev.producer, ev))
                .collect()
        } else {
            std::collections::HashMap::new()
        };
        let ready_producers: std::collections::HashSet<ObjectId> =
            ready_by_producer.keys().copied().collect();

        for (&id, obj) in self.objects.iter_mut() {
            if !obj.is_constructed() || !obj.is_alive() {
                continue;
            }
            // C++ isDisabled residual: EMP / hacked / underpowered / unmanned
            // structures do not advance production while disabled.
            if obj.is_disabled() {
                continue;
            }
            if let Some(building) = obj.building_data.as_mut() {
                let pf = team_power_factor.get(&obj.team).copied().unwrap_or(1.0);
                // Under PRODUCTION_AUTHORITY, GameWorld ticks queue progress;
                // host only exits delay + completes when writeback already finished the head.
                let completed_prod = if sole {
                    // Wave 464/614: GameWorld sole-ticks progress + exit delay and
                    // records ready producers on writeback; host try_completes only
                    // ready IDs (Wave 713: empty ready log ⇒ no host scan).
                    if ready_producers.contains(&id) {
                        building.try_complete_production()
                    } else {
                        None
                    }
                } else {
                    building.update_production(dt, pf)
                };
                // GameWorld production residual: snapshot queue progress each tick
                // unless sole-tick owns progress (Wave 477) — then enqueue/complete logs
                // + writeback carry structure; GW advances progress/exit delay.
                if !building.production_queue.is_empty()
                    && !crate::gameworld_shadow::gameworld_production_sole_tick_enabled()
                {
                    let items: Vec<crate::game_logic::host_production_progress_log::HostProductionQueueItem> =
                        building
                            .production_queue
                            .iter()
                            .take(16)
                            .map(|it| {
                                crate::game_logic::host_production_progress_log::HostProductionQueueItem {
                                    template_name: it.template_name.clone(),
                                    progress: it.progress,
                                    total_time: it.total_time,
                                    cost_supplies: it.cost.supplies,
                                    is_upgrade: it.is_upgrade(),
                                    quantity_total: it.quantity_total.max(1),
                                    quantity_produced: it.quantity_produced,
                                }
                            })
                            .collect();
                    crate::game_logic::host_production_progress_log::record(
                        id,
                        items,
                        building.exit_delay_remaining,
                        pf,
                    );
                } else if crate::gameworld_shadow::gameworld_production_sole_tick_enabled() {
                    // Wave 477: still publish power factor for GW sole-tick rate without
                    // stomping queue progress via full progress-log apply.
                    crate::game_logic::host_production_progress_log::record_power_factor_only(
                        id, pf,
                    );
                }
                if let Some((completed, kind)) = completed_prod {
                    match kind {
                        ProductionKind::Upgrade => {
                            upgrade_completions.push((obj.team, completed, id));
                        }
                        ProductionKind::Unit => {
                            let mut rally = building.rally_point;
                            // Spawn slightly offset from the building facing to reduce clumping.
                            let forward = obj.thing.get_direction_vector();
                            let base =
                                obj.get_position() + forward * obj.selection_radius.max(10.0);
                            // Deterministic jitter based on template bytes (simple FNV-1a).
                            let mut hash: u32 = 0x811c9dc5;
                            for &b in completed.as_bytes() {
                                hash ^= b as u32;
                                hash = hash.wrapping_mul(0x01000193);
                            }
                            let angle = (hash as f32) * 0.001;
                            let radius = 3.0 + (hash as f32 % 5.0);
                            let jitter = Vec3::new(angle.cos(), 0.0, angle.sin()) * radius;
                            let mut spawn_pos = base + jitter;
                            // C++ UnitCreatePoint residual sample for China barracks family.
                            let pname = obj.template_name.to_ascii_lowercase();
                            if pname.contains("chinabarracks")
                                || (pname.contains("barracks") && pname.contains("china"))
                            {
                                spawn_pos = crate::game_logic::host_production_buildable_command_residual::transform_model_exit_offset(
                                    obj.get_position(),
                                    forward,
                                    crate::game_logic::host_production_buildable_command_residual::CHINA_BARRACKS_UNIT_CREATE_MODEL,
                                );
                            }
                            // Wave 735: under sole-tick, GameWorld ready-log pose/rally
                            // and template are authoritative for the completion spawn.
                            // Wave 736: queue GW pre-spawned entity bind for host ObjectId.
                            let mut completed_name = completed;
                            if sole {
                                if let Some(ev) = ready_by_producer.get(&id) {
                                    if !ev.template_name.is_empty() {
                                        completed_name = ev.template_name.clone();
                                    }
                                    if let Some(p) = ev.spawn_pos {
                                        spawn_pos = Vec3::new(p[0], p[1], p[2]);
                                    }
                                    if let Some(r) = ev.rally {
                                        rally = Some(Vec3::new(r[0], r[1], r[2]));
                                    }
                                    if let Some(raw) = ev.gw_entity_raw {
                                        crate::game_logic::host_production_ready_log::push_pending_bind(
                                            raw,
                                        );
                                    }
                                }
                            }
                            unit_completions.push((obj.team, completed_name, spawn_pos, rally, id));
                        }
                    }
                }
            }
        }

        (upgrade_completions, unit_completions)
    }

    /// Wave 595: host upgrade production completion residual (still host-side under
    /// PRODUCTION_AUTHORITY; GameWorld sole-ticks queue progress only).
    /// Wave 608: via `host_apply_upgrade_production_completions`.
    pub(super) fn apply_upgrade_production_completions(
        &mut self,
        upgrade_completions: Vec<(Team, String, ObjectId)>,
    ) {
        // Wave 608: thin wrapper — production complete apply via host helper.
        self.host_apply_upgrade_production_completions(upgrade_completions)
    }

    /// Wave 595: host upgrade production completion residual (still host-side under
    /// PRODUCTION_AUTHORITY; GameWorld sole-ticks queue progress only).
    pub(super) fn host_apply_upgrade_production_completions(
        &mut self,
        upgrade_completions: Vec<(Team, String, ObjectId)>,
    ) {
        // Wave 608: host production complete/spawn apply residual.
        // Wave 595: host upgrade production completion residual.
        for (team, upgrade_name, producer_id) in upgrade_completions {
            // Door + construction-complete flash residual on producer.
            if let Some(prod) = self.objects.get_mut(&producer_id) {
                let now = self.frame.max(1);
                prod.set_construction_complete_condition_at(now);
                prod.start_production_door_cycle(self.frame);
                self.production_door_cycles = self.production_door_cycles.saturating_add(1);
            }
            // Wave 483: refresh GW producer queue after host pop (sole-tick skips
            // per-frame progress log; Complete path snapshots host queue).
            crate::game_logic::host_production_log::record_complete(
                producer_id,
                upgrade_name.clone(),
                ObjectId(0),
            );
            // Unlock via player queue drain + host apply path.
            let player_id = self.players.values().find(|p| p.team == team).map(|p| p.id);
            if let Some(pid) = player_id {
                let already = self
                    .players
                    .get(&pid)
                    .map(|p| p.has_unlocked_upgrade(&upgrade_name))
                    .unwrap_or(false);
                if let Some(player) = self.players.get_mut(&pid) {
                    // Remove from queued set without refund (research finished).
                    if let Some(queued) = player.find_queued_upgrade_name(&upgrade_name) {
                        player.queued_upgrades.remove(&queued);
                    }
                    if !player.has_unlocked_upgrade(&upgrade_name) {
                        player.unlocked_sciences.insert(upgrade_name.clone());
                    }
                }
                if !already {
                    self.apply_host_upgrade_complete(team, pid, &upgrade_name);
                }
            }
        }
    }

    /// Wave 595: host unit production completion residual — spawn, door, exit delay,
    /// rally path. GameWorld sole-ticks progress; host still completes/spawns.
    /// Wave 608: via `host_apply_unit_production_completions`.
    pub(super) fn apply_unit_production_completions(
        &mut self,
        unit_completions: Vec<(Team, String, Vec3, Option<Vec3>, ObjectId)>,
    ) {
        // Wave 608: thin wrapper — production complete apply via host helper.
        self.host_apply_unit_production_completions(unit_completions)
    }

    /// Wave 615: host production unit spawn residual.
    ///
    /// Still host ObjectId authority (`create_object` + spawn log). GameWorld
    /// receives the unit via host_spawn_log / production Complete channel after
    /// sole-tick readiness (Waves 614/608). Wave 679: successful IDs enter
    /// `host_production_spawn_ready_log` before door/notify/exit residual.
    /// Not full GW spawn-ID authority.

    /// Wave 740: rebuild-hole worker/structure spawn with optional GW entity bind.
    /// Under construction sole-tick, prefers free GW entity raw as ObjectId and
    /// binds without a second Spawn.
    /// Wave 741: missing GW entity raw under construction sole-tick is fail-closed
    /// (default). Incomplete harnesses may set
    /// GENERALS_RUNTIME_HOST_REBUILD_SPAWN_WITHOUT_GW_BIND=1.
    /// playable_claim stays false.
    pub(super) fn host_spawn_rebuild_bound_object(
        &mut self,
        template: &str,
        team: Team,
        spawn_pos: Vec3,
        gw_entity_raw: Option<u32>,
    ) -> Option<ObjectId> {
        if crate::gameworld_shadow::gameworld_construction_sole_tick_enabled() {
            if let Some(raw) = gw_entity_raw {
                crate::gameworld_shadow::set_next_host_spawn_bind_entity(raw);
                let preferred = ObjectId(raw);
                if raw != 0 && !self.objects.contains_key(&preferred) {
                    let saved_next = self.next_object_id;
                    self.next_object_id = preferred;
                    let spawned = self.create_object(template, team, spawn_pos);
                    let after = self.next_object_id.0;
                    self.next_object_id = ObjectId(saved_next.0.max(after));
                    if spawned.is_some() {
                        return spawned;
                    }
                    self.next_object_id = saved_next;
                }
                // Bind present: allocate host id and map to pre-spawned entity.
                return self.create_object(template, team, spawn_pos);
            }
            let allow_without_bind =
                std::env::var_os("GENERALS_RUNTIME_HOST_REBUILD_SPAWN_WITHOUT_GW_BIND")
                    .is_some_and(|v| {
                        let s = v.to_string_lossy();
                        !(s.is_empty()
                            || s == "0"
                            || s.eq_ignore_ascii_case("false")
                            || s.eq_ignore_ascii_case("no"))
                    });
            if !allow_without_bind {
                log::debug!(
                    "Wave 741: construction sole-tick rebuild spawn denied without GW entity bind (template={template})"
                );
                return None;
            }
        }
        self.create_object(template, team, spawn_pos)
    }

    pub(super) fn host_spawn_production_unit(
        &mut self,
        template: &str,
        team: Team,
        spawn_pos: Vec3,
    ) -> Option<ObjectId> {
        // Wave 615: host production spawn residual.
        // Wave 736: under sole-tick, bind host ObjectId to GW pre-spawned entity
        // (entity-first).
        // Wave 737: when the GW entity raw id is free on the host, prefer it as the
        // production ObjectId so host ID space tracks GW entity-first spawns.
        // Wave 738: under sole-tick, spawn without a GW entity bind is fail-closed
        // (default). Incomplete harnesses may set
        // GENERALS_RUNTIME_HOST_PRODUCTION_SPAWN_WITHOUT_GW_BIND=1.
        // Collision on preferred id still falls back to allocate_object_id *with* bind.
        // playable_claim stays false.
        // Wave 761: entity-first ObjectId bind under production sole-tick OR
        // coupled shadow (dual path still prefers GW pre-spawned entity raw id).
        if crate::gameworld_shadow::gameworld_production_sole_tick_enabled()
            || crate::gameworld_shadow::shadow_coupled_tick_active()
        {
            if let Some(raw) = crate::game_logic::host_production_ready_log::pop_pending_bind() {
                crate::gameworld_shadow::set_next_host_spawn_bind_entity(raw);
                let preferred = ObjectId(raw);
                if raw != 0 && !self.objects.contains_key(&preferred) {
                    let saved_next = self.next_object_id;
                    self.next_object_id = preferred;
                    let spawned = self.create_object(template, team, spawn_pos);
                    // Keep monotonic next_id at least past both saved and allocated.
                    let after = self.next_object_id.0;
                    self.next_object_id = ObjectId(saved_next.0.max(after));
                    if spawned.is_some() {
                        return spawned;
                    }
                    // create_object failed — restore and fall through with bind still set
                    // only if create_object did not consume it (template miss).
                    self.next_object_id = saved_next;
                }
                // Bind present (preferred collision or create miss): host allocate + map.
                return self.create_object(template, team, spawn_pos);
            }
            let allow_without_bind =
                std::env::var_os("GENERALS_RUNTIME_HOST_PRODUCTION_SPAWN_WITHOUT_GW_BIND")
                    .is_some_and(|v| {
                        let s = v.to_string_lossy();
                        !(s.is_empty()
                            || s == "0"
                            || s.eq_ignore_ascii_case("false")
                            || s.eq_ignore_ascii_case("no"))
                    });
            if !allow_without_bind {
                log::debug!(
                    "Wave 738: sole-tick production spawn denied without GW entity bind (template={template})"
                );
                return None;
            }
        }
        self.create_object(template, team, spawn_pos)
    }

    /// Wave 595: host unit production completion residual — spawn, door, exit delay,
    /// rally path. GameWorld sole-ticks progress; host still completes/spawns.
    pub(super) fn host_apply_unit_production_completions(
        &mut self,
        unit_completions: Vec<(Team, String, Vec3, Option<Vec3>, ObjectId)>,
    ) {
        // Wave 608: host production complete/spawn apply residual.
        // Wave 595: host unit production completion residual.
        for (team, template, spawn_pos, rally, producer_id) in unit_completions {
            // Wave 615: production unit spawn via host helper (still host ID authority).
            let new_id =
                match self.apply_production_authority_op(ProductionAuthorityOp::SpawnUnit {
                    template: template.clone(),
                    team,
                    spawn_pos,
                }) {
                    ProductionAuthorityResult::Spawned(id) => id,
                    _ => None,
                };
            if let Some(new_id) = new_id {
                crate::game_logic::host_production_log::record_complete(
                    producer_id,
                    template.clone(),
                    new_id,
                );
                // Wave 679: production spawn ObjectId ready residual —
                // host door/notify/exit/path apply drains the ready log.
                crate::game_logic::host_production_spawn_ready_log::record(
                    new_id,
                    producer_id,
                    template,
                    [spawn_pos.x, spawn_pos.y, spawn_pos.z],
                    rally.map(|r| [r.x, r.y, r.z]),
                );
                let _ = self.apply_production_authority_op(
                    ProductionAuthorityOp::ApplySpawnReadyCompletions,
                );
            }
        }
    }

    /// Wave 679: drain production-spawn ready log and apply host presentation residual
    /// (notify/door/exit/path) for the newly allocated host ObjectId.
    /// Still host ObjectId authority — not full GameWorld spawn-ID ownership.
    pub fn host_apply_production_spawn_ready_completions(&mut self) -> usize {
        // Wave 679: drain production-spawn ready log and apply host presentation residual.
        let events = crate::game_logic::host_production_spawn_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let new_id = ev.unit;
            let producer_id = ev.producer;
            let template = ev.template;
            let mut spawn_pos = Vec3::new(ev.spawn_pos[0], ev.spawn_pos[1], ev.spawn_pos[2]);
            let rally = ev.rally.map(|r| Vec3::new(r[0], r[1], r[2]));
            // Wave 739: under production sole-tick, GameWorld ready-log pose is
            // authoritative — do not re-jitter/reposition the unit here (host
            // create_object already placed at GW exit pose). Non-sole path keeps
            // host stacking jitter residual.
            let sole = crate::gameworld_shadow::gameworld_production_sole_tick_enabled();
            let jitter_dir = if sole {
                Vec3::ZERO
            } else {
                Vec3::new(
                    (spawn_pos.x * 17.0 + spawn_pos.z).sin(),
                    0.0,
                    (spawn_pos.z * 31.0 + spawn_pos.x).cos(),
                )
                .normalize_or_zero()
            };
            // C++ VoiceCreated + UnitReady residual.
            self.notify_unit_production_complete(new_id, producer_id, &template);
            // C++ ProductionUpdate door + CONSTRUCTION_COMPLETE residual on producer.
            if let Some(prod) = self.objects.get_mut(&producer_id) {
                let now = self.frame.max(1);
                prod.set_construction_complete_condition_at(now);
                prod.start_production_door_cycle(self.frame);
                self.production_door_cycles = self.production_door_cycles.saturating_add(1);
                // C++ QueueProductionExitUpdate ExitDelay residual after release.
                if let Some(building) = prod.building_data.as_mut() {
                    let delay = crate::game_logic::host_dock_contain_exit_heal_residual::queue_exit_delay_seconds_for_template(
                        &prod.template_name,
                    );
                    building.arm_exit_delay(delay);
                    // Wave 480: under sole-tick, progress log is power-only —
                    // publish exit arm so GW QueueProductionExitUpdate advances.
                    if crate::gameworld_shadow::gameworld_production_sole_tick_enabled() {
                        crate::game_logic::host_production_progress_log::record_exit_delay_only(
                            producer_id,
                            delay,
                        );
                    }
                }
            }
            // SCIENCE_StealthFighter residual: record gated production spawn.
            if crate::game_logic::host_stealth_fighter::requires_stealth_fighter_science(&template)
            {
                self.stealth_fighter_science.record_production_spawn();
            }
            // Wave 739: sole-tick keeps create_object/GW exit pose; non-sole
            // applies host stacking jitter + factory exit pose residual.
            if !sole {
                if let Some(unit) = self.objects.get(&new_id) {
                    let selection_radius = unit.selection_radius.max(4.0);
                    spawn_pos += jitter_dir * selection_radius;
                }
                if let Some(unit) = self.objects.get_mut(&new_id) {
                    if crate::gameworld_shadow::gameworld_movement_authority_live() {
                        crate::game_logic::host_move_log::record(
                            new_id,
                            Some([spawn_pos.x, spawn_pos.y, spawn_pos.z]),
                        );
                        // Factory exit residual still needs host pose for same-frame doors.
                        unit.set_position(spawn_pos);
                        unit.record_host_movement();
                    } else {
                        unit.set_position(spawn_pos);
                    }
                }
            }
            // C++ QueueProductionExitUpdate exit path residual:
            // natural rally first; custom rally appended; else double natural
            // so Red Guards do not stack on the door.
            let (natural, forward) = if let Some(prod) = self.objects.get(&producer_id) {
                let f = prod.thing.get_direction_vector();
                let natural = crate::game_logic::host_production_buildable_command_residual::transform_model_exit_offset(
                    prod.get_position(),
                    f,
                    crate::game_logic::host_production_buildable_command_residual::CHINA_BARRACKS_NATURAL_RALLY_MODEL,
                );
                // Generic residual: if producer is not China barracks family, fall back
                // to forward * selection_radius natural.
                let p_name = prod.template_name.to_ascii_lowercase();
                let natural = if p_name.contains("chinabarracks")
                    || (p_name.contains("barracks") && p_name.contains("china"))
                {
                    natural
                } else {
                    prod.get_position() + f * prod.selection_radius.max(10.0)
                };
                (natural, f)
            } else {
                (spawn_pos, glam::Vec3::new(0.0, 0.0, -1.0))
            };
            self.path_approach_with_state(new_id, natural, AIState::Moving);
            if let Some(rally_point) = rally {
                let _ = self.append_unit_waypoint(new_id, rally_point);
            } else {
                // Double natural residual (C++ exitPath.push_back(tmp) twice).
                let doubled = natural + forward.normalize_or_zero() * 5.0;
                let _ = self.append_unit_waypoint(new_id, doubled);
            }
            n = n.saturating_add(1);
        }
        n
    }

    /// C++ GameLogic starting-unit residual (PlayerTemplate StartingUnit0..N).
    /// Spawns each active skirmish/SP player's starting construction unit near their
    /// base if they do not already own a matching mobile builder.
    pub(crate) fn spawn_skirmish_starting_units(&mut self) {
        use crate::game_logic::host_faction_skirmish_residual::{
            find_player_template_by_side, find_player_template_residual,
        };

        let mut player_ids: Vec<u32> = self.players.keys().copied().collect();
        player_ids.sort_unstable();

        for pid in player_ids {
            let Some(player) = self.players.get(&pid).cloned() else {
                continue;
            };
            if !player.is_alive || player.team == Team::Neutral {
                continue;
            }

            let side = match player.team {
                Team::USA => "America",
                Team::China => "China",
                Team::GLA => "GLA",
                Team::Neutral => continue,
            };
            let residual = find_player_template_by_side(side)
                .or_else(|| find_player_template_residual("FactionAmerica"));
            let Some(residual) = residual else {
                log::warn!(
                    "Skirmish starting unit residual: no player template for side={} player={}",
                    side,
                    pid
                );
                continue;
            };

            // --- Starting building (C++ placeStartingStructures) ---
            let mut base = self.team_base_position(player.team);
            if base.is_none() {
                // Wave 831/832: place at Player_N_Start when map has no faction army.
                let building = residual.starting_building;
                let mut pos_opt: Option<Vec3> = None;
                if !building.is_empty() {
                    if let Ok(starts) =
                        super::script_loader::parse_player_start_waypoints(&self.map_name)
                    {
                        let want_idx = if player.start_position >= 0 {
                            player.start_position as u32
                        } else {
                            pid
                        };
                        if let Some((_, wp, _rally)) =
                            starts.iter().find(|(idx, _, _)| *idx == want_idx)
                        {
                            let mut pos = Vec3::new(wp.x, wp.z, wp.y);
                            if let Some(h) = self.terrain_height_at(Vec3::new(pos.x, 0.0, pos.z)) {
                                pos.y = h;
                            }
                            pos_opt = Some(pos);
                        } else if let Some((_, wp, _)) = starts.first() {
                            let mut pos = Vec3::new(wp.x, wp.z, wp.y);
                            if let Some(h) = self.terrain_height_at(Vec3::new(pos.x, 0.0, pos.z)) {
                                pos.y = h;
                            }
                            pos_opt = Some(pos);
                        }
                    }
                }
                let allow_seed_building = pos_opt.is_some()
                    || std::env::var_os("GENERALS_RUNTIME_HOST_SEED_STARTING_BUILDING")
                        .is_some_and(|v| {
                            let s = v.to_string_lossy();
                            !(s.is_empty()
                                || s == "0"
                                || s.eq_ignore_ascii_case("false")
                                || s.eq_ignore_ascii_case("no"))
                        });
                if allow_seed_building && !building.is_empty() {
                    let mut pos = pos_opt.unwrap_or_else(|| {
                        let (bmin, bmax) = self.world_bounds();
                        let t = (pid as f32 + 1.0) / (self.players.len().max(1) as f32 + 1.0);
                        Vec3::new(
                            bmin.x + (bmax.x - bmin.x) * t,
                            0.0,
                            bmin.z + (bmax.z - bmin.z) * 0.2,
                        )
                    });
                    if let Some(h) = self.terrain_height_at(Vec3::new(pos.x, 0.0, pos.z)) {
                        pos.y = h;
                    }
                    self.ensure_ai_faction_templates(player.team);
                    if self.create_object(building, player.team, pos).is_some() {
                        base = Some(pos);
                        log::info!(
                            "Wave 831/832: seeded starting building {} for player {} at {:?}",
                            building,
                            pid,
                            pos
                        );
                    }
                }
            }

            let Some(base_pos0) = base.or_else(|| self.team_base_position(player.team)) else {
                continue;
            };
            let mut base_pos = base_pos0;
            if let Some(h) = self.terrain_height_at(Vec3::new(base_pos.x, 0.0, base_pos.z)) {
                base_pos.y = h;
            }

            // --- Starting units 0..9 (C++ placeStartingUnits / MAX_MP_STARTING_UNITS) ---
            // Wave 832: walk residual.starting_units; retail usually only unit0 (dozer).
            let unit_names: Vec<&str> = residual
                .starting_units
                .iter()
                .copied()
                .filter(|n| !n.is_empty())
                .collect();
            if unit_names.is_empty() {
                continue;
            }
            self.ensure_ai_faction_templates(player.team);
            for (i, unit_name) in unit_names.iter().enumerate() {
                // Skip if this exact starting unit template already exists for the team.
                let already = self.objects.values().any(|o| {
                    o.team == player.team
                        && o.is_alive()
                        && o.template_name.eq_ignore_ascii_case(unit_name)
                });
                // For builders/workers: also treat any mobile constructor as present.
                let is_builder = unit_name.to_ascii_lowercase().contains("dozer")
                    || unit_name.to_ascii_lowercase().contains("worker");
                let has_builder = is_builder
                    && self.objects.values().any(|o| {
                        o.team == player.team
                            && o.is_alive()
                            && o.is_mobile()
                            && (o.can_construct()
                                || o.template_name.to_ascii_lowercase().contains("dozer")
                                || o.template_name.to_ascii_lowercase().contains("worker"))
                    });
                if already || has_builder {
                    continue;
                }

                // Offset around yard like C++ minRadius/maxRadius residual.
                let mut unit_pos =
                    base_pos + Vec3::new(40.0 + (i as f32) * 12.0, 0.0, -40.0 - (i as f32) * 6.0);
                if let Some(h) = self.terrain_height_at(Vec3::new(unit_pos.x, 0.0, unit_pos.z)) {
                    unit_pos.y = h;
                }
                if let Some(id) = self.create_object(unit_name, player.team, unit_pos) {
                    log::info!(
                        "Wave 832: starting unit player={} team={:?} spawned {} id={:?}",
                        pid,
                        player.team,
                        unit_name,
                        id
                    );
                } else if i == 0 {
                    // Fallback retail short names for unit0 only.
                    let fallback = match player.team {
                        Team::USA => "AmericaVehicleDozer",
                        Team::China => "ChinaVehicleDozer",
                        Team::GLA => "GLAInfantryWorker",
                        Team::Neutral => "",
                    };
                    if !fallback.is_empty() {
                        if let Some(id) = self.create_object(fallback, player.team, unit_pos) {
                            log::info!(
                                "Wave 832: starting unit fallback player={} {} id={:?}",
                                pid,
                                fallback,
                                id
                            );
                        }
                    }
                }
            }
        }
    }

    pub(super) fn ensure_non_shell_player_presence(
        &mut self,
        parsed_settings: Option<&super::script_loader::MapMetadata>,
    ) {
        if self.game_mode == GameMode::Shell {
            return;
        }
        // Wave 732: free fallback CC/dozer seeding is opt-in only (default fail-closed).
        // Retail maps must supply start objects. Vertical-slice/incomplete catalogs may set
        // GENERALS_RUNTIME_HOST_SEED_START_PRESENCE=1.
        let allow_seed =
            std::env::var_os("GENERALS_RUNTIME_HOST_SEED_START_PRESENCE").is_some_and(|v| {
                let s = v.to_string_lossy();
                !(s.is_empty()
                    || s == "0"
                    || s.eq_ignore_ascii_case("false")
                    || s.eq_ignore_ascii_case("no"))
            });
        if !allow_seed {
            return;
        }

        let mut team_order = Vec::new();
        let mut player_ids: Vec<u32> = self.players.keys().copied().collect();
        player_ids.sort_unstable();
        for player_id in player_ids {
            let Some(player) = self.players.get(&player_id) else {
                continue;
            };
            if player.team == Team::Neutral || team_order.contains(&player.team) {
                continue;
            }
            team_order.push(player.team);
        }
        if team_order.is_empty() {
            return;
        }

        let default_bounds_min = Vec3::new(-300.0, 0.0, -300.0);
        let default_bounds_max = Vec3::new(300.0, 0.0, 300.0);
        let (bounds_min, bounds_max) = parsed_settings
            .and_then(|meta| {
                meta.world_min.zip(meta.world_max).map(|(min, max)| {
                    (
                        Vec3::new(min.x, min.y, min.z),
                        Vec3::new(max.x, max.y, max.z),
                    )
                })
            })
            .filter(|(min, max)| (max.x - min.x).abs() >= 1.0 && (max.z - min.z).abs() >= 1.0)
            .unwrap_or((default_bounds_min, default_bounds_max));

        let span = bounds_max - bounds_min;
        let spawn_positions = [
            Vec3::new(
                bounds_min.x + span.x * 0.20,
                0.0,
                bounds_min.z + span.z * 0.20,
            ),
            Vec3::new(
                bounds_max.x - span.x * 0.20,
                0.0,
                bounds_max.z - span.z * 0.20,
            ),
            Vec3::new(
                bounds_max.x - span.x * 0.20,
                0.0,
                bounds_min.z + span.z * 0.20,
            ),
            Vec3::new(
                bounds_min.x + span.x * 0.20,
                0.0,
                bounds_max.z - span.z * 0.20,
            ),
            Vec3::new(
                bounds_min.x + span.x * 0.50,
                0.0,
                bounds_min.z + span.z * 0.15,
            ),
            Vec3::new(
                bounds_min.x + span.x * 0.50,
                0.0,
                bounds_max.z - span.z * 0.15,
            ),
        ];

        let mut spawned_count = 0usize;
        for (index, team) in team_order.into_iter().enumerate() {
            let has_presence = self
                .objects
                .values()
                .any(|object| object.team == team && object.is_alive());
            if has_presence {
                continue;
            }

            let mut spawn_position = spawn_positions[index % spawn_positions.len()];
            if let Some(ground_height) =
                self.terrain_height_at(Vec3::new(spawn_position.x, 0.0, spawn_position.z))
            {
                spawn_position.y = ground_height;
            }

            // Ensure faction template catalog (CC, barracks, combat units) exists.
            self.ensure_ai_faction_templates(team);

            let primary_template = match team {
                Team::USA => "USA_CommandCenter",
                Team::GLA => "GLA_CommandCenter",
                Team::China => "China_CommandCenter",
                Team::Neutral => "CommandCenter",
            };

            if self
                .create_object(primary_template, team, spawn_position)
                .is_none()
            {
                // Fallbacks for incomplete catalogs.
                let _ = self
                    .create_object("CommandCenter", team, spawn_position)
                    .or_else(|| self.create_object("GoldenCC", team, spawn_position));
            }
            // Seed a worker for the first human-capable team so DozerConstruct can start.
            if index == 0 {
                let dozer_pos = spawn_position + Vec3::new(20.0, 0.0, 0.0);
                let dozer_name = match team {
                    Team::USA => "USA_Dozer",
                    Team::China => "China_Dozer",
                    Team::GLA => "GLA_Worker",
                    Team::Neutral => "GoldenDozer",
                };
                if self.create_object(dozer_name, team, dozer_pos).is_none() {
                    // Install a minimal dozer if faction worker missing.
                    if !self.templates.contains_key("GoldenDozer") {
                        let mut d = ThingTemplate::new("GoldenDozer");
                        d.set_health(300.0);
                        d.set_cost(1000, 0);
                        d.add_kind_of(KindOf::Vehicle);
                        d.add_kind_of(KindOf::Worker);
                        d.add_kind_of(KindOf::Selectable);
                        self.templates.insert("GoldenDozer".into(), d);
                    }
                    let _ = self.create_object("GoldenDozer", team, dozer_pos);
                }
            }
            spawned_count += 1;
        }

        if spawned_count > 0 {
            log::info!(
                "Seeded {} fallback player start structures for non-shell map '{}'",
                spawned_count,
                self.map_name
            );
        }
    }

    pub(super) fn configure_victory_rules_for_map(&mut self, map_name: &str) {
        let rules = victory_rules_for_map(map_name);
        self.victory_conditions.set_victory_conditions(rules);
        log::info!(
            "Configured victory rules for map '{}': require units = {}, require buildings = {}",
            map_name,
            rules.requires_units(),
            rules.requires_buildings()
        );
    }

    pub(super) fn convert_script_event(&self, event: &ScriptEvent) -> Option<MissionScriptEvent> {
        use ScriptValue as Val;
        let mut params = HashMap::new();
        let event_type = match event {
            ScriptEvent::PlayerDefeated { player_id } => {
                params.insert("player_id".into(), Val::PlayerId(*player_id));
                "player_defeated"
            }
            ScriptEvent::AllianceStateChanged { player_id, state } => {
                params.insert("player_id".into(), Val::PlayerId(*player_id));
                params.insert(
                    "state".into(),
                    Val::String(format!("{:?}", state).to_lowercase()),
                );
                "alliance_state_changed"
            }
            ScriptEvent::RevealMapForPlayer { player_id } => {
                params.insert("player_id".into(), Val::PlayerId(*player_id));
                "reveal_map_for_player"
            }
            ScriptEvent::CompletedSpecialPower {
                player_id,
                special_power_name,
                creator_id,
            } => {
                params.insert("player_id".into(), Val::PlayerId(*player_id));
                params.insert(
                    "special_power".into(),
                    Val::String(special_power_name.clone()),
                );
                params.insert("creator_id".into(), Val::String(creator_id.to_string()));
                "completed_special_power"
            }
        };

        Some(MissionScriptEvent {
            event_type: event_type.to_string(),
            parameters: params,
            timestamp: std::time::Instant::now(),
            priority: ScriptPriority::Normal,
        })
    }

    /// Move an object to a target position using pathfinding.
    /// Falls back to direct movement if no path is found.
    /// If `ai_state_override` is provided, sets that AI state after moving.
    pub(super) fn move_object_with_pathfinding(
        &mut self,
        object_id: ObjectId,
        target_position: Vec3,
        ai_state_override: Option<AIState>,
    ) {
        let start_pos = self.objects.get(&object_id).map(|obj| obj.get_position());

        let start_pos = match start_pos {
            Some(pos) => pos,
            None => return,
        };

        let decision_auth = crate::gameworld_shadow::gameworld_ai_decision_authority_live();
        let apply_state = |logic: &mut Self, state: AIState| {
            if decision_auth {
                let ordinal =
                    crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(&state);
                crate::game_logic::host_ai_decision_log::record_set_state(object_id, ordinal);
            } else if let Some(obj) = logic.objects.get_mut(&object_id) {
                obj.set_ai_state(state);
            }
        };

        // Short distance — skip pathfinding overhead and go direct.
        if start_pos.distance(target_position) < 20.0 {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.move_to(target_position);
            }
            if let Some(state) = ai_state_override {
                apply_state(self, state);
            }
            return;
        }

        // Attempt A* pathfinding.
        let path = self
            .pathfinding_system
            .find_path(start_pos, target_position, &self.objects);

        let mut state_to_apply: Option<AIState> = None;
        if let Some(obj) = self.objects.get_mut(&object_id) {
            if let Some(waypoints) = path {
                if waypoints.len() >= 2 {
                    obj.movement.path = waypoints;
                    obj.record_host_movement();
                    obj.movement.current_path_index = 1; // skip start node
                                                         // target_position will be set to path[1] by update_movement
                    obj.movement.target_position = Some(obj.movement.path[1]);
                    obj.set_status_moving(true);
                    // Final destination for shadow move channel (not intermediate waypoint).
                    crate::game_logic::host_move_log::record(
                        object_id,
                        Some([target_position.x, target_position.y, target_position.z]),
                    );
                    state_to_apply = Some(ai_state_override.unwrap_or(AIState::Moving));
                } else {
                    obj.move_to(target_position);
                    state_to_apply = ai_state_override;
                }
            } else {
                // No path found — fall back to direct movement.
                obj.move_to(target_position);
                state_to_apply = ai_state_override;
            }
        }
        if let Some(state) = state_to_apply {
            apply_state(self, state);
        }
    }

    /// Update movement for all objects
    pub(super) fn update_movement(&mut self, object_ids: &[ObjectId], dt: f32) {
        // GameWorld movement authority: path integrate + pose last-write runs in
        // shadow_session_after_host_tick via GameWorld::step_movement. Host still
        // owns path *commands* (move_to / attack-move logs) earlier in the frame.
        // Wave 875: movement authority early-return honesty — GW sole integrate.
        if crate::gameworld_shadow::gameworld_movement_authority_live() {
            let _ = (object_ids, dt);
            return;
        }
        for &id in object_ids {
            if let Some(obj) = self.objects.get_mut(&id) {
                // Horizontal (XZ) distance — path grid / terrain height use Y separately,
                // and 3D distance falsely stalls waypoint advance when |ΔY| is large.
                let horiz = |a: Vec3, b: Vec3| {
                    let dx = a.x - b.x;
                    let dz = a.z - b.z;
                    (dx * dx + dz * dz).sqrt()
                };

                if !obj.movement.path.is_empty()
                    && obj.movement.current_path_index < obj.movement.path.len()
                {
                    let current_pos = obj.get_position();
                    let waypoint = obj.movement.path[obj.movement.current_path_index];
                    if horiz(current_pos, waypoint) < 5.0 {
                        obj.movement.current_path_index += 1;
                        if obj.movement.current_path_index >= obj.movement.path.len() {
                            let do_evac = obj.pending_evacuate_on_stop;
                            let and_exit = obj.pending_exit_after_evacuate;
                            obj.stop_moving();
                            if do_evac {
                                // Defer mut borrow: process after this get_mut ends.
                                // Use a side channel via pending flags re-set for post-pass.
                                obj.pending_evacuate_on_stop = true;
                                obj.pending_exit_after_evacuate = and_exit;
                            }
                            continue;
                        }
                    }

                    let waypoint = obj.movement.path[obj.movement.current_path_index];
                    // Keep unit height; path cells often sit at Y=0 from the grid.
                    let mut target = waypoint;
                    target.y = current_pos.y;
                    obj.movement.target_position = Some(target);
                }

                if let Some(target_pos) = obj.movement.target_position {
                    let current_pos = obj.get_position();
                    // March in the XZ plane; do not dive to Y=0 path cells.
                    let mut flat_target = target_pos;
                    flat_target.y = current_pos.y;
                    let direction = (flat_target - current_pos).normalize_or_zero();

                    if direction.length() > 0.0 {
                        // Calculate new position and orientation
                        let target_velocity = direction * obj.movement.max_speed;
                        let velocity_diff = target_velocity - obj.movement.velocity;
                        let max_accel = obj.movement.acceleration * dt;

                        let new_velocity = if velocity_diff.length() <= max_accel {
                            target_velocity
                        } else {
                            obj.movement.velocity + velocity_diff.normalize() * max_accel
                        };

                        // Persist velocity — without this, every frame restarts from 0 and
                        // units crawl at ~accel*dt per frame (pure-march combat stalls OOR).
                        obj.movement.velocity = new_velocity;
                        obj.record_host_movement();

                        let new_position = current_pos + new_velocity * dt;
                        let desired_angle = (-new_velocity.z).atan2(new_velocity.x);
                        let reached_target = horiz(current_pos, flat_target) < 2.0;

                        obj.set_position(new_position);
                        obj.set_orientation(desired_angle);
                        if reached_target {
                            // Only stop when there is no further path waypoint.
                            // Mid-path "reached" is handled by index advance above.
                            if obj.movement.path.is_empty()
                                || obj.movement.current_path_index + 1 >= obj.movement.path.len()
                            {
                                obj.stop_moving();
                            } else {
                                obj.movement.current_path_index += 1;
                                let mut next = obj.movement.path[obj.movement.current_path_index];
                                next.y = new_position.y;
                                obj.movement.target_position = Some(next);
                            }
                        }
                    } else {
                        // Already on target (zero horizontal delta).
                        obj.movement.velocity = Vec3::ZERO;
                        obj.record_host_movement();
                        if obj.movement.path.is_empty()
                            || obj.movement.current_path_index + 1 >= obj.movement.path.len()
                        {
                            obj.stop_moving();
                        }
                    }
                }
            }
        }

        // C++ AICMD_MOVE_TO_POSITION_AND_EVACUATE arrival residual.
        let mut evac_now: Vec<(ObjectId, bool)> = Vec::new();
        for &id in object_ids {
            if let Some(obj) = self.objects.get(&id) {
                if obj.pending_evacuate_on_stop
                    && obj.movement.path.is_empty()
                    && !obj.status.moving
                {
                    evac_now.push((id, obj.pending_exit_after_evacuate));
                }
            }
        }
        for (id, and_exit) in evac_now {
            let _ = self.evacuate_container_now(id, and_exit);
        }
    }

    #[cfg(test)]
    pub fn update_movement_for_test(&mut self, object_ids: &[ObjectId], dt: f32) {
        self.update_movement(object_ids, dt);
    }

    /// Update AI behavior for all objects
    /// Enhanced with AI decision system for intelligent behavior

    /// Drain global fire-spawn queue into host CombatSystem (fire-spawn authority apply).
    pub(crate) fn drain_pending_projectiles_into_combat(&mut self) {
        crate::game_logic::combat::drain_pending_projectiles(
            &mut self.combat_system,
            &self.objects,
        );
    }

    /// Hit-only projectile pass after GameWorld flight integrate writeback.
    pub(crate) fn resolve_projectiles_hits_only(&mut self) -> Vec<ObjectId> {
        self.combat_system.refresh_homing_aims(&self.objects);
        self.combat_system.update_projectiles_with_countermeasures(
            0.0,
            &mut self.objects,
            Some(&mut self.countermeasures),
            self.frame,
        )
    }

    pub(super) fn update_ai(&mut self, object_ids: &[ObjectId], dt: f32) {
        use crate::ai_decisions::*;
        self.flush_countermeasure_flare_spawns();
        // Wave 806: under coupled shadow, lifetime owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_countermeasure_flare_objects();
        }

        let mut ai_commands = Vec::new();
        let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP; // Convert frame to seconds
        let game_phase = GamePhase::from_time(current_time);
        // Campaign maps place thousands of decorative props. Skip AI for
        // non-combat, non-structure objects so frame cost stays reasonable.
        let dense_world = object_ids.len() > 400;

        // BattlePlan pack/unpack door residual (AnimationTime 7000ms → 210 frames).
        self.tick_battle_plan_door_residuals();

        // First pass: Dispatch object AI through the existing state machine.
        for &object_id in object_ids {
            // Expire DISABLED_HACKED / DISABLED_EMP / Frenzy residual timers.
            let mut topple_kill = false;
            let mut lifetime_kill = false;
            let mut poison_kill = false;
            let mut defector_audio: Vec<String> = Vec::new();
            if let Some(obj) = self.objects.get_mut(&object_id) {
                // Wave 761: under coupled GameWorld shadow, status timer expire
                // (faerie/repulsor/disable/frenzy/continuous-fire/selection flash)
                // is owned by `tick_status_timer_expirations` + writeback. Host must
                // not dual-expire mid-frame. Eject-invulnerable stays host-only
                // (no GW until_frame field yet).
                let peel_status_timers = crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active();
                if !peel_status_timers {
                    obj.tick_disabled_hacked(self.frame);
                    obj.tick_selection_flash();
                    obj.tick_disabled_emp(self.frame);
                    obj.tick_disabled_paralyzed(self.frame);
                    obj.tick_weapon_bonus_frenzy(self.frame);
                    obj.tick_faerie_fire(self.frame);
                }
                // Wave 762: under coupled shadow, eject-invulnerable expire is
                // owned by GW tick_status_timer_expirations + writeback.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    obj.tick_eject_invulnerable(self.frame);
                }
                // Wave 766: under coupled shadow, ObjectDefectionHelper timer is
                // owned by GW tick_status_timer_expirations + writeback.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    // C++ ObjectDefectionHelper::update residual.
                    obj.tick_defection_helper(self.frame);
                }
                // Snapshot FireWeaponPower residual before further mut uses.
                let fwp_shot = obj.fire_weapon_power.as_ref().and_then(|req| {
                    if req.shots_remaining == 0 {
                        None
                    } else if req.has_location {
                        Some((req.target_x, req.target_z, true))
                    } else {
                        let p = obj.get_position();
                        Some((p.x, p.z, false))
                    }
                });
                if let Some((tx, tz, _has_loc)) = fwp_shot {
                    obj.target_location = Some(glam::Vec3::new(tx, 0.0, tz));
                    obj.set_ai_state(crate::game_logic::AIState::Attacking);
                    if let Some(req) = obj.fire_weapon_power.as_mut() {
                        req.shots_remaining = req.shots_remaining.saturating_sub(1);
                        if req.shots_remaining == 0 {
                            obj.fire_weapon_power = None;
                        }
                    }
                }
                // Drain defector audio residual (collect then queue outside obj borrow).
                defector_audio = obj
                    .defection_helper
                    .as_mut()
                    .map(|d| d.drain_audio())
                    .unwrap_or_default();
                // C++ PoisonedBehavior::update residual (DoT).
                // Wave 769: under coupled shadow, PoisonedBehavior DoT is owned by
                // GW tick_status_timer_expirations + host_poison_dot_log drain.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    if let Some((dot, death_ty)) = obj.tick_poisoned_behavior(self.frame) {
                        // Apply as UNRESISTABLE so it doesn't re-infect (C++).
                        let killed = obj.take_damage_from_typed_death(
                            dot,
                            None,
                            crate::game_logic::combat::DamageType::Unresistable,
                            death_ty,
                        );
                        if killed {
                            poison_kill = true;
                        }
                    }
                }
                // Wave 778: under coupled shadow, FWWDB continuous is owned by
                // GW tick_status_timer_expirations + host_fwwd_continuous_log drain.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    if let Some(w) = obj.tick_fire_weapon_when_damaged_continuous(self.frame) {
                        // Prefer pending reaction (from onDamage) over continuous same frame.
                        if obj.pending_fire_when_damaged_weapon.is_none() {
                            obj.pending_fire_when_damaged_weapon = Some(w);
                        }
                    }
                }
                // C++ LifetimeUpdate residual.
                // Wave 745: under damage authority, do not zero host HP / stamp
                // destroyed mid-frame (dual with GW HP writeback). Mark-for-destroy
                // owns lethal residual; non-authority path keeps host HP clear.
                // Wave 768: under coupled shadow, LifetimeUpdate expire is owned by
                // GW tick_status_timer_expirations + host_lifetime_expire_log drain.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    if obj.tick_lifetime_update(self.frame) {
                        lifetime_kill = true;
                        if !crate::gameworld_shadow::gameworld_damage_authority_live() {
                            obj.health.current = 0.0;
                            obj.status.destroyed = true;
                            obj.refresh_model_condition_bits();
                        }
                    }
                }
                // Wave 761: continuous-fire coast + repulsor expire peel under coupled.
                // Wave 761: CF coast + repulsor peel under coupled.
                // Wave 765: subdual heal owned by GW when coupled.
                // Wave 767: fire-sound loop stop owned by GW tick_status_timer_expirations
                // when coupled (non-coupled still via tick_continuous_fire_coast).
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    obj.tick_continuous_fire_coast(self.frame);
                    obj.tick_repulsor_status(self.frame);
                }
                // Wave 763: under coupled shadow, force-reload-when-idle is owned by
                // GW tick_status_timer_expirations + weapon_stats/continuous-fire writeback.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    obj.tick_force_reload_when_idle(self.frame);
                }
                obj.tick_spy_vision_disabled(self.frame);
                if obj.tick_disguise_transition() {
                    self.bomb_truck_disguise.record_transition_halfpoint();
                }
                // Wave 770: under coupled shadow, ToppleUpdate fall is owned by
                // GW tick_status_timer_expirations + host_topple_kill_log drain.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    // C++ ToppleUpdate::update residual (trees / crushable props).
                    topple_kill = obj.tick_topple();
                }
                // C++ StructureToppleUpdate::update residual (buildings).
                // C++ HeightDieUpdate residual (bombs/missiles).
                // Wave 771: under coupled shadow, HeightDieUpdate is owned by
                // GW tick_status_timer_expirations + host_height_die_kill_log drain.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    if !topple_kill && obj.tick_height_die(self.frame, 0.0) {
                        topple_kill = true;
                    }
                }
                // Wave 772: under coupled shadow, JetSlowDeathBehavior is owned by
                // GW tick_status_timer_expirations + host_jet_slow_death_kill_log drain.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    // C++ JetSlowDeathBehavior residual.
                    if !topple_kill
                        && obj
                            .jet_slow_death
                            .as_ref()
                            .map(|j| j.is_active())
                            .unwrap_or(false)
                    {
                        topple_kill = obj.tick_jet_slow_death(self.frame, 0.0);
                    }
                }
                // Wave 773: under coupled shadow, HelicopterSlowDeathBehavior is owned by
                // GW tick_status_timer_expirations + host_heli_slow_death_kill_log drain.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    // C++ HelicopterSlowDeathBehavior residual.
                    if !topple_kill
                        && obj
                            .helicopter_slow_death
                            .as_ref()
                            .map(|h| h.is_active())
                            .unwrap_or(false)
                    {
                        topple_kill = obj.tick_helicopter_slow_death(self.frame, 0.0);
                    }
                }
                // Wave 774: under coupled shadow, SlowDeathBehavior is owned by
                // GW tick_status_timer_expirations + host_slow_death_kill_log drain.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    if !topple_kill
                        && obj
                            .slow_death
                            .as_ref()
                            .map(|s| s.is_active())
                            .unwrap_or(false)
                    {
                        topple_kill = obj.tick_slow_death(self.frame);
                    }
                }
                // Wave 775: under coupled shadow, StructureCollapseUpdate is owned by
                // GW tick_status_timer_expirations + host_structure_collapse_kill_log drain.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    if !topple_kill && obj.structure_collapse_data.is_some() {
                        topple_kill = obj.tick_structure_collapse(self.frame);
                    }
                }
                // Wave 776: under coupled shadow, StructureToppleUpdate is owned by
                // GW tick_status_timer_expirations + host_structure_topple_kill_log drain.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    if !topple_kill && obj.structure_topple_data.is_some() {
                        topple_kill = obj.tick_structure_topple(self.frame);
                    }
                }
            }
            // Wave 777: under coupled shadow, StructureTopple crush sweep is owned by
            // GW tick_status_timer_expirations + host_structure_topple_crush_log drain.
            if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                && crate::gameworld_shadow::shadow_coupled_tick_active())
            {
                // C++ StructureToppleUpdate::applyCrushingDamage residual.
                if self
                    .objects
                    .get(&object_id)
                    .and_then(|o| o.structure_topple_data.as_ref())
                    .map(|d| d.is_active() || topple_kill)
                    .unwrap_or(false)
                {
                    let samples = self
                        .objects
                        .get_mut(&object_id)
                        .map(|o| o.take_structure_topple_crush_samples())
                        .unwrap_or_default();
                    if !samples.is_empty() {
                        self.apply_structure_topple_crush_samples(object_id, samples);
                    }
                }
            }
            // C++ FireWeaponWhenDamagedBehavior forceFire residual (reaction + continuous).
            if let Some(wname) = self
                .objects
                .get_mut(&object_id)
                .and_then(|o| o.take_pending_fire_when_damaged_weapon())
            {
                let _ = self.apply_fire_weapon_when_damaged_named(object_id, &wname);
            }
            // C++ TransitionDamageFX + FXListDie residuals → audio queue.
            {
                let pos = self
                    .objects
                    .get(&object_id)
                    .map(|o| o.get_position())
                    .unwrap_or(glam::Vec3::ZERO);
                let (transition_evs, death_fx, death_audio) =
                    if let Some(o) = self.objects.get_mut(&object_id) {
                        let te = o.take_pending_transition_damage_fx();
                        let (df, da) = o.take_pending_death_fx_audio();
                        (te, df, da)
                    } else {
                        (Vec::new(), None, None)
                    };
                for ev in transition_evs {
                    if let Some(a) = ev.audio_name {
                        self.queue_audio_event(
                            AudioEventRequest::new(&a)
                                .with_object(object_id)
                                .with_position(pos)
                                .with_priority(140),
                        );
                    }
                    // FX name residual: store as high-priority audio-tagged event for client peel.
                    if let Some(fx) = ev.fx_name {
                        self.queue_audio_event(
                            AudioEventRequest::new(&format!("FX:{fx}"))
                                .with_object(object_id)
                                .with_position(pos)
                                .with_priority(130),
                        );
                    }
                }
                if let Some(a) = death_audio {
                    self.queue_audio_event(
                        AudioEventRequest::new(&a)
                            .with_object(object_id)
                            .with_position(pos)
                            .with_priority(200),
                    );
                }
                if let Some(fx) = death_fx {
                    self.queue_audio_event(
                        AudioEventRequest::new(&format!("FX:{fx}"))
                            .with_object(object_id)
                            .with_position(pos)
                            .with_priority(190),
                    );
                }
            }
            // C++ CreateObjectDie residual (spawn after death FX).
            self.apply_pending_create_object_die(object_id);
            if lifetime_kill {
                self.mark_object_for_destruction(object_id, None);
                continue;
            }
            for a in defector_audio {
                self.queue_audio_event(
                    crate::game_logic::AudioEventRequest::new(a.as_str())
                        .with_object(object_id)
                        .with_priority(80),
                );
            }
            if poison_kill {
                self.mark_object_for_destruction(object_id, None);
                continue;
            }
            if topple_kill {
                // Completed topple: queue destroy (structure Done bypasses re-topple).
                self.mark_object_for_destruction(object_id, None);
                continue;
            }
            // OCL_EjectPilotViaParachute residual sink (elevated pilot → ground).
            self.tick_eject_parachute_residual(object_id);
            // AmericaCrateParachute residual sink (cargo crate freefall → OpenDist → land).
            self.tick_crate_parachute_residual(object_id);
            // PilotFindVehicleUpdate residual: AI idle pilot auto-scan for
            // recrewable unmanned vehicles (ScanRate 1000ms / range 300 / MinHealth 0.5).
            // C++ human players sleep forever — host residual: is_local → skip.
            // Base-center fallback residual when no vehicle found (m_didMoveToBase).
            self.try_pilot_find_vehicle_residual(object_id);
            // AutoFindHealingUpdate residual: AI idle injured USA infantry → HealPad.
            // Pilot / Ranger / MissileDefender / Pathfinder / ColonelBurton residual.
            // ScanRate 1000ms / ScanRange 300 / NeverHeal 0.85 (AlwaysHeal busy path fail-closed).
            self.try_auto_find_healing_residual(object_id);
            // AutoFindRepair residual: AI idle damaged vehicles → RepairPad/WarFactory.
            self.try_auto_find_repair_residual(object_id);
            self.try_auto_resume_construction_residual(object_id);
            // C++ AIIdleState: CAN_BE_REPULSED idle units flee closest repulsor.
            let _ = self.try_idle_repulse(object_id);
            // C++ AIIdleState: checkForCrateToPickup → aiMoveToObject.
            let _ = self.try_idle_crate_pickup(object_id);
            if let Some(obj) = self.objects.get(&object_id) {
                let can_attack = obj.can_attack();
                if dense_world
                    && !can_attack
                    && !obj.is_kind_of(KindOf::Structure)
                    && !obj.is_kind_of(KindOf::Worker)
                    && !obj.is_kind_of(KindOf::Infantry)
                    && !obj.is_kind_of(KindOf::Vehicle)
                    && !obj.is_kind_of(KindOf::Aircraft)
                    && obj.target.is_none()
                    && matches!(obj.ai_state, AIState::Idle)
                {
                    continue;
                }
                let position = obj.get_position();
                let team = obj.team;
                let ai_state = obj.ai_state.clone();
                let current_target = obj.target;
                if let Some(command) = self.process_ai_behavior(
                    object_id,
                    ai_state,
                    current_target,
                    position,
                    team,
                    can_attack,
                    self.frame,
                    dt,
                ) {
                    ai_commands.push(command);
                }
            }
        }

        // Second pass: Handle production buildings
        for &object_id in object_ids {
            let (team, is_production_building) = match self.objects.get(&object_id) {
                Some(obj)
                    if obj.is_kind_of(KindOf::Structure)
                        && obj.is_constructed()
                        && obj.is_alive() =>
                {
                    let is_production_building = obj.template_name.contains("Barracks")
                        || obj.template_name.contains("WarFactory")
                        || obj.template_name.contains("ArmsDealer");
                    (obj.team, is_production_building)
                }
                _ => continue,
            };

            if !is_production_building {
                continue;
            }

            // Find which player owns this building.
            let player_id = self
                .players
                .iter()
                .find_map(|(pid, player)| (player.team == team).then_some(*pid));

            let Some(pid) = player_id else {
                continue;
            };

            // Check if should produce units (every 10 seconds).
            if !self.frame.is_multiple_of(600) {
                continue;
            }

            if let Some(unit_to_produce) =
                AIDecisionSystem::select_production_unit(self, team, game_phase, pid)
            {
                // Queue unit production (in a full implementation)
                log::trace!(
                    "AI Building {} queuing production of {}",
                    object_id,
                    unit_to_produce
                );

                self.enqueue_production(object_id, unit_to_produce);
            }
        }

        // GuardRetaliate victim-death / return residual.
        self.tick_guard_retaliate_states();
        // HijackerUpdate ride residual.
        self.tick_hijacker_updates();
        // C++ UndeadBody + BattleBusSlowDeathBehavior residual.
        self.tick_battle_bus_slow_deaths();
        // C++ AssaultTransportAIUpdate wounded-retrieve residual.
        self.tick_assault_transport_updates();
        // C++ DeployStyleAIUpdate pack/unpack residual.
        self.tick_deploy_style_updates();
        // C++ CommandButtonHuntUpdate residual.
        self.tick_command_button_hunt_updates();

        // Apply all AI commands (or log-only when GameWorld owns decision apply).
        let decision_auth = crate::gameworld_shadow::gameworld_ai_decision_authority_live();
        for command in ai_commands {
            if decision_auth {
                // Record only — shadow applies SetAttackTarget/SetMoveTarget/SetAiState.
                match &command {
                    AICommand::AttackTarget {
                        object_id,
                        target_id,
                    } => crate::game_logic::host_ai_decision_log::record_attack(
                        *object_id, *target_id,
                    ),
                    AICommand::StopAttack { object_id } => {
                        crate::game_logic::host_ai_decision_log::record_stop_attack(*object_id)
                    }
                    AICommand::MoveTo {
                        object_id,
                        position,
                    } => crate::game_logic::host_ai_decision_log::record_move_to(
                        *object_id, *position,
                    ),
                    AICommand::SetAIState { object_id, state } => {
                        let ordinal =
                            crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(&state);
                        crate::game_logic::host_ai_decision_log::record_set_state(
                            *object_id, ordinal,
                        );
                    }
                }
            } else {
                self.apply_ai_command(command);
            }
        }

        // Resolve command-driven support states (guard/repair/docking/garrison) after AI decisions.
        self.update_support_states(object_ids, dt);
    }

    /// Update combat for all objects.
    ///
    /// Fail-closed residual: uses secondary when present and selected by
    /// `Object::select_combat_weapon_slot` (prefer secondary vs structures when
    /// secondary damage is better; alternate secondary when primary not ready).
    /// Not full C++ AutoChoose / PreferredAgainst matrices.
    ///
    /// `pub(crate)` so residual/unit tests can exercise the fire path directly.
    /// C++ JetAIUpdate RETURN_TO_BASE residual: rearm empty jet weapons near
    /// a friendly airfield (FSAirfield / name residual). Fail-closed vs full
    /// ParkingPlace reserve / taxi matrix.
    /// C++ JetOrHeliCirclingDeadAirfieldState residual for all empty RTB jets.
    pub(crate) fn tick_out_of_ammo_jet_damage(&mut self) {
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive()
                    && (o.is_kind_of(KindOf::Aircraft) || o.object_type == ObjectType::Aircraft)
                    && o.needs_return_to_base_rearm()
            })
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            if self.try_return_to_base_rearm(id) {
                continue;
            }
            if let Some(jet) = self.objects.get_mut(&id) {
                let _ = jet.apply_out_of_ammo_damage_frame();
            }
        }
    }

    /// C++ ParkingPlaceBehavior / JetAIUpdate airfield residual helper.
    pub(crate) fn is_friendly_airfield(obj: &Object, jet_team: Team) -> bool {
        if !obj.is_alive() || obj.team != jet_team || obj.status.under_construction {
            return false;
        }
        obj.is_kind_of(KindOf::FSAirfield)
            || (obj.is_kind_of(KindOf::Structure) && {
                let n = obj.template_name.to_ascii_lowercase();
                n.contains("airfield") || n.contains("airbase") || n.contains("hangar")
            })
    }

    /// Retail airfield parking capacity residual (NumRows 2 × NumCols 2 = 4).
    pub(crate) fn airfield_parking_capacity() -> usize {
        use crate::game_logic::host_dock_contain_exit_heal_residual::{
            PARKING_PLACE_AIRFIELD_NUM_COLS, PARKING_PLACE_AIRFIELD_NUM_ROWS,
        };
        (PARKING_PLACE_AIRFIELD_NUM_ROWS.max(0) as usize)
            * (PARKING_PLACE_AIRFIELD_NUM_COLS.max(0) as usize)
    }

    /// Count jets currently docked at this airfield (contained_by residual).
    pub(crate) fn airfield_parked_count(&self, airfield_id: ObjectId) -> usize {
        self.objects
            .values()
            .filter(|o| {
                o.is_alive()
                    && o.contained_by == Some(airfield_id)
                    && (o.is_kind_of(KindOf::Aircraft) || o.object_type == ObjectType::Aircraft)
            })
            .count()
    }

    /// C++ Weapon ContinueAttackRange residual (AIStates attack transfer).
    ///
    /// When a kill lands and the firing weapon has ContinueAttackRange > 0,
    /// retarget the nearest living same-team-as-victim object within that
    /// radius of the original victim position (mine-clear chain residual).

    /// C++ Weapon computeApproachTarget residual for host attack moves.

    /// C++ MinimumAttackRange residual: back away when too close to fire.
    ///
    /// Returns true when a backup move was issued.

    /// C++ ScatterRadius residual for instant update_combat hits.
    ///
    /// Returns true when the shot misses the intended target after scatter offset.

    /// Resolve ScatterRadius for an instant shot: (misses_intended, impact_pos, splash_r).
    pub(crate) fn resolve_instant_scatter_shot(
        &self,
        attacker_id: ObjectId,
        target_id: ObjectId,
        slot: u8,
        target_pos: glam::Vec3,
    ) -> (bool, glam::Vec3, f32) {
        use crate::game_logic::weapon_bootstrap::{
            host_effective_scatter_radius, host_primary_damage_radius_for_weapon_name,
            host_secondary_damage_radius_for_weapon_name, scatter_impact_offset,
            scatter_misses_intended_target, scatter_seed_for_shot, DEFAULT_SCATTER_HIT_RADIUS,
        };
        let (wname, tgt_inf, hit_r, weapon_splash) = {
            let attacker = match self.objects.get(&attacker_id) {
                Some(a) => a,
                None => return (false, target_pos, 0.0),
            };
            let target = match self.objects.get(&target_id) {
                Some(t) => t,
                None => return (false, target_pos, 0.0),
            };
            let wname = if slot == 1 {
                attacker
                    .thing
                    .template
                    .secondary_weapon_name
                    .clone()
                    .or_else(|| attacker.thing.template.primary_weapon_name.clone())
            } else {
                attacker.thing.template.primary_weapon_name.clone()
            };
            let hit_r = if target.selection_radius > 0.0 {
                target.selection_radius
            } else {
                DEFAULT_SCATTER_HIT_RADIUS
            };
            let splash = attacker
                .weapon_slot(slot)
                .map(|w| w.splash_radius.max(0.0))
                .unwrap_or(0.0);
            (wname, target.is_kind_of(KindOf::Infantry), hit_r, splash)
        };
        let Some(name) = wname else {
            return (false, target_pos, weapon_splash);
        };
        let scatter = host_effective_scatter_radius(&name, tgt_inf);
        let primary_r = host_primary_damage_radius_for_weapon_name(&name);
        let secondary_r = host_secondary_damage_radius_for_weapon_name(&name);
        let splash_r = weapon_splash.max(primary_r).max(secondary_r);
        if scatter <= 0.0 {
            return (false, target_pos, splash_r);
        }
        let seed = scatter_seed_for_shot(attacker_id.0, target_id.0, self.frame);
        let offset = scatter_impact_offset(seed, scatter);
        let impact = target_pos + offset;
        let misses = scatter_misses_intended_target(scatter, seed, hit_r);
        (misses, impact, splash_r)
    }

    /// Apply residual area damage at a scatter impact point (missed intended target).
    ///
    /// Deals full `weapon_damage` within primary splash radius and half in the
    /// outer ring to secondary radius. Respects team vs RadiusDamageAffects ENEMIES
    /// residual (host default: enemies + neutrals).

    /// C++ dealDamageInternal dual-radius residual after a direct hit.
    ///
    /// Intended target is already damaged; splash hits others in primary/secondary
    /// rings using PrimaryDamage / SecondaryDamage peels and RadiusDamageAffects.

    /// C++ shockwave residual around an impact (hit or miss splash).

    /// Sample TerrainLogic-ish cliff/water residual at world position for stun destruction.

    /// C++ TerrainLogic::setWaterHeight damage residual.
    ///
    /// When water rises, every object currently underwater takes `damage_amount`
    /// as DAMAGE_WATER (DEATH_NORMAL). Airborne/aircraft and projectiles skip.
    /// Returns number of objects damaged.
    pub fn apply_water_rise_damage(&mut self, damage_amount: f32) -> u32 {
        if !(damage_amount > 0.0) {
            return 0;
        }
        let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        let mut hit = 0u32;
        let mut destroy: Vec<ObjectId> = Vec::new();
        for id in ids {
            let pos = match self.objects.get(&id) {
                Some(o) => o.get_position(),
                None => continue,
            };
            let (_cliff, water) = self.sample_stun_surface_at(pos);
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            obj.cell_is_underwater = water;
            if !water || !obj.is_alive() || obj.status.destroyed {
                continue;
            }
            if obj.is_kind_of(KindOf::Aircraft) || obj.is_kind_of(KindOf::Projectile) {
                continue;
            }
            // Naval/hover peels: skip if template suggests boat/ship/amphibious.
            let n = obj.template_name.to_ascii_lowercase();
            if n.contains("boat")
                || n.contains("ship")
                || n.contains("hover")
                || n.contains("amphib")
                || n.contains("carrier")
                || n.contains("destroyer")
                || n.contains("battleship")
            {
                continue;
            }
            let killed = obj.take_damage_from_typed(
                damage_amount,
                None,
                crate::game_logic::combat::DamageType::Water,
            );
            hit = hit.saturating_add(1);
            if killed || obj.status.destroyed || obj.health.current <= 0.0 {
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction(id, None);
        }
        hit
    }

    /// Refresh underwater/cliff cells; on dry→wet edge apply residual water damage.
    ///
    /// C++ only damages on water-rise events; edge detection approximates that when
    /// terrain water state changes under units (flood scripts / map water).
    pub fn refresh_surface_cells_and_water_edge_damage(&mut self, edge_damage: f32) -> u32 {
        let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        let mut hit = 0u32;
        let mut destroy: Vec<ObjectId> = Vec::new();
        for id in ids {
            let (pos, was_under) = match self.objects.get(&id) {
                Some(o) => (o.get_position(), o.cell_is_underwater),
                None => continue,
            };
            let (_cliff, water) = self.sample_stun_surface_at(pos);
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            obj.cell_is_cliff = _cliff;
            obj.cell_is_underwater = water;
            let entered = water && !was_under;
            if !entered || !(edge_damage > 0.0) {
                continue;
            }
            if !obj.is_alive() || obj.status.destroyed {
                continue;
            }
            if obj.is_kind_of(KindOf::Aircraft) || obj.is_kind_of(KindOf::Projectile) {
                continue;
            }
            let n = obj.template_name.to_ascii_lowercase();
            if n.contains("boat")
                || n.contains("ship")
                || n.contains("hover")
                || n.contains("amphib")
                || n.contains("carrier")
                || n.contains("destroyer")
                || n.contains("battleship")
            {
                continue;
            }
            let killed = obj.take_damage_from_typed(
                edge_damage,
                None,
                crate::game_logic::combat::DamageType::Water,
            );
            hit = hit.saturating_add(1);
            if killed || obj.status.destroyed || obj.health.current <= 0.0 {
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction(id, None);
        }
        hit
    }

    pub(crate) fn sample_stun_surface_at(&self, pos: glam::Vec3) -> (bool, bool) {
        if let Some(t) = self.terrain.as_ref() {
            return (t.is_cliff_at_world(pos), t.is_underwater_at_world(pos));
        }
        // Fall back to gamelogic TerrainLogic singleton when Main terrain is unset.
        if let Ok(tl) = gamelogic::terrain::get_terrain_logic().read() {
            // Host XZ ground plane == C++ XY for terrain queries.
            let cliff = tl.is_cliff_cell(pos.x, pos.z);
            let water = tl.is_underwater(pos.x, pos.z, None, None);
            return (cliff, water);
        }
        (false, false)
    }

    /// Advance Physics stun residual on all shocked units.
    ///
    /// Refreshes cell_is_cliff / cell_is_underwater from terrain before each tick
    /// so testStunnedUnitForDestruction sees live surface residual.

    /// C++ PhysicsBehavior onCollide vehicle crash weapon residual.
    ///
    /// Fires temp crash weapon name residual (splash damage at vehicle) and
    /// destroys the vehicle when falling into a structure.

    /// C++ PhysicsBehavior onCollide immobile bounce residual (stiffness / parachute).
    ///
    /// Returns true if bounce was applied. Vehicle crash path is separate
    /// (`apply_vehicle_crash_into_immobile`).

    /// C++ PhysicsBehavior::checkForOverlapCollision residual between two objects.
    ///
    /// `same_team` treats relationship as allies (no crush).

    /// C++ PhysicsBehavior::onCollide residual orchestration (host).
    ///
    /// Order: ignore-collisions gate → mutual parachute skip → overlap crush
    /// → immobile bounce/crash. Sets last_collidee when a real collide runs.
    /// Returns true if the pair was handled (skip generic bounce force).
    pub fn try_physics_collide(&mut self, a_id: ObjectId, b_id: ObjectId, us_radius: f32) -> bool {
        // Snapshot flags without holding borrows across mutates.
        let (
            a_ignore_b,
            b_ignore_a,
            a_para,
            b_para,
            a_team,
            b_team,
            b_immobile,
            a_infantry,
            b_unmanned,
        ) = {
            let Some(a) = self.objects.get(&a_id) else {
                return false;
            };
            let Some(b) = self.objects.get(&b_id) else {
                return false;
            };
            (
                a.is_ignoring_collisions_with(b_id),
                b.is_ignoring_collisions_with(a_id),
                a.is_parachuting(),
                b.is_parachuting(),
                a.team,
                b.team,
                b.is_kind_of(crate::game_logic::KindOf::Structure)
                    || b.is_kind_of(
                        crate::game_logic::KindOf::Structure, /* immobile residual */
                    )
                    || !b.can_move(),
                a.is_kind_of(crate::game_logic::KindOf::Infantry),
                b.status.disabled_unmanned,
            )
        };
        if a_ignore_b || b_ignore_a {
            return true; // ignore = handled (no bounce)
        }
        // C++ both parachuting: never collide.
        if a_para && b_para {
            return true;
        }
        // C++ PhysicsUpdate infantry→unmanned vehicle pilot residual.
        if a_infantry && b_unmanned {
            if self.try_infantry_unmanned_reclaim(a_id, b_id) {
                if let Some(a) = self.objects.get_mut(&a_id) {
                    a.last_collidee = Some(b_id);
                }
                return true;
            }
        } else {
            let b_inf = self
                .objects
                .get(&b_id)
                .map(|o| o.is_kind_of(crate::game_logic::KindOf::Infantry))
                .unwrap_or(false);
            let a_unm = self
                .objects
                .get(&a_id)
                .map(|o| o.status.disabled_unmanned)
                .unwrap_or(false);
            if b_inf && a_unm {
                if self.try_infantry_unmanned_reclaim(b_id, a_id) {
                    if let Some(a) = self.objects.get_mut(&a_id) {
                        a.last_collidee = Some(b_id);
                    }
                    return true;
                }
            }
        }

        let same_team = a_team == b_team;
        // C++ ToppleUpdate::onCollide residual: crusher_level > 1 topples trees/props.
        if self.try_topple_on_collide(a_id, b_id) || self.try_topple_on_collide(b_id, a_id) {
            if let Some(a) = self.objects.get_mut(&a_id) {
                a.last_collidee = Some(b_id);
            }
            return true;
        }
        // Overlap crush (may handle the pair).
        if self.apply_overlap_crush_check(a_id, b_id, same_team) {
            if let Some(a) = self.objects.get_mut(&a_id) {
                a.last_collidee = Some(b_id);
            }
            return true;
        }
        // Immobile bounce path.
        if b_immobile {
            // Still honor allowCollideForce residual.
            let allow = self
                .objects
                .get(&a_id)
                .map(|a| a.allow_collide_force)
                .unwrap_or(true);
            if !allow {
                return true; // handled as no-force
            }
            let handled = self.apply_immobile_collide_bounce(a_id, b_id, us_radius);
            if handled {
                if let Some(a) = self.objects.get_mut(&a_id) {
                    a.last_collidee = Some(b_id);
                }
            }
            return handled;
        }
        // Mobile-mobile: AI processCollision residual (usually no bounce force).
        let frame = self.frame;
        let b_snap = match self.objects.get(&b_id) {
            Some(b) => b.clone(),
            None => return false,
        };
        let allow_force = match self.objects.get_mut(&a_id) {
            Some(a) => a.ai_process_collision(&b_snap, frame),
            None => return false,
        };
        let (req_away, a_pos) = {
            let Some(a) = self.objects.get_mut(&a_id) else {
                return false;
            };
            a.last_collidee = Some(b_id);
            if a.is_blocked {
                a.apply_blocked_speed_cap();
            }
            let req = a.request_other_move_away.take();
            (req, a.get_position())
        };
        if let Some(other_id) = req_away {
            if let Some(other) = self.objects.get_mut(&other_id) {
                other.ai_move_away_from_unit(a_id, a_pos);
                // Absolute ignore-until frame (2 sec residual if already yielding later).
                if other.ignore_collisions_until_frame > 0
                    && other.ignore_collisions_until_frame < 100_000
                {
                    // relative sentinel → absolute
                    other.ignore_collisions_until_frame = frame.saturating_add(60);
                }
            }
        }
        if !allow_force {
            return true; // AI handled / no force
        }
        // Panic bounce residual: small separation impulse on XZ.
        if let Some(a) = self.objects.get_mut(&a_id) {
            let us = a.get_position();
            let them = b_snap.get_position();
            let mut dx = us.x - them.x;
            let mut dz = us.z - them.z;
            let len = (dx * dx + dz * dz).sqrt().max(1.0);
            dx /= len;
            dz /= len;
            a.movement.velocity.x += dx * 0.5;
            a.movement.velocity.z += dz * 0.5;
        }
        true
    }

    /// C++ ToppleUpdate::onCollide residual — `crusher` may topple `prop`.
    pub(super) fn try_topple_on_collide(&mut self, crusher_id: ObjectId, prop_id: ObjectId) -> bool {
        let (level, cpos, speed) = {
            let Some(c) = self.objects.get(&crusher_id) else {
                return false;
            };
            if c.status.destroyed || !c.is_alive() {
                return false;
            }
            let pos = c.get_position();
            let sp = (c.movement.velocity.x * c.movement.velocity.x
                + c.movement.velocity.z * c.movement.velocity.z)
                .sqrt();
            (c.crusher_level, pos, sp)
        };
        if !crate::game_logic::host_topple::crusher_can_topple(level) {
            return false;
        }
        let (kill_now, handled) = {
            let Some(prop) = self.objects.get_mut(&prop_id) else {
                return false;
            };
            let kill_now = prop.try_topple_from_crusher(level, cpos.x, cpos.z, speed.max(1.0));
            let handled = prop
                .topple_data
                .as_ref()
                .map(|t| {
                    !matches!(
                        t.state,
                        crate::game_logic::host_topple::HostToppleState::Upright
                    )
                })
                .unwrap_or(false)
                || kill_now;
            (kill_now, handled)
        };
        if kill_now {
            self.mark_object_for_destruction(prop_id, None);
        }
        handled
    }

    pub fn apply_overlap_crush_check(
        &mut self,
        crusher_id: ObjectId,
        crushee_id: ObjectId,
        same_team: bool,
    ) -> bool {
        // Split borrow: take crushee out, mutate both, put back.
        let Some(mut crushee) = self.objects.remove(&crushee_id) else {
            return false;
        };
        let result = if let Some(crusher) = self.objects.get_mut(&crusher_id) {
            crusher.check_for_overlap_collision(&mut crushee, same_team)
        } else {
            false
        };
        self.objects.insert(crushee_id, crushee);
        result
    }

    pub fn apply_immobile_collide_bounce(
        &mut self,
        mover_id: ObjectId,
        immobile_id: ObjectId,
        us_radius: f32,
    ) -> bool {
        use crate::game_logic::host_partition_collision_physics_residual::PHYSICS_STRUCTURE_STIFFNESS_DEFAULT_RESIDUAL;
        let (mover_para, imm_center, imm_ok) = {
            let Some(m) = self.objects.get(&mover_id) else {
                return false;
            };
            let Some(imm) = self.objects.get(&immobile_id) else {
                return false;
            };
            let imm_ok = imm.is_kind_of(crate::game_logic::KindOf::Structure)
                || imm.is_kind_of(
                    crate::game_logic::KindOf::Structure, /* immobile residual */
                )
                || !imm.can_move();
            (m.is_parachuting(), imm.get_position(), imm_ok)
        };
        if !imm_ok {
            return false;
        }
        let mut applied = false;
        if let Some(m) = self.objects.get_mut(&mover_id) {
            if mover_para {
                m.apply_parachute_building_bounce_out(imm_center, us_radius);
                return true;
            }
            if m.status.destroyed {
                return false;
            }
            let _ = m.apply_structure_stiffness_bounce(
                imm_center,
                PHYSICS_STRUCTURE_STIFFNESS_DEFAULT_RESIDUAL,
                crate::game_logic::Object::SHOCK_MASS,
            );
            applied = true;
        }
        if applied {
            // After stiffness bounce, try vehicle crash residual if applicable.
            let _ = self.apply_vehicle_crash_into_immobile(mover_id, immobile_id);
        }
        applied
    }

    pub fn apply_vehicle_crash_into_immobile(
        &mut self,
        vehicle_id: ObjectId,
        other_id: ObjectId,
    ) -> Option<&'static str> {
        use crate::game_logic::host_partition_collision_physics_residual::{
            vehicle_crash_destroys_vehicle, vehicle_crash_weapon_name,
        };
        let outcome = {
            let Some(v) = self.objects.get(&vehicle_id) else {
                return None;
            };
            let Some(o) = self.objects.get(&other_id) else {
                return None;
            };
            v.evaluate_vehicle_crash_into(o)
        };
        let weapon = vehicle_crash_weapon_name(outcome)?;
        let pos = self
            .objects
            .get(&vehicle_id)
            .map(|o| o.get_position())
            .unwrap_or(glam::Vec3::ZERO);
        // Residual temp weapon: deal explosion damage to vehicle (and mark crash).
        // Fail-closed vs full WeaponStore::createAndFireTempWeapon OCL/FX matrix.
        const CRASH_DAMAGE: f32 = 100.0;
        if let Some(v) = self.objects.get_mut(&vehicle_id) {
            let _ = v.take_damage_from_typed(
                CRASH_DAMAGE,
                Some(vehicle_id),
                crate::game_logic::combat::DamageType::Explosive,
            );
        }
        if vehicle_crash_destroys_vehicle(outcome) {
            if let Some(v) = self.objects.get_mut(&vehicle_id) {
                // Damage authority: HP last-writer via damage log; destroy flag stays host.
                let hp = v.health.current.max(1.0);
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    crate::game_logic::host_damage_log::record(
                        vehicle_id,
                        hp,
                        Some(vehicle_id),
                        true,
                    );
                } else {
                    v.health.current = 0.0;
                }
                v.status.destroyed = true;
                v.status.death_type = crate::game_logic::host_usa_pilot::HostDeathType::Exploded;
            }
            // Also queue crash audio residual.
            self.queue_audio_event(
                AudioEventRequest::new(weapon)
                    .with_object(vehicle_id)
                    .with_position(pos)
                    .with_priority(200),
            );
        } else {
            self.queue_audio_event(
                AudioEventRequest::new(weapon)
                    .with_object(vehicle_id)
                    .with_position(pos)
                    .with_priority(160),
            );
        }
        Some(weapon)
    }

    /// C++ partition collide residual: pairwise near-object physics collide.
    ///
    /// Partition cell broadphase (cell size 40) + selection_radius XZ spheres.
    /// Advances overlap frame after pairs. Fail-closed vs full ghost/shroud cells.
    /// Returns number of pairs that invoked try_physics_collide successfully.

    /// C++ AIUpdateInterface::privateFaceObject residual.
    pub fn private_face_object(&mut self, unit_id: ObjectId, target_id: ObjectId) -> bool {
        let Some(target_pos) = self.objects.get(&target_id).map(|o| o.get_position()) else {
            return false;
        };
        let Some(u) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        if !u.can_move() {
            return false;
        }
        u.is_blocked = false;
        u.is_blocked_and_stuck = false;
        // Host-immediate engagement residual; log for GameWorld last-write.
        u.target = Some(target_id);
        // Face without full path — spin in place residual.
        let _ = u.face_position(target_pos, 1.0 / 30.0);
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_attack(unit_id, target_id);
        }
        true
    }

    /// C++ AIUpdateInterface::privateFacePosition residual.
    pub fn private_face_position(&mut self, unit_id: ObjectId, pos: glam::Vec3) -> bool {
        let Some(u) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        if !u.can_move() {
            return false;
        }
        u.is_blocked = false;
        u.is_blocked_and_stuck = false;
        let _ = u.face_position(pos, 1.0 / 30.0);
        true
    }

    /// C++ AIUpdateInterface::privateIdle residual.
    pub fn private_idle(&mut self, unit_id: ObjectId) -> bool {
        let Some(u) = self.objects.get(&unit_id) else {
            return false;
        };
        if u.is_kind_of(crate::game_logic::KindOf::Projectile) {
            return false;
        }
        let decision_auth = crate::gameworld_shadow::gameworld_ai_decision_authority_live();
        if decision_auth {
            // Stop movement residual stays host; engagement/state via decision log.
            if let Some(u) = self.objects.get_mut(&unit_id) {
                u.stop_moving();
            }
            crate::game_logic::host_ai_decision_log::record_stop_attack(unit_id);
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 0);
        // Idle
        } else if let Some(u) = self.objects.get_mut(&unit_id) {
            u.stop_moving();
            u.set_status_attacking(false);
            u.target = None;
            u.set_ai_state(AIState::Idle);
        }
        true
    }

    #[cfg(test)]
    pub fn private_idle_for_test(&mut self, unit_id: ObjectId) -> bool {
        self.private_idle(unit_id)
    }

    /// C++ AIAttackAimAtTargetState::onEnter residual.

    /// C++ AIAttackState::onEnter residual — start nested AttackStateMachine at AIM.

    /// C++ Object::chooseBestWeaponForTarget / AIAttackState::chooseWeapon residual.
    ///
    /// Locks `active_weapon_slot` to the PreferMostDamage residual choice.
    /// Returns false when no legal weapon exists for the victim (or ground).
    pub fn choose_best_weapon_for_target(
        &mut self,
        unit_id: ObjectId,
        victim_id: Option<ObjectId>,
        current_time: f32,
    ) -> bool {
        let Some(u) = self.objects.get(&unit_id) else {
            return false;
        };
        if !u.is_alive() {
            return false;
        }
        // Ground attack residual: any ready weapon is enough.
        let Some(vid) = victim_id else {
            let has = u.weapon.is_some() || u.secondary_weapon.is_some();
            return has;
        };
        let Some(v) = self.objects.get(&vid) else {
            return false;
        };
        // Snapshot selection without holding mut borrow across get_mut.
        let slot = u.select_combat_weapon_slot(v, current_time);
        let Some(slot) = slot else {
            // Not ready this frame — still pick a legal slot that can target
            // (ammo/reload residual may clear next frame).
            let primary_legal = u.weapon.as_ref().is_some_and(|w| u.can_target_with(v, w));
            let secondary_legal = u
                .secondary_weapon
                .as_ref()
                .is_some_and(|w| u.can_target_with(v, w));
            if !primary_legal && !secondary_legal {
                return false;
            }
            let fallback = if primary_legal { 0u8 } else { 1u8 };
            if let Some(uu) = self.objects.get_mut(&unit_id) {
                uu.set_active_weapon_slot(fallback);
            }
            return true;
        };
        if let Some(uu) = self.objects.get_mut(&unit_id) {
            uu.set_active_weapon_slot(slot);
        }
        true
    }

    /// C++ cannotPossiblyAttackObject state condition residual.
    ///
    /// True when the attacker cannot possibly continue attacking the victim
    /// (dead, no weapon, same team residual, stealthed undetected).

    /// C++ WeaponSet::getAbleToAttackSpecificObject residual (host-simplified).

    /// C++ AIUpdateInterface::transferAttack residual.
    ///
    /// Retargets units attacking `from_id` onto `to_id` (rebuild hole / create-object die).
    pub fn transfer_attack(&mut self, from_id: ObjectId, to_id: ObjectId) -> usize {
        let new_alive = self
            .objects
            .get(&to_id)
            .map(|o| o.is_alive() && !o.status.destroyed)
            .unwrap_or(false);
        if !new_alive {
            return 0;
        }
        let decision_auth = crate::gameworld_shadow::gameworld_ai_decision_authority_live();
        let mut transferred = 0usize;
        let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        for id in ids {
            if id == from_id || id == to_id {
                continue;
            }
            // Snapshot engagement before mut borrow.
            let (had_target, had_turret) = {
                let Some(u) = self.objects.get(&id) else {
                    continue;
                };
                (
                    u.target == Some(from_id),
                    u.turret_target_id == Some(from_id),
                )
            };
            if !had_target && !had_turret {
                continue;
            }
            if had_target {
                // C++ transferAttack always retargets on the host immediately
                // (rebuild-hole / death handoff). Still log under decision authority
                // so GameWorld can last-write, but never leave host pointing at a corpse.
                if let Some(u) = self.objects.get_mut(&id) {
                    u.target = Some(to_id);
                }
                if decision_auth {
                    crate::game_logic::host_ai_decision_log::record_attack(id, to_id);
                }
            }
            if had_turret {
                // Turret aim residual stays host (not AI decision channel).
                if let Some(u) = self.objects.get_mut(&id) {
                    u.turret_target_id = Some(to_id);
                    u.turret_force_attacking = true;
                    if matches!(
                        u.turret_substate,
                        crate::game_logic::object::TurretSubState::Idle
                            | crate::game_logic::object::TurretSubState::Hold
                            | crate::game_logic::object::TurretSubState::Recenter
                    ) {
                        u.turret_substate = crate::game_logic::object::TurretSubState::Aim;
                    }
                }
            }
            transferred += 1;
        }
        transferred
    }

    #[cfg(test)]
    pub fn transfer_attack_for_test(&mut self, from_id: ObjectId, to_id: ObjectId) -> usize {
        self.transfer_attack(from_id, to_id)
    }

    /// Drive mood auto-acquire for idle units (AI + player AutoAcquireEnemiesWhenIdle).
    pub(crate) fn tick_mood_auto_acquire(&mut self, object_ids: &[ObjectId]) {
        for &id in object_ids {
            let (is_player_local, do_check) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let is_local = self
                    .player_id_for_team(o.team)
                    .and_then(|pid| self.players.get(&pid))
                    .map(|p| p.is_local)
                    .unwrap_or(false);
                // C++ attack-move auto-acquire: Idle OR AttackMoving/is_attack_path.
                let idle = matches!(o.ai_state, AIState::Idle) && o.target.is_none();
                let attack_moving = matches!(o.ai_state, AIState::AttackMoving) || o.is_attack_path;
                let want = (idle || attack_moving)
                    && o.auto_acquire_when_idle
                    && o.is_alive()
                    && o.can_attack()
                    && o.target.is_none();
                (is_local, want)
            };
            if do_check {
                // C++ AutoAcquireEnemiesWhenIdle applies to player and AI units.
                let _ = self.try_mood_auto_acquire(id, is_player_local);
            }
        }
    }

    #[cfg(test)]
    pub fn tick_mood_auto_acquire_for_test(&mut self, object_ids: &[ObjectId]) {
        self.tick_mood_auto_acquire(object_ids);
    }

    /// C++ AIUpdateInterface::getMoodMatrixActionAdjustment residual.
    ///
    /// Uses `ai_attitude` on the object (-2 Sleep .. +2 Aggressive).
    /// Player-controlled residual always returns ACTION_OK.

    /// C++ AI vision mood factor residual (AI_VISIONFACTOR_MOOD).
    pub fn adjusted_vision_range_for_mood(&self, unit_id: ObjectId) -> f32 {
        let Some(obj) = self.objects.get(&unit_id) else {
            return 0.0;
        };
        let base = obj.vision_range.max(0.0);
        // Attitude multipliers residual (fail-closed approximate).
        let mult = match obj.ai_attitude.clamp(-2, 2) {
            -2 => 0.0, // Sleep: ignore all
            -1 => 1.0, // Passive: wait-for-attack (range still used for last-attacker)
            0 => 1.0,  // Normal
            1 => 1.25, // Alert
            _ => 1.5,  // Aggressive
        };
        base * mult
    }

    /// C++ AIUpdateInterface::getNextMoodTarget residual.
    ///
    /// Returns a candidate enemy to auto-acquire, or None.

    /// C++ AI::findClosestEnemy residual (host simplified partition filters).
    ///
    /// Qualifiers: see `find_enemy_flags`. When the hunter has an AttackPriorityInfo
    /// set, uses priority-distance scoring (C++ modPriority = pri - dist/modifier).

    /// Register/replace a named AttackPriorityInfo set.

    /// Bridge C++ ScriptEngine AttackPriorityInfo / object sets into host GameLogic.
    ///
    /// Copies named priority sets and applies per-object set names so
    /// `find_closest_enemy` priority scoring matches script actions.

    /// Parse C++ AttitudeType name/ordinal residual to host i8.
    pub fn parse_attitude_token(token: &str) -> i8 {
        use crate::game_logic::host_strategy_center::HostAiAttitude;
        match token.trim().to_ascii_uppercase().as_str() {
            "SLEEP" | "-2" => HostAiAttitude::Sleep.as_i8(),
            "PASSIVE" | "-1" => HostAiAttitude::Passive.as_i8(),
            "NORMAL" | "0" => HostAiAttitude::Normal.as_i8(),
            "ALERT" | "DEFENSIVE" | "1" => HostAiAttitude::Alert.as_i8(),
            "AGGRESSIVE" | "2" => HostAiAttitude::Aggressive.as_i8(),
            _ => HostAiAttitude::Normal.as_i8(),
        }
    }

    /// C++ AIUpdateInterface::setAttitude residual (host mood matrix field).
    pub fn set_unit_attitude(&mut self, unit_id: ObjectId, attitude_i8: i8) -> bool {
        let Some(u) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        u.set_ai_attitude_i8(attitude_i8.clamp(-2, 2));
        true
    }

    /// C++ ScriptActions::updateNamedSetAttitude residual.
    pub fn set_named_unit_attitude(&mut self, unit_name: &str, attitude_token: &str) -> bool {
        let id = self.find_object_id_by_name(unit_name);
        let Some(id) = id else {
            return false;
        };
        self.set_unit_attitude(id, Self::parse_attitude_token(attitude_token))
    }

    /// Apply attack priority set name to all objects on a host team.
    pub fn apply_attack_priority_set_to_team(
        &mut self,
        team: crate::game_logic::Team,
        set_name: Option<&str>,
    ) -> usize {
        let mut n = 0usize;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.team == team)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.set_unit_attack_priority_set(id, set_name);
            n += 1;
        }
        n
    }

    /// Inherit team prototype attack priority + initial attitude when missing.
    ///
    /// C++ Object creation: ai->setAttitude(proto initial); setAttackInfo(proto priority name).
    pub fn inherit_team_ai_defaults(&mut self, unit_id: ObjectId) {
        let team = match self.objects.get(&unit_id).map(|o| o.team) {
            Some(t) => t,
            None => return,
        };
        let team_name = match team {
            crate::game_logic::Team::USA => "America",
            crate::game_logic::Team::China => "China",
            crate::game_logic::Team::GLA => "GLA",
            crate::game_logic::Team::Neutral => return,
        };
        let (prio_name, initial_att_i8) = {
            let Ok(factory) = gamelogic::team::get_team_factory().lock() else {
                return;
            };
            let Some(proto) = factory.find_team_prototype(team_name) else {
                return;
            };
            let name = proto.get_attack_priority_name().as_str().to_string();
            // C++ Object.cpp: ai->setAttitude(proto->getTemplateInfo()->m_initialTeamAttitude)
            let att = match proto.get_initial_team_attitude() {
                gamelogic::team::AttitudeType::Sleep => -2i8,
                gamelogic::team::AttitudeType::Passive => -1,
                gamelogic::team::AttitudeType::Normal => 0,
                gamelogic::team::AttitudeType::Alert => 1,
                gamelogic::team::AttitudeType::Aggressive => 2,
                gamelogic::team::AttitudeType::Invalid => 0,
            };
            (name, att)
        };
        if let Some(u) = self.objects.get_mut(&unit_id) {
            if u.attack_priority_set.is_none() && !prio_name.is_empty() {
                u.attack_priority_set = Some(prio_name);
            }
            // Only apply prototype attitude when still at default Normal (0).
            // Script/setAttitude overrides must win over re-inherit.
            if u.ai_attitude == 0 && initial_att_i8 != 0 {
                u.set_ai_attitude_i8(initial_att_i8.clamp(-2, 2));
            }
        }
    }

    /// Resolve host Team from C++/script team name residual.
    pub fn resolve_host_team_name(team_name: &str) -> Option<crate::game_logic::Team> {
        let n = team_name.trim().to_ascii_lowercase();
        let n = n.strip_prefix("team").unwrap_or(&n);
        let n = n.strip_prefix("player").unwrap_or(n);
        match n {
            "usa" | "america" | "us" | "player_1" | "plyrus" | "teamamerica" => {
                Some(crate::game_logic::Team::USA)
            }
            "china" | "prc" | "player_2" | "plyrchina" | "teamchina" => {
                Some(crate::game_logic::Team::China)
            }
            "gla" | "player_3" | "plyrgla" | "teamgla" => Some(crate::game_logic::Team::GLA),
            "neutral" | "civilian" | "pne" => Some(crate::game_logic::Team::Neutral),
            _ => None,
        }
    }

    /// C++ ScriptActions::updateTeamSetAttitude / AIGroup::setAttitude residual.
    /// Applies attitude to every living member of the host team.
    pub fn set_team_attitude(&mut self, team: crate::game_logic::Team, attitude_i8: i8) -> usize {
        let att = attitude_i8.clamp(-2, 2);
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.team == team && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut n = 0usize;
        for id in ids {
            if self.set_unit_attitude(id, att) {
                n += 1;
            }
        }
        n
    }

    /// Named team attitude token residual (TEAM_SET_ATTITUDE script action).
    pub fn set_team_attitude_by_name(&mut self, team_name: &str, attitude_token: &str) -> usize {
        let Some(team) = Self::resolve_host_team_name(team_name) else {
            return 0;
        };
        self.set_team_attitude(team, Self::parse_attitude_token(attitude_token))
    }

    /// Look up host ObjectId by unit name (named object tracker residual).
    pub fn find_object_id_by_name(&self, name: &str) -> Option<ObjectId> {
        // Prefer host object name residual (production path — no engine_object_id).
        let lower = name.to_ascii_lowercase();
        if let Some((id, _)) = self.objects.iter().find(|(_, o)| {
            (!o.name.is_empty() && o.name.eq_ignore_ascii_case(name))
                || o.thing.template.name.eq_ignore_ascii_case(name)
                || o.template_name.eq_ignore_ascii_case(name)
                || o.thing.template.name.to_ascii_lowercase().contains(&lower)
        }) {
            return Some(*id);
        }
        // Engine named tracker residual only when dual-world object bridge is enabled.
        None
    }

    /// C++ TAiData::m_enableRepulsors residual.
    pub fn set_enable_repulsors(&mut self, enabled: bool) {
        self.enable_repulsors = enabled;
        crate::game_logic::host_repulsor_gate::set_enabled(enabled);
    }

    /// C++ Object::setStatus(OBJECT_STATUS_REPULSOR) residual.

    /// C++ Player::setLogicalRetaliationModeEnabled residual.
    pub fn set_logical_retaliation_mode(&mut self, player_id: u32, enabled: bool) {
        if let Some(p) = self.players.get_mut(&player_id) {
            p.logical_retaliation_mode_enabled = enabled;
        }
    }

    /// Enable logical retaliation for all local (human) players.
    pub fn set_all_local_logical_retaliation(&mut self, enabled: bool) {
        for p in self.players.values_mut() {
            if p.is_local {
                p.logical_retaliation_mode_enabled = enabled;
            }
        }
    }

    /// C++ ActiveBody::shouldRetaliateAgainstAggressor residual.
    pub fn should_retaliate_against_aggressor(
        &self,
        victim_id: ObjectId,
        damager_id: ObjectId,
    ) -> bool {
        let Some(victim) = self.objects.get(&victim_id) else {
            return false;
        };
        let Some(damager) = self.objects.get(&damager_id) else {
            return false;
        };
        if !damager.is_alive() && !damager.status.destroyed {
            // still allow if alive
        }
        if !damager.is_alive() {
            return false;
        }
        // Airborne targets never trigger friend retaliation.
        if damager.status.airborne_target || damager.is_kind_of(KindOf::Aircraft) {
            return false;
        }
        // Enemies only.
        if damager.team == victim.team
            || damager.team == Team::Neutral
            || victim.team == Team::Neutral
        {
            return false;
        }
        let vp = victim.get_position();
        let dp = damager.get_position();
        let dx = vp.x - dp.x;
        let dz = vp.z - dp.z;
        let d2 = dx * dx + dz * dz;
        let max_d = self.max_retaliate_distance;
        if d2 > max_d * max_d {
            return false;
        }
        // Controlling player must be human (local residual).
        let human = self
            .players
            .values()
            .find(|p| p.team == victim.team)
            .map(|p| p.is_local && p.logical_retaliation_mode_enabled)
            .unwrap_or(false);
        if !human {
            return false;
        }
        if victim.is_kind_of(KindOf::Drone) {
            return false;
        }
        true
    }

    /// C++ ActiveBody::shouldRetaliate residual (friend unit eligibility).
    pub fn should_retaliate_friend(&self, unit_id: ObjectId) -> bool {
        let Some(obj) = self.objects.get(&unit_id) else {
            return false;
        };
        if !obj.is_alive() || obj.status.destroyed {
            return false;
        }
        if obj.is_kind_of(KindOf::CannotRetaliate)
            || obj.is_kind_of(KindOf::Immobile)
            || obj.is_kind_of(KindOf::Structure)
            || obj.is_kind_of(KindOf::Drone)
        {
            return false;
        }
        // Idle only.
        if !matches!(obj.ai_state, AIState::Idle) || obj.target.is_some() {
            return false;
        }
        // Stealthed + not detected → no.
        if obj.status.stealthed && !obj.status.detected {
            return false;
        }
        obj.can_attack()
    }

    /// C++ ActiveBody damage path friend retaliation residual.
    ///
    /// Nearby allied idle mobile units that can attack the damager enter
    /// GuardRetaliate (host: attack damager). Invoked even if victim died.
    pub fn try_friends_retaliate(&mut self, victim_id: ObjectId, damager_id: ObjectId) -> usize {
        if !self.should_retaliate_against_aggressor(victim_id, damager_id) {
            return 0;
        }
        let (vpos, vteam, vradius) = {
            let Some(v) = self.objects.get(&victim_id) else {
                return 0;
            };
            // Bounding circle residual: use collision radius or default 10.
            let r = if v.is_kind_of(KindOf::Structure) {
                40.0
            } else {
                10.0
            };
            (v.get_position(), v.team, r)
        };
        let range = self.retaliate_friends_radius + vradius;
        let range_sq = range * range;
        let candidates: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(id, o)| {
                **id != victim_id && **id != damager_id && o.team == vteam && o.is_alive() && {
                    let p = o.get_position();
                    let dx = p.x - vpos.x;
                    let dz = p.z - vpos.z;
                    dx * dx + dz * dz <= range_sq
                }
            })
            .map(|(id, _)| *id)
            .collect();

        let mut n = 0usize;
        for fid in candidates {
            if !self.should_retaliate_friend(fid) {
                continue;
            }
            // AbleToAttack residual.
            match self.get_able_to_attack_specific_object(
                fid,
                damager_id,
                AbleToAttackType::NewTarget,
                false,
            ) {
                CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving => {}
                _ => continue,
            }
            // C++ aiGuardRetaliate residual.
            if let Some(friend) = self.objects.get_mut(&fid) {
                let anchor = friend.get_position();
                friend.begin_guard_retaliate(damager_id, Some(anchor), None);
                n += 1;
            }
        }
        n
    }
    /// Tick all GuardRetaliating units (victim death / return residual).
    pub fn tick_guard_retaliate_states(&mut self) {
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| matches!(o.ai_state, AIState::GuardRetaliating))
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let victim_id = self.objects.get(&id).and_then(|o| o.guard_retaliate_victim);
            let (alive, vpos) = match victim_id {
                Some(vid) => match self.objects.get(&vid) {
                    Some(v) if v.is_alive() && !v.status.destroyed => {
                        (true, Some(v.get_position()))
                    }
                    _ => (false, None),
                },
                None => (false, None),
            };
            if let Some(o) = self.objects.get_mut(&id) {
                o.tick_guard_retaliate(alive, vpos);
            }
        }
    }

    pub fn set_unit_repulsor(&mut self, unit_id: ObjectId, repulsor: bool) -> bool {
        let Some(u) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        u.status.repulsor = repulsor;
        true
    }

    /// C++ ScriptActions::doNamedSetRepulsor residual.
    pub fn set_named_unit_repulsor(&mut self, unit_name: &str, repulsor: bool) -> bool {
        let Some(id) = self.find_object_id_by_name(unit_name) else {
            return false;
        };
        self.set_unit_repulsor(id, repulsor)
    }

    /// C++ ScriptActions::doTeamSetRepulsor residual.
    pub fn set_team_repulsor(&mut self, team: crate::game_logic::Team, repulsor: bool) -> usize {
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.team == team && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut n = 0usize;
        for id in ids {
            if self.set_unit_repulsor(id, repulsor) {
                n += 1;
            }
        }
        n
    }

    pub fn set_team_repulsor_by_name(&mut self, team_name: &str, repulsor: bool) -> usize {
        let Some(team) = Self::resolve_host_team_name(team_name) else {
            return 0;
        };
        self.set_team_repulsor(team, repulsor)
    }

    /// C++ PartitionFilterRepulsor + AI::findClosestRepulsor residual.
    ///
    /// Returns closest living repulsor in range:
    /// - OBJECT_STATUS_REPULSOR flag, OR
    /// - enemy able-to-attack structure, OR  
    /// - enemy able-to-attack non-structure (C++ filter residual simplified)
    /// Fail-closed vs full PartitionManager filters / stealth reject.
    pub(super) fn find_closest_repulsor(&self, unit_id: ObjectId, range: f32) -> Option<(ObjectId, f32)> {
        if !self.enable_repulsors {
            return None;
        }
        let me = self.objects.get(&unit_id)?;
        if !me.is_alive() {
            return None;
        }
        let my_pos = me.get_position();
        let my_team = me.team;
        // Pure residual acquire: nearest repulsor in range (XZ).
        // Dead units only count when explicitly flagged repulsor (ActiveBody residual).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(&oid, other)| {
                if oid == unit_id {
                    return None;
                }
                let alive = other.is_alive();
                if !alive && !other.status.repulsor {
                    return None;
                }
                // Stealth residual: stealthed + not detected + not disguised → reject
                if other.status.stealthed && !other.status.detected && !other.status.disguised {
                    return None;
                }
                let is_flag_repulsor = other.status.repulsor;
                let is_enemy = other.team != my_team
                    && other.team != Team::Neutral
                    && my_team != Team::Neutral;
                let enemy_attacker = is_enemy && other.can_attack();
                // C++ PartitionFilterRepulsor: flag OR (enemy attackers; structures only if can attack)
                let allow = is_flag_repulsor || enemy_attacker;
                if !allow {
                    return None;
                }
                Some(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id: oid,
                        team: other.team,
                        position: other.get_position(),
                        // Keep dead flagged-repulsors eligible for distance pick.
                        is_alive: true,
                        is_neutral: other.team == Team::Neutral,
                        under_construction: other.status.under_construction,
                        combat_kind: true,
                        effectively_stealthed: false,
                        is_air: other.is_kind_of(KindOf::Aircraft) || other.status.airborne_target,
                        eject_invulnerable: false,
                    },
                )
            })
            .collect();
        crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
            Some(unit_id),
            (my_pos.x, my_pos.z),
            candidates,
            range.max(0.0),
            |_| true,
        )
        .map(|(id, dist, _)| (id, dist))
    }

    /// C++ AIIdleState repulsor branch + AIMoveAwayFromRepulsors residual.
    ///
    /// For KINDOF_CAN_BE_REPULSED idle units: flee closest repulsor via
    /// ai_move_away_from_unit / request_safe_path residual.

    /// C++ AIUpdateInterface::notifyCrate host bridge.
    pub fn notify_unit_crate(&mut self, unit_id: ObjectId, crate_id: ObjectId) -> bool {
        let Some(u) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        u.notify_crate(crate_id);
        true
    }

    /// C++ AIIdleState crate branch residual.
    ///
    /// If unit has a pending crate notification and the crate still exists in
    /// the money-crate registry (or as a live object), move to pick it up.
    pub fn try_idle_crate_pickup(&mut self, unit_id: ObjectId) -> bool {
        let crate_id = {
            let Some(u) = self.objects.get_mut(&unit_id) else {
                return false;
            };
            if !u.is_alive() || u.status.destroyed {
                return false;
            }
            // C++ only while idle
            if !matches!(u.ai_state, AIState::Idle) || u.target.is_some() {
                return false;
            }
            // Computer AI residual: humans don't auto-notify; path still works if notified.
            match u.check_for_crate_to_pickup() {
                Some(id) => id,
                None => return false,
            }
        };
        // Crate must still exist and be a registered money crate (or live object).
        let crate_alive = self
            .objects
            .get(&crate_id)
            .map(|c| c.is_alive() && !c.status.destroyed)
            .unwrap_or(false);
        if !crate_alive {
            return false;
        }
        // Prefer registered money crates (wouldLikeToCollide residual).
        let is_money = self.host_money_crates.get(crate_id).is_some();
        if !is_money {
            // Fail-closed: only money crate residual for now.
            return false;
        }
        let crate_pos = self.objects.get(&crate_id).map(|c| c.get_position());
        let Some(pos) = crate_pos else {
            return false;
        };
        if let Some(u) = self.objects.get_mut(&unit_id) {
            if !u.can_move() {
                return false;
            }
            // C++ aiMoveToObject residual.
            u.move_to(pos);
            u.requested_victim_id = Some(crate_id);
        } else {
            return false;
        }
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.set_ai_state(AIState::Moving);
        }
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 1);
            // Moving
        }
        true
    }

    /// When an AI computer unit kills and a money crate is spawned nearby, notify.
    ///
    /// C++ CreateCrateDie: only PLAYER_COMPUTER killers get notifyCrate.

    /// C++ CreateCrateDie::onDie residual for host destruction processing.
    ///
    /// Uses victim template `create_crate_data` list + last_damage_source as killer.
    /// Spawns money crates and notifies computer killers.

    /// C++ SalvageCrateCollide::executeCrateBehavior residual.
    ///
    /// Priority: armor set → weapon set (chance) → level (chance) → money.
    /// Returns (kind label, money granted).

    /// C++ VeterancyCrateCollide::executeCrateBehavior residual (non-pilot AOE).
    ///
    /// Grants `levels` to picker and same-team allies within effect_range
    /// (0 range = picker only). Fail-closed: not full pilot goal-object gate.

    /// C++ HealCrateCollide::executeCrateBehavior residual.
    ///
    /// Heals all living objects owned by the picker's team to full health.

    /// C++ ShroudCrateCollide::executeCrateBehavior residual.
    ///
    /// Permanently reveals the map for the picker's player (PartitionManager
    /// revealMapForPlayer).

    /// C++ ScriptEngine::transferObjectName residual (host bridge).
    ///
    /// Moves a script-visible name from `from_id` to `to_id` via host name field
    /// + NamedObjectTracker when available.

    /// C++ targetCanEject residual: vehicle has EjectPilotDie interface.
    ///
    /// Host residual: eligible eject-pilot template + vehicle kind.
    pub fn vehicle_supports_hijacker_ride(&self, vehicle_id: ObjectId) -> bool {
        use crate::game_logic::host_usa_pilot::is_eject_pilot_eligible_template;
        let Some(v) = self.objects.get(&vehicle_id) else {
            return false;
        };
        if !v.is_alive() || v.status.destroyed {
            return false;
        }
        if v.is_kind_of(KindOf::Aircraft) || v.object_type == ObjectType::Aircraft {
            return false;
        }
        if !(v.is_kind_of(KindOf::Vehicle) || v.object_type == ObjectType::Vehicle) {
            return false;
        }
        is_eject_pilot_eligible_template(&v.template_name)
    }

    /// C++ HijackerUpdate airborne exit residual: ThingFactory newObject
    /// (ParachuteName) + ContainModule::addToContain(hijacker).
    ///
    /// Host residual: spawn AmericaParachute, dock rider inside, apply
    /// AmericaParachute freefall/open residual on both container and rider.
    pub(super) fn put_hijacker_in_airborne_parachute(&mut self, rider_id: ObjectId, eject_pos: glam::Vec3) {
        use crate::game_logic::host_car_bomb::HIJACKER_PARACHUTE_NAME;
        use crate::game_logic::{KindOf, ThingTemplate};

        // Ensure AmericaParachute template exists for residual spawn.
        // C++ KINDOF_PARACHUTE — host has no Parachute kind; Vehicle + max_transport=1
        // residual so ContainModule::addToContain bookkeeping can hold the rider.
        if !self.templates.contains_key(HIJACKER_PARACHUTE_NAME) {
            let mut chute_tpl = ThingTemplate::new(HIJACKER_PARACHUTE_NAME);
            chute_tpl.add_kind_of(KindOf::Vehicle).set_health(1.0);
            self.templates
                .insert(HIJACKER_PARACHUTE_NAME.to_string(), chute_tpl);
        }

        let rider_team = self
            .objects
            .get(&rider_id)
            .map(|o| o.team)
            .unwrap_or(crate::game_logic::Team::Neutral);

        let mut pos = eject_pos;
        // Keep elevated for freefall residual (C++ m_ejectPos may already be high).
        if pos.y < 50.0 {
            pos.y = 50.0;
        }

        let Some(chute_id) = self.create_object(HIJACKER_PARACHUTE_NAME, rider_team, pos) else {
            // Fail-closed: still parachute the rider without a container object.
            if let Some(r) = self.objects.get_mut(&rider_id) {
                r.set_position(pos);
                crate::game_logic::host_ground_height_log::record(rider_id, 0.0, false);
                if crate::gameworld_shadow::gameworld_movement_authority_live() {
                    crate::game_logic::host_move_log::record(rider_id, Some([pos.x, pos.y, pos.z]));
                    r.record_host_movement();
                }
                r.apply_eject_parachuting();
            }
            return;
        };

        // Container residual bookkeeping + parachute physics on chute.
        {
            if let Some(chute) = self.objects.get_mut(&chute_id) {
                // Force 1-slot AmericaParachute residual capacity.
                chute.max_transport = 1;
                chute.record_host_contain_capacity();
                if !chute.enter_transport(rider_id) {
                    // Fail-closed: force occupant list even if kind gate rejects.
                    if !chute.occupants.contains(&rider_id) {
                        chute.occupants.push(rider_id);
                    }
                }
                chute.apply_eject_parachuting();
                // Parachute is not selectable residual (C++ drawable on container).
                chute.set_status_unselectable(true);
                chute.set_status_no_collisions(true);
            }
        }

        // Rider: contained + parachuting residual (hidden inside chute).
        if let Some(r) = self.objects.get_mut(&rider_id) {
            r.set_contained_by(Some(chute_id));
            r.set_ai_state(crate::game_logic::AIState::Docked);
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_set_state(rider_id, 12);
                // Docked
            }
            r.set_position(pos);
            crate::game_logic::host_ground_height_log::record(rider_id, 0.0, false);
            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                crate::game_logic::host_move_log::record(rider_id, Some([pos.x, pos.y, pos.z]));
                r.record_host_movement();
            }
            r.apply_eject_parachuting();
            // Still not selectable while in chute (partition restore already cleared
            // MASKED from vehicle ride; chute contain keeps soft-hide).
            r.set_status_unselectable(true);
            r.set_status_no_collisions(true);
            r.set_status_masked(true);
        }

        self.car_bomb.record_airborne_parachute_put();
        // Tag air path honesty shared with EjectPilotDie air OCL residual.
        self.usa_pilot.record_air_ejection();
    }

    /// C++ CommandButtonHuntUpdate::setCommandButton residual (scripts/AI).
    pub fn start_command_button_hunt(
        &mut self,
        unit_id: ObjectId,
        mode: crate::game_logic::host_command_button_hunt::HostCommandButtonHuntMode,
    ) -> bool {
        let frame = self.frame;
        let Some(unit) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.start_command_button_hunt(mode, frame);
        self.command_button_hunt_reg.record_start();
        true
    }

    /// C++ CommandButtonHuntUpdate::update residual for enter modes.
    pub fn tick_command_button_hunt_updates(&mut self) {
        use crate::game_logic::host_command_button_hunt::{
            hunt_allows_kind, hunt_allows_team, HostCommandButtonHuntMode,
            COMMAND_BUTTON_HUNT_SCAN_RANGE,
        };

        let frame = self.frame;

        let busy: std::collections::HashSet<ObjectId> =
            self.pending_special_abilities.keys().copied().collect();
        let hunters: Vec<(ObjectId, HostCommandButtonHuntMode, Team, glam::Vec3)> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                let h = o.command_button_hunt.as_ref()?;
                if !h.due(frame) {
                    return None;
                }
                // C++: quit if last command not from AI — host residual: only when Idle.
                if !matches!(o.ai_state, AIState::Idle) {
                    return None;
                }
                if busy.contains(id) {
                    return None;
                }
                Some((*id, h.mode, o.team, o.get_position()))
            })
            .collect();

        for (hunter_id, mode, hunter_team, hunter_pos) in hunters {
            self.command_button_hunt_reg.record_scan();
            if let Some(h) = self
                .objects
                .get_mut(&hunter_id)
                .and_then(|o| o.command_button_hunt.as_mut())
            {
                h.schedule_next(frame);
            }

            let mut best: Option<(ObjectId, f32)> = None;
            for (tid, t) in self.objects.iter() {
                if *tid == hunter_id || !t.is_alive() {
                    continue;
                }
                let same_team = t.team == hunter_team;
                let target_neutral = t.team == Team::Neutral;
                if !hunt_allows_team(mode, same_team, target_neutral) {
                    continue;
                }
                let is_veh = t.is_kind_of(KindOf::Vehicle);
                let is_str = t.is_kind_of(KindOf::Structure);
                let is_air = t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target;
                if !hunt_allows_kind(mode, is_veh, is_str, is_air) {
                    continue;
                }
                // Hijack residual: cannot re-hijack.
                if matches!(mode, HostCommandButtonHuntMode::HijackVehicle) && t.is_hijacked() {
                    continue;
                }
                let d = hunter_pos.distance(t.get_position());
                if d > COMMAND_BUTTON_HUNT_SCAN_RANGE {
                    continue;
                }
                if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                    best = Some((*tid, d));
                }
            }

            let Some((target_id, _)) = best else {
                continue;
            };

            let ability = match mode {
                HostCommandButtonHuntMode::HijackVehicle => {
                    PendingSpecialAbility::Hijack { target_id }
                }
                HostCommandButtonHuntMode::ConvertToCarBomb => {
                    PendingSpecialAbility::CarBomb { target_id }
                }
                HostCommandButtonHuntMode::SabotageBuilding => {
                    PendingSpecialAbility::Sabotage { target_id }
                }
            };
            self.queue_pending_special_ability(hunter_id, ability);
            // Walk/path toward target residual.
            if let Some(tp) = self.objects.get(&target_id).map(|t| t.get_position()) {
                if let Some(u) = self.objects.get_mut(&hunter_id) {
                    u.target = Some(target_id);
                    u.set_ai_state(AIState::SpecialAbility);
                }
                let _ = self.assign_unit_path(hunter_id, tp, &[]);
            }
            self.command_button_hunt_reg.record_target();
        }
    }

    /// C++ DeployStyleAIUpdate pack/unpack timer residual.
    pub fn tick_deploy_style_updates(&mut self) {
        let frame = self.frame;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.deploy_style.is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let Some(ds) = obj.deploy_style.as_mut() else {
                continue;
            };
            let (ready_atk, ready_mv) = ds.tick(frame);
            if ready_atk {
                obj.set_deployed(true);
            }
            if ready_mv {
                obj.set_deployed(false);
            }
        }
    }

    /// Ensure DeployStyle unit is unpacking/unpacked before fire residual.
    /// Returns false if fire should be deferred this frame.
    pub fn ensure_deploy_style_ready_to_fire(&mut self, id: ObjectId) -> bool {
        let frame = self.frame;
        let mut started = false;
        let mut blocked = false;
        let ready = {
            let Some(obj) = self.objects.get_mut(&id) else {
                return true;
            };
            let Some(ds) = obj.deploy_style.as_mut() else {
                return true;
            };
            if ds.is_ready_to_attack() {
                true
            } else {
                if ds.begin_deploy(frame) {
                    started = true;
                    obj.stop_moving();
                    obj.set_status_moving(false);
                } else {
                    blocked = true;
                }
                false
            }
        };
        if started {
            self.deploy_style_reg.record_deploy();
        }
        if blocked {
            self.deploy_style_reg.record_blocked_fire();
        }
        ready
    }

    /// C++ AssaultTransportAIUpdate wounded-retrieve + healthy re-exit residual.
    pub fn tick_assault_transport_updates(&mut self) {
        use crate::game_logic::host_troop_crawler::{
            is_assault_member_healthy, is_assault_member_wounded,
        };

        let crawler_ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_troop_crawler_style_container()
                    && o.assault_transport
                        .as_ref()
                        .map(|a| a.active)
                        .unwrap_or(false)
            })
            .map(|(id, _)| *id)
            .collect();

        for crawler_id in crawler_ids {
            let (target_raw, members) = {
                let Some(c) = self.objects.get(&crawler_id) else {
                    continue;
                };
                let Some(a) = c.assault_transport.as_ref() else {
                    continue;
                };
                (a.designated_target, a.member_ids.clone())
            };
            let Some(target_raw) = target_raw else {
                continue;
            };
            let target_id = ObjectId(target_raw);
            // Target dead → clear assault.
            let target_alive = self
                .objects
                .get(&target_id)
                .map(|t| t.is_alive())
                .unwrap_or(false);
            if !target_alive {
                if let Some(c) = self.objects.get_mut(&crawler_id) {
                    if let Some(a) = c.assault_transport.as_mut() {
                        a.clear();
                    }
                }
                continue;
            }

            let mut still_members: Vec<u32> = Vec::new();
            let crawler_pos = self
                .objects
                .get(&crawler_id)
                .map(|c| c.get_position())
                .unwrap_or(Vec3::ZERO);

            for mid_raw in members {
                let mid = ObjectId(mid_raw);
                let Some(member) = self.objects.get(&mid) else {
                    continue;
                };
                if !member.is_alive() {
                    continue;
                }
                still_members.push(mid_raw);
                let contained = member.contained_by == Some(crawler_id);
                let wounded =
                    is_assault_member_wounded(member.health.current, member.health.maximum);
                let healthy =
                    is_assault_member_healthy(member.health.current, member.health.maximum);

                if contained {
                    // Full health → re-exit and resume attack (C++ isMemberHealthy).
                    if healthy {
                        if let Some(c) = self.objects.get_mut(&crawler_id) {
                            c.remove_occupant(mid);
                        }
                        if let Some(unit) = self.objects.get_mut(&mid) {
                            unit.set_contained_by(None);
                            let offset = Vec3::new(6.0, 0.0, 0.0);
                            unit.set_position(crawler_pos + offset);
                        }
                        self.apply_engagement_decision_aware(mid, target_id);
                        self.troop_crawler.record_healthy_redeploy();
                    }
                    continue;
                }

                // Outside + wounded → re-enter for heal.
                if wounded {
                    // Instant residual enter (path AI deferred).
                    if let Some(c) = self.objects.get_mut(&crawler_id) {
                        if !c.occupants.contains(&mid) && c.can_contain() {
                            let _ = c.add_occupant(mid);
                        }
                    }
                    if let Some(unit) = self.objects.get_mut(&mid) {
                        unit.set_contained_by(Some(crawler_id));
                        unit.stop_moving();
                        unit.target = None;
                        unit.set_status_attacking(false);
                        unit.set_position(crawler_pos);
                    }
                    self.troop_crawler.record_wounded_retrieve();
                    continue;
                }

                // Outside + not wounded → keep attacking designated target.
                if let Some(unit) = self.objects.get(&mid) {
                    if unit.target != Some(target_id) {
                        self.apply_engagement_decision_aware(mid, target_id);
                    }
                }
            }

            if let Some(c) = self.objects.get_mut(&crawler_id) {
                if let Some(a) = c.assault_transport.as_mut() {
                    a.member_ids = still_members;
                    if a.member_ids.is_empty() && c.occupants.is_empty() {
                        a.clear();
                    }
                }
            }
        }
    }

    /// C++ UndeadBody + BattleBusSlowDeathBehavior first-life / empty-hulk residual.
    pub fn tick_battle_bus_slow_deaths(&mut self) {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_battle_bus::battle_bus_undeath_passenger_damage;

        let frame = self.frame;

        // Snapshot bus state without overlapping borrows.
        let mut bus_snapshots: Vec<(ObjectId, bool, Vec<ObjectId>, usize, f32)> = Vec::new();
        for (id, o) in self.objects.iter() {
            if !o.is_battle_bus_transport {
                continue;
            }
            let Some(body) = o.battle_bus_body.as_ref() else {
                continue;
            };
            bus_snapshots.push((
                *id,
                body.pending_passenger_damage,
                o.occupants.clone(),
                o.occupants.len(),
                o.get_position().z,
            ));
        }

        let mut passenger_hits: Vec<(ObjectId, f32)> = Vec::new();
        for (bus_id, pending, occupants, _count, _z) in &bus_snapshots {
            if !*pending {
                continue;
            }
            if let Some(bus) = self.objects.get_mut(bus_id) {
                if let Some(body) = bus.battle_bus_body.as_mut() {
                    body.pending_passenger_damage = false;
                }
            }
            for pid in occupants {
                if let Some(p) = self.objects.get(pid) {
                    let dmg = battle_bus_undeath_passenger_damage(p.health.maximum.max(1.0));
                    passenger_hits.push((*pid, dmg));
                }
            }
            self.battle_bus.record_undeath_detonate();
        }

        for (pid, dmg) in passenger_hits {
            if let Some(p) = self.objects.get_mut(&pid) {
                if p.is_alive() {
                    let _ = p.take_damage_from_typed(dmg, None, DamageType::Explosive);
                }
            }
        }

        let mut empty_kills: Vec<ObjectId> = Vec::new();
        for (bus_id, _pending, _occ, passenger_count, z) in &bus_snapshots {
            let above = *z > 0.5;
            let Some(bus) = self.objects.get_mut(bus_id) else {
                continue;
            };
            let (_landed, empty_kill) =
                bus.tick_battle_bus_slow_death(frame, above, *passenger_count);
            if empty_kill {
                empty_kills.push(*bus_id);
            }
        }

        for bus_id in empty_kills {
            self.battle_bus.record_empty_hulk_destruction();
            if let Some(bus) = self.objects.get_mut(&bus_id) {
                if let Some(body) = bus.battle_bus_body.as_mut() {
                    body.mark_real_death();
                }
                let hp = bus.health.current.max(1.0) + 1.0;
                let _ = bus.take_damage_from_typed(hp, None, DamageType::Unresistable);
            }
            if self
                .objects
                .get(&bus_id)
                .map(|o| !o.is_alive() || o.status.destroyed)
                .unwrap_or(false)
            {
                let _ = self.destroy_object(bus_id);
            }
        }
    }

    /// Tick HijackerUpdate residual for all in-vehicle hijackers.
    pub fn tick_hijacker_updates(&mut self) {
        let riders: Vec<(ObjectId, ObjectId)> = self
            .objects
            .iter()
            .filter(|(_, o)| o.hijacker_in_vehicle)
            .filter_map(|(id, o)| o.hijack_vehicle_id.map(|vid| (*id, vid)))
            .collect();
        for (rider_id, vehicle_id) in riders {
            let vehicle_alive = self
                .objects
                .get(&vehicle_id)
                .map(|v| v.is_alive() && !v.status.destroyed)
                .unwrap_or(false);
            if !vehicle_alive {
                let (epos, air) = {
                    let r = self.objects.get(&rider_id);
                    (
                        r.and_then(|o| o.hijacker_eject_pos)
                            .or_else(|| r.map(|o| o.get_position()))
                            .unwrap_or(glam::Vec3::ZERO),
                        r.map(|o| o.hijacker_was_airborne).unwrap_or(false),
                    )
                };
                if let Some(r) = self.objects.get_mut(&rider_id) {
                    r.end_hijacker_in_vehicle(epos, air);
                }
                // C++ HijackerUpdate: if m_wasTargetAirborne → PutInContainer
                // AmericaParachute (m_parachuteName) at m_ejectPos.
                if air {
                    self.put_hijacker_in_airborne_parachute(rider_id, epos);
                }
                continue;
            }
            let (vpos, air, vlevel, vxp) = {
                let v = self.objects.get(&vehicle_id).unwrap();
                (
                    v.get_position(),
                    v.status.airborne_target || v.get_position().y > 5.0,
                    v.experience.level,
                    v.experience.current,
                )
            };
            // Sync vehicle veterancy MAX back onto vehicle too.
            if let Some(v) = self.objects.get_mut(&vehicle_id) {
                // Will re-read rider level after tick
                let _ = v;
            }
            if let Some(r) = self.objects.get_mut(&rider_id) {
                r.tick_hijacker_in_vehicle(vpos, air, vlevel, vxp);
            }
            // Apply MAX level to vehicle from rider after tick.
            let rlevel = self
                .objects
                .get(&rider_id)
                .map(|r| r.experience.level)
                .unwrap_or(vlevel);
            if let Some(v) = self.objects.get_mut(&vehicle_id) {
                use crate::game_logic::VeterancyLevel;
                let rank = |l: VeterancyLevel| -> u8 {
                    match l {
                        VeterancyLevel::Rookie => 0,
                        VeterancyLevel::Veteran => 1,
                        VeterancyLevel::Elite => 2,
                        VeterancyLevel::Heroic => 3,
                    }
                };
                if rank(rlevel) > rank(v.experience.level) {
                    let prev = v.experience.level;
                    v.experience.level = rlevel;
                    v.apply_veterancy_bonuses(prev, rlevel);
                }
            }
        }
    }
    pub fn transfer_script_object_name(&mut self, from_id: ObjectId, to_id: ObjectId) -> bool {
        use gamelogic::scripting::engine::get_named_object_tracker;
        let name = self
            .objects
            .get(&from_id)
            .map(|o| o.name.clone())
            .filter(|n| !n.is_empty());
        let Some(n) = name else {
            return false;
        };
        if let Some(t) = self.objects.get_mut(&to_id) {
            t.name = n.clone();
            t.record_host_identity();
        }
        if let Some(f) = self.objects.get_mut(&from_id) {
            f.name.clear();
        }
        // Register on tracker with host ObjectId (no dual-world engine id).
        let tracker_id = to_id.0;
        let tracker = get_named_object_tracker();
        let _ = tracker.register_named_object(n, tracker_id);
        true
    }
    pub fn execute_shroud_crate_behavior(&mut self, picker_id: ObjectId) -> bool {
        let team = match self.objects.get(&picker_id) {
            Some(p) if p.is_alive() => p.team,
            _ => return false,
        };
        // Map host team → player id residual.
        let player_id = self
            .players
            .iter()
            .find(|(_, p)| p.team == team)
            .map(|(id, _)| *id)
            .unwrap_or(match team {
                Team::USA => 0,
                Team::China => 1,
                Team::GLA => 2,
                Team::Neutral => 255,
            });
        self.partition_manager.reveal_map_for_player(player_id);
        true
    }
    pub fn execute_heal_crate_behavior(&mut self, picker_id: ObjectId) -> usize {
        let team = match self.objects.get(&picker_id) {
            Some(p) if p.is_alive() => p.team,
            _ => return 0,
        };
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.team == team && o.is_alive() && !o.status.destroyed)
            .map(|(id, _)| *id)
            .collect();
        let mut n = 0usize;
        for id in ids {
            if let Some(o) = self.objects.get_mut(&id) {
                let max = o.health.maximum;
                if o.health.current < max {
                    Self::write_object_health_authority_aware(o, max);
                    n += 1;
                }
            }
        }
        n
    }

    /// C++ UnitCrateCollide::executeCrateBehavior residual.
    ///
    /// Spawns `count` units of `unit_type` on picker's team near picker.
    pub fn execute_unit_crate_behavior(
        &mut self,
        picker_id: ObjectId,
        unit_type: &str,
        count: u32,
    ) -> usize {
        let (team, origin, ori) = match self.objects.get(&picker_id) {
            Some(p) if p.is_alive() => (p.team, p.get_position(), p.get_orientation()),
            _ => return 0,
        };
        if unit_type.is_empty() || count == 0 {
            return 0;
        }
        // Ensure template
        if !self.templates.contains_key(unit_type) {
            let mut t = ThingTemplate::new(unit_type);
            // Guess kind from name residual
            if unit_type.contains("Tank") || unit_type.contains("Vehicle") {
                t.add_kind_of(KindOf::Vehicle);
            } else if unit_type.contains("Jet") || unit_type.contains("Aircraft") {
                t.add_kind_of(KindOf::Aircraft);
            } else {
                t.add_kind_of(KindOf::Infantry);
            }
            t.add_kind_of(KindOf::Attackable);
            self.templates.insert(unit_type.to_string(), t);
        }
        let mut spawned = 0usize;
        for i in 0..count {
            let ang = ori + (i as f32) * 0.9;
            let pos = glam::Vec3::new(
                origin.x + ang.cos() * (8.0 + i as f32 * 4.0),
                origin.y,
                origin.z + ang.sin() * (8.0 + i as f32 * 4.0),
            );
            if let Some(nid) = self.create_object(unit_type, team, pos) {
                if let Some(o) = self.objects.get_mut(&nid) {
                    o.set_orientation(ori);
                }
                spawned += 1;
            }
        }
        spawned
    }
    pub fn execute_veterancy_crate_behavior(
        &mut self,
        picker_id: ObjectId,
        effect_range: f32,
        levels: u8,
    ) -> usize {
        let (team, origin) = match self.objects.get(&picker_id) {
            Some(p) if p.is_alive() => (p.team, p.get_position()),
            _ => return 0,
        };
        let levels = levels.max(1);
        let range_sq = effect_range.max(0.0) * effect_range.max(0.0);
        let targets: Vec<ObjectId> = if effect_range <= 0.0 {
            vec![picker_id]
        } else {
            self.objects
                .iter()
                .filter(|(id, o)| {
                    o.team == team
                        && o.is_alive()
                        && !o.status.destroyed
                        && !o.is_kind_of(KindOf::Structure)
                        && {
                            let p = o.get_position();
                            let dx = p.x - origin.x;
                            let dz = p.z - origin.z;
                            dx * dx + dz * dz <= range_sq
                        }
                })
                .map(|(id, _)| *id)
                .collect()
        };
        let mut n = 0usize;
        for tid in targets {
            if let Some(o) = self.objects.get_mut(&tid) {
                // can_level_up = trainable residual: not heroic already handled inside
                let g = o.gain_exp_for_level(levels, true);
                if g > 0 {
                    n += 1;
                }
            }
        }
        n
    }
    pub fn execute_salvage_crate_behavior(
        &mut self,
        picker_id: ObjectId,
        money_provided: u32,
        seed: u32,
    ) -> (&'static str, u32) {
        use crate::game_logic::host_gamedata_lobby_residual::{
            SALVAGE_LEVEL_CHANCE_RESIDUAL, SALVAGE_WEAPON_CHANCE_RESIDUAL,
        };
        use crate::game_logic::host_rng_residual::pure_logic_random_real;
        use crate::game_logic::VeterancyLevel;

        let Some(picker) = self.objects.get_mut(&picker_id) else {
            return ("none", 0);
        };
        // Armor salvager path (no percent).
        if picker.is_kind_of(KindOf::ArmorSalvager) && picker.armor_crate_upgrade < 2 {
            picker.apply_salvage_armor_upgrade();
            return ("armor", 0);
        }
        // Weapon salvager path.
        if picker.is_kind_of(KindOf::WeaponSalvager) && picker.weapon_crate_upgrade < 2 {
            let roll = pure_logic_random_real(seed, 1, 0.0, 1.0);
            if SALVAGE_WEAPON_CHANCE_RESIDUAL >= 1.0 - f32::EPSILON
                || roll < SALVAGE_WEAPON_CHANCE_RESIDUAL
            {
                picker.apply_salvage_weapon_upgrade();
                return ("weapon", 0);
            }
        }
        // Level path.
        let can_level = !matches!(picker.experience.level, VeterancyLevel::Heroic);
        if can_level {
            let roll = pure_logic_random_real(seed, 2, 0.0, 1.0);
            if SALVAGE_LEVEL_CHANCE_RESIDUAL >= 1.0 - f32::EPSILON
                || roll < SALVAGE_LEVEL_CHANCE_RESIDUAL
            {
                picker.apply_salvage_level_gain();
                return ("level", 0);
            }
        }
        // Money fallback.
        let money = if money_provided > 0 {
            money_provided
        } else {
            crate::game_logic::host_create_crate_die::salvage_money_roll(seed, 3)
        };
        ("money", money)
    }
    pub fn try_create_crates_on_die(
        &mut self,
        victim_id: ObjectId,
        victim_pos: glam::Vec3,
        victim_team: Team,
        crate_data: &[String],
        killer_id: Option<ObjectId>,
    ) -> usize {
        if crate_data.is_empty() {
            return 0;
        }
        // Ally kill → no crate (C++ Relationship ALLIES).
        if let Some(kid) = killer_id {
            if let Some(k) = self.objects.get(&kid) {
                if k.team == victim_team {
                    return 0;
                }
            }
        }
        let seed = crate::game_logic::host_create_crate_die::crate_die_seed(
            victim_id, killer_id, self.frame,
        );
        let mut spawned = 0usize;
        for (i, name) in crate_data.iter().enumerate() {
            let draw = (i as u32).wrapping_mul(7);
            let Some(req) =
                crate::game_logic::host_create_crate_die::try_roll_crate_spawn(name, seed, draw)
            else {
                continue;
            };
            // Spawn offset residual (fail-closed vs findPositionAround).
            let ang = (seed.wrapping_add(i as u32) as f32) * 0.7;
            let pos = glam::Vec3::new(
                victim_pos.x + ang.cos() * 5.0,
                victim_pos.y,
                victim_pos.z + ang.sin() * 5.0,
            );
            // Ensure template exists.
            if !self.templates.contains_key(&req.object_name) {
                let t = ThingTemplate::new(&req.object_name);
                // Crates are non-combat pickups.
                self.templates.insert(req.object_name.clone(), t);
            }
            let Some(crate_id) = self.create_object(&req.object_name, Team::Neutral, pos) else {
                continue;
            };
            if req.is_shroud_crate {
                self.host_money_crates.register_shroud_crate(crate_id);
            } else if req.is_heal_crate {
                self.host_money_crates.register_heal_crate(crate_id);
            } else if req.is_unit_crate {
                self.host_money_crates.register_unit_crate(
                    crate_id,
                    &req.unit_crate_type,
                    req.unit_crate_count,
                );
            } else if req.is_veterancy {
                self.host_money_crates.register_level_up_crate(
                    crate_id,
                    req.veterancy_effect_range,
                    req.veterancy_levels,
                );
            } else if req.object_name.eq_ignore_ascii_case("SalvageCrate") {
                self.host_money_crates
                    .register_salvage_crate(crate_id, req.money_provided);
            } else {
                self.host_money_crates.register(
                    crate_id,
                    req.money_provided,
                    req.building_pickup,
                    if req.building_pickup { 25 } else { 0 },
                );
            }
            // C++ DeletionUpdate residual on crate object.
            self.host_money_crates.arm_default_deletion(
                crate_id,
                self.frame,
                crate_id.0.wrapping_add(self.frame),
            );
            if let Some(kid) = killer_id {
                let _ = self.notify_computer_killer_of_crate(kid, crate_id);
            }
            spawned += 1;
        }
        spawned
    }
    pub fn notify_computer_killer_of_crate(
        &mut self,
        killer_id: ObjectId,
        crate_id: ObjectId,
    ) -> bool {
        let killer_team = match self.objects.get(&killer_id) {
            Some(k) if k.is_alive() => k.team,
            _ => return false,
        };
        // Computer = non-local player residual.
        let is_computer = self
            .players
            .values()
            .find(|p| p.team == killer_team)
            .map(|p| !p.is_local)
            .unwrap_or(true); // no player record → treat as AI
        if !is_computer {
            return false;
        }
        self.notify_unit_crate(killer_id, crate_id)
    }
    pub fn try_idle_repulse(&mut self, unit_id: ObjectId) -> bool {
        if !self.enable_repulsors {
            return false;
        }
        let (vision, is_idle, can_be, alive, pos) = {
            let Some(u) = self.objects.get(&unit_id) else {
                return false;
            };
            if !u.is_alive() || u.status.destroyed {
                return false;
            }
            if !u.is_kind_of(crate::game_logic::KindOf::CanBeRepulsed) {
                return false;
            }
            // C++ ai->isIdle()
            let idle = matches!(u.ai_state, crate::game_logic::AIState::Idle)
                && u.target.is_none()
                && u.move_away_from.is_none();
            if !idle {
                return false;
            }
            let vision = u.vision_range.max(50.0);
            (vision, idle, true, true, u.get_position())
        };
        let _ = (is_idle, can_be, alive, pos);
        let Some((rep_id, _)) = self.find_closest_repulsor(unit_id, vision) else {
            return false;
        };
        let rep_pos = match self.objects.get(&rep_id) {
            Some(r) => r.get_position(),
            None => return false,
        };
        if let Some(u) = self.objects.get_mut(&unit_id) {
            // C++ chooseLocomotorSet(PANIC) + requestSafePath residual (fail-closed).
            u.ai_move_away_from_unit(rep_id, rep_pos);
            let _ = u.begin_request_safe_path(
                rep_id,
                u.move_away_destination.unwrap_or(rep_pos),
                self.frame,
            );
            true
        } else {
            false
        }
    }

    pub fn sync_attack_priority_from_script_engine(&mut self) {
        use gamelogic::scripting::engine::get_script_engine;
        let engine_arc = get_script_engine();
        let Ok(guard) = engine_arc.read() else {
            return;
        };
        let Some(engine) = guard.as_ref() else {
            return;
        };

        // Import all named attack priority sets from script engine.
        // Engine stores Vec with index 0 default; named entries from 1..
        // Public API: get_attack_info by name; also iterate via reflection if available.
        // Fail-closed: pull known sets from object_attack_priority_sets values + common names.
        let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        // Collect set names referenced by objects.
        // We need access to object_attack_priority_sets - use public get if exists.
        // Engine may only expose get_object_attack_priority_set per id.

        // Sync host objects: host ObjectId is the script-engine key.
        let object_ids: Vec<(ObjectId, Option<u32>)> = self
            .objects
            .iter()
            .map(|(id, _o)| (*id, Some(id.0)))
            .collect();

        for (host_id, eng_opt) in object_ids {
            let Some(eng_id) = eng_opt else {
                continue;
            };
            let set_name = engine
                .get_object_attack_priority_set(eng_id)
                .map(|s| s.to_string());
            if let Some(ref name) = set_name {
                names.insert(name.clone());
            }
            if let Some(u) = self.objects.get_mut(&host_id) {
                u.attack_priority_set = set_name.filter(|s| !s.is_empty());
            }
        }

        // Also import any set already registered on host (no-op) and pull definitions.
        for name in names {
            if let Some(info) = engine.get_attack_info(&name) {
                let mut host = AttackPriorityInfo::new(info.get_name());
                host.default_priority = info.default_priority;
                for (tmpl, pri) in &info.priority_map {
                    host.set_priority_template(tmpl, *pri);
                }
                self.register_attack_priority_set(host);
            }
        }
        // Drop engine borrow before inheriting team defaults.
        drop(guard);
        drop(engine_arc);
        let need_inherit: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.attack_priority_set.is_none() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        for id in need_inherit {
            self.inherit_team_ai_defaults(id);
        }
    }

    pub fn register_attack_priority_set(&mut self, info: AttackPriorityInfo) {
        let key = info.name.to_ascii_lowercase();
        self.attack_priority_sets.insert(key, info);
    }

    /// C++ AIUpdateInterface::setAttackInfo residual (by set name).
    pub fn set_unit_attack_priority_set(&mut self, unit_id: ObjectId, set_name: Option<&str>) {
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.attack_priority_set = set_name.map(|s| s.to_string());
        }
    }

    /// Resolve AttackPriorityInfo for a unit (None = default closest-only).
    pub fn attack_priority_info_for(&self, unit_id: ObjectId) -> Option<&AttackPriorityInfo> {
        let name = self.objects.get(&unit_id)?.attack_priority_set.as_ref()?;
        self.attack_priority_sets.get(&name.to_ascii_lowercase())
    }

    /// Priority for a candidate target under optional info (includes kind residual).
    pub fn attack_priority_for_target(
        &self,
        info: &AttackPriorityInfo,
        target: &crate::game_logic::object::Object,
    ) -> i32 {
        let mut pri = info.get_priority_for_template(&target.thing.template.name);
        for (kind, &kp) in &info.kind_priorities {
            let hit = match kind.as_str() {
                "infantry" => target.is_kind_of(crate::game_logic::KindOf::Infantry),
                "vehicle" => target.is_kind_of(crate::game_logic::KindOf::Vehicle),
                "structure" | "building" => {
                    target.is_kind_of(crate::game_logic::KindOf::Structure)
                        || target.object_type == crate::game_logic::ObjectType::Building
                }
                "aircraft" => target.is_kind_of(crate::game_logic::KindOf::Aircraft),
                _ => false,
            };
            if hit && kp > pri {
                pri = kp;
            }
        }
        if !target.contained_units().is_empty() {
            for cid in target.contained_units() {
                if let Some(c) = self.objects.get(&cid) {
                    let cp = info.get_priority_for_template(&c.thing.template.name);
                    if cp > pri {
                        pri = cp;
                    }
                }
            }
        }
        pri
    }

    pub fn find_closest_enemy(
        &self,
        unit_id: ObjectId,
        range: f32,
        qualifiers: u32,
    ) -> Option<ObjectId> {
        use find_enemy_flags::*;
        let Some(me) = self.objects.get(&unit_id) else {
            return None;
        };
        if !me.is_alive() {
            return None;
        }
        if (qualifiers & CAN_ATTACK) != 0 && !me.can_attack() {
            return None;
        }
        let me_pos = me.get_position();
        let me_team = me.team;
        let attack_buildings = (qualifiers & ATTACK_BUILDINGS) != 0;
        let within_ar = (qualifiers & WITHIN_ATTACK_RANGE) != 0;
        let need_los = (qualifiers & CAN_SEE) != 0;
        let _unfogged = (qualifiers & UNFOGGED) != 0;
        let _ignore_insig = (qualifiers & IGNORE_INSIGNIFICANT_BUILDINGS) != 0;
        let prio = self.attack_priority_info_for(unit_id);

        let mut best_dist: Option<(ObjectId, f32)> = None;
        let mut best_prio: Option<(ObjectId, i32, i32)> = None; // id, eff, actual

        for (&oid, obj) in self.objects.iter() {
            if oid == unit_id {
                continue;
            }
            if !obj.is_targetable_by_enemy_of(me_team) {
                continue;
            }
            let is_bldg = obj.is_kind_of(crate::game_logic::KindOf::Structure)
                || obj.object_type == crate::game_logic::ObjectType::Building;
            if is_bldg && !attack_buildings {
                let bldg_can_attack =
                    obj.can_attack() || obj.weapon.is_some() || obj.secondary_weapon.is_some();
                if !bldg_can_attack {
                    continue;
                }
            }
            let opos = obj.get_position();
            let dx = opos.x - me_pos.x;
            let dz = opos.z - me_pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            if dist > range {
                continue;
            }
            if within_ar && !me.is_within_attack_range(obj) {
                continue;
            }
            if need_los {
                if self.attack_view_blocked(unit_id, Some(oid), opos)
                    || self.pathfinding_system.is_attack_view_blocked(me_pos, opos)
                {
                    continue;
                }
            }
            if (qualifiers & CAN_ATTACK) != 0 {
                let r = self.get_able_to_attack_specific_object(
                    unit_id,
                    oid,
                    AbleToAttackType::NewTarget,
                    false,
                );
                if !matches!(
                    r,
                    CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                ) {
                    continue;
                }
            }

            if let Some(info) = prio {
                let cur = self.attack_priority_for_target(info, obj);
                if cur == 0 {
                    continue; // C++ skip zero priority
                }
                let modifier = (dist / ATTACK_PRIORITY_DISTANCE_MODIFIER) as i32;
                let mut mod_pri = cur - modifier;
                if mod_pri < 1 {
                    mod_pri = 1;
                }
                match best_prio {
                    Some((_, eff, act)) if mod_pri > eff || (mod_pri == eff && cur > act) => {
                        best_prio = Some((oid, mod_pri, cur));
                    }
                    None => best_prio = Some((oid, mod_pri, cur)),
                    _ => {}
                }
            } else {
                match best_dist {
                    Some((_, bd)) if dist < bd => best_dist = Some((oid, dist)),
                    None => best_dist = Some((oid, dist)),
                    _ => {}
                }
            }
        }
        if prio.is_some() {
            best_prio.map(|(id, _, _)| id)
        } else {
            best_dist.map(|(id, _)| id)
        }
    }

    pub fn get_next_mood_target(
        &mut self,
        unit_id: ObjectId,
        called_by_ai: bool,
        called_during_idle: bool,
        is_player_controlled: bool,
    ) -> Option<ObjectId> {
        let now = self.frame;
        // Snapshot gates.
        let (
            alive,
            using_ability,
            attacking,
            stealthed,
            auto_idle,
            attitude,
            last_dmg,
            pos,
            team,
            rate,
            next_check,
        ) = {
            let o = self.objects.get(&unit_id)?;
            if o.is_kind_of(crate::game_logic::KindOf::Projectile) {
                return None;
            }
            (
                o.is_alive() && !o.status.destroyed,
                o.status.using_ability,
                o.status.attacking || o.ai_state == AIState::Attacking,
                o.status.stealthed && !o.status.detected,
                o.auto_acquire_when_idle,
                o.ai_attitude,
                o.last_damage_source,
                o.get_position(),
                o.team,
                o.mood_attack_check_rate.max(1),
                o.next_mood_check_time,
            )
        };
        if !alive || using_ability {
            return None;
        }
        if called_during_idle && !auto_idle {
            return None;
        }
        // Stealthed idle acquire residual: block unless contained-fire (not ported).
        if called_during_idle && stealthed {
            return None;
        }
        // Sleep mood: no acquire.
        if attitude <= -2 && !is_player_controlled {
            return None;
        }

        // Passive mood: return last damage source if legal enemy.
        if attitude == -1 && !is_player_controlled {
            if let Some(src) = last_dmg {
                if src != unit_id {
                    let ok = matches!(
                        self.get_able_to_attack_specific_object(
                            unit_id,
                            src,
                            AbleToAttackType::NewTarget,
                            false
                        ),
                        CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                    );
                    if ok {
                        return Some(src);
                    }
                }
            }
            // Passive without recent attacker: no proactive acquire.
            return None;
        }

        if called_by_ai {
            if now < next_check && next_check != 0 {
                return None;
            }
            // Schedule next check.
            if let Some(o) = self.objects.get_mut(&unit_id) {
                o.next_mood_check_time = now.saturating_add(rate);
            }
        }

        let mut range = self.adjusted_vision_range_for_mood(unit_id);
        if range <= 0.0 {
            return None;
        }
        // Container radius residual omitted (fail-closed).

        // Human AI residual: only within attack range.
        if called_by_ai && is_player_controlled {
            if let Some(o) = self.objects.get(&unit_id) {
                let wr = o
                    .weapon
                    .as_ref()
                    .map(|w| w.range)
                    .or_else(|| o.secondary_weapon.as_ref().map(|w| w.range))
                    .unwrap_or(0.0);
                range = wr.min(range);
            }
        }

        // C++ findClosestEnemy flags residual.
        use find_enemy_flags::*;
        let mut flags = CAN_ATTACK;
        if let Some(o) = self.objects.get(&unit_id) {
            if o.is_kind_of(crate::game_logic::KindOf::Structure) || !o.can_move() {
                flags |= CAN_SEE;
            }
        }
        if called_by_ai && is_player_controlled {
            flags |= WITHIN_ATTACK_RANGE | UNFOGGED;
        }
        let _ = (pos, team);
        self.find_closest_enemy(unit_id, range, flags)
    }

    /// Idle mood auto-acquire: if idle and mood allows, set attack target.
    pub fn try_mood_auto_acquire(
        &mut self,
        unit_id: ObjectId,
        is_player_controlled: bool,
    ) -> Option<ObjectId> {
        let eligible = self.objects.get(&unit_id).map(|o| {
            let idle = matches!(o.ai_state, AIState::Idle) && !o.status.attacking;
            let attack_moving = matches!(o.ai_state, AIState::AttackMoving) || o.is_attack_path;
            (idle || attack_moving) && o.target.is_none() && o.is_alive()
        })?;
        if !eligible {
            return None;
        }
        if !self.mood_allows_attack(unit_id, is_player_controlled) {
            return None;
        }
        let victim = self.get_next_mood_target(unit_id, true, true, is_player_controlled)?;
        // Host-immediate attack enter + decision log inside attack_state_enter.
        if self.attack_state_enter(unit_id, victim) == AttackMachineResult::Continue {
            Some(victim)
        } else {
            None
        }
    }

    pub fn get_mood_matrix_action_adjustment(
        &self,
        unit_id: ObjectId,
        action: MoodMatrixAction,
        is_player_controlled: bool,
    ) -> u32 {
        use mood_action_adjust::*;
        let Some(obj) = self.objects.get(&unit_id) else {
            return ACTION_OK;
        };
        // Mob member residual: IGNORED_IN_GUI KindOf not fully ported.
        if is_player_controlled {
            return ACTION_OK;
        }
        // Attitude ordinals: -2 Sleep, -1 Passive, 0 Normal, 1 Alert, 2 Aggressive.
        let mood = obj.ai_attitude.clamp(-2, 2);
        match action {
            MoodMatrixAction::Idle => match mood {
                -2 => ACTION_OK | AFFECT_RANGE_IGNORE_ALL,
                -1 => ACTION_OK | AFFECT_RANGE_WAIT_FOR_ATTACK,
                0 => ACTION_OK,
                1 => ACTION_OK | AFFECT_RANGE_ALERT,
                _ => ACTION_OK | AFFECT_RANGE_AGGRESSIVE,
            },
            MoodMatrixAction::Move => match mood {
                -2 => ACTION_TO_IDLE | AFFECT_RANGE_IGNORE_ALL,
                -1 => ACTION_OK | AFFECT_RANGE_WAIT_FOR_ATTACK,
                0 => ACTION_OK,
                1 => ACTION_TO_ATTACK_MOVE | AFFECT_RANGE_ALERT,
                _ => ACTION_TO_ATTACK_MOVE | AFFECT_RANGE_AGGRESSIVE,
            },
            MoodMatrixAction::Attack => match mood {
                -2 => ACTION_TO_IDLE | AFFECT_RANGE_IGNORE_ALL,
                _ => ACTION_OK,
            },
            MoodMatrixAction::AttackMove => match mood {
                -2 => ACTION_TO_IDLE | AFFECT_RANGE_IGNORE_ALL,
                -1 | 0 => ACTION_OK,
                1 => ACTION_OK | AFFECT_RANGE_ALERT,
                _ => ACTION_OK | AFFECT_RANGE_AGGRESSIVE,
            },
        }
    }

    /// True when mood allows attack action (MAA_Action_Ok bit set, not forced to idle).
    pub fn mood_allows_attack(&self, unit_id: ObjectId, is_player_controlled: bool) -> bool {
        use mood_action_adjust::*;
        let adj = self.get_mood_matrix_action_adjustment(
            unit_id,
            MoodMatrixAction::Attack,
            is_player_controlled,
        );
        (adj & ACTION_OK) != 0 && (adj & ACTION_TO_IDLE) == 0
    }

    pub fn get_able_to_attack_specific_object(
        &self,
        unit_id: ObjectId,
        victim_id: ObjectId,
        attack_type: AbleToAttackType,
        from_player: bool,
    ) -> CanAttackResult {
        let Some(source) = self.objects.get(&unit_id) else {
            return CanAttackResult::NotPossible;
        };
        let Some(victim) = self.objects.get(&victim_id) else {
            return CanAttackResult::NotPossible;
        };
        // Basic sanity.
        if !source.is_alive()
            || !victim.is_alive()
            || source.status.destroyed
            || victim.status.destroyed
            || unit_id == victim_id
        {
            return CanAttackResult::NotPossible;
        }
        // MASKED residual: under_construction structures not attackable until built? skip.
        // UNATTACKABLE kind residual: name-token fail-closed.
        // UNATTACKABLE residual: KindOf token not yet ported; skip name gate.
        // NO_ATTACK_FROM_AI residual not fully tracked; skip unless AI path later.

        let force = attack_type.is_forced();
        let same_owner_force = force && source.team == victim.team;

        // Stealth residual.
        let mut allow_stealth_block = true;
        if source.status.ignoring_stealth || same_owner_force {
            allow_stealth_block = false;
        }
        if force && victim.status.disguised {
            allow_stealth_block = false;
        }
        if allow_stealth_block
            && victim.status.stealthed
            && !victim.status.detected
            && !victim.status.disguised
        {
            return CanAttackResult::NotPossible;
        }

        // Relationship residual: ENEMIES required unless force / mine.
        let enemies = source.team != victim.team;
        let is_mine = false; // KindOf::Projectile residual pending full matrix
        if !enemies && !force && !(is_mine && source.team != victim.team) {
            // Player command rejects non-enemies; AI/script may continue.
            if from_player {
                return CanAttackResult::NotPossible;
            }
            // AI residual: still allow if not same team allies — same team blocked.
            if source.team == victim.team {
                return CanAttackResult::NotPossible;
            }
        }

        // Contained in enclosing container residual.
        if victim.contained_by.is_some() {
            // Fail-closed: treat any contained victim as not directly attackable
            // unless force (garrison fire residual elsewhere).
            if !force {
                return CanAttackResult::NotPossible;
            }
        }

        // Weapon legality / range residual.
        self.get_able_to_use_weapon_against_target(unit_id, Some(victim_id), None, attack_type)
    }

    /// C++ WeaponSet::getAbleToUseWeaponAgainstTarget residual.
    pub fn get_able_to_use_weapon_against_target(
        &self,
        unit_id: ObjectId,
        victim_id: Option<ObjectId>,
        pos: Option<glam::Vec3>,
        _attack_type: AbleToAttackType,
    ) -> CanAttackResult {
        let Some(source) = self.objects.get(&unit_id) else {
            return CanAttackResult::NotPossible;
        };
        if source.weapon.is_none() && source.secondary_weapon.is_none() {
            return CanAttackResult::InvalidShot;
        }

        let target_pos = if let Some(vid) = victim_id {
            let Some(v) = self.objects.get(&vid) else {
                return CanAttackResult::NotPossible;
            };
            // Kind legality residual (air/ground), NOT range — range decides
            // Possible vs PossibleAfterMoving below (C++ WeaponSet split).
            let target_is_air =
                v.is_kind_of(crate::game_logic::KindOf::Aircraft) || v.status.airborne_target;
            let kind_ok = |w: &crate::game_logic::Weapon| {
                if target_is_air {
                    w.can_target_air
                } else {
                    w.can_target_ground
                }
            };
            // Stealthed gate already applied in get_able_to_attack_specific_object;
            // still block here if stealthed and not ignoring (defense in depth).
            if v.is_effectively_stealthed()
                && v.team != source.team
                && !source.status.ignoring_stealth
            {
                return CanAttackResult::NotPossible;
            }
            let primary_ok = source.weapon.as_ref().is_some_and(kind_ok);
            let secondary_ok = source.secondary_weapon.as_ref().is_some_and(kind_ok);
            if !primary_ok && !secondary_ok {
                return CanAttackResult::InvalidShot;
            }
            v.get_position()
        } else if let Some(p) = pos {
            p
        } else {
            return CanAttackResult::NotPossible;
        };

        let within = if let Some(vid) = victim_id {
            let v = self.objects.get(&vid).unwrap();
            source.is_within_attack_range(v)
        } else {
            source.is_within_attack_range_pos(target_pos)
        };

        // Contact / invalid pitch residual not expanded — range gate only.
        if within {
            CanAttackResult::Possible
        } else {
            // Mobile residual: max_speed > 0 or can_move.
            let mobile = source.can_move() || source.movement.max_speed > 1e-3;
            if mobile {
                CanAttackResult::PossibleAfterMoving
            } else {
                // Immobile and out of range → invalid shot residual.
                CanAttackResult::InvalidShot
            }
        }
    }

    pub fn cannot_possibly_attack_object(
        &self,
        unit_id: ObjectId,
        victim_id: ObjectId,
        force_attacking: bool,
    ) -> bool {
        let Some(obj) = self.objects.get(&unit_id) else {
            return true;
        };
        // C++ callers check isAbleToAttack before getAbleToAttackSpecificObject.
        if !obj.can_attack() {
            return true;
        }
        let attack_type = if force_attacking {
            AbleToAttackType::ContinuedTargetForced
        } else {
            AbleToAttackType::ContinuedTarget
        };
        // AI command residual (not player click).
        let result =
            self.get_able_to_attack_specific_object(unit_id, victim_id, attack_type, false);
        !matches!(
            result,
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        )
    }

    pub fn attack_state_enter(
        &mut self,
        unit_id: ObjectId,
        victim_id: ObjectId,
    ) -> AttackMachineResult {
        {
            let Some(u) = self.objects.get(&unit_id) else {
                return AttackMachineResult::Failure;
            };
            if !u.is_alive() || u.status.under_construction {
                return AttackMachineResult::Failure;
            }
            if u.weapon.is_none() && u.secondary_weapon.is_none() {
                return AttackMachineResult::Failure;
            }
            if u.is_kind_of(crate::game_logic::KindOf::Projectile) {
                return AttackMachineResult::Failure;
            }
        }
        {
            let Some(v) = self.objects.get(&victim_id) else {
                return AttackMachineResult::Failure;
            };
            if !v.is_alive() || v.status.destroyed {
                return AttackMachineResult::Failure;
            }
        }
        // C++ getMoodMatrixActionAdjustment(MM_Action_Attack) residual.
        // AI-controlled sleep mood refuses attack (→ idle).
        if !self.mood_allows_attack(unit_id, false) {
            return AttackMachineResult::Failure;
        }
        // C++ cannotPossiblyAttackObject residual on enter.
        if self.cannot_possibly_attack_object(unit_id, victim_id, false) {
            return AttackMachineResult::Failure;
        }
        // C++ AIAttackState::chooseWeapon residual (PreferMostDamage).
        let t = self.frame as f32 * LOGIC_FRAME_TIMESTEP;
        if !self.choose_best_weapon_for_target(unit_id, Some(victim_id), t) {
            return AttackMachineResult::Failure;
        }
        let decision_auth = crate::gameworld_shadow::gameworld_ai_decision_authority_live();
        {
            let Some(u) = self.objects.get_mut(&unit_id) else {
                return AttackMachineResult::Failure;
            };
            // Nested SM bookkeeping stays host; engagement authority is log-only when on.
            u.attack_substate = crate::game_logic::AttackSubState::AimAtTarget;
            if u.max_shots_to_fire == 0 {
                u.max_shots_to_fire = -1;
            }
            if !decision_auth {
                u.target = Some(victim_id);
                u.set_status_attacking(true);
                u.set_ai_state(AIState::Attacking);
            }
        }
        if decision_auth {
            crate::game_logic::host_ai_decision_log::record_attack(unit_id, victim_id);
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 2);
            // Attacking
        }
        let _ = self.attack_aim_at_target_enter(unit_id);
        AttackMachineResult::Continue
    }

    /// C++ AIAttackState::onExit residual.
    pub fn attack_state_exit(&mut self, unit_id: ObjectId) {
        self.attack_aim_at_target_exit(unit_id);
        self.attack_fire_weapon_exit(unit_id);
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.set_status_attacking(false);
            u.status.is_aiming_weapon = false;
            u.status.is_firing_weapon = false;
            u.attack_substate = crate::game_logic::AttackSubState::AimAtTarget;
        }
    }

    /// Drive nested AttackStateMachine for units that entered via attack_state_enter.
    ///
    /// Units owned by the SM have is_aiming_weapon / is_firing_weapon or a non-Aim
    /// substate after approach. Legacy update_combat still owns plain Attacking units.
    pub(crate) fn tick_nested_attack_machines(
        &mut self,
        object_ids: &[ObjectId],
        current_time: f32,
        logic_frame: u32,
    ) {
        let mut stop: Vec<ObjectId> = Vec::new();
        for &id in object_ids {
            let (drive, victim) = {
                let Some(u) = self.objects.get(&id) else {
                    continue;
                };
                if u.ai_state != AIState::Attacking {
                    continue;
                }
                let sm_owned = u.status.is_aiming_weapon
                    || u.status.is_firing_weapon
                    || !matches!(
                        u.attack_substate,
                        crate::game_logic::AttackSubState::AimAtTarget
                    );
                if !sm_owned {
                    continue;
                }
                let Some(vid) = u.target else {
                    stop.push(id);
                    continue;
                };
                (true, vid)
            };
            if !drive {
                continue;
            }
            match self.tick_attack_state_machine(id, victim, current_time, logic_frame, 0.35) {
                AttackMachineResult::Continue => {}
                AttackMachineResult::Success | AttackMachineResult::Failure => {
                    stop.push(id);
                }
            }
        }
        for id in stop {
            self.attack_state_exit(id);
            self.stop_attack_decision_aware(id);
        }
    }

    /// C++ AttackStateMachine + AIAttackState::update residual (object attack).

    /// C++ canPursue residual for AttackStateMachine CHASE vs APPROACH.

    /// C++ outOfWeaponRangeObject state condition residual.
    ///
    /// True when view is LOS-blocked or target is outside attack range
    /// (unless leech-range is active).
    pub fn out_of_weapon_range_object(&self, unit_id: ObjectId, victim_id: ObjectId) -> bool {
        let Some(obj) = self.objects.get(&unit_id) else {
            return true;
        };
        let Some(victim) = self.objects.get(&victim_id) else {
            return true;
        };
        if obj.weapon.is_none() && obj.secondary_weapon.is_none() {
            return true;
        }
        // Leech range residual: temporarily unlimited while engaged.
        let leech = obj.leech_range_active();
        if leech {
            return false;
        }
        // Contact weapon residual: skip LOS false positives at tiny ranges.
        let contact = obj
            .weapon
            .as_ref()
            .map(|w| w.range <= 5.0 || w.min_range > 0.0 && w.range <= w.min_range * 2.0)
            .unwrap_or(false);
        // Ground LOS residual.
        let on_ground = !obj.status.airborne_target
            || obj.is_kind_of(crate::game_logic::KindOf::Structure)
            || obj.contained_by.is_some()
            || !obj.can_move();
        let victim_air = victim.status.airborne_target;
        if !contact && on_ground && !victim_air {
            let from = obj.get_position();
            let to = victim.get_position();
            if self.attack_view_blocked(unit_id, Some(victim_id), to)
                || self.pathfinding_system.is_attack_view_blocked(from, to)
            {
                return true;
            }
        }
        !obj.is_within_attack_range(victim)
    }

    /// C++ wantToSquishTarget state condition residual.
    ///
    /// AI computer crush/squish chase preference (not player).
    pub fn want_to_squish_target(&self, unit_id: ObjectId, victim_id: ObjectId) -> bool {
        let Some(obj) = self.objects.get(&unit_id) else {
            return false;
        };
        let Some(victim) = self.objects.get(&victim_id) else {
            return false;
        };
        if victim.contained_by.is_some() {
            return false;
        }
        // Fail-closed: host does not track player type on every object; use team AI residual.
        // Prefer vehicles crushing infantry.
        // DontAutoCrushInfantry residual: skip if template name hints "dozer" without crush.
        obj.can_crush_only(victim, false)
    }

    pub fn should_chase_attack_target(&self, unit_id: ObjectId, victim_id: ObjectId) -> bool {
        let Some(u) = self.objects.get(&unit_id) else {
            return false;
        };
        let Some(v) = self.objects.get(&victim_id) else {
            return false;
        };
        if !u.is_alive() || !v.is_alive() {
            return false;
        }
        // Immobile / projectile residual: never chase.
        if !u.can_move() || u.is_kind_of(crate::game_logic::KindOf::Projectile) {
            return false;
        }
        // Stealthed undetected residual.
        if v.status.stealthed && !v.status.detected && !v.status.disguised {
            return false;
        }
        // C++ wantToSquishTarget → CHASE_TARGET condition residual.
        if self.want_to_squish_target(unit_id, victim_id) {
            return true;
        }
        u.can_pursue_target(v)
    }

    pub fn tick_attack_state_machine(
        &mut self,
        unit_id: ObjectId,
        victim_id: ObjectId,
        current_time: f32,
        logic_frame: u32,
        max_turn_rad: f32,
    ) -> AttackMachineResult {
        {
            let Some(u) = self.objects.get(&unit_id) else {
                return AttackMachineResult::Failure;
            };
            if !u.is_alive() || u.status.under_construction {
                return AttackMachineResult::Failure;
            }
            if u.weapon.is_none() && u.secondary_weapon.is_none() {
                return AttackMachineResult::Failure;
            }
            if u.max_shots_to_fire == 0 {
                return AttackMachineResult::Failure;
            }
        }
        {
            let Some(v) = self.objects.get(&victim_id) else {
                return AttackMachineResult::Success;
            };
            if !v.is_alive() || v.status.destroyed {
                return AttackMachineResult::Success;
            }
        }
        // C++ cannotPossiblyAttackObject condition → EXIT_MACHINE_WITH_FAILURE.
        if self.cannot_possibly_attack_object(unit_id, victim_id, false) {
            return AttackMachineResult::Failure;
        }
        // Re-evaluate weapon choice every frame (C++ AIAttackState::update).
        let _ = self.choose_best_weapon_for_target(unit_id, Some(victim_id), current_time);

        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.set_target(Some(victim_id));
            u.set_status_attacking(true);
            u.set_ai_state(AIState::Attacking);
        }
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_attack(unit_id, victim_id);
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 2);
        }

        let in_range = {
            let Some(u) = self.objects.get(&unit_id) else {
                return AttackMachineResult::Failure;
            };
            let Some(v) = self.objects.get(&victim_id) else {
                return AttackMachineResult::Success;
            };
            u.is_within_attack_range(v)
        };
        // C++ outOfWeaponRangeObject condition (range + LOS, leech-aware).
        let out_of_wr = self.out_of_weapon_range_object(unit_id, victim_id);

        let sub = self
            .objects
            .get(&unit_id)
            .map(|u| u.attack_substate)
            .unwrap_or(crate::game_logic::AttackSubState::AimAtTarget);

        use crate::game_logic::AttackSubState;
        match sub {
            AttackSubState::AimAtTarget => {
                if out_of_wr {
                    let chase = self.should_chase_attack_target(unit_id, victim_id);
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.attack_substate = if chase {
                            AttackSubState::ChaseTarget
                        } else {
                            AttackSubState::ApproachTarget
                        };
                        u.status.is_aiming_weapon = false;
                    }
                    let _ = self.attack_approach_compute_path(unit_id, Some(victim_id), None);
                    return AttackMachineResult::Continue;
                }
                match self.attack_aim_at_target_update(unit_id, victim_id, max_turn_rad) {
                    AttackAimResult::Success => {
                        if let Some(u) = self.objects.get_mut(&unit_id) {
                            u.attack_substate = AttackSubState::FireWeapon;
                            u.status.is_aiming_weapon = false;
                        }
                        let _ = self.attack_fire_weapon_enter(unit_id);
                    }
                    AttackAimResult::Continue => {}
                    AttackAimResult::Failure => return AttackMachineResult::Failure,
                }
                AttackMachineResult::Continue
            }
            AttackSubState::FireWeapon => {
                if out_of_wr {
                    let chase = self.should_chase_attack_target(unit_id, victim_id);
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.attack_substate = if chase {
                            AttackSubState::ChaseTarget
                        } else {
                            AttackSubState::ApproachTarget
                        };
                        u.status.is_firing_weapon = false;
                    }
                    let _ = self.attack_approach_compute_path(unit_id, Some(victim_id), None);
                    return AttackMachineResult::Continue;
                }
                let fire = self.attack_fire_weapon_update(unit_id, victim_id, current_time);
                match fire {
                    AttackFireResult::Continue => {
                        // PRE_ATTACK wind-up — stay in FIRE.
                    }
                    AttackFireResult::Success | AttackFireResult::Failure => {
                        // C++ both edges return to AIM_AT_TARGET.
                        self.attack_fire_weapon_exit(unit_id);
                        if let Some(u) = self.objects.get_mut(&unit_id) {
                            u.attack_substate = AttackSubState::AimAtTarget;
                        }
                        let _ = self.attack_aim_at_target_enter(unit_id);
                    }
                }
                AttackMachineResult::Continue
            }
            AttackSubState::ApproachTarget => {
                if in_range {
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.attack_substate = AttackSubState::AimAtTarget;
                        u.set_status_moving(false);
                    }
                    let _ = self.attack_aim_at_target_enter(unit_id);
                    return AttackMachineResult::Continue;
                }
                // Fleeing victim may upgrade Approach → Chase mid-path.
                if self.should_chase_attack_target(unit_id, victim_id) {
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.attack_substate = AttackSubState::ChaseTarget;
                    }
                }
                let _ = self.attack_approach_compute_path(unit_id, Some(victim_id), None);
                if let Some(u) = self.objects.get_mut(&unit_id) {
                    u.approach_timestamp = logic_frame;
                    u.set_status_moving(true);
                }
                AttackMachineResult::Continue
            }
            AttackSubState::ChaseTarget => {
                // C++ AIAttackPursueTargetState residual.
                // Stealthed undetected victim: drop to approach.
                {
                    let stealth_bail = self
                        .objects
                        .get(&victim_id)
                        .map(|v| v.status.stealthed && !v.status.detected && !v.status.disguised)
                        .unwrap_or(false);
                    if stealth_bail {
                        if let Some(u) = self.objects.get_mut(&unit_id) {
                            u.attack_substate = AttackSubState::ApproachTarget;
                        }
                        return AttackMachineResult::Continue;
                    }
                }
                if in_range {
                    // Match speeds residual while goal still in range (victim * 0.95).
                    let victim_spd = self
                        .objects
                        .get(&victim_id)
                        .map(|v| v.forward_speed_2d().abs())
                        .unwrap_or(0.0);
                    if let Some(attacker) = self.objects.get_mut(&unit_id) {
                        let desired = victim_spd * 0.95;
                        let vel = attacker.movement.velocity;
                        let speed = (vel.x * vel.x + vel.z * vel.z).sqrt();
                        if speed > 1e-3 && desired > 0.0 {
                            let scale = (desired / speed).min(1.0);
                            attacker.movement.velocity.x *= scale;
                            attacker.movement.velocity.z *= scale;
                        }
                        attacker.attack_substate = AttackSubState::AimAtTarget;
                        attacker.set_status_moving(false);
                    }
                    let _ = self.attack_aim_at_target_enter(unit_id);
                    return AttackMachineResult::Continue;
                }
                // canPursue false → drop to Approach (C++ onEnter SUCCESS → approach).
                if !self.should_chase_attack_target(unit_id, victim_id) {
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.attack_substate = AttackSubState::ApproachTarget;
                    }
                }
                let _ = self.attack_approach_compute_path(unit_id, Some(victim_id), None);
                if let Some(u) = self.objects.get_mut(&unit_id) {
                    u.approach_timestamp = logic_frame;
                    u.set_status_moving(true);
                }
                AttackMachineResult::Continue
            }
        }
    }

    /// C++ AIUpdateInterface::setTurretTargetObject residual.
    pub fn set_turret_target_object(
        &mut self,
        unit_id: ObjectId,
        victim_id: Option<ObjectId>,
        force_attacking: bool,
    ) {
        if let Some(vid) = victim_id {
            let alive = self
                .objects
                .get(&vid)
                .map(|v| v.is_alive() && !v.status.destroyed)
                .unwrap_or(false);
            if !alive {
                if let Some(u) = self.objects.get_mut(&unit_id) {
                    u.set_turret_target_object(None, false);
                    if u.turret_substate == crate::game_logic::object::TurretSubState::Hold {
                        u.turret_hold_until_frame =
                            self.frame.saturating_add(u.turret_recenter_frames.max(1));
                        u.turret_holding = true;
                    }
                }
                return;
            }
        }
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.set_turret_target_object(victim_id, force_attacking);
            if victim_id.is_none()
                && u.turret_substate == crate::game_logic::object::TurretSubState::Hold
            {
                u.turret_hold_until_frame =
                    self.frame.saturating_add(u.turret_recenter_frames.max(1));
                u.turret_holding = true;
            }
        }
    }

    /// C++ TurretAIAimTurretState::update residual — rotate turret toward victim.
    ///
    /// Returns Success when within REL_THRESH (~2°), Continue while turning,
    /// Failure if no target / dead.

    /// C++ TurretAI state machine residual (AIM/FIRE/HOLD/RECENTER).
    ///
    /// Call once per frame for turret-enabled units.
    pub fn tick_turret_state_machine(
        &mut self,
        unit_id: ObjectId,
        current_time: f32,
        logic_frame: u32,
    ) -> AttackAimResult {
        use crate::game_logic::object::TurretSubState;

        let (enabled, sub, tid) = {
            let Some(u) = self.objects.get(&unit_id) else {
                return AttackAimResult::Failure;
            };
            if !u.turret_enabled || !u.is_alive() {
                return AttackAimResult::Failure;
            }
            (true, u.turret_substate, u.turret_target_id)
        };
        let _ = enabled;

        match sub {
            TurretSubState::Idle | TurretSubState::IdleScan => {
                // Idle residual owned by strategy-center host; no-op here.
                AttackAimResult::Continue
            }
            TurretSubState::Aim => {
                let Some(vid) = tid else {
                    // No target → HOLD
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.turret_substate = TurretSubState::Hold;
                        u.turret_hold_until_frame =
                            logic_frame.saturating_add(u.turret_recenter_frames.max(1));
                        u.turret_holding = true;
                        u.record_host_turret();
                    }
                    return AttackAimResult::Continue;
                };
                // Dead victim → HOLD
                let alive = self
                    .objects
                    .get(&vid)
                    .map(|v| v.is_alive() && !v.status.destroyed)
                    .unwrap_or(false);
                if !alive {
                    self.set_turret_target_object(unit_id, None, false);
                    return AttackAimResult::Continue;
                }
                // OOR while aiming stays in AIM (body approach elsewhere).
                match self.tick_turret_aim(unit_id, 1.0) {
                    AttackAimResult::Success => {
                        if let Some(u) = self.objects.get_mut(&unit_id) {
                            u.turret_substate = TurretSubState::Fire;
                        }
                        let _ = self.attack_fire_weapon_enter(unit_id);
                        AttackAimResult::Success
                    }
                    other => other,
                }
            }
            TurretSubState::Fire => {
                let Some(vid) = tid else {
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.turret_substate = TurretSubState::Aim;
                    }
                    return AttackAimResult::Continue;
                };
                // C++ fireConditions: outOfWeaponRangeObject → AIM
                if self.out_of_weapon_range_object(unit_id, vid) {
                    self.attack_fire_weapon_exit(unit_id);
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.turret_substate = TurretSubState::Aim;
                    }
                    return AttackAimResult::Continue;
                }
                let fire = self.attack_fire_weapon_update(unit_id, vid, current_time);
                match fire {
                    AttackFireResult::Continue => AttackAimResult::Continue,
                    AttackFireResult::Success | AttackFireResult::Failure => {
                        // C++ FIRE success and failure both return to AIM.
                        self.attack_fire_weapon_exit(unit_id);
                        if let Some(u) = self.objects.get_mut(&unit_id) {
                            u.turret_substate = TurretSubState::Aim;
                        }
                        AttackAimResult::Continue
                    }
                }
            }
            TurretSubState::Hold => {
                let done = {
                    let Some(u) = self.objects.get(&unit_id) else {
                        return AttackAimResult::Failure;
                    };
                    logic_frame >= u.turret_hold_until_frame && u.turret_hold_until_frame > 0
                };
                if done {
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.turret_holding = false;
                        u.record_host_turret();
                        u.turret_hold_until_frame = 0;
                        u.turret_substate = TurretSubState::Recenter;
                        u.turret_idle_recentering = true;
                    }
                }
                AttackAimResult::Continue
            }
            TurretSubState::Recenter => {
                // C++ rate modifier 0.5 toward natural angle/pitch.
                let (ang_ok, pitch_ok) = {
                    let Some(u) = self.objects.get_mut(&unit_id) else {
                        return AttackAimResult::Failure;
                    };
                    let nat_a = u.turret_natural_angle_deg.to_radians();
                    let nat_p = u.turret_natural_pitch_deg.to_radians();
                    let a = u.turn_turret_towards_angle_rad(nat_a, 0.5, 0.0);
                    let p = u.turn_turret_towards_pitch_rad(nat_p, 0.5);
                    (a, p)
                };
                if ang_ok && pitch_ok {
                    if let Some(u) = self.objects.get_mut(&unit_id) {
                        u.turret_substate = TurretSubState::Idle;
                        u.turret_idle_recentering = false;
                        u.turret_rotating = false;
                    }
                    AttackAimResult::Success
                } else {
                    AttackAimResult::Continue
                }
            }
        }
    }

    /// Drive TurretAI SM for all turret-enabled objects.
    pub(crate) fn tick_all_turret_state_machines(
        &mut self,
        object_ids: &[ObjectId],
        current_time: f32,
        logic_frame: u32,
    ) {
        for &id in object_ids {
            let enabled = self
                .objects
                .get(&id)
                .map(|o| o.turret_enabled && o.is_alive())
                .unwrap_or(false);
            if enabled {
                let _ = self.tick_turret_state_machine(id, current_time, logic_frame);
            }
        }
    }

    pub fn tick_turret_aim(
        &mut self,
        unit_id: ObjectId,
        max_rate_modifier: f32,
    ) -> AttackAimResult {
        const REL_THRESH: f32 = 0.035; // ~2 degrees
        let (victim_pos, has_target) = {
            let Some(u) = self.objects.get(&unit_id) else {
                return AttackAimResult::Failure;
            };
            if !u.turret_enabled {
                return AttackAimResult::Failure;
            }
            let Some(tid) = u.turret_target_id else {
                return AttackAimResult::Failure;
            };
            let Some(v) = self.objects.get(&tid) else {
                return AttackAimResult::Failure;
            };
            if !v.is_alive() || v.status.destroyed {
                return AttackAimResult::Failure;
            }
            (v.get_position(), true)
        };
        let _ = has_target;
        let Some(u) = self.objects.get_mut(&unit_id) else {
            return AttackAimResult::Failure;
        };
        u.turret_substate = crate::game_logic::object::TurretSubState::Aim;
        // Relative angle body→target in world, convert to body-relative turret aim.
        // Host residual: body orientation + turret angle compose aim direction.
        let body_ori = u.get_orientation();
        let rel_world = u.relative_angle_2d_to(victim_pos); // already body-relative
                                                            // Desired turret angle = current body-relative target heading.
                                                            // relative_angle_2d_to returns how much body must turn; for turret,
                                                            // desired turret yaw (body-relative) = current turret + (rel - 0) wait:
                                                            // C++: relAngle = getRelativeAngle2D(obj, enemyPos) which is target bearing
                                                            // relative to object orientation. Turret angle is also relative to parent.
                                                            // So desired turret angle = relAngle (if turret 0 faces body forward).
        let desired_turret = rel_world;
        let aligned = u.turn_turret_towards_angle_rad(
            desired_turret,
            max_rate_modifier.max(0.01),
            REL_THRESH,
        );
        // Clear unused body_ori warning path: used for future pitch.
        let _ = body_ori;
        if aligned {
            AttackAimResult::Success
        } else {
            AttackAimResult::Continue
        }
    }

    pub fn attack_aim_at_target_enter(&mut self, unit_id: ObjectId) -> bool {
        let (alive, has_wpn, tid) = {
            let Some(u) = self.objects.get(&unit_id) else {
                return false;
            };
            (
                u.is_alive(),
                u.weapon.is_some() || u.secondary_weapon.is_some(),
                u.target,
            )
        };
        if !alive || !has_wpn {
            return false;
        }
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.set_status_aiming_weapon(true);
            u.set_status_attacking(true);
            u.set_ai_state(AIState::Attacking);
        }
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 2);
            // Attacking
        }
        // C++ AIAttackAimAtTargetState sets turret target when tur != INVALID.
        if let Some(vid) = tid {
            self.set_turret_target_object(unit_id, Some(vid), false);
        }
        true
    }

    /// C++ AIAttackAimAtTargetState::onExit residual.
    pub fn attack_aim_at_target_exit(&mut self, unit_id: ObjectId) {
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.status.is_aiming_weapon = false;
        }
    }

    /// C++ AIAttackAimAtTargetState::update residual (body turn, no turret).
    ///
    /// Turns in place toward victim using AcceptableAimDelta; returns Success
    /// when |relAngle| < aimDelta.
    pub fn attack_aim_at_target_update(
        &mut self,
        unit_id: ObjectId,
        victim_id: ObjectId,
        max_turn_rad: f32,
    ) -> AttackAimResult {
        // Snapshot victim position + liveness.
        let victim_pos = {
            let Some(v) = self.objects.get(&victim_id) else {
                return AttackAimResult::Failure;
            };
            if !v.is_alive() || v.status.destroyed {
                return AttackAimResult::Failure;
            }
            v.get_position()
        };

        // Ensure turret tracks the same victim (C++ setTurretTargetObject).
        self.set_turret_target_object(unit_id, Some(victim_id), false);

        let (body_aimed, range_ok, turret_enabled) = {
            let Some(u) = self.objects.get_mut(&unit_id) else {
                return AttackAimResult::Failure;
            };
            if !u.is_alive() {
                return AttackAimResult::Failure;
            }
            if u.weapon.is_none() && u.secondary_weapon.is_none() {
                return AttackAimResult::Failure;
            }
            u.set_status_aiming_weapon(true);
            u.set_target(Some(victim_id));
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_attack(unit_id, victim_id);
            }
            let slot = u.active_weapon_slot;
            let body_aimed = u.turn_toward_position(victim_pos, slot, max_turn_rad.max(0.05));
            let us = u.get_position();
            let dx = victim_pos.x - us.x;
            let dz = victim_pos.z - us.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let range = u
                .weapon
                .as_ref()
                .map(|w| w.range)
                .or_else(|| u.secondary_weapon.as_ref().map(|w| w.range))
                .unwrap_or(0.0)
                * u.battle_plan_range_multiplier();
            let range_ok = dist <= range + 1e-3;
            (body_aimed, range_ok, u.turret_enabled)
        };

        // C++ body-aim path: if no turret turn rate, body alignment is enough.
        // When turret enabled, both body (or immobile) and turret must align.
        let turret_res = if turret_enabled {
            self.tick_turret_aim(unit_id, 1.0)
        } else {
            AttackAimResult::Success
        };

        let turret_ok = matches!(turret_res, AttackAimResult::Success);
        if body_aimed && turret_ok {
            return AttackAimResult::Success;
        }
        if !range_ok && !body_aimed {
            return AttackAimResult::Failure;
        }
        // Still turning body or turret.
        if matches!(turret_res, AttackAimResult::Failure) && !body_aimed {
            return AttackAimResult::Failure;
        }
        AttackAimResult::Continue
    }

    pub fn attack_fire_weapon_enter(&mut self, unit_id: ObjectId) -> bool {
        {
            let Some(u) = self.objects.get_mut(&unit_id) else {
                return false;
            };
            if !u.is_alive() {
                return false;
            }
            u.set_status_firing_weapon(true);
            u.set_status_attacking(true);
            u.set_ai_state(AIState::Attacking);
        }
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 2);
            // Attacking
        }
        true
    }

    /// C++ AIAttackFireWeaponState::onExit residual — clear IS_FIRING_WEAPON.
    pub fn attack_fire_weapon_exit(&mut self, unit_id: ObjectId) {
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.status.is_firing_weapon = false;
        }
    }

    /// C++ AIAttackFireWeaponState::update residual — fire once at victim.
    ///
    /// Object path only (ground attack uses fire_at with a dummy/position path later).
    pub fn attack_fire_weapon_update(
        &mut self,
        unit_id: ObjectId,
        victim_id: ObjectId,
        current_time: f32,
    ) -> AttackFireResult {
        // Snapshot checks without holding mut.
        let (alive, can_fire_now, pre_continue, in_range) = {
            let Some(atk) = self.objects.get(&unit_id) else {
                return AttackFireResult::Failure;
            };
            if !atk.is_alive() {
                return AttackFireResult::Failure;
            }
            let Some(vic) = self.objects.get(&victim_id) else {
                return AttackFireResult::Failure;
            };
            if !vic.is_alive() || vic.status.destroyed {
                return AttackFireResult::Failure;
            }
            let pre_continue = atk.pre_attack_target == Some(victim_id)
                && atk.pre_attack_ready_at > current_time + 1e-6;
            let can = atk.can_fire(current_time);
            let range_ok = atk.is_within_attack_range(vic);
            (true, can, pre_continue, range_ok)
        };
        let _ = alive;
        if pre_continue {
            return AttackFireResult::Continue;
        }
        if !can_fire_now {
            // Try fire_at anyway — it may arm pre-attack.
        }
        if !in_range && !pre_continue {
            // Still allow fire_at to arm pre-attack if can_fire and target set.
            // Out of range without wind-up => failure.
            let Some(atk) = self.objects.get(&unit_id) else {
                return AttackFireResult::Failure;
            };
            if atk.pre_attack_ready_at <= 0.0 {
                return AttackFireResult::Failure;
            }
        }

        let (victim_infantry, victim_faerie) = self
            .objects
            .get(&victim_id)
            .map(|v| (v.is_kind_of(KindOf::Infantry), v.is_faerie_fire()))
            .unwrap_or((false, false));
        let fired = {
            let Some(u) = self.objects.get_mut(&unit_id) else {
                return AttackFireResult::Failure;
            };
            u.set_status_firing_weapon(true);
            u.set_target(Some(victim_id));
            u.set_ai_state(AIState::Attacking);
            u.set_status_attacking(true);
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_attack(unit_id, victim_id);
                crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 2);
            }
            // TARGET_FAERIE_FIRE ROF residual when victim is painted.
            u.fire_at_ex(victim_id, current_time, victim_infantry, victim_faerie)
        };

        if fired {
            // max_shots_to_fire decremented inside Object::fire_at_ex (Weapon::m_maxShotCount).
            AttackFireResult::Success
        } else {
            // fire_at false: either pre-attack armed or cannot fire.
            let Some(u) = self.objects.get(&unit_id) else {
                return AttackFireResult::Failure;
            };
            if u.pre_attack_ready_at > current_time + 1e-6 {
                AttackFireResult::Continue
            } else {
                AttackFireResult::Failure
            }
        }
    }

    pub fn attack_can_fire_at(
        &self,
        attacker_id: ObjectId,
        victim_id: ObjectId,
        current_time: f32,
        require_los: bool,
    ) -> bool {
        let Some(atk) = self.objects.get(&attacker_id) else {
            return false;
        };
        let Some(vic) = self.objects.get(&victim_id) else {
            return false;
        };
        if !atk.is_alive() || !vic.is_alive() {
            return false;
        }
        if !atk.can_fire(current_time) {
            return false;
        }
        if !atk.has_max_shots_remaining() {
            return false;
        }
        if !atk.is_within_attack_range(vic) {
            return false;
        }
        if require_los {
            let from = atk.get_position();
            let to = vic.get_position();
            if self.pathfinding_system.is_attack_view_blocked(from, to) {
                return false;
            }
            let eye_a = atk.selection_radius.max(5.0) * 0.5;
            let eye_b = vic.selection_radius.max(5.0) * 0.5;
            let a = glam::Vec3::new(from.x, from.y + eye_a, from.z);
            let b = glam::Vec3::new(to.x, to.y + eye_b, to.z);
            if !self.is_clear_line_of_sight_terrain(a, b) {
                return false;
            }
        }
        true
    }

    /// C++ AIUpdateInterface::privateMoveToPosition residual.
    pub fn private_move_to_position(&mut self, unit_id: ObjectId, pos: glam::Vec3) -> bool {
        let Some(u) = self.objects.get(&unit_id) else {
            return false;
        };
        if !u.can_move() || !u.is_alive() {
            return false;
        }
        let was_idle = matches!(u.ai_state, AIState::Idle);
        // Clear blocked residual on new move order.
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.is_blocked = false;
            u.is_blocked_and_stuck = false;
            u.num_frames_blocked = 0;
            if !was_idle {
                // C++ temporary AI_MOVE_TO for 20 seconds when non-idle AI command.
                u.temporary_move_frames = 20 * 30;
            }
        }
        self.request_object_path(unit_id, pos)
    }

    /// C++ AIUpdateInterface::privateStop residual.
    pub fn private_stop(&mut self, unit_id: ObjectId) -> bool {
        let Some(u) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        u.stop_moving();
        u.set_status_attacking(false);
        // Always clear host engagement same-frame (player Stop / privateStop residual).
        // Decision authority still logs so GameWorld can last-write idle/stop.
        u.target = None;
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_stop_attack(unit_id);
        }
        true
    }

    /// C++ AIAttackApproachTargetState::computePath residual.
    ///
    /// Returns true if approach path is valid/in-progress (stay in approach).
    /// Returns false if should leave approach (no weapon / should pursue).
    pub fn attack_approach_compute_path(
        &mut self,
        unit_id: ObjectId,
        victim_id: Option<ObjectId>,
        fixed_pos: Option<glam::Vec3>,
    ) -> bool {
        let frame = self.frame;
        let (mobile, stuck, waiting, path_empty, approach_ts, prev_vic, has_weapon, from) = {
            let Some(u) = self.objects.get(&unit_id) else {
                return false;
            };
            if !u.can_move() {
                return false;
            }
            (
                true,
                u.is_blocked_and_stuck,
                u.waiting_for_path,
                u.movement.path.is_empty(),
                u.approach_timestamp,
                u.prev_victim_pos,
                u.weapon.is_some() || u.secondary_weapon.is_some(),
                u.get_position(),
            )
        };
        let _ = mobile;

        if waiting {
            return true;
        }

        let mut force_repath = stuck;
        if !force_repath && path_empty && !waiting {
            force_repath = true;
        }
        if !force_repath
            && frame.saturating_sub(approach_ts) < crate::game_logic::MIN_RECOMPUTE_TIME_RESIDUAL
        {
            return true;
        }

        if let Some(vid) = victim_id {
            let Some(victim) = self.objects.get(&vid) else {
                return false;
            };
            if !victim.is_alive() {
                return false;
            }
            if !has_weapon {
                return false;
            }
            let vic_pos = victim.get_position();
            // Center position residual (geometry center ≈ position for host).
            let center = vic_pos;
            if !force_repath {
                if let Some(prev) = prev_vic {
                    if crate::game_logic::is_same_position_residual(from, prev, center) {
                        return true;
                    }
                }
            }
            if let Some(u) = self.objects.get_mut(&unit_id) {
                u.prev_victim_pos = Some(center);
                u.approach_timestamp = frame;
            }
            // Contact weapon: ignore obstacle + path into target residual handled by assign.
            let _ = self.request_attack_path(unit_id, Some(vid), center);
            true
        } else if let Some(pos) = fixed_pos {
            if !force_repath {
                return true; // fixed positions don't move
            }
            if let Some(u) = self.objects.get_mut(&unit_id) {
                u.approach_timestamp = frame;
            }
            let _ = self.request_attack_path(unit_id, None, pos);
            true
        } else {
            false
        }
    }

    /// C++ AIUpdateInterface::requestAttackPath residual.
    ///
    /// Rate-limits repath (<3 frames → queue 2s). On accept, runs findAttackPath.
    pub fn request_attack_path(
        &mut self,
        unit_id: ObjectId,
        victim_id: Option<ObjectId>,
        victim_pos: glam::Vec3,
    ) -> bool {
        let frame = self.frame;
        let can_compute = {
            let Some(u) = self.objects.get_mut(&unit_id) else {
                return false;
            };
            if !u.can_move() || !u.is_alive() {
                return false;
            }
            u.begin_request_attack_path(victim_id, victim_pos, frame)
        };
        if !can_compute {
            return false; // deferred
        }
        let ok = self.assign_unit_attack_path(unit_id, victim_id, victim_pos);
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.waiting_for_path = false;
            if !ok {
                u.is_attack_path = false;
            }
        }
        ok
    }

    /// C++ AIUpdateInterface::privateAttackObject residual.
    pub fn private_attack_object(
        &mut self,
        unit_id: ObjectId,
        victim_id: ObjectId,
        max_shots: i32,
    ) -> bool {
        let victim_pos = match self.objects.get(&victim_id) {
            Some(v) if v.is_alive() => v.get_position(),
            _ => return false,
        };
        if let Some(u) = self.objects.get_mut(&unit_id) {
            u.set_max_shots_to_fire(max_shots);
        } else {
            return false;
        }
        // C++ AIAttackState::onEnter residual via nested AttackStateMachine.
        if self.attack_state_enter(unit_id, victim_id) == AttackMachineResult::Failure {
            return false;
        }
        // Prefer attack path if out of range / LOS blocked residual handled by assign.
        let _ = self.request_attack_path(unit_id, Some(victim_id), victim_pos);
        true
    }

    #[cfg(test)]
    pub fn private_attack_object_for_test(
        &mut self,
        unit_id: ObjectId,
        victim_id: ObjectId,
        max_shots: i32,
    ) -> bool {
        self.private_attack_object(unit_id, victim_id, max_shots)
    }

    /// C++ AIUpdateInterface::requestPath residual.
    ///
    /// Uses host PathfindingSystem A* when available; fail-closed straight path.
    pub fn request_object_path(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        let Some(obj) = self.objects.get(&id) else {
            return false;
        };
        if obj.status.destroyed || !obj.is_alive() {
            return false;
        }
        let start = obj.get_position();
        // Snapshot objects for pathfinder dynamic obstacles.
        let waypoints = self
            .pathfinding_system
            .find_path(start, destination, &self.objects)
            .filter(|p| p.len() >= 2)
            .unwrap_or_else(|| vec![destination]);
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.request_path(destination, Some(waypoints));
            true
        } else {
            false
        }
    }

    pub(crate) fn tick_physics_collisions_all(&mut self) -> u32 {
        // Per-frame blocked bookkeeping residual (before new collide pairs).
        // Snapshot ground heights before mut pass (terrain borrow).
        // Only mobile / physics-active bodies need terrain samples + motion step —
        // sampling every structure on Lone Eagle (~900 objs) dominated host frames.
        let ground_heights: Vec<(ObjectId, f32)> = {
            let mut out = Vec::new();
            for (id, o) in self.objects.iter() {
                if o.status.destroyed || !o.is_alive() {
                    continue;
                }
                // Structures/immobile skip full physics motion residual.
                if !(o.can_move()
                    || o.shock_stun_frames > 0
                    || o.bounce_audio_pending > 0
                    || o.movement.velocity.length_squared() > 1e-6)
                {
                    continue;
                }
                let p = o.get_position();
                let g = self
                    .terrain_height_at(glam::Vec3::new(p.x, 0.0, p.z))
                    .unwrap_or(0.0);
                out.push((*id, g));
            }
            out
        };
        for o in self.objects.values_mut() {
            if o.status.destroyed || !o.is_alive() {
                continue;
            }
            o.clear_blocked_frame_state();
            o.tick_move_away_state();
            o.tick_path_queue();
            // C++ PhysicsBehavior update residual order: friction → integrate accel → motion.
            if o.can_move() {
                o.apply_frictional_forces();
            }
            // Immobile structures still clear accel residual cheaply.
            o.integrate_physics_accel();
        }
        for (id, ground_y) in ground_heights {
            if let Some(o) = self.objects.get_mut(&id) {
                let _ = o.tick_physics_motion_step(ground_y);
            }
        }
        // Rebuild partition cells (C++ registerObject residual each update).
        // Keep FOW reveal residual; only re-register live objects.
        self.partition_manager.clear_registered_objects();
        // O(1) id → pose/radius for pair resolution (was linear find per neighbor).
        let mut entry_by_id: std::collections::HashMap<u32, (glam::Vec3, f32)> =
            std::collections::HashMap::new();
        let mut mobile_ids: Vec<ObjectId> = Vec::new();
        for (id, o) in self.objects.iter() {
            if !o.is_alive() || o.status.destroyed {
                continue;
            }
            let pos = o.get_position();
            let r = o.selection_radius.max(1.0);
            self.partition_manager
                .register_object_at(id.0, pos.x, pos.z);
            entry_by_id.insert(id.0, (pos, r));
            // Only mobile bodies initiate collide queries (structures stay as
            // partition obstacles via neighbor lookup).
            if o.can_move() {
                mobile_ids.push(*id);
            }
        }
        mobile_ids.sort_by_key(|id| id.0);

        let mut handled = 0u32;
        let mut seen_pairs: std::collections::HashSet<(u32, u32)> =
            std::collections::HashSet::new();
        for a_id in &mobile_ids {
            let Some((a_pos, a_r)) = entry_by_id.get(&a_id.0).copied() else {
                continue;
            };
            let neighbors = self.partition_manager.neighbor_object_ids(a_pos.x, a_pos.z);
            for b_raw in neighbors {
                if b_raw == a_id.0 {
                    continue;
                }
                let lo = a_id.0.min(b_raw);
                let hi = a_id.0.max(b_raw);
                if !seen_pairs.insert((lo, hi)) {
                    continue;
                }
                let Some((b_pos, b_r)) = entry_by_id.get(&b_raw).copied() else {
                    continue;
                };
                let dx = a_pos.x - b_pos.x;
                let dz = a_pos.z - b_pos.z;
                let sum = a_r + b_r;
                if dx * dx + dz * dz > sum * sum {
                    continue;
                }
                let b_id = ObjectId(b_raw);
                if self.try_physics_collide(*a_id, b_id, a_r) {
                    handled = handled.saturating_add(1);
                } else if self.try_physics_collide(b_id, *a_id, b_r) {
                    handled = handled.saturating_add(1);
                }
            }
        }
        // C++ previousOverlap = currentOverlap end of physics update residual.
        for id in mobile_ids {
            if let Some(o) = self.objects.get_mut(&id) {
                o.advance_physics_overlap_frame();
            }
        }
        // Stun/bounce residual on non-mobile still advances overlap bookkeeping.
        let extra: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive()
                    && !o.status.destroyed
                    && !o.can_move()
                    && (o.shock_stun_frames > 0 || o.bounce_audio_pending > 0)
            })
            .map(|(id, _)| *id)
            .collect();
        for id in extra {
            if let Some(o) = self.objects.get_mut(&id) {
                o.advance_physics_overlap_frame();
            }
        }
        handled
    }

    pub(crate) fn tick_shock_stun_all(&mut self) {
        // Include bounce_audio_pending so land audio drains even after stun ends.
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| o.shock_stun_frames > 0 || o.bounce_audio_pending > 0)
            .map(|(id, _)| *id)
            .collect();
        let mut bounce_audio: Vec<(ObjectId, String, glam::Vec3, f32)> = Vec::new();
        for id in ids {
            let pos = self
                .objects
                .get(&id)
                .map(|o| o.get_position())
                .unwrap_or(glam::Vec3::ZERO);
            let (cliff, water) = self.sample_stun_surface_at(pos);
            if let Some(o) = self.objects.get_mut(&id) {
                o.cell_is_cliff = cliff;
                o.cell_is_underwater = water;
                if o.shock_stun_frames > 0 {
                    // Wave 764: under coupled shadow, GW sole-decrements frames;
                    // host keeps tumble/bounce physics only.
                    if crate::gameworld_shadow::gameworld_shadow_enabled()
                        && crate::gameworld_shadow::shadow_coupled_tick_active()
                    {
                        o.tick_shock_stun_physics_only();
                    } else {
                        o.tick_shock_stun();
                    }
                }
                while let Some((name, vol)) = o.take_bounce_audio_pending() {
                    let p = o.get_position();
                    bounce_audio.push((id, name, p, vol));
                }
            }
        }
        // C++ TheAudio->addAudioEvent bounce residual.
        for (id, name, pos, vol) in bounce_audio {
            let pri = (64.0 + vol * 136.0).clamp(0.0, 255.0) as u8;
            self.queue_audio_event(
                AudioEventRequest::new(&name)
                    .with_object(id)
                    .with_position(pos)
                    .with_priority(pri),
            );
        }
    }

    pub(crate) fn apply_shock_wave_at_impact(
        &mut self,
        impact: glam::Vec3,
        weapon_name: Option<&str>,
        skip_id: Option<ObjectId>,
    ) -> u32 {
        use crate::game_logic::weapon_bootstrap::{
            compute_shock_wave_force, host_shock_wave_amount_for_weapon_name,
            host_shock_wave_radius_for_weapon_name, host_shock_wave_taper_for_weapon_name,
        };
        let Some(name) = weapon_name else {
            return 0;
        };
        let amount = host_shock_wave_amount_for_weapon_name(name);
        let radius = host_shock_wave_radius_for_weapon_name(name);
        let taper = host_shock_wave_taper_for_weapon_name(name);
        if amount <= 0.0 || radius <= 0.0 {
            return 0;
        }
        let r2 = radius * radius;
        let ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if skip_id == Some(*id) {
                    return None;
                }
                if !obj.is_alive() {
                    return None;
                }
                let p = obj.get_position();
                let dx = p.x - impact.x;
                let dz = p.z - impact.z;
                if dx * dx + dz * dz > r2 {
                    return None;
                }
                Some(*id)
            })
            .collect();
        let mut n = 0u32;
        for id in ids {
            let pos = match self.objects.get(&id) {
                Some(o) => o.get_position(),
                None => continue,
            };
            let Some(force) = compute_shock_wave_force(impact, pos, amount, radius, taper) else {
                continue;
            };
            if let Some(o) = self.objects.get_mut(&id) {
                if o.apply_shock_wave_impulse(force) {
                    n = n.saturating_add(1);
                }
            }
        }
        n
    }

    pub(crate) fn apply_instant_hit_splash_at(
        &mut self,
        impact: glam::Vec3,
        primary_damage: f32,
        secondary_damage: f32,
        primary_radius: f32,
        secondary_radius: f32,
        attacker_id: ObjectId,
        attacker_team: Team,
        intended_id: ObjectId,
        weapon_name: Option<&str>,
    ) -> u32 {
        let max_r = primary_radius.max(secondary_radius);
        if max_r <= 0.0 || (primary_damage <= 0.0 && secondary_damage <= 0.0) {
            return 0;
        }
        use crate::game_logic::host_ai_path_combat_residual_wave105::{
            WEAPON_AFFECTS_ENEMIES, WEAPON_AFFECTS_NEUTRALS,
        };
        use crate::game_logic::weapon_bootstrap::{
            host_radius_damage_affects_for_weapon_name, radius_damage_affects_victim,
        };
        let affects = weapon_name
            .map(host_radius_damage_affects_for_weapon_name)
            .unwrap_or(WEAPON_AFFECTS_ENEMIES | WEAPON_AFFECTS_NEUTRALS);
        let shooter_template = self
            .objects
            .get(&attacker_id)
            .map(|a| a.template_name.clone())
            .unwrap_or_default();
        let primary_sq = primary_radius * primary_radius;
        let secondary_sq = max_r * max_r;
        let candidates: Vec<(ObjectId, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if *id == intended_id {
                    return None;
                }
                if !obj.is_alive() {
                    return None;
                }
                if obj.is_eject_invulnerable() {
                    return None;
                }
                let airborne = obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target;
                let same_tmpl = !shooter_template.is_empty()
                    && obj.template_name.eq_ignore_ascii_case(&shooter_template);
                if !radius_damage_affects_victim(
                    affects,
                    attacker_team,
                    attacker_id,
                    *id,
                    obj.team,
                    airborne,
                    same_tmpl,
                ) {
                    return None;
                }
                let p = obj.get_position();
                let dx = p.x - impact.x;
                let dz = p.z - impact.z;
                let d2 = dx * dx + dz * dz;
                if d2 > secondary_sq {
                    return None;
                }
                Some((*id, d2))
            })
            .collect();
        let mut hits = 0u32;
        let mut destroy: Vec<ObjectId> = Vec::new();
        for (id, d2) in candidates {
            let dmg = if primary_radius > 0.0 && d2 <= primary_sq {
                primary_damage
            } else if secondary_damage > 0.0 {
                secondary_damage
            } else {
                0.0
            };
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let dead = obj.take_damage_from(dmg, Some(attacker_id));
                hits = hits.saturating_add(1);
                if dead {
                    destroy.push(id);
                }
            }
        }
        for id in destroy {
            self.mark_object_for_destruction(id, Some(attacker_team));
        }
        let _ = self.apply_shock_wave_at_impact(impact, weapon_name, None);
        hits
    }

    pub(crate) fn apply_scatter_miss_splash_at(
        &mut self,
        impact: glam::Vec3,
        weapon_damage: f32,
        splash_radius: f32,
        attacker_id: ObjectId,
        attacker_team: Team,
        skip_id: ObjectId,
        weapon_name: Option<&str>,
    ) -> u32 {
        if weapon_damage <= 0.0 || splash_radius <= 0.0 {
            return 0;
        }
        use crate::game_logic::weapon_bootstrap::{
            host_radius_damage_affects_for_weapon_name, radius_damage_affects_victim,
        };
        // Default residual when name unknown: ENEMIES|NEUTRALS.
        let affects = weapon_name
            .map(host_radius_damage_affects_for_weapon_name)
            .unwrap_or_else(|| {
                use crate::game_logic::host_ai_path_combat_residual_wave105::{
                    WEAPON_AFFECTS_ENEMIES, WEAPON_AFFECTS_NEUTRALS,
                };
                WEAPON_AFFECTS_ENEMIES | WEAPON_AFFECTS_NEUTRALS
            });
        let shooter_template = self
            .objects
            .get(&attacker_id)
            .map(|a| a.template_name.clone())
            .unwrap_or_default();
        let primary_r = splash_radius;
        let secondary_r = splash_radius * 1.5; // outer residual taper ring
        let primary_sq = primary_r * primary_r;
        let secondary_sq = secondary_r * secondary_r;
        let candidates: Vec<(ObjectId, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if *id == skip_id {
                    return None;
                }
                if !obj.is_alive() {
                    return None;
                }
                if obj.is_eject_invulnerable() {
                    return None;
                }
                let airborne = obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target;
                let same_tmpl = !shooter_template.is_empty()
                    && obj.template_name.eq_ignore_ascii_case(&shooter_template);
                if !radius_damage_affects_victim(
                    affects,
                    attacker_team,
                    attacker_id,
                    *id,
                    obj.team,
                    airborne,
                    same_tmpl,
                ) {
                    return None;
                }
                let p = obj.get_position();
                let dx = p.x - impact.x;
                let dz = p.z - impact.z;
                let d2 = dx * dx + dz * dz;
                if d2 > secondary_sq {
                    return None;
                }
                Some((*id, d2.sqrt()))
            })
            .collect();
        let mut hits = 0u32;
        let mut destroy: Vec<ObjectId> = Vec::new();
        for (id, dist) in candidates {
            let dmg = if dist * dist <= primary_sq {
                weapon_damage
            } else {
                weapon_damage * 0.5
            };
            if dmg <= 0.0 {
                continue;
            }
            if let Some(obj) = self.objects.get_mut(&id) {
                let dead = obj.take_damage_from(dmg, Some(attacker_id));
                hits = hits.saturating_add(1);
                if dead {
                    destroy.push(id);
                }
            }
        }
        for id in destroy {
            self.mark_object_for_destruction(id, Some(attacker_team));
        }
        let _ = self.apply_shock_wave_at_impact(impact, weapon_name, Some(skip_id));
        hits
    }

    pub(crate) fn instant_scatter_misses_shot(
        &self,
        attacker_id: ObjectId,
        target_id: ObjectId,
        slot: u8,
    ) -> bool {
        use crate::game_logic::weapon_bootstrap::{
            host_effective_scatter_radius, scatter_misses_intended_target, scatter_seed_for_shot,
            DEFAULT_SCATTER_HIT_RADIUS,
        };
        let (wname, tgt_inf, hit_r) = {
            let attacker = match self.objects.get(&attacker_id) {
                Some(a) => a,
                None => return false,
            };
            let target = match self.objects.get(&target_id) {
                Some(t) => t,
                None => return false,
            };
            let wname = if slot == 1 {
                attacker
                    .thing
                    .template
                    .secondary_weapon_name
                    .as_deref()
                    .or(attacker.thing.template.primary_weapon_name.as_deref())
            } else {
                attacker.thing.template.primary_weapon_name.as_deref()
            };
            let hit_r = if target.selection_radius > 0.0 {
                target.selection_radius
            } else {
                DEFAULT_SCATTER_HIT_RADIUS
            };
            (
                wname.map(|s| s.to_string()),
                target.is_kind_of(KindOf::Infantry),
                hit_r,
            )
        };
        let Some(name) = wname else {
            return false;
        };
        let scatter = host_effective_scatter_radius(&name, tgt_inf);
        if scatter <= 0.0 {
            return false;
        }
        let seed = scatter_seed_for_shot(attacker_id.0, target_id.0, self.frame);
        scatter_misses_intended_target(scatter, seed, hit_r)
    }

    pub(crate) fn try_min_range_backup(
        &mut self,
        attacker_id: ObjectId,
        target_pos: glam::Vec3,
        min_range: f32,
    ) -> bool {
        use crate::game_logic::weapon_bootstrap::{
            compute_min_range_backup_pos, is_inside_minimum_attack_range,
        };
        let Some(attacker) = self.objects.get(&attacker_id) else {
            return false;
        };
        if !attacker.can_move() || !attacker.is_alive() {
            return false;
        }
        let src = attacker.get_position();
        let dx = src.x - target_pos.x;
        let dz = src.z - target_pos.z;
        let dist = (dx * dx + dz * dz).sqrt();
        if !is_inside_minimum_attack_range(dist, min_range) {
            return false;
        }
        let dest = compute_min_range_backup_pos(src, target_pos, min_range);
        // Direct backup residual (fail-closed vs full reverse-pathfind matrix).
        if let Some(a) = self.objects.get_mut(&attacker_id) {
            a.movement.path.clear();
            a.movement.current_path_index = 0;
            a.record_host_movement();
            a.movement.target_position = Some(dest);
            a.set_ai_state(AIState::Attacking);
            a.set_status_attacking(true);
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_set_state(attacker_id, 2);
                // Attacking
            }
            a.set_status_moving(true);
            crate::game_logic::host_move_log::record(attacker_id, Some([dest.x, dest.y, dest.z]));
            return true;
        }
        false
    }

    pub(crate) fn approach_pos_for_attack(
        &self,
        attacker_id: ObjectId,
        target_pos: glam::Vec3,
        weapon_range: f32,
        weapon_name: Option<&str>,
    ) -> glam::Vec3 {
        let contact = weapon_name
            .map(crate::game_logic::weapon_bootstrap::host_is_contact_weapon_name)
            .unwrap_or(false)
            || crate::game_logic::weapon_bootstrap::is_contact_weapon_range(weapon_range);
        if contact {
            return target_pos;
        }
        let src = self
            .objects
            .get(&attacker_id)
            .map(|o| o.get_position())
            .unwrap_or(target_pos);
        crate::game_logic::weapon_bootstrap::compute_approach_target_pos(
            src,
            target_pos,
            weapon_range,
        )
    }

    pub(crate) fn try_continue_attack_after_kill(
        &mut self,
        attacker_id: ObjectId,
        dead_victim_id: ObjectId,
        original_victim_pos: glam::Vec3,
        continue_range: f32,
        victim_team: Team,
    ) -> bool {
        if continue_range <= 0.0 {
            return false;
        }
        // Pure residual acquire: nearest same-team victim near kill position (XZ).
        let candidates: Vec<_> = self
            .objects
            .iter()
            .filter_map(|(&id, obj)| {
                if id == attacker_id || id == dead_victim_id || !obj.is_alive() {
                    return None;
                }
                if obj.team != victim_team {
                    return None;
                }
                Some(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id,
                        team: obj.team,
                        position: obj.get_position(),
                        is_alive: true,
                        is_neutral: obj.team == Team::Neutral,
                        under_construction: obj.status.under_construction,
                        combat_kind: true,
                        effectively_stealthed: obj.is_effectively_stealthed(),
                        is_air: obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target,
                        eject_invulnerable: obj.is_eject_invulnerable(),
                    },
                )
            })
            .collect();
        let Some((next_id, _, _)) =
            crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
                Some(attacker_id),
                (original_victim_pos.x, original_victim_pos.z),
                candidates,
                continue_range,
                |_| true,
            )
        else {
            return false;
        };
        // Continue-attack residual: under AI decision authority, log AttackTarget
        // (+ Attacking state) for GameWorld apply/writeback; host stays clean.
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_attack(attacker_id, next_id);
            crate::game_logic::host_ai_decision_log::record_set_state(attacker_id, 2); // Attacking
            return true;
        }
        if let Some(attacker) = self.objects.get_mut(&attacker_id) {
            attacker.target = Some(next_id);
            attacker.set_ai_state(AIState::Attacking);
            attacker.set_status_attacking(true);
            true
        } else {
            false
        }
    }

    #[cfg(test)]
    pub fn try_continue_attack_after_kill_for_test(
        &mut self,
        attacker_id: ObjectId,
        dead_victim_id: ObjectId,
        original_victim_pos: glam::Vec3,
        continue_range: f32,
        victim_team: Team,
    ) -> bool {
        self.try_continue_attack_after_kill(
            attacker_id,
            dead_victim_id,
            original_victim_pos,
            continue_range,
            victim_team,
        )
    }

    /// After a combat kill: grant XP, ContinueAttackRange retarget, else stop.
    pub(crate) fn continue_or_stop_after_kill(
        &mut self,
        attacker_id: ObjectId,
        dead_victim_id: ObjectId,
        original_victim_pos: glam::Vec3,
        victim_team: Team,
        weapon_name: Option<&str>,
        kill_xp: f32,
    ) {
        if let Some(attacker) = self.objects.get_mut(&attacker_id) {
            if kill_xp > 0.0 {
                attacker.gain_experience(kill_xp);
            }
        }
        let cont = weapon_name
            .map(crate::game_logic::weapon_bootstrap::host_continue_attack_range_for_weapon_name)
            .unwrap_or(0.0);
        if self.try_continue_attack_after_kill(
            attacker_id,
            dead_victim_id,
            original_victim_pos,
            cont,
            victim_team,
        ) {
            return;
        }
        self.stop_attack_decision_aware(attacker_id);
    }

    /// Ensure runway slot vector sized for airfield (HasRunways residual).
    pub(crate) fn airfield_runway_slots_mut(
        &mut self,
        airfield_id: ObjectId,
    ) -> &mut Vec<Option<ObjectId>> {
        use crate::game_logic::host_dock_contain_exit_heal_residual::{
            airfield_runway_count, PARKING_PLACE_AIRFIELD_HAS_RUNWAYS,
            PARKING_PLACE_AIRFIELD_NUM_COLS,
        };
        let n = airfield_runway_count(
            PARKING_PLACE_AIRFIELD_HAS_RUNWAYS,
            PARKING_PLACE_AIRFIELD_NUM_COLS,
        );
        let slots = self
            .runway_reservations
            .entry(airfield_id)
            .or_insert_with(|| vec![None; n.max(1)]);
        if slots.len() < n {
            slots.resize(n, None);
        }
        slots
    }

    /// C++ ParkingPlaceBehavior::transferRunwayReservationToNext / reserve residual.
    ///
    /// Returns runway index when a free runway is reserved for `jet_id`.
    pub(crate) fn reserve_airfield_runway(
        &mut self,
        airfield_id: ObjectId,
        jet_id: ObjectId,
    ) -> Option<usize> {
        // Already holding a runway?
        if let Some(slots) = self.runway_reservations.get(&airfield_id) {
            if let Some(idx) = slots.iter().position(|s| *s == Some(jet_id)) {
                return Some(idx);
            }
        }
        let slots = self.airfield_runway_slots_mut(airfield_id);
        if let Some(idx) = slots.iter().position(|s| s.is_none()) {
            slots[idx] = Some(jet_id);
            return Some(idx);
        }
        None
    }

    /// Release any runway held by this jet (all airfields).
    pub(crate) fn release_airfield_runway_for_jet(&mut self, jet_id: ObjectId) {
        for slots in self.runway_reservations.values_mut() {
            for s in slots.iter_mut() {
                if *s == Some(jet_id) {
                    *s = None;
                }
            }
        }
    }

    /// Count reserved runways at airfield.
    pub(crate) fn airfield_runway_reserved_count(&self, airfield_id: ObjectId) -> usize {
        self.runway_reservations
            .get(&airfield_id)
            .map(|s| s.iter().filter(|x| x.is_some()).count())
            .unwrap_or(0)
    }

    /// C++ JetAIUpdate runway takeoff residual: reserve runway, taxi to prep, then climb.
    ///
    /// Returns false when HasRunways and all runways are busy (jet stays docked).
    pub(crate) fn try_runway_takeoff_from_airfield(&mut self, jet_id: ObjectId) -> bool {
        use crate::game_logic::host_dock_contain_exit_heal_residual::{
            PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT, PARKING_PLACE_AIRFIELD_HAS_RUNWAYS,
            PARKING_PLACE_RUNWAY_PREP_SPACING,
        };
        let (af_id, team) = {
            let Some(jet) = self.objects.get(&jet_id) else {
                return false;
            };
            if !(jet.is_kind_of(KindOf::Aircraft) || jet.object_type == ObjectType::Aircraft) {
                return false;
            }
            let parked = jet.is_parked_at_airfield() || jet.contained_by.is_some();
            if !parked {
                // Already free — still clear any stale reservation.
                self.release_airfield_runway_for_jet(jet_id);
                return true;
            }
            let af = jet.contained_by.or_else(|| {
                // hangar list residual
                None
            });
            (af, jet.team)
        };
        let Some(af_id) = af_id else {
            // No airfield link — fall through to legacy takeoff.
            let _ = self.release_jet_from_airfield_parking(jet_id);
            return true;
        };
        if !PARKING_PLACE_AIRFIELD_HAS_RUNWAYS {
            let _ = self.release_jet_from_airfield_parking(jet_id);
            return true;
        }
        let Some(runway_idx) = self.reserve_airfield_runway(af_id, jet_id) else {
            // All runways busy — remain docked this frame.
            return false;
        };
        // Taxi residual: offset prep position by runway index, then climb.
        if let Some(af) = self.objects.get(&af_id).map(|o| o.get_position()) {
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                let mut prep = af;
                // Simplified two-runway layout along X.
                prep.x += (runway_idx as f32 - 0.5) * PARKING_PLACE_RUNWAY_PREP_SPACING;
                prep.y = PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT;
                // Host-immediate taxi prep snap; log for GameWorld pose last-write.
                jet.set_position(prep);
                if crate::gameworld_shadow::gameworld_movement_authority_live() {
                    crate::game_logic::host_move_log::record(
                        jet_id,
                        Some([prep.x, prep.y, prep.z]),
                    );
                    jet.record_host_movement();
                }
            }
        }
        let _ = team;
        let ok = self.release_jet_from_airfield_parking(jet_id);
        // Keep runway reserved until clear tick releases it.
        if !ok {
            // If release failed, free runway.
            self.release_airfield_runway_for_jet(jet_id);
            return false;
        }
        // Re-assert reservation after release (release shouldn't clear runway).
        let _ = self.reserve_airfield_runway(af_id, jet_id);
        true
    }

    /// Release runway reservations once jets are clear of the airfield.
    pub(crate) fn tick_airfield_runway_clear(&mut self) {
        use crate::game_logic::host_dock_contain_exit_heal_residual::PARKING_PLACE_RUNWAY_CLEAR_DIST;
        let clear_sq = PARKING_PLACE_RUNWAY_CLEAR_DIST * PARKING_PLACE_RUNWAY_CLEAR_DIST;
        let mut to_clear: Vec<(ObjectId, usize)> = Vec::new();
        let airfields: Vec<ObjectId> = self.runway_reservations.keys().copied().collect();
        for af_id in airfields {
            let af_pos = match self.objects.get(&af_id) {
                Some(o) if o.is_alive() => o.get_position(),
                _ => {
                    // Airfield gone — drop all.
                    self.runway_reservations.remove(&af_id);
                    continue;
                }
            };
            let Some(slots) = self.runway_reservations.get(&af_id) else {
                continue;
            };
            for (idx, holder) in slots.iter().enumerate() {
                let Some(jet_id) = *holder else {
                    continue;
                };
                let clear = match self.objects.get(&jet_id) {
                    None => true,
                    Some(jet) if !jet.is_alive() => true,
                    Some(jet) if jet.contained_by.is_some() => true, // re-docked
                    Some(jet) => {
                        let p = jet.get_position();
                        let dx = p.x - af_pos.x;
                        let dz = p.z - af_pos.z;
                        let d2 = dx * dx + dz * dz;
                        // Clear once airborne and away, or no longer attacking from pad.
                        jet.status.airborne_target && d2 >= clear_sq
                    }
                };
                if clear {
                    to_clear.push((af_id, idx));
                }
            }
        }
        for (af_id, idx) in to_clear {
            if let Some(slots) = self.runway_reservations.get_mut(&af_id) {
                if idx < slots.len() {
                    slots[idx] = None;
                }
            }
        }
    }

    /// C++ JetAIUpdate leave ParkingPlace residual (clear hangar slot + takeoff).
    pub(crate) fn release_jet_from_airfield_parking(&mut self, jet_id: ObjectId) -> bool {
        let (team, af_hint) = {
            let Some(jet) = self.objects.get(&jet_id) else {
                return false;
            };
            (jet.team, jet.contained_by)
        };
        let af_hint = af_hint.or_else(|| {
            self.objects.iter().find_map(|(id, o)| {
                if Self::is_friendly_airfield(o, team) && o.contained_units().contains(&jet_id) {
                    Some(*id)
                } else {
                    None
                }
            })
        });
        let took_off = {
            let Some(jet) = self.objects.get_mut(&jet_id) else {
                return false;
            };
            let was_parked = jet.is_parked_at_airfield() || jet.contained_by.is_some();
            let af = jet.takeoff_from_airfield_parking();
            jet.set_contained_by(None);
            was_parked || af.is_some()
        };
        let mut freed = false;
        if let Some(af_id) = af_hint {
            if let Some(af) = self.objects.get_mut(&af_id) {
                freed = af.remove_occupant(jet_id);
            }
            // C++ releaseSpace → setHoldDoorOpen(door, false) when slot empty.
            // Fail-closed: clear hold when airfield has no remaining parked jets.
            let still_parked = self.airfield_parked_count(af_id) > 0;
            if let Some(af) = self.objects.get_mut(&af_id) {
                if !still_parked {
                    af.set_production_door_hold_open(false, self.frame);
                }
            }
        }
        // Success if we launched, or cleaned a lingering parking slot, or jet is free.
        took_off || freed || af_hint.is_some()
    }

    pub(crate) fn try_return_to_base_rearm(&mut self, jet_id: ObjectId) -> bool {
        let (needs, jet_team, jet_pos) = {
            let Some(jet) = self.objects.get(&jet_id) else {
                return false;
            };
            if !jet.needs_return_to_base_rearm() {
                return false;
            }
            (true, jet.team, jet.get_position())
        };
        if !needs {
            return false;
        }
        // C++ JetAIUpdate final dock / rearm proximity residual.
        const REARM_RANGE: f32 = 120.0;
        // C++ seek-airfield path residual (map-wide).
        const RTB_SEEK_RANGE: f32 = 50_000.0;
        let candidates: Vec<_> = self
            .objects
            .values()
            .filter(|obj| obj.id != jet_id && Self::is_friendly_airfield(obj, jet_team))
            .map(
                |obj| crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                    id: obj.id,
                    team: obj.team,
                    position: obj.get_position(),
                    is_alive: obj.is_alive(),
                    is_neutral: obj.team == Team::Neutral,
                    under_construction: obj.status.under_construction,
                    combat_kind: true,
                    effectively_stealthed: false,
                    is_air: false,
                    eject_invulnerable: false,
                },
            )
            .collect();
        // Prefer in-range airfield for immediate dock; else nearest map-wide for RTB path.
        let in_range = crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
            jet_id,
            jet_team,
            jet_pos,
            candidates.iter().cloned(),
            |_| REARM_RANGE,
            |_| true,
        );
        let af_id = if let Some((id, _, _)) = in_range {
            id
        } else {
            let Some((id, _, _)) =
                crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
                    jet_id,
                    jet_team,
                    jet_pos,
                    candidates,
                    |_| RTB_SEEK_RANGE,
                    |_| true,
                )
            else {
                return false;
            };
            // C++ JetAIUpdate RETURN_TO_BASE path residual: fly toward airfield.
            let af_pos = self
                .objects
                .get(&id)
                .map(|o| o.get_position())
                .unwrap_or(jet_pos);
            if let Some(jet) = self.objects.get_mut(&jet_id) {
                jet.target = None;
                jet.set_status_attacking(false);
                if !matches!(jet.ai_state, AIState::Moving | AIState::Docked) {
                    jet.set_ai_state(AIState::Moving);
                }
            }
            let _ = self.assign_unit_path(jet_id, af_pos, &[]);
            // Not docked yet — suppress out-of-ammo damage while RTB en route.
            return true;
        };
        let af_pos = self
            .objects
            .get(&af_id)
            .map(|o| o.get_position())
            .unwrap_or(jet_pos);
        // Capacity residual: refuse dock if parking places full (unless already parked here).
        let already = self
            .objects
            .get(&jet_id)
            .map(|j| j.contained_by == Some(af_id))
            .unwrap_or(false);
        if !already {
            let parked = self.airfield_parked_count(af_id);
            if parked >= Self::airfield_parking_capacity() {
                return false;
            }
        }
        // C++ HasRunways landing residual: need a free runway to final-approach dock.
        // Jets already parked here skip the runway gate.
        let runway_idx = if already {
            None
        } else {
            match self.reserve_airfield_runway(af_id, jet_id) {
                Some(i) => Some(i),
                None => return false, // hold off RTB dock until a runway frees
            }
        };
        // C++ JetAIUpdate RETURN_TO_BASE + Weapon ClipReload airfield rearm residual:
        // dock immediately, wait ClipReload frames, then restore ammo.
        {
            let Some(jet) = self.objects.get_mut(&jet_id) else {
                self.release_airfield_runway_for_jet(jet_id);
                return false;
            };
            // C++ setProducer + park residual: dock at airfield hangar.
            jet.set_contained_by(Some(af_id));
            jet.set_ai_state(AIState::Docked);
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_set_state(jet_id, 12);
                // Docked
            }
            jet.set_status_moving(false);
            jet.status.airborne_target = false;
            jet.movement.path.clear();
            jet.movement.current_path_index = 0;
            jet.record_host_movement();
            jet.movement.target_position = None;
            // Snap to airfield pad residual (hangar park).
            // Landing taxi: approach along reserved runway offset then settle.
            use crate::game_logic::host_dock_contain_exit_heal_residual::PARKING_PLACE_RUNWAY_PREP_SPACING;
            let mut pad = af_pos;
            if let Some(idx) = runway_idx {
                pad.x += (idx as f32 - 0.5) * PARKING_PLACE_RUNWAY_PREP_SPACING;
            }
            pad.y = af_pos.y;
            // Host-immediate hangar dock snap; log for GameWorld pose last-write.
            jet.set_position(pad);
            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                crate::game_logic::host_move_log::record(jet_id, Some([pad.x, pad.y, pad.z]));
                jet.movement.target_position = Some(pad);
                jet.record_host_movement();
            }
            // Arm ClipReload timer on first dock while empty (8000ms standard / 2000ms King-Black).
            // clip_reload_time == 0 → immediate rearm residual (legacy test / unknown weapon).
            if jet.needs_return_to_base_rearm() && jet.airfield_rearm_ready_frame.is_none() {
                let frames = jet.airfield_rearm_clip_reload_frames();
                if frames > 0 {
                    jet.airfield_rearm_ready_frame = Some(self.frame.saturating_add(frames));
                }
            }
        }
        let rearmed = {
            let Some(jet) = self.objects.get_mut(&jet_id) else {
                self.release_airfield_runway_for_jet(jet_id);
                return false;
            };
            if !jet.needs_return_to_base_rearm() {
                jet.airfield_rearm_ready_frame = None;
                true
            } else if let Some(ready) = jet.airfield_rearm_ready_frame {
                if self.frame < ready {
                    // Hangar ClipReload in progress — suppress OOA damage via caller.
                    false
                } else if jet.rearm_return_to_base_weapons() {
                    jet.airfield_rearm_ready_frame = None;
                    true
                } else {
                    false
                }
            } else if jet.rearm_return_to_base_weapons() {
                true
            } else {
                false
            }
        };
        // Dock always succeeded; only fail closed if we could not dock (handled above).
        // While ClipReload pending, still return true so OOA damage is suppressed.
        if !rearmed {
            // Docked + ClipReload in progress: free runway, park, suppress OOA damage.
            self.release_airfield_runway_for_jet(jet_id);
            if let Some(af) = self.objects.get_mut(&af_id) {
                if let Some(building) = af.building_data.as_mut() {
                    if !building.garrisoned_units.contains(&jet_id) {
                        building.garrisoned_units.push(jet_id);
                    }
                } else if !af.occupants.contains(&jet_id) {
                    af.occupants.push(jet_id);
                }
                af.set_production_door_hold_open(true, self.frame);
            }
            return true;
        }
        // Docked: free the landing runway immediately (space now hangar-parked).
        self.release_airfield_runway_for_jet(jet_id);
        // Register parking slot on the airfield (ParkingPlace reserve residual).
        // Structures report transport_capacity 0, so bypass add_occupant gates and
        // write the hangar roster directly (NumRows×NumCols capacity already checked).
        if let Some(af) = self.objects.get_mut(&af_id) {
            if let Some(building) = af.building_data.as_mut() {
                if !building.garrisoned_units.contains(&jet_id) {
                    building.garrisoned_units.push(jet_id);
                }
            } else if !af.occupants.contains(&jet_id) {
                af.occupants.push(jet_id);
            }
            // C++ ParkingPlaceBehavior::reserveSpace → setHoldDoorOpen(door, true).
            af.set_production_door_hold_open(true, self.frame);
        }
        true
    }

    /// C++ ParkingPlaceBehavior heal residual for docked aircraft at airfields.
    ///
    /// Retail AmericaAirfield HealAmountPerSecond **10** → **10/30** HP per frame.
    pub(crate) fn tick_airfield_parking_heal(&mut self) {
        use crate::game_logic::host_countermeasures::aircraft_has_countermeasures_upgrade;
        use crate::game_logic::host_dock_contain_exit_heal_residual::{
            parking_place_heal_per_frame, PARKING_PLACE_AIRFIELD_HEAL_AMOUNT_PER_SEC,
        };
        let heal = parking_place_heal_per_frame(PARKING_PLACE_AIRFIELD_HEAL_AMOUNT_PER_SEC);
        // Docked jets with Countermeasures (ReloadTime=0 / MustReloadAtAirfield residual)
        // reload flares even when already at full HP.
        // Parking heal residual keys off hangar association (contained_by airfield).
        // Under AI_DECISION_AUTHORITY host AIState::Docked is writeback-owned; contained_by
        // is still host-local after RTB rearm.
        let jet_ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, o)| {
                o.is_alive()
                    && (o.is_kind_of(KindOf::Aircraft) || o.object_type == ObjectType::Aircraft)
                    && o.contained_by.is_some()
                    && (o.ai_state == AIState::Docked
                        || crate::gameworld_shadow::gameworld_ai_decision_authority_live())
                    && (o.health.current + 1e-3 < o.health.maximum
                        || o.needs_return_to_base_rearm()
                        || aircraft_has_countermeasures_upgrade(&o.applied_upgrades))
            })
            .map(|(id, _)| *id)
            .collect();
        for jid in jet_ids {
            let (af_ok, has_cm) = {
                let Some(jet) = self.objects.get(&jid) else {
                    continue;
                };
                let Some(af_id) = jet.contained_by else {
                    continue;
                };
                let af_ok = self
                    .objects
                    .get(&af_id)
                    .map(|af| Self::is_friendly_airfield(af, jet.team))
                    .unwrap_or(false);
                let has_cm = aircraft_has_countermeasures_upgrade(&jet.applied_upgrades);
                (af_ok, has_cm)
            };
            if !af_ok {
                continue;
            }
            if has_cm {
                // C++ JetAIUpdate → CountermeasuresBehaviorInterface::reloadCountermeasures
                // when landing / docked at airfield (ReloadTime=0 residual).
                self.countermeasures.reload_at_airfield(jid);
            }
            if let Some(jet) = self.objects.get_mut(&jid) {
                // Also top-up RTB ammo while parked (continuous rearm residual).
                if jet.needs_return_to_base_rearm() {
                    let _ = jet.rearm_return_to_base_weapons();
                }
                if heal > 0.0 && jet.health.current + 1e-3 < jet.health.maximum {
                    jet.heal(heal);
                }
            }
        }
    }

    pub(crate) fn update_combat(&mut self, object_ids: &[ObjectId], _dt: f32) {
        for &attacker_id in object_ids {
            // RETURN_TO_BASE residual: attempt airfield rearm before fire checks.
            // Out-of-ammo HP drip is applied once per frame in tick_out_of_ammo_jet_damage.
            let _ = self.try_return_to_base_rearm(attacker_id);
            // Early gates + docked/garrisoned flags in one immutable scope.
            let (docked_sortie, docked_passenger, garrisoned) = {
                let Some(attacker) = self.objects.get(&attacker_id) else {
                    continue;
                };
                // Need at least one weapon slot bound.
                if attacker.weapon.is_none() && attacker.secondary_weapon.is_none() {
                    continue;
                }
                // ECM jam residual: C++ canFireWeapon DISABLED_SUBDUED — no fire while jammed.
                if attacker.status.weapons_jammed || attacker.is_disabled() {
                    continue;
                }
                // Nested AttackStateMachine residual owns aim/fire/approach for these units.
                if attacker.status.is_aiming_weapon
                    || attacker.status.is_firing_weapon
                    || !matches!(
                        attacker.attack_substate,
                        crate::game_logic::AttackSubState::AimAtTarget
                    )
                {
                    continue;
                }
                // Interaction orders set `target` without being attacks.
                if matches!(
                    attacker.ai_state,
                    AIState::Capturing
                        | AIState::SpecialAbility
                        | AIState::Repairing
                        | AIState::Entering
                        | AIState::Docking
                        | AIState::Constructing
                        | AIState::Gathering
                        | AIState::ReturningResources
                        | AIState::SeekingRepair
                        | AIState::SeekingHealing
                ) {
                    continue;
                }
                let is_ac = attacker.is_kind_of(KindOf::Aircraft)
                    || attacker.object_type == ObjectType::Aircraft;
                let docked_sortie =
                    attacker.ai_state == AIState::Docked && is_ac && attacker.target.is_some();
                let docked_passenger = attacker.ai_state == AIState::Docked && !docked_sortie;
                let garrisoned = attacker.ai_state == AIState::Garrisoned;
                (docked_sortie, docked_passenger, garrisoned)
            };
            if docked_sortie {
                let _ = self.try_runway_takeoff_from_airfield(attacker_id);
            } else if docked_passenger {
                self.try_transport_passenger_residual_fire(attacker_id);
                continue;
            }
            if garrisoned {
                self.try_garrison_residual_fire(attacker_id);
                continue;
            }
            let Some(attacker) = self.objects.get(&attacker_id) else {
                continue;
            };
            // Base-defense residual: Patriot / Gattling (and FSBaseDefense) auto-acquire
            // and fire at nearby enemies without a manual AttackObject order.
            // Respect skirmish AI pause so golden clear is not structure-counterfired.
            {
                let is_defense = crate::game_logic::host_base_defense::is_base_defense_structure(
                    &attacker.template_name,
                    attacker.is_kind_of(KindOf::Structure),
                    attacker.is_kind_of(KindOf::FSBaseDefense),
                );
                let defense_auto_ok = is_defense
                    && attacker.is_constructed()
                    && attacker.can_attack()
                    && matches!(
                        attacker.ai_state,
                        AIState::Idle | AIState::Attacking | AIState::Patrolling
                    )
                    && !self.skirmish_ai_auto_engage_paused(attacker.team);
                if defense_auto_ok {
                    // Residual owns base-defense fire (nearest-in-range each shot).
                    // Manual AttackObject is not required; structures never chase.
                    self.try_base_defense_residual_fire(attacker_id);
                    continue;
                }
            }
            // Strategy Center Bombardment turret residual: StrategyCenterGun auto-fire
            // only while Bombardment plan is active (C++ enableTurret residual).
            {
                use crate::game_logic::host_strategy_center::{
                    is_strategy_center_template, HostBattlePlan,
                };
                let is_sc = is_strategy_center_template(&attacker.template_name)
                    || attacker.is_kind_of(KindOf::FSStrategyCenter);
                let sc_auto_ok = is_sc
                    && attacker.is_constructed()
                    && attacker.weapon.is_some()
                    && attacker.can_attack()
                    && matches!(
                        attacker.ai_state,
                        AIState::Idle | AIState::Attacking | AIState::Patrolling
                    )
                    && !self.skirmish_ai_auto_engage_paused(attacker.team);
                if sc_auto_ok {
                    // Player residual gate: active plan must be Bombardment.
                    let pid = self.player_id_for_team(attacker.team).unwrap_or(0);
                    if self.battle_plans.active_plan_for_player(pid)
                        == Some(HostBattlePlan::Bombardment)
                    {
                        self.try_strategy_center_bombardment_turret_fire(attacker_id);
                        continue;
                    }
                }
            }
            // Sentry Drone residual: with gun upgrade, AutoAcquireEnemiesWhenIdle
            // fires at nearest enemy without manual AttackObject.
            // Fail-closed: not full DeployStyle pack/unpack / turret-only-deployed.
            {
                use crate::game_logic::host_sentry_drone::{
                    is_sentry_drone_template, sentry_auto_fire_eligible,
                };
                let is_sentry = is_sentry_drone_template(&attacker.template_name);
                let idle_ok = matches!(
                    attacker.ai_state,
                    AIState::Idle | AIState::Attacking | AIState::Patrolling
                );
                let sentry_auto_ok = sentry_auto_fire_eligible(
                    is_sentry,
                    attacker.weapon.is_some(),
                    attacker.is_alive(),
                    attacker.can_attack(),
                    idle_ok,
                ) && !self.skirmish_ai_auto_engage_paused(attacker.team)
                    // Only residual-own auto-fire when no explicit player target.
                    && attacker.target.is_none()
                    && attacker.target_location.is_none();
                if sentry_auto_ok {
                    self.try_sentry_drone_residual_fire(attacker_id);
                    continue;
                }
            }
            // Hellfire Drone residual: AutoAcquireEnemiesWhenIdle fires at nearest enemy.
            // Fail-closed: not full SlavedUpdate wander / master attack bonus matrix.
            {
                use crate::game_logic::host_slave_drones::{
                    hellfire_auto_fire_eligible, is_hellfire_drone_template,
                };
                let is_hf = is_hellfire_drone_template(&attacker.template_name);
                let idle_ok = matches!(
                    attacker.ai_state,
                    AIState::Idle | AIState::Attacking | AIState::Patrolling
                );
                let hf_auto_ok = hellfire_auto_fire_eligible(
                    is_hf,
                    attacker.weapon.is_some(),
                    attacker.is_alive(),
                    attacker.can_attack(),
                    idle_ok,
                ) && !self.skirmish_ai_auto_engage_paused(attacker.team)
                    && attacker.target.is_none()
                    && attacker.target_location.is_none();
                if hf_auto_ok {
                    self.try_hellfire_drone_residual_fire(attacker_id);
                    continue;
                }
            }
            let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP;

            // TARGET_FAERIE_FIRE residual: painted targets grant 150% ROF readiness.
            let target_has_faerie = attacker
                .target
                .and_then(|tid| self.objects.get(&tid))
                .map(|t| t.is_faerie_fire())
                .unwrap_or(false);

            // Any slot ready on reload timer? If all reloading, skip (no chase).
            let any_ready = attacker.weapon.as_ref().is_some_and(|w| {
                Object::weapon_ready_vs_target(w, current_time, target_has_faerie)
            }) || attacker.secondary_weapon.as_ref().is_some_and(|w| {
                Object::weapon_ready_vs_target(w, current_time, target_has_faerie)
            });
            if !any_ready {
                continue;
            }

            let attacker_team = attacker.team;
            let target_id = attacker.target;
            let target_location = attacker.target_location;
            let overcharge = attacker.overcharge_enabled;
            drop(attacker);

            // C++ DeployStyleAIUpdate: must unpack before fire when in attack range.
            if !self.ensure_deploy_style_ready_to_fire(attacker_id) {
                continue;
            }

            let mut fired_slot: Option<u8> = None;

            // Standard object-to-object attack.
            if let Some(target_id) = target_id {
                let target_status = self
                    .objects
                    .get(&target_id)
                    .map(|target| (target.is_alive(), target.get_position()));

                let Some((target_alive, target_position)) = target_status else {
                    self.stop_attack_decision_aware(attacker_id);
                    continue;
                };

                if !target_alive {
                    self.stop_attack_decision_aware(attacker_id);
                    continue;
                }

                // Choose primary/secondary, then fire if legal target.
                // Stealthed + undetected: drop the engagement (C++ AIStates residual).
                let (selected_slot, enemy_or_forced, target_stealthed_hidden) = {
                    if let (Some(attacker), Some(target)) =
                        (self.objects.get(&attacker_id), self.objects.get(&target_id))
                    {
                        let stealthed_hidden =
                            target.is_effectively_stealthed() && target.team != attacker.team;
                        // InvulnerableTime residual: enemies treat as ALLIES (skip auto fire).
                        let invuln_hidden =
                            target.is_eject_invulnerable() && target.team != attacker.team;
                        let enemy_or_forced = attacker.force_attack || attacker.team != target.team;
                        let slot = if enemy_or_forced && !stealthed_hidden && !invuln_hidden {
                            attacker.select_combat_weapon_slot(target, current_time)
                        } else {
                            None
                        };
                        (slot, enemy_or_forced, stealthed_hidden || invuln_hidden)
                    } else {
                        (None, false, false)
                    }
                };

                if target_stealthed_hidden {
                    self.stop_attack_decision_aware(attacker_id);
                    continue;
                }

                if let Some(slot) = selected_slot {
                    // C++ isAttackViewBlockedByObstacle residual: do not fire through
                    // buildings; chase instead (falls through to OOR chase when we
                    // clear selected fire by treating as out-of-LOS).
                    if self.attack_view_blocked(attacker_id, Some(target_id), target_position) {
                        // Ready but LOS blocked → findAttackPath residual (firing cell).
                        let combat_chase_ok = self
                            .objects
                            .get(&attacker_id)
                            .map(|attacker| {
                                attacker.can_move()
                                    && matches!(
                                        attacker.ai_state,
                                        AIState::Idle
                                            | AIState::Moving
                                            | AIState::Attacking
                                            | AIState::AttackMoving
                                            | AIState::Patrolling
                                            | AIState::AttackingGround
                                    )
                            })
                            .unwrap_or(false);
                        if combat_chase_ok {
                            let _ = self.assign_unit_attack_path(
                                attacker_id,
                                Some(target_id),
                                target_position,
                            );
                        }
                        continue;
                    }

                    // C++ AIStates AcceptableAimDelta residual: do not fire until facing
                    // is within aim delta; turn in place toward the target instead.
                    {
                        let decision_auth =
                            crate::gameworld_shadow::gameworld_ai_decision_authority_live();
                        let aim_ok = if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                            if !decision_auth {
                                attacker.set_ai_state(AIState::Attacking);
                                attacker.set_status_attacking(true);
                                attacker.target = Some(target_id);
                            }
                            // Stationary / can-turn-in-place residual: complete the yaw
                            // this frame (fail-closed vs loco turn-rate matrix). Moving
                            // attackers use a bounded step so chase still turns gradually.
                            let max_step = if attacker.status.moving && attacker.can_move() {
                                0.2
                            } else {
                                std::f32::consts::PI
                            };
                            attacker.turn_toward_position(target_position, slot, max_step)
                        } else {
                            true
                        };
                        if !aim_ok {
                            continue;
                        }
                    }

                    // C++ Weapon MinTargetPitch/MaxTargetPitch residual: reject shots
                    // whose elevation angle is outside the weapon loft window.
                    {
                        let pitch_ok = if let Some(attacker) = self.objects.get(&attacker_id) {
                            let wname = if slot == 1 {
                                attacker
                                    .thing
                                    .template
                                    .secondary_weapon_name
                                    .as_deref()
                                    .or(attacker.thing.template.primary_weapon_name.as_deref())
                            } else {
                                attacker.thing.template.primary_weapon_name.as_deref()
                            };
                            let limits = wname
                                .map(crate::game_logic::weapon_bootstrap::host_target_pitch_limits_for_weapon_name)
                                .unwrap_or_default();
                            let src_half = {
                                let b = &attacker.thing.geometry.bounds_max.y
                                    - attacker.thing.geometry.bounds_min.y;
                                (b * 0.5).max(0.0)
                            };
                            let (tgt_above, tgt_below) = self
                                .objects
                                .get(&target_id)
                                .map(|t| {
                                    let h = (t.thing.geometry.bounds_max.y
                                        - t.thing.geometry.bounds_min.y)
                                        .max(0.0);
                                    // Position is typically feet; above ≈ full height, below ≈ 0.
                                    (h, 0.0_f32)
                                })
                                .unwrap_or((0.0, 0.0));
                            crate::game_logic::weapon_bootstrap::is_pitch_within_limits_geom(
                                attacker.get_position(),
                                target_position,
                                &limits,
                                src_half,
                                tgt_above,
                                tgt_below,
                            )
                        } else {
                            true
                        };
                        if !pitch_ok {
                            // Out of pitch: keep engagement but do not fire this frame
                            // (C++ AI continues aiming / repositioning).
                            if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                attacker.set_ai_state(AIState::Attacking);
                                attacker.set_status_attacking(true);
                                attacker.set_target(Some(target_id));
                            }
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                crate::game_logic::host_ai_decision_log::record_attack(
                                    attacker_id,
                                    target_id,
                                );
                                crate::game_logic::host_ai_decision_log::record_set_state(
                                    attacker_id,
                                    2,
                                );
                            }
                            continue;
                        }
                    }

                    // C++ PreAttackType residual: wind-up before first discharge of
                    // shot / attack / clip. Shared Object helpers match fire_at.
                    {
                        let pre_blocked = if let Some(attacker) = self.objects.get_mut(&attacker_id)
                        {
                            // Mirror Object::fire_at pre-attack gate without spawning.
                            let pre_delay = attacker
                                .weapon_slot(slot)
                                .map(|w| w.pre_attack_delay.max(0.0))
                                .unwrap_or(0.0);
                            let prefire = {
                                let name =
                                    if slot == 1 {
                                        attacker.thing.template.secondary_weapon_name.as_deref().or(
                                            attacker.thing.template.primary_weapon_name.as_deref(),
                                        )
                                    } else {
                                        attacker.thing.template.primary_weapon_name.as_deref()
                                    };
                                name.map(
                                    crate::game_logic::weapon_bootstrap::host_prefire_type_for_weapon_name,
                                )
                                .unwrap_or(
                                    crate::game_logic::weapon_bootstrap::HostPrefireType::PerShot,
                                )
                            };
                            let apply = attacker
                                .pre_attack_delay_applies(slot, target_id, prefire, pre_delay);
                            if apply {
                                let needs_arm = attacker.pre_attack_target != Some(target_id)
                                    || attacker.pre_attack_ready_at <= 0.0;
                                if needs_arm {
                                    attacker.pre_attack_target = Some(target_id);
                                    attacker.pre_attack_ready_at = current_time + pre_delay;
                                    attacker.activate_leech_range_for_slot(slot);
                                }
                                if current_time + 1e-6 < attacker.pre_attack_ready_at {
                                    attacker.set_target(Some(target_id));
                                    attacker.set_ai_state(AIState::Attacking);
                                    attacker.set_status_attacking(true);
                                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live(
                                    ) {
                                        crate::game_logic::host_ai_decision_log::record_attack(
                                            attacker_id,
                                            target_id,
                                        );
                                        crate::game_logic::host_ai_decision_log::record_set_state(
                                            attacker_id,
                                            2,
                                        );
                                    }
                                    true
                                } else {
                                    false
                                }
                            } else {
                                attacker.pre_attack_target = Some(target_id);
                                false
                            }
                        } else {
                            false
                        };
                        if pre_blocked {
                            continue;
                        }
                    }

                    // GLA car-bomb residual: firing the SuicideCarBomb weapon detonates
                    // at self (DamageDealtAtSelfPosition) and destroys the car bomb.
                    let is_carbomb = self
                        .objects
                        .get(&attacker_id)
                        .map(|a| a.status.is_carbomb)
                        .unwrap_or(false);
                    if is_carbomb {
                        let _ = self.detonate_car_bomb(attacker_id);
                        continue;
                    }

                    let mut weapon_damage = self
                        .objects
                        .get(&attacker_id)
                        .and_then(|a| a.weapon_slot(slot))
                        .map(|w| w.damage)
                        .unwrap_or(0.0);
                    if overcharge {
                        weapon_damage *= 1.1;
                    }
                    // Frenzy / Rage residual: FRENZY_ONE/TWO/THREE DAMAGE mult.
                    // Strategy Center Bombardment residual: BATTLEPLAN_BOMBARDMENT DAMAGE 120%.
                    if let Some(attacker) = self.objects.get(&attacker_id) {
                        weapon_damage *= attacker.frenzy_damage_multiplier();
                        weapon_damage *= attacker.battle_plan_damage_multiplier();
                    }

                    fired_slot = Some(slot);

                    // Aurora dive bomb residual: queue delayed area damage at target.
                    // AuroraBomb projectile flight residual closed; FuelAir gas OCL path.
                    // Instant single-target take_damage is skipped; AOE applies after delay.
                    // Keep fired_slot so last_fire_time / particles / audio still run.
                    let aurora_queued = {
                        use crate::game_logic::host_aurora_bomb::{
                            aurora_bomb_kind_for_template, is_aurora_aircraft_template,
                        };
                        let aurora = self.objects.get(&attacker_id).and_then(|a| {
                            if is_aurora_aircraft_template(&a.template_name) {
                                Some(aurora_bomb_kind_for_template(&a.template_name))
                            } else {
                                None
                            }
                        });
                        if let Some(kind) = aurora {
                            let impact = target_position;
                            let _ =
                                self.queue_aurora_bomb(kind, attacker_id, attacker_team, impact);
                            true
                        } else {
                            false
                        }
                    };

                    if aurora_queued {
                        // Shot consumed; delayed dive residual pending (no instant HP damage).
                    } else {
                        // Avenger Target Designator residual: paint FAERIE_FIRE (no HP damage).
                        let avenger_paint = {
                            use crate::game_logic::host_avenger::{
                                is_avenger_template, should_apply_faerie_fire_paint,
                                AVENGER_FAERIE_FIRE_DURATION_FRAMES, AVENGER_PAINT_AUDIO,
                            };
                            self.objects
                                .get(&attacker_id)
                                .map(|a| {
                                    should_apply_faerie_fire_paint(
                                        is_avenger_template(&a.template_name),
                                        slot,
                                        true,
                                        enemy_or_forced,
                                    )
                                })
                                .unwrap_or(false)
                        };
                        let avenger_air = {
                            use crate::game_logic::host_avenger::{
                                is_avenger_template, should_apply_avenger_air_laser,
                            };
                            let target_is_air = self
                                .objects
                                .get(&target_id)
                                .map(|t| t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target)
                                .unwrap_or(false);
                            self.objects
                                .get(&attacker_id)
                                .map(|a| {
                                    should_apply_avenger_air_laser(
                                        is_avenger_template(&a.template_name),
                                        slot,
                                        target_is_air,
                                        true,
                                        enemy_or_forced,
                                    )
                                })
                                .unwrap_or(false)
                        };
                        // Humvee air TOW residual damage boost vs aircraft.
                        let humvee_air_tow = {
                            use crate::game_logic::host_humvee::{
                                humvee_prefer_air_tow, is_humvee_template, HUMVEE_AIR_TOW_DAMAGE,
                            };
                            let target_is_air = self
                                .objects
                                .get(&target_id)
                                .map(|t| t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target)
                                .unwrap_or(false);
                            self.objects
                                .get(&attacker_id)
                                .map(|a| {
                                    humvee_prefer_air_tow(
                                        is_humvee_template(&a.template_name),
                                        a.has_upgrade_tag(
                                            crate::game_logic::host_upgrades::UPGRADE_AMERICA_TOW,
                                        ) || a.has_upgrade_tag("Upgrade_AmericaTOWMissile"),
                                        target_is_air,
                                    ) && slot == 1
                                })
                                .unwrap_or(false)
                        };
                        if humvee_air_tow {
                            weapon_damage = crate::game_logic::host_humvee::HUMVEE_AIR_TOW_DAMAGE;
                        }

                        // TARGET_FAERIE_FIRE ROF honesty when shooting a painted target.
                        if self
                            .objects
                            .get(&target_id)
                            .map(|t| t.is_faerie_fire())
                            .unwrap_or(false)
                        {
                            self.avenger.record_rof_grant();
                        }

                        if avenger_paint {
                            use crate::game_logic::host_avenger::{
                                AVENGER_FAERIE_FIRE_DURATION_FRAMES, AVENGER_PAINT_AUDIO,
                            };
                            let until = self
                                .frame
                                .saturating_add(AVENGER_FAERIE_FIRE_DURATION_FRAMES);
                            if let Some(target) = self.objects.get_mut(&target_id) {
                                target.apply_faerie_fire(until);
                            }
                            self.avenger.record_paint();
                            let muzzle = self
                                .objects
                                .get(&attacker_id)
                                .map(|a| a.get_position())
                                .unwrap_or(target_position);
                            self.queue_audio_event(
                                AudioEventRequest::new(AVENGER_PAINT_AUDIO)
                                    .with_object(attacker_id)
                                    .with_position(muzzle)
                                    .with_priority(140),
                            );
                            // Status residual: no hitpoint damage from designator.
                        } else if avenger_air {
                            self.avenger.record_air_laser_fire();
                            if let Some(target) = self.objects.get_mut(&target_id) {
                                let destroyed =
                                    target.take_damage_from(weapon_damage, Some(attacker_id));
                                if destroyed {
                                    let victim_pos = target.get_position();
                                    let victim_team = target.team;
                                    self.mark_object_for_destruction(
                                        target_id,
                                        Some(attacker_team),
                                    );
                                    let wname = self.objects.get(&attacker_id).and_then(|a| {
                                        a.thing.template.primary_weapon_name.clone().or_else(|| {
                                            a.thing.template.secondary_weapon_name.clone()
                                        })
                                    });
                                    self.continue_or_stop_after_kill(
                                        attacker_id,
                                        target_id,
                                        victim_pos,
                                        victim_team,
                                        wname.as_deref(),
                                        20.0,
                                    );
                                }
                            }
                        } else {
                            // Nuke Cannon primary residual: area shell + medium radiation field.
                            let nuke_primary = {
                                use crate::game_logic::host_nuke_cannon::{
                                    is_nuke_cannon_template, should_apply_nuke_cannon_primary,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_nuke_cannon_primary(
                                            is_nuke_cannon_template(&a.template_name),
                                            slot,
                                        )
                                    })
                                    .unwrap_or(false)
                            };

                            // Neutron shell residual: Nuke Cannon secondary applies blast
                            // (kill infantry / unman vehicles) instead of HP take_damage.
                            let neutron_blast = {
                                use crate::game_logic::host_neutron_shell::{
                                    is_nuke_cannon_template, should_apply_neutron_blast,
                                    UPGRADE_CHINA_NEUTRON_SHELLS,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_neutron_blast(
                                            a.has_upgrade_tag(UPGRADE_CHINA_NEUTRON_SHELLS)
                                                || a.has_upgrade_tag("Upgrade_ChinaNeutronShells"),
                                            slot,
                                            is_nuke_cannon_template(&a.template_name),
                                        )
                                    })
                                    .unwrap_or(false)
                            };

                            if nuke_primary {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_nuke_cannon_shell_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    // Honesty fire count is recorded on impact via apply;
                                    // count residual fire at spawn for combat-gate honesty.
                                    (1, false)
                                } else {
                                    self.apply_nuke_cannon_primary_at(
                                        impact,
                                        Some(attacker_id),
                                        attacker_team,
                                    )
                                };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 20.0);
                                    }
                                }
                            } else if neutron_blast {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_neutron_cannon_shell_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (ik, vu, _vk) = if spawned {
                                    // Blast deferred to shell DetonateCallsKill residual.
                                    self.neutron_shell_residual_blasts =
                                        self.neutron_shell_residual_blasts.saturating_add(1);
                                    (0, 0, 0)
                                } else {
                                    self.apply_neutron_blast_at(
                                        impact,
                                        attacker_team,
                                        Some(attacker_id),
                                        true,
                                    )
                                };
                                // Stop attack after residual blast shot (slow reload residual).
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    // Award residual XP for infantry kills / unmans.
                                    let xp = (ik as f32) * 20.0 + (vu as f32) * 30.0;
                                    if xp > 0.0 {
                                        attacker.gain_experience(xp);
                                    }
                                }
                            } else if {
                                // Helix residual: PRIMARY HelixMinigunWeapon intended-only.
                                // When portable gattling addon is installed, the Overlord/Helix
                                // gattling residual path already applies primary + passenger.
                                use crate::game_logic::host_helix_minigun::should_apply_helix_minigun_residual;
                                use crate::game_logic::host_overlord_addons::is_helix_template;
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_helix_minigun_residual(
                                            is_helix_template(&a.template_name),
                                            slot,
                                        ) && !a.has_overlord_gattling_residual()
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let (hits, _destroyed_any) = self.apply_helix_minigun_residual_at(
                                    impact,
                                    Some(attacker_id),
                                    Some(target_id),
                                );
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 6.0);
                                    }
                                }
                            } else if {
                                // Comanche residual: 20mm primary / anti-tank secondary / rocket pods secondary.
                                use crate::game_logic::host_comanche_rocket_pods::{
                                    is_comanche_template, should_apply_comanche_antitank_residual,
                                    should_apply_comanche_cannon_residual,
                                    should_apply_rocket_pod_area_attack,
                                    UPGRADE_COMANCHE_ROCKET_PODS,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| is_comanche_template(&a.template_name))
                                    .unwrap_or(false)
                            } {
                                use crate::game_logic::host_comanche_rocket_pods::{
                                    is_comanche_template, should_apply_comanche_antitank_residual,
                                    should_apply_comanche_cannon_residual,
                                    should_apply_rocket_pod_area_attack,
                                    UPGRADE_COMANCHE_ROCKET_PODS,
                                };
                                let impact = target_position;
                                let (has_pods, is_comanche) = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        (
                                            a.has_upgrade_tag(UPGRADE_COMANCHE_ROCKET_PODS)
                                                || a.has_upgrade_tag("Upgrade_ComancheRocketPods"),
                                            is_comanche_template(&a.template_name),
                                        )
                                    })
                                    .unwrap_or((false, false));
                                let (hits, _destroyed_any) = if should_apply_rocket_pod_area_attack(
                                    is_comanche,
                                    has_pods,
                                    slot,
                                ) {
                                    {
                                        use crate::game_logic::host_comanche_rocket_pods::{
                                            rocket_pod_scatter_impact, ROCKET_POD_CLIP_SIZE,
                                        };
                                        let idx = {
                                            let shot = self
                                                .comanche_rocket_pod_shot_index
                                                .entry(attacker_id)
                                                .or_insert(0);
                                            let i = *shot;
                                            *shot = shot.saturating_add(1)
                                                % ROCKET_POD_CLIP_SIZE.max(1);
                                            i
                                        };
                                        let (sx, sy, sz) = rocket_pod_scatter_impact(
                                            impact.x, impact.y, impact.z, idx,
                                        );
                                        let aim = Vec3::new(sx, sy, sz);
                                        let from = self
                                            .objects
                                            .get(&attacker_id)
                                            .map(|o| o.get_position())
                                            .unwrap_or(impact);
                                        let _ = self.spawn_comanche_rocket_pod_projectile(
                                            attacker_id,
                                            from,
                                            aim,
                                            idx,
                                        );
                                        // Area residual still centers on intended aim
                                        // (ScatterTarget is projectile flight residual).
                                        self.apply_comanche_rocket_pod_area_at(
                                            impact,
                                            Some(attacker_id),
                                        )
                                    }
                                } else if should_apply_comanche_antitank_residual(
                                    is_comanche,
                                    slot,
                                    has_pods,
                                ) {
                                    self.apply_comanche_antitank_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                } else if should_apply_comanche_cannon_residual(is_comanche, slot) {
                                    self.apply_comanche_cannon_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                } else {
                                    (0, false)
                                };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 10.0);
                                    }
                                }
                            } else if {
                                // GLA Rocket Buggy residual: long-range rocket + splash / scatter.
                                use crate::game_logic::host_rocket_buggy::{
                                    is_rocket_buggy_template, should_apply_rocket_buggy_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_rocket_buggy_residual(
                                            is_rocket_buggy_template(&a.template_name),
                                        )
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_rocket_buggy_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.rocket_buggy_residual_fires =
                                        self.rocket_buggy_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.apply_rocket_buggy_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 8.0);
                                    }
                                }
                            } else if {
                                // GLA SCUD launcher residual: area blast (+ toxin field on secondary).
                                use crate::game_logic::host_scud_launcher::{
                                    is_scud_launcher_template, should_apply_scud_area,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_scud_area(is_scud_launcher_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let toxin = {
                                    use crate::game_logic::host_scud_launcher::scud_toxin_warhead_for_slot;
                                    self.objects
                                        .get(&attacker_id)
                                        .map(|a| {
                                            scud_toxin_warhead_for_slot(&a.template_name, slot)
                                        })
                                        .unwrap_or(slot == 1)
                                };
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_scud_launcher_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                        toxin,
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    (1, false) // fire residual honesty; blast deferred to impact
                                } else {
                                    self.apply_scud_area_at(
                                        impact,
                                        Some(attacker_id),
                                        attacker_team,
                                        toxin,
                                    )
                                };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 15.0);
                                    }
                                }
                            } else if {
                                // GLA Technical residual: MG direct or cannon/RPG splash salvage tiers.
                                use crate::game_logic::host_technical::{
                                    is_technical_template, should_apply_technical_splash,
                                    TechnicalWeaponTier,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        if !is_technical_template(&a.template_name) {
                                            return false;
                                        }
                                        let tier = Self::technical_tier_from_object(a);
                                        // Always apply residual path for technical (MG direct or splash).
                                        let _ = should_apply_technical_splash(true, tier);
                                        true
                                    })
                                    .unwrap_or(false)
                            } {
                                use crate::game_logic::host_technical::{
                                    should_apply_technical_cannon_shell,
                                    should_apply_technical_rpg_missile,
                                    TechnicalWeaponTier as TechTier,
                                };
                                let impact = target_position;
                                let tier = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| Self::technical_tier_from_object(a))
                                    .unwrap_or(TechTier::Base);
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let (hits, _destroyed_any) =
                                    if should_apply_technical_rpg_missile(true, tier) {
                                        let spawned = self
                                            .spawn_technical_rpg_missile_projectile(
                                                attacker_id,
                                                from,
                                                impact,
                                                Some(target_id),
                                            )
                                            .is_some();
                                        if spawned {
                                            self.technical_residual_fires =
                                                self.technical_residual_fires.saturating_add(1);
                                            (1, false)
                                        } else {
                                            self.apply_technical_residual_at(
                                                impact,
                                                Some(attacker_id),
                                                Some(target_id),
                                            )
                                        }
                                    } else if should_apply_technical_cannon_shell(true, tier) {
                                        let spawned = self
                                            .spawn_technical_cannon_shell_projectile(
                                                attacker_id,
                                                from,
                                                impact,
                                                Some(target_id),
                                            )
                                            .is_some();
                                        if spawned {
                                            self.technical_residual_fires =
                                                self.technical_residual_fires.saturating_add(1);
                                            (1, false)
                                        } else {
                                            self.apply_technical_residual_at(
                                                impact,
                                                Some(attacker_id),
                                                Some(target_id),
                                            )
                                        }
                                    } else {
                                        self.apply_technical_residual_at(
                                            impact,
                                            Some(attacker_id),
                                            Some(target_id),
                                        )
                                    };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 6.0);
                                    }
                                }
                            } else if {
                                // GLA Marauder residual: salvage fire-rate tiers + small splash.
                                use crate::game_logic::host_marauder::{
                                    is_marauder_template, should_apply_marauder_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_marauder_residual(is_marauder_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                use crate::game_logic::host_marauder::{
                                    MarauderWeaponTier, MARAUDER_SPEED_TIER0, MARAUDER_SPEED_TIER1,
                                    MARAUDER_SPEED_TIER2,
                                };
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let speed = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| match Self::marauder_tier_from_object(a) {
                                        MarauderWeaponTier::Two => MARAUDER_SPEED_TIER2,
                                        MarauderWeaponTier::One => MARAUDER_SPEED_TIER1,
                                        MarauderWeaponTier::Base => a
                                            .weapon
                                            .as_ref()
                                            .map(|w| w.projectile_speed)
                                            .filter(|s| *s > 1.0)
                                            .unwrap_or(MARAUDER_SPEED_TIER0),
                                    })
                                    .unwrap_or(MARAUDER_SPEED_TIER0);
                                let spawned = self
                                    .spawn_marauder_shell_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                        speed,
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.marauder_residual_fires =
                                        self.marauder_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.apply_marauder_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 10.0);
                                    }
                                }
                            } else if {
                                // GLA Scorpion residual: gun splash or rocket dual-radius secondary.
                                use crate::game_logic::host_scorpion::{
                                    is_scorpion_template, should_apply_scorpion_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_scorpion_residual(is_scorpion_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = if slot == 0 {
                                    self.spawn_scorpion_shell_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                        slot,
                                    )
                                    .is_some()
                                } else {
                                    self.spawn_scorpion_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                        slot,
                                    )
                                    .is_some()
                                };
                                let (hits, _destroyed_any) = if spawned {
                                    if slot == 0 {
                                        self.scorpion_residual_fires =
                                            self.scorpion_residual_fires.saturating_add(1);
                                    } else {
                                        self.scorpion_residual_missile_fires =
                                            self.scorpion_residual_missile_fires.saturating_add(1);
                                    }
                                    (1, false)
                                } else {
                                    self.apply_scorpion_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                        slot,
                                    )
                                };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 8.0);
                                    }
                                }
                            } else if {
                                // USA Tomahawk residual: dual-radius long-range missile.
                                use crate::game_logic::host_tomahawk::{
                                    is_tomahawk_template, should_apply_tomahawk_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_tomahawk_residual(is_tomahawk_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_tomahawk_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.tomahawk_residual_fires =
                                        self.tomahawk_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.apply_tomahawk_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 15.0);
                                    }
                                }
                            } else if {
                                // USA Raptor residual: jet missiles + Laser Missiles splash.
                                use crate::game_logic::host_raptor::{
                                    is_raptor_template, should_apply_raptor_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_raptor_residual(is_raptor_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_raptor_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.raptor_residual_fires =
                                        self.raptor_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.apply_raptor_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 12.0);
                                    }
                                }
                            } else if {
                                // China MiG residual: dual-radius napalm / Nuke missiles + field residual.
                                use crate::game_logic::host_mig::{
                                    is_mig_template, should_apply_mig_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_mig_residual(is_mig_template(&a.template_name))
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_mig_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.mig_residual_fires =
                                        self.mig_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.apply_mig_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 12.0);
                                    }
                                }
                            } else if {
                                // America Fire Base residual: howitzer primary-radius splash.
                                use crate::game_logic::host_fire_base::{
                                    is_fire_base_template, should_apply_fire_base_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_fire_base_residual(is_fire_base_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_fire_base_shell_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.fire_base_residual_fires =
                                        self.fire_base_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.apply_fire_base_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 8.0);
                                    }
                                }
                            } else if {
                                // USA Stealth Fighter residual: jet missiles splash + bunker-buster structure path.
                                use crate::game_logic::host_stealth_fighter::{
                                    is_stealth_fighter_template,
                                    should_apply_stealth_fighter_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_stealth_fighter_residual(
                                            is_stealth_fighter_template(&a.template_name),
                                        )
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_stealth_jet_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.stealth_fighter_residual_fires =
                                        self.stealth_fighter_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.apply_stealth_fighter_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 12.0);
                                    }
                                }
                            } else if {
                                // USA Battle Drone residual: intended-only MG fire.
                                use crate::game_logic::host_slave_drones::is_battle_drone_template;
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| is_battle_drone_template(&a.template_name))
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let (hits, _destroyed_any) = self.apply_battle_drone_residual_at(
                                    impact,
                                    Some(attacker_id),
                                    Some(target_id),
                                );
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 2.0);
                                    }
                                }
                            } else if {
                                // China Overlord / Emperor residual: dual-radius main gun (no gattling addon).
                                use crate::game_logic::host_overlord_gun::{
                                    is_overlord_gun_chassis, should_apply_overlord_gun_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_overlord_gun_residual(
                                            is_overlord_gun_chassis(&a.template_name),
                                            a.has_overlord_gattling_residual(),
                                        )
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_overlord_shell_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    (1, false)
                                } else {
                                    self.apply_overlord_gun_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 12.0);
                                    }
                                }
                            } else if {
                                // GLA Jarmen Kell residual: primary sniper (intended-only).
                                use crate::game_logic::host_jarmen_kell::{
                                    is_jarmen_kell_template, should_apply_jarmen_kell_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_jarmen_kell_residual(is_jarmen_kell_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let (hits, _destroyed_any) = self.apply_jarmen_kell_residual_at(
                                    impact,
                                    Some(attacker_id),
                                    Some(target_id),
                                );
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 15.0);
                                    }
                                }
                            } else if {
                                // USA Crusader/Paladin residual: GenericTankShell Bezier + splash.
                                use crate::game_logic::host_usa_tanks::{
                                    is_paladin_template, should_apply_usa_tank_gun_residual,
                                    CRUSADER_WEAPON_SPEED, PALADIN_WEAPON_SPEED,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| should_apply_usa_tank_gun_residual(&a.template_name))
                                    .unwrap_or(false)
                            } {
                                use crate::game_logic::host_usa_tanks::{
                                    is_paladin_template, CRUSADER_WEAPON_SPEED,
                                    PALADIN_WEAPON_SPEED,
                                };
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let (speed, is_pal) = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        let pal = is_paladin_template(&a.template_name);
                                        let spd = a
                                            .weapon
                                            .as_ref()
                                            .map(|w| w.projectile_speed)
                                            .filter(|s| *s > 1.0)
                                            .unwrap_or(if pal {
                                                PALADIN_WEAPON_SPEED
                                            } else {
                                                CRUSADER_WEAPON_SPEED
                                            });
                                        (spd, pal)
                                    })
                                    .unwrap_or((CRUSADER_WEAPON_SPEED, false));
                                let _ = is_pal;
                                let spawned = self
                                    .spawn_usa_tank_shell_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        speed,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    (1, false)
                                } else {
                                    self.apply_usa_tank_gun_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        if let Some(w) = attacker.weapon.as_mut() {
                                            w.last_fire_time =
                                                self.frame as f32 * LOGIC_FRAME_TIMESTEP;
                                        }
                                    }
                                }
                                let _ = hits;
                            } else if {
                                // China Battlemaster residual: tank gun splash + Uranium damage residual.
                                use crate::game_logic::host_battlemaster::{
                                    is_battlemaster_template, should_apply_battlemaster_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_battlemaster_residual(
                                            is_battlemaster_template(&a.template_name),
                                        )
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_battlemaster_shell_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    (1, false)
                                } else {
                                    self.apply_battlemaster_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 10.0);
                                    }
                                }
                            } else if {
                                // China Tank Hunter residual: RPG splash + AA capable residual.
                                use crate::game_logic::host_tank_hunter::{
                                    is_tank_hunter_template, should_apply_tank_hunter_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_tank_hunter_residual(is_tank_hunter_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_tank_hunter_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.tank_hunter_residual_fires =
                                        self.tank_hunter_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.apply_tank_hunter_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 8.0);
                                    }
                                }
                            } else if {
                                // China Red Guard residual: bayonet one-shot vs close infantry, else gun.
                                use crate::game_logic::host_red_guard::{
                                    is_red_guard_template, should_apply_red_guard_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_red_guard_residual(is_red_guard_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let (hits, _destroyed_any) = self.apply_red_guard_residual_at(
                                    target_position,
                                    Some(attacker_id),
                                    Some(target_id),
                                );
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 5.0);
                                    }
                                }
                            } else if {
                                // GLA RPG Trooper residual: rocket splash + AA capable residual.
                                use crate::game_logic::host_rpg_trooper::{
                                    is_rpg_trooper_template, should_apply_rpg_trooper_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_rpg_trooper_residual(is_rpg_trooper_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_rpg_trooper_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.rpg_trooper_residual_fires =
                                        self.rpg_trooper_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.apply_rpg_trooper_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 8.0);
                                    }
                                }
                            } else if {
                                // GLA Terrorist residual: SuicideDynamitePack self-detonation.
                                use crate::game_logic::host_terrorist::{
                                    is_terrorist_template, should_apply_terrorist_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_terrorist_residual(is_terrorist_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let (hits, _destroyed_any) = self.apply_terrorist_residual_at(
                                    Some(attacker_id),
                                    Some(target_id),
                                );
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 10.0);
                                    }
                                }
                            } else if {
                                // USA Missile Defender residual: missile splash + laser guided secondary.
                                use crate::game_logic::host_missile_defender::{
                                    is_missile_defender_template,
                                    should_apply_missile_defender_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_missile_defender_residual(
                                            is_missile_defender_template(&a.template_name),
                                        )
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let laser_slot = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.active_weapon_slot == 1)
                                    .unwrap_or(false);
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_missile_defender_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                        laser_slot,
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    if laser_slot {
                                        self.missile_defender_residual_laser_fires = self
                                            .missile_defender_residual_laser_fires
                                            .saturating_add(1);
                                    } else {
                                        self.missile_defender_residual_fires =
                                            self.missile_defender_residual_fires.saturating_add(1);
                                    }
                                    (1, false)
                                } else {
                                    self.apply_missile_defender_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                        laser_slot,
                                    )
                                };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 8.0);
                                    }
                                }
                            } else if {
                                // GLA Rebel residual: machine gun intended-only residual.
                                use crate::game_logic::host_gla_rebel::{
                                    is_gla_rebel_template, should_apply_rebel_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_rebel_residual(is_gla_rebel_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let (hits, _destroyed_any) = self.apply_rebel_residual_at(
                                    target_position,
                                    Some(attacker_id),
                                    Some(target_id),
                                );
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 5.0);
                                    }
                                }
                            } else if {
                                // USA Ranger residual: rifle intended-only or FlashBang dual-radius splash.
                                use crate::game_logic::host_ranger::{
                                    is_ranger_template, should_apply_ranger_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_ranger_residual(is_ranger_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let flash_slot = slot == 1;
                                let impact = target_position;
                                let (hits, _destroyed_any) = if flash_slot {
                                    let from = self
                                        .objects
                                        .get(&attacker_id)
                                        .map(|a| a.get_position())
                                        .unwrap_or(impact);
                                    let spawned = self
                                        .spawn_flashbang_grenade_projectile(
                                            attacker_id,
                                            from,
                                            impact,
                                            Some(target_id),
                                        )
                                        .is_some();
                                    if spawned {
                                        self.ranger_residual_flashbang_fires =
                                            self.ranger_residual_flashbang_fires.saturating_add(1);
                                        (1, false)
                                    } else {
                                        self.apply_ranger_residual_at(
                                            impact,
                                            Some(attacker_id),
                                            Some(target_id),
                                            true,
                                        )
                                    }
                                } else {
                                    self.apply_ranger_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                        false,
                                    )
                                };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 5.0);
                                    }
                                }
                            } else if {
                                // USA Humvee TOW residual: HumveeMissile / PatriotMissile flight + splash.
                                use crate::game_logic::host_humvee::{
                                    is_humvee_template, should_apply_humvee_tow_residual,
                                };
                                use crate::game_logic::host_upgrades::UPGRADE_AMERICA_TOW;
                                let (is_hv, has_tow) = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        (
                                            is_humvee_template(&a.template_name),
                                            a.has_upgrade_tag(UPGRADE_AMERICA_TOW)
                                                || a.has_upgrade_tag("Upgrade_AmericaTOWMissile"),
                                        )
                                    })
                                    .unwrap_or((false, false));
                                should_apply_humvee_tow_residual(is_hv, has_tow, slot == 1)
                            } {
                                use crate::game_logic::host_humvee::{
                                    humvee_prefer_air_tow as hv_air_tow,
                                    HUMVEE_TOW_FIRE_AUDIO as HV_TOW_AUDIO,
                                };
                                let impact = target_position;
                                let target_is_air = self
                                    .objects
                                    .get(&target_id)
                                    .map(|t| {
                                        t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target
                                    })
                                    .unwrap_or(false);
                                let air = hv_air_tow(true, true, target_is_air);
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_humvee_tow_missile_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                        air,
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.humvee_tow_residual_fires =
                                        self.humvee_tow_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.humvee_tow_residual_fires =
                                        self.humvee_tow_residual_fires.saturating_add(1);
                                    self.apply_humvee_tow_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                        air,
                                    )
                                };
                                let _ = HV_TOW_AUDIO;
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 10.0);
                                    }
                                }
                            } else if {
                                // China MiniGunner residual: ground gun or AA secondary hit.
                                use crate::game_logic::host_minigunner::{
                                    is_minigunner_template, should_apply_minigunner_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_minigunner_residual(is_minigunner_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let (hits, _destroyed_any) = self.apply_minigunner_residual_at(
                                    target_position,
                                    Some(attacker_id),
                                    Some(target_id),
                                    slot,
                                );
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 5.0);
                                    }
                                }
                            } else if {
                                // Colonel Burton residual: knife one-shot vs close infantry, else sniper.
                                use crate::game_logic::host_colonel_burton::{
                                    is_colonel_burton_template, should_apply_burton_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_burton_residual(is_colonel_burton_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let (hits, _destroyed_any) = self.apply_burton_residual_at(
                                    target_position,
                                    Some(attacker_id),
                                    Some(target_id),
                                );
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 10.0);
                                    }
                                }
                            } else if {
                                // China Troop Crawler residual: TroopCrawlerAssault DEPLOY → unload + attack.
                                use crate::game_logic::host_troop_crawler::{
                                    is_troop_crawler_template,
                                    should_apply_troop_crawler_assault_deploy,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_troop_crawler_assault_deploy(
                                            is_troop_crawler_template(&a.template_name),
                                        )
                                    })
                                    .unwrap_or(false)
                            } {
                                let _ordered =
                                    self.apply_troop_crawler_assault_deploy(attacker_id, target_id);
                                // DEPLOY residual deals no meaningful HP damage (PrimaryDamage ~0).
                            } else if {
                                // China Dragon Tank residual: DragonTankFlameProjectile flight + dual-radius splash.
                                use crate::game_logic::host_dragon_tank::{
                                    is_dragon_tank_template, should_apply_dragon_flame_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_dragon_flame_residual(is_dragon_tank_template(
                                            &a.template_name,
                                        ))
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let from = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| a.get_position())
                                    .unwrap_or(impact);
                                let spawned = self
                                    .spawn_dragon_flame_projectile(
                                        attacker_id,
                                        from,
                                        impact,
                                        Some(target_id),
                                    )
                                    .is_some();
                                let (hits, _destroyed_any) = if spawned {
                                    self.dragon_tank_residual_fires =
                                        self.dragon_tank_residual_fires.saturating_add(1);
                                    (1, false)
                                } else {
                                    self.dragon_tank_residual_fires =
                                        self.dragon_tank_residual_fires.saturating_add(1);
                                    self.apply_dragon_flame_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                    )
                                };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 8.0);
                                    }
                                }
                            } else if {
                                // China Gattling Tank residual: ground gun or AA secondary hit.
                                use crate::game_logic::host_gattling_tank::is_gattling_tank_template;
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| is_gattling_tank_template(&a.template_name))
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let (hits, _destroyed_any) = self.apply_gattling_tank_residual_at(
                                    impact,
                                    Some(attacker_id),
                                    Some(target_id),
                                    slot,
                                );
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 5.0);
                                    }
                                }
                            } else if {
                                // Overlord / Helix / Emperor portable gattling addon residual.
                                use crate::game_logic::host_overlord_addons::should_apply_overlord_gattling_residual;
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_overlord_gattling_residual(
                                            a.has_overlord_gattling_residual(),
                                        )
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let (hits, _destroyed_any) = self
                                    .apply_overlord_gattling_residual_at(
                                        impact,
                                        Some(attacker_id),
                                        Some(target_id),
                                        slot,
                                    );
                                // Primary OverlordTankGun / HelixMinigun still applies when
                                // slot==0: passenger gattling residual is extra damage path
                                // handled inside apply_overlord_gattling (primary + passenger).
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 5.0);
                                    }
                                }
                            } else if {
                                // GLA Combat Cycle residual: rider weapon fire / suicide residual.
                                use crate::game_logic::host_combat_cycle::{
                                    is_combat_cycle_template, should_apply_combat_cycle_residual,
                                };
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        should_apply_combat_cycle_residual(
                                            is_combat_cycle_template(&a.template_name),
                                        )
                                    })
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let (hits, _destroyed_any) = self.apply_combat_cycle_residual_at(
                                    impact,
                                    Some(attacker_id),
                                    Some(target_id),
                                );
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 8.0);
                                    }
                                }
                            } else if {
                                // GLA Toxin Tractor residual: poison stream primary or contaminate spray.
                                use crate::game_logic::host_toxin_tractor::is_toxin_tractor_template;
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| is_toxin_tractor_template(&a.template_name))
                                    .unwrap_or(false)
                            } {
                                let impact = target_position;
                                let spray = slot == 1;
                                let (hits, _destroyed_any) = if spray {
                                    self.apply_toxin_tractor_spray_at(
                                        impact,
                                        Some(attacker_id),
                                        attacker_team,
                                    )
                                } else {
                                    let from = self
                                        .objects
                                        .get(&attacker_id)
                                        .map(|a| a.get_position())
                                        .unwrap_or(impact);
                                    let spawned = self
                                        .spawn_toxin_stream_projectile(
                                            attacker_id,
                                            from,
                                            impact,
                                            Some(target_id),
                                        )
                                        .is_some();
                                    if spawned {
                                        (1, false)
                                    } else {
                                        self.apply_toxin_tractor_stream_at(
                                            impact,
                                            Some(attacker_id),
                                            Some(target_id),
                                            attacker_team,
                                        )
                                    }
                                };
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    if hits > 0 {
                                        attacker.gain_experience((hits as f32) * 5.0);
                                    }
                                }
                            } else {
                                // Bunker Buster residual: kill garrisoned occupants + amplify bunker damage.
                                // KILL_GARRISONED residual: microwave-style kill floor(damage) occupants.
                                let (bunker_buster_hit, kill_garrisoned_hit) = {
                                    use crate::game_logic::host_bunker_buster::{
                                        is_bunker_buster_carrier, is_kill_garrisoned_clearer,
                                        should_apply_bunker_buster, should_apply_kill_garrisoned,
                                        UPGRADE_AMERICA_BUNKER_BUSTERS,
                                    };
                                    let target_is_structure = self
                                        .objects
                                        .get(&target_id)
                                        .map(|t| t.is_kind_of(KindOf::Structure))
                                        .unwrap_or(false);
                                    self.objects
                                        .get(&attacker_id)
                                        .map(|a| {
                                            let has_upgrade = a
                                                .has_upgrade_tag(UPGRADE_AMERICA_BUNKER_BUSTERS)
                                                || a.has_upgrade_tag(
                                                    "Upgrade_AmericaBunkerBusters",
                                                );
                                            let carrier =
                                                is_bunker_buster_carrier(&a.template_name);
                                            let wname = if slot == 1 {
                                                a.thing
                                                    .template
                                                    .secondary_weapon_name
                                                    .as_deref()
                                                    .or(a.thing.template.primary_weapon_name.as_deref())
                                            } else {
                                                a.thing.template.primary_weapon_name.as_deref()
                                            };
                                            let clearer = is_kill_garrisoned_clearer(
                                                &a.template_name,
                                            ) || wname
                                                .map(
                                                    crate::game_logic::host_bunker_buster::is_kill_garrisoned_clearer_weapon,
                                                )
                                                .unwrap_or(false);
                                            (
                                                should_apply_bunker_buster(
                                                    has_upgrade,
                                                    carrier,
                                                    target_is_structure,
                                                ),
                                                should_apply_kill_garrisoned(
                                                    clearer,
                                                    target_is_structure,
                                                ),
                                            )
                                        })
                                        .unwrap_or((false, false))
                                };

                                if bunker_buster_hit {
                                    let (kills, structure_dmg, destroyed) = self
                                        .apply_bunker_buster_to_target(
                                            target_id,
                                            attacker_team,
                                            weapon_damage,
                                            Some(attacker_id),
                                        );
                                    if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                        let xp = (kills as f32) * 15.0 + structure_dmg * 0.05;
                                        if xp > 0.0 {
                                            attacker.gain_experience(xp);
                                        }
                                        if destroyed {
                                            self.stop_attack_decision_aware(attacker_id);
                                        }
                                    }
                                } else if kill_garrisoned_hit {
                                    let kills = self.apply_kill_garrisoned_to_target(
                                        target_id,
                                        attacker_team,
                                        weapon_damage,
                                        Some(attacker_id),
                                    );
                                    if kills > 0 {
                                        if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                            attacker.gain_experience((kills as f32) * 15.0);
                                        }
                                    }
                                } else if {
                                    let (sc_miss, sc_impact, sc_splash) = self
                                        .resolve_instant_scatter_shot(
                                            attacker_id,
                                            target_id,
                                            slot,
                                            target_position,
                                        );
                                    if sc_miss {
                                        // C++ ScatterRadius residual: miss intended; splash at offset.
                                        if sc_splash > 0.0 {
                                            let wname_splash =
                                                self.objects.get(&attacker_id).and_then(|a| {
                                                    if slot == 1 {
                                                        a.thing
                                                            .template
                                                            .secondary_weapon_name
                                                            .clone()
                                                            .or_else(|| {
                                                                a.thing
                                                                    .template
                                                                    .primary_weapon_name
                                                                    .clone()
                                                            })
                                                    } else {
                                                        a.thing.template.primary_weapon_name.clone()
                                                    }
                                                });
                                            let hits = self.apply_scatter_miss_splash_at(
                                                sc_impact,
                                                weapon_damage,
                                                sc_splash,
                                                attacker_id,
                                                attacker_team,
                                                target_id,
                                                wname_splash.as_deref(),
                                            );
                                            if hits > 0 {
                                                if let Some(attacker) =
                                                    self.objects.get_mut(&attacker_id)
                                                {
                                                    attacker.gain_experience((hits as f32) * 5.0);
                                                }
                                            }
                                        }
                                        true
                                    } else {
                                        false
                                    }
                                } {
                                    // Miss path handled above (splash optional).
                                } else {
                                    if let Some(target) = self.objects.get_mut(&target_id) {
                                        let destroyed = target
                                            .take_damage_from(weapon_damage, Some(attacker_id));
                                        if destroyed {
                                            // C++ parity: XP is based on victim's ExperienceValue.
                                            let kill_xp = target.thing.template.experience_value
                                                * Self::veterancy_xp_multiplier(
                                                    target.experience.level,
                                                );
                                            let victim_pos = target.get_position();
                                            let victim_team = target.team;
                                            let cont_weapon = {
                                                // Prefer the weapon name used for this shot when known.
                                                None::<&str>
                                            };
                                            self.mark_object_for_destruction(
                                                target_id,
                                                Some(attacker_team),
                                            );
                                            // Resolve attacker weapon name after releasing target borrow.
                                            let wname =
                                                self.objects.get(&attacker_id).and_then(|a| {
                                                    a.thing
                                                        .template
                                                        .primary_weapon_name
                                                        .clone()
                                                        .or_else(|| {
                                                            a.thing
                                                                .template
                                                                .secondary_weapon_name
                                                                .clone()
                                                        })
                                                });
                                            let _ = cont_weapon;
                                            self.continue_or_stop_after_kill(
                                                attacker_id,
                                                target_id,
                                                victim_pos,
                                                victim_team,
                                                wname.as_deref(),
                                                kill_xp,
                                            );
                                        }
                                    }
                                    // C++ dual-radius splash residual after direct hit.
                                    {
                                        use crate::game_logic::weapon_bootstrap::{
                                            host_primary_damage_radius_for_weapon_name,
                                            host_secondary_damage_for_weapon_name,
                                            host_secondary_damage_radius_for_weapon_name,
                                        };
                                        let wname = self.objects.get(&attacker_id).and_then(|a| {
                                            if slot == 1 {
                                                a.thing
                                                    .template
                                                    .secondary_weapon_name
                                                    .clone()
                                                    .or_else(|| {
                                                        a.thing.template.primary_weapon_name.clone()
                                                    })
                                            } else {
                                                a.thing.template.primary_weapon_name.clone()
                                            }
                                        });
                                        let (pr, sr, sd) = if let Some(ref n) = wname {
                                            (
                                                host_primary_damage_radius_for_weapon_name(n),
                                                host_secondary_damage_radius_for_weapon_name(n),
                                                host_secondary_damage_for_weapon_name(n),
                                            )
                                        } else {
                                            (0.0, 0.0, 0.0)
                                        };
                                        let splash_weapon = self
                                            .objects
                                            .get(&attacker_id)
                                            .and_then(|a| a.weapon_slot(slot))
                                            .map(|w| w.splash_radius.max(0.0))
                                            .unwrap_or(0.0);
                                        let primary_r = pr.max(splash_weapon);
                                        let secondary_r = sr.max(if primary_r > 0.0 {
                                            primary_r * 1.5
                                        } else {
                                            0.0
                                        });
                                        if primary_r > 0.0 || secondary_r > 0.0 {
                                            let sec_dmg =
                                                if sd > 0.0 { sd } else { weapon_damage * 0.5 };
                                            let hits = self.apply_instant_hit_splash_at(
                                                target_position,
                                                weapon_damage,
                                                sec_dmg,
                                                primary_r,
                                                secondary_r,
                                                attacker_id,
                                                attacker_team,
                                                target_id,
                                                wname.as_deref(),
                                            );
                                            if hits > 0 {
                                                if let Some(attacker) =
                                                    self.objects.get_mut(&attacker_id)
                                                {
                                                    attacker.gain_experience((hits as f32) * 5.0);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        } // end !avenger_paint / !avenger_air residual branch
                    } // end !aurora_queued

                    // Inferno Cannon residual: InfernoTankShell Bezier flight → FireFieldSmall.
                    // Fail-closed: instant FireFieldSmall zone if shell spawn fails.
                    // Skipped for Aurora (delayed dive residual already queued).
                    if !aurora_queued {
                        use crate::game_logic::host_inferno_cannon::is_inferno_cannon_template;
                        let is_inferno = self
                            .objects
                            .get(&attacker_id)
                            .map(|a| is_inferno_cannon_template(&a.template_name))
                            .unwrap_or(false);
                        if is_inferno {
                            let impact = target_position;
                            let upgraded = self
                                .objects
                                .get(&attacker_id)
                                .map(|a| {
                                    crate::game_logic::host_inferno_cannon::has_black_napalm_upgrade(
                                        &a.applied_upgrades,
                                    )
                                })
                                .unwrap_or(false);
                            let from = self
                                .objects
                                .get(&attacker_id)
                                .map(|a| a.get_position())
                                .unwrap_or(impact);
                            let spawned = self
                                .spawn_inferno_shell_projectile(
                                    attacker_id,
                                    from,
                                    impact,
                                    Some(target_id),
                                    upgraded,
                                )
                                .is_some();
                            if !spawned {
                                let _ = self.spawn_inferno_fire_zone(
                                    attacker_id,
                                    attacker_team,
                                    impact,
                                    upgraded,
                                );
                            }
                        }
                    }
                } else if enemy_or_forced {
                    // Ready weapons but out of range / cannot hit.
                    // MinimumAttackRange residual: if too close, back away instead
                    // of chasing into the dead zone (artillery / rocket safety).
                    let (min_r, max_r, can_chase) = self
                        .objects
                        .get(&attacker_id)
                        .map(|attacker| {
                            let w = attacker
                                .weapon
                                .as_ref()
                                .or(attacker.secondary_weapon.as_ref());
                            let min_r = w.map(|w| w.min_range).unwrap_or(0.0);
                            let max_r = w.map(|w| w.range).unwrap_or(0.0)
                                * attacker.battle_plan_range_multiplier();
                            let can = attacker.can_move()
                                && matches!(
                                    attacker.ai_state,
                                    AIState::Idle
                                        | AIState::Moving
                                        | AIState::Attacking
                                        | AIState::AttackMoving
                                        | AIState::Patrolling
                                        | AIState::AttackingGround
                                );
                            (min_r, max_r, can)
                        })
                        .unwrap_or((0.0, 0.0, false));
                    let too_close = {
                        let src = self
                            .objects
                            .get(&attacker_id)
                            .map(|a| a.get_position())
                            .unwrap_or(target_position);
                        let dx = src.x - target_position.x;
                        let dz = src.z - target_position.z;
                        let dist = (dx * dx + dz * dz).sqrt();
                        crate::game_logic::weapon_bootstrap::is_inside_minimum_attack_range(
                            dist, min_r,
                        )
                    };
                    if too_close && can_chase {
                        let _ = self.try_min_range_backup(attacker_id, target_position, min_r);
                        continue;
                    }
                    // Pathfind toward target (not straight-line through buildings).
                    // Do not clobber interaction orders that also set `target`
                    // (CaptureBuilding, SpecialAbility, Repair, Enter, etc.).
                    let combat_chase_ok = can_chase;
                    let _ = max_r;
                    if combat_chase_ok {
                        // findAttackPath residual: path to in-range LOS cell, not target cell.
                        // Contact weapons path to the target; others stand off at range*0.9.
                        //
                        // Repath throttle: A* every OOR frame thrash-hangs Lone Eagle
                        // (hundreds of attackers × large static grid). Keep following an
                        // existing path; repath periodically or when path is exhausted.
                        let has_active_path = self
                            .objects
                            .get(&attacker_id)
                            .map(|a| {
                                a.movement.current_path_index < a.movement.path.len()
                                    || a.movement.target_position.is_some()
                            })
                            .unwrap_or(false);
                        // ~0.5s at 30 Hz when already marching; always plan when idle/stuck.
                        let repath_due = !has_active_path || (self.frame % 15 == 0);
                        if repath_due {
                            let (wrange, wname) = self
                                .objects
                                .get(&attacker_id)
                                .map(|a| {
                                    let r = a
                                        .weapon
                                        .as_ref()
                                        .map(|w| w.range)
                                        .or_else(|| a.secondary_weapon.as_ref().map(|w| w.range))
                                        .unwrap_or(50.0);
                                    let n =
                                        a.thing.template.primary_weapon_name.clone().or_else(
                                            || a.thing.template.secondary_weapon_name.clone(),
                                        );
                                    (r, n)
                                })
                                .unwrap_or((50.0, None));
                            let approach = self.approach_pos_for_attack(
                                attacker_id,
                                target_position,
                                wrange,
                                wname.as_deref(),
                            );
                            if !self.assign_unit_attack_path(attacker_id, Some(target_id), approach)
                            {
                                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                    // Fallback: direct march if A* / attack-path fails.
                                    attacker.movement.path.clear();
                                    attacker.movement.current_path_index = 0;
                                    attacker.movement.target_position = Some(approach);
                                    crate::game_logic::host_move_log::record(
                                        attacker_id,
                                        Some([approach.x, approach.y, approach.z]),
                                    );
                                    attacker.set_status_moving(true);
                                    attacker.set_ai_state(AIState::Attacking);
                                    if crate::gameworld_shadow::gameworld_ai_decision_authority_enabled(
                                    ) {
                                        crate::game_logic::host_ai_decision_log::record_set_state(
                                            attacker_id,
                                            2,
                                        ); // Attacking
                                    }
                                    attacker.set_status_attacking(true);
                                }
                            }
                        }
                    }
                }
            } else if let Some(target_location) = target_location {
                // Force-attack-ground: consume a shot when the location is in range and apply damage
                // to the nearest hittable object around the designated impact point.
                // Comanche Rocket Pods residual: secondary ground AOE when slot-locked.
                // Fail-closed residual: other units still use primary for ground fire.
                let rocket_pod_ground = {
                    use crate::game_logic::host_comanche_rocket_pods::{
                        is_comanche_template, rocket_pod_ground_fire_active,
                        UPGRADE_COMANCHE_ROCKET_PODS,
                    };
                    self.objects
                        .get(&attacker_id)
                        .map(|a| {
                            rocket_pod_ground_fire_active(
                                is_comanche_template(&a.template_name),
                                a.has_upgrade_tag(UPGRADE_COMANCHE_ROCKET_PODS)
                                    || a.has_upgrade_tag("Upgrade_ComancheRocketPods"),
                                a.secondary_weapon.is_some(),
                                a.active_weapon_slot,
                            )
                        })
                        .unwrap_or(false)
                };

                let can_fire_at_location = {
                    if let Some(attacker) = self.objects.get(&attacker_id) {
                        if rocket_pod_ground {
                            attacker
                                .secondary_weapon
                                .as_ref()
                                .map(|w| {
                                    Object::weapon_ready(w, current_time)
                                        && w.can_target_ground
                                        && attacker.position.distance(target_location) <= w.range
                                })
                                .unwrap_or(false)
                        } else {
                            attacker
                                .weapon
                                .as_ref()
                                .map(|w| {
                                    Object::weapon_ready(w, current_time)
                                        && w.can_target_ground
                                        && attacker.position.distance(target_location) <= w.range
                                })
                                .unwrap_or(false)
                        }
                    } else {
                        false
                    }
                };

                if can_fire_at_location {
                    // AcceptableAimDelta residual for force-attack-ground.
                    let ground_slot: u8 = if rocket_pod_ground { 1 } else { 0 };
                    let aim_ok = if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                        attacker.set_ai_state(AIState::AttackingGround);
                        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                            crate::game_logic::host_ai_decision_log::record_set_state(
                                attacker_id,
                                4,
                            ); // AttackingGround
                        }
                        attacker.set_status_attacking(true);
                        let max_step = if attacker.status.moving && attacker.can_move() {
                            0.2
                        } else {
                            std::f32::consts::PI
                        };
                        attacker.turn_toward_position(target_location, ground_slot, max_step)
                    } else {
                        true
                    };
                    if !aim_ok {
                        continue;
                    }

                    // Pitch window residual for ground fire.
                    {
                        let pitch_ok = if let Some(attacker) = self.objects.get(&attacker_id) {
                            let wname = if ground_slot == 1 {
                                attacker
                                    .thing
                                    .template
                                    .secondary_weapon_name
                                    .as_deref()
                                    .or(attacker.thing.template.primary_weapon_name.as_deref())
                            } else {
                                attacker.thing.template.primary_weapon_name.as_deref()
                            };
                            let limits = wname
                                .map(crate::game_logic::weapon_bootstrap::host_target_pitch_limits_for_weapon_name)
                                .unwrap_or_default();
                            crate::game_logic::weapon_bootstrap::is_pitch_within_limits(
                                attacker.get_position(),
                                target_location,
                                &limits,
                            )
                        } else {
                            true
                        };
                        if !pitch_ok {
                            continue;
                        }
                    }
                    let mut weapon_damage = if rocket_pod_ground {
                        self.objects
                            .get(&attacker_id)
                            .and_then(|a| a.secondary_weapon.as_ref())
                            .map(|w| w.damage)
                            .unwrap_or(0.0)
                    } else {
                        self.objects
                            .get(&attacker_id)
                            .and_then(|a| a.weapon.as_ref())
                            .map(|w| w.damage)
                            .unwrap_or(0.0)
                    };
                    if overcharge {
                        weapon_damage *= 1.1;
                    }
                    // Frenzy / Rage residual: FRENZY_ONE/TWO/THREE DAMAGE mult.
                    // Strategy Center Bombardment residual: BATTLEPLAN_BOMBARDMENT DAMAGE 120%.
                    if let Some(attacker) = self.objects.get(&attacker_id) {
                        weapon_damage *= attacker.frenzy_damage_multiplier();
                        weapon_damage *= attacker.battle_plan_damage_multiplier();
                    }

                    fired_slot = Some(if rocket_pod_ground { 1 } else { 0 });

                    // Aurora dive bomb residual on force-attack-ground.
                    let aurora_ground_queued = {
                        use crate::game_logic::host_aurora_bomb::{
                            aurora_bomb_kind_for_template, is_aurora_aircraft_template,
                        };
                        let aurora = self.objects.get(&attacker_id).and_then(|a| {
                            if is_aurora_aircraft_template(&a.template_name) {
                                Some(aurora_bomb_kind_for_template(&a.template_name))
                            } else {
                                None
                            }
                        });
                        if let Some(kind) = aurora {
                            let _ = self.queue_aurora_bomb(
                                kind,
                                attacker_id,
                                attacker_team,
                                target_location,
                            );
                            true
                        } else {
                            false
                        }
                    };

                    if !aurora_ground_queued {
                        // GLA Rocket Buggy / SCUD residual ground force-fire AOE.
                        let buggy_ground = self
                            .objects
                            .get(&attacker_id)
                            .map(|a| {
                                crate::game_logic::host_rocket_buggy::is_rocket_buggy_template(
                                    &a.template_name,
                                )
                            })
                            .unwrap_or(false);
                        let scud_ground = self
                            .objects
                            .get(&attacker_id)
                            .map(|a| {
                                crate::game_logic::host_scud_launcher::is_scud_launcher_template(
                                    &a.template_name,
                                )
                            })
                            .unwrap_or(false);
                        let tomahawk_ground = self
                            .objects
                            .get(&attacker_id)
                            .map(|a| {
                                crate::game_logic::host_tomahawk::is_tomahawk_template(
                                    &a.template_name,
                                )
                            })
                            .unwrap_or(false);

                        if rocket_pod_ground {
                            // Retail FIRE_WEAPON tertiary at position → scatter projectile + area.
                            use crate::game_logic::host_comanche_rocket_pods::{
                                rocket_pod_scatter_impact, ROCKET_POD_CLIP_SIZE,
                            };
                            let idx = {
                                let shot = self
                                    .comanche_rocket_pod_shot_index
                                    .entry(attacker_id)
                                    .or_insert(0);
                                let i = *shot;
                                *shot = shot.saturating_add(1) % ROCKET_POD_CLIP_SIZE.max(1);
                                i
                            };
                            let (sx, sy, sz) = rocket_pod_scatter_impact(
                                target_location.x,
                                target_location.y,
                                target_location.z,
                                idx,
                            );
                            let aim = Vec3::new(sx, sy, sz);
                            let from = self
                                .objects
                                .get(&attacker_id)
                                .map(|o| o.get_position())
                                .unwrap_or(target_location);
                            let _ = self.spawn_comanche_rocket_pod_projectile(
                                attacker_id,
                                from,
                                aim,
                                idx,
                            );
                            let (hits, _) = self.apply_comanche_rocket_pod_area_at(
                                target_location,
                                Some(attacker_id),
                            );
                            if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                if hits > 0 {
                                    attacker.gain_experience((hits as f32) * 10.0);
                                }
                            }
                            let _ = weapon_damage; // area residual owns damage
                        } else if buggy_ground {
                            let from = self
                                .objects
                                .get(&attacker_id)
                                .map(|a| a.get_position())
                                .unwrap_or(target_location);
                            let spawned = self
                                .spawn_rocket_buggy_missile_projectile(
                                    attacker_id,
                                    from,
                                    target_location,
                                    None,
                                )
                                .is_some();
                            let (hits, _) = if spawned {
                                self.rocket_buggy_residual_fires =
                                    self.rocket_buggy_residual_fires.saturating_add(1);
                                (1, false)
                            } else {
                                self.apply_rocket_buggy_residual_at(
                                    target_location,
                                    Some(attacker_id),
                                    None,
                                )
                            };
                            if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                if hits > 0 {
                                    attacker.gain_experience((hits as f32) * 8.0);
                                }
                            }
                            let _ = weapon_damage;
                        } else if scud_ground {
                            // Ground force-fire: stock uses primary explosive; Chem SCUD
                            // residual uses anthrax primary (slot 0 toxin warhead).
                            let toxin = {
                                use crate::game_logic::host_scud_launcher::scud_toxin_warhead_for_slot;
                                self.objects
                                    .get(&attacker_id)
                                    .map(|a| scud_toxin_warhead_for_slot(&a.template_name, 0))
                                    .unwrap_or(false)
                            };
                            let from = self
                                .objects
                                .get(&attacker_id)
                                .map(|a| a.get_position())
                                .unwrap_or(target_location);
                            let spawned = self
                                .spawn_scud_launcher_missile_projectile(
                                    attacker_id,
                                    from,
                                    target_location,
                                    None,
                                    toxin,
                                )
                                .is_some();
                            let (hits, _) = if spawned {
                                (1, false)
                            } else {
                                self.apply_scud_area_at(
                                    target_location,
                                    Some(attacker_id),
                                    attacker_team,
                                    toxin,
                                )
                            };
                            if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                if hits > 0 {
                                    attacker.gain_experience((hits as f32) * 15.0);
                                }
                            }
                            let _ = weapon_damage;
                        } else if tomahawk_ground {
                            let from = self
                                .objects
                                .get(&attacker_id)
                                .map(|a| a.get_position())
                                .unwrap_or(target_location);
                            let spawned = self
                                .spawn_tomahawk_missile_projectile(
                                    attacker_id,
                                    from,
                                    target_location,
                                    None,
                                )
                                .is_some();
                            let (hits, _) = if spawned {
                                self.tomahawk_residual_fires =
                                    self.tomahawk_residual_fires.saturating_add(1);
                                (1, false)
                            } else {
                                self.apply_tomahawk_residual_at(
                                    target_location,
                                    Some(attacker_id),
                                    None,
                                )
                            };
                            if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                if hits > 0 {
                                    attacker.gain_experience((hits as f32) * 15.0);
                                }
                            }
                            let _ = weapon_damage;
                        } else if let Some(ground_target_id) =
                            self.find_ground_attack_victim(attacker_id, target_location)
                        {
                            if let Some(target) = self.objects.get_mut(&ground_target_id) {
                                let destroyed =
                                    target.take_damage_from(weapon_damage, Some(attacker_id));
                                if destroyed {
                                    let kill_xp = target.thing.template.experience_value
                                        * Self::veterancy_xp_multiplier(target.experience.level);
                                    self.mark_object_for_destruction(
                                        ground_target_id,
                                        Some(attacker_team),
                                    );
                                    if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                                        attacker.gain_experience(kill_xp);
                                    }
                                }
                            }
                        }

                        // Inferno Cannon residual: ground attack also seeds FireFieldSmall.
                        if !rocket_pod_ground && !buggy_ground && !scud_ground {
                            use crate::game_logic::host_inferno_cannon::is_inferno_cannon_template;
                            let is_inferno = self
                                .objects
                                .get(&attacker_id)
                                .map(|a| is_inferno_cannon_template(&a.template_name))
                                .unwrap_or(false);
                            if is_inferno {
                                let upgraded = self
                                    .objects
                                    .get(&attacker_id)
                                    .map(|a| {
                                        crate::game_logic::host_inferno_cannon::has_black_napalm_upgrade(
                                            &a.applied_upgrades,
                                        )
                                    })
                                    .unwrap_or(false);
                                let _ = self.spawn_inferno_fire_zone(
                                    attacker_id,
                                    attacker_team,
                                    target_location,
                                    upgraded,
                                );
                            }
                        }
                    }
                }
            }

            if let Some(slot) = fired_slot {
                // Pathfinder residual sniper honesty (any successful fire from residual).
                if self
                    .objects
                    .get(&attacker_id)
                    .map(|a| {
                        crate::game_logic::host_pathfinder::is_pathfinder_template(&a.template_name)
                    })
                    .unwrap_or(false)
                {
                    self.pathfinder_residual_sniper_fires =
                        self.pathfinder_residual_sniper_fires.saturating_add(1);
                }

                // Quad Cannon residual honesty: ground primary vs AA secondary fires.
                if self
                    .objects
                    .get(&attacker_id)
                    .map(|a| {
                        crate::game_logic::host_quad_cannon::is_quad_cannon_template(
                            &a.template_name,
                        )
                    })
                    .unwrap_or(false)
                {
                    if slot == 1 {
                        self.quad_cannon_residual_aa_fires =
                            self.quad_cannon_residual_aa_fires.saturating_add(1);
                    } else {
                        self.quad_cannon_residual_ground_fires =
                            self.quad_cannon_residual_ground_fires.saturating_add(1);
                    }
                }

                // China Gattling Tank residual: advance continuous-fire ramp + honesty.
                if self
                    .objects
                    .get(&attacker_id)
                    .map(|a| {
                        crate::game_logic::host_gattling_tank::is_gattling_tank_template(
                            &a.template_name,
                        )
                    })
                    .unwrap_or(false)
                {
                    self.advance_gattling_continuous_fire(attacker_id, target_id, slot);
                }

                // China MiniGunner residual: advance continuous-fire ramp + honesty.
                if self
                    .objects
                    .get(&attacker_id)
                    .map(|a| {
                        crate::game_logic::host_minigunner::is_minigunner_template(&a.template_name)
                    })
                    .unwrap_or(false)
                {
                    self.advance_minigunner_continuous_fire(attacker_id, target_id, slot);
                }

                // Combat particle residual: weapon fire → muzzle (+ impact) registry entries.
                let muzzle_pos = self
                    .objects
                    .get(&attacker_id)
                    .map(|a| a.get_position())
                    .unwrap_or(Vec3::ZERO);
                let fire_target = target_id.filter(|id| self.objects.contains_key(id));
                let impact_pos = fire_target
                    .and_then(|id| self.objects.get(&id).map(|t| t.get_position()))
                    .or(target_location);
                let fire_frame = self.frame;
                let (fire_fx, det_fx) = {
                    let a = self.objects.get(&attacker_id);
                    let (tname, pwn, swn) = a
                        .map(|o| {
                            (
                                o.template_name.as_str(),
                                o.thing.template.primary_weapon_name.as_deref(),
                                o.thing.template.secondary_weapon_name.as_deref(),
                            )
                        })
                        .unwrap_or(("", None, None));
                    crate::game_logic::weapon_bootstrap::host_weapon_fx_for_unit_slot(
                        tname, pwn, swn, slot,
                    )
                };
                let (fire_ocl, det_ocl) = {
                    let a = self.objects.get(&attacker_id);
                    let (tname, pwn, swn) = a
                        .map(|o| {
                            (
                                o.template_name.as_str(),
                                o.thing.template.primary_weapon_name.as_deref(),
                                o.thing.template.secondary_weapon_name.as_deref(),
                            )
                        })
                        .unwrap_or(("", None, None));
                    crate::game_logic::weapon_bootstrap::host_weapon_ocl_for_unit_slot(
                        tname, pwn, swn, slot,
                    )
                };
                // C++ Weapon::fireWeaponTemplate FireFX stealth gate residual:
                // stealthed+undetected+non-disguised suppress muzzle FX unless
                // PlayFXWhenStealthed or KINDOF_MINE.
                let suppress_fire_fx = {
                    let a = self.objects.get(&attacker_id);
                    a.map(|o| {
                        let is_mine = {
                            let n = o.template_name.to_ascii_lowercase();
                            n.contains("mine")
                                || n.contains("demotrap")
                                || n.contains("booby")
                        };
                        let hidden = o.status.stealthed
                            && !o.status.detected
                            && !o.status.disguised
                            && !is_mine;
                        if !hidden {
                            return false;
                        }
                        let wname = if slot == 1 {
                            o.thing
                                .template
                                .secondary_weapon_name
                                .as_deref()
                                .or(o.thing.template.primary_weapon_name.as_deref())
                        } else {
                            o.thing.template.primary_weapon_name.as_deref()
                        };
                        let play = wname
                            .map(
                                crate::game_logic::weapon_bootstrap::host_play_fx_when_stealthed_for_weapon_name,
                            )
                            .unwrap_or(false);
                        !play
                    })
                    .unwrap_or(false)
                };
                if !suppress_fire_fx {
                    let _ = self.combat_particles.spawn_weapon_fire_fx_named_ocl(
                        muzzle_pos,
                        impact_pos,
                        fire_frame,
                        attacker_id,
                        fire_target,
                        &fire_fx,
                        &det_fx,
                        &fire_ocl,
                        // Ballistic detonation OCL is applied at real impact via projectiles.
                        // Instant combat residual still stamps det_ocl at fire-time impact.
                        &det_ocl,
                    );
                }
                // C++ Weapon.ini LaserName residual: short-lived combat beam for
                // presentation / laser_segment_upload observe path.
                {
                    let a = self.objects.get(&attacker_id);
                    let (tname, pwn, swn) = a
                        .map(|o| {
                            (
                                o.template_name.as_str(),
                                o.thing.template.primary_weapon_name.as_deref(),
                                o.thing.template.secondary_weapon_name.as_deref(),
                            )
                        })
                        .unwrap_or(("", None, None));
                    let laser_name =
                        crate::game_logic::weapon_bootstrap::host_laser_name_for_unit_slot(
                            tname, pwn, swn, slot,
                        );
                    if !laser_name.is_empty() {
                        let laser_bone =
                            crate::game_logic::weapon_bootstrap::host_laser_bone_name_for_unit_slot(
                                tname, pwn, swn, slot,
                            );
                        let to = impact_pos.unwrap_or(muzzle_pos);
                        let laser_name_owned = laser_name.clone();
                        self.weapon_lasers.push(
                            crate::game_logic::host_weapon_laser::ResidualWeaponLaser::with_bone(
                                laser_name,
                                laser_bone,
                                attacker_id,
                                fire_target,
                                (muzzle_pos.x, muzzle_pos.y, muzzle_pos.z),
                                (to.x, to.y, to.z),
                                fire_frame,
                            ),
                        );
                        let _ = self.spawn_weapon_laser_beam_object(
                            &laser_name_owned,
                            attacker_id,
                            fire_target,
                            muzzle_pos,
                            to,
                        );
                    }
                }

                // Audio residual (hq-7zxm slice): weapon fire → real AudioEventRequest.
                // Prefer Weapon.ini FireSound via store name; fallback "WeaponFire".
                let fire_sound = {
                    let a = self.objects.get(&attacker_id);
                    let (tname, pwn, swn) = a
                        .map(|o| {
                            (
                                o.template_name.as_str(),
                                o.thing.template.primary_weapon_name.as_deref(),
                                o.thing.template.secondary_weapon_name.as_deref(),
                            )
                        })
                        .unwrap_or(("", None, None));
                    crate::game_logic::weapon_bootstrap::host_fire_sound_for_unit_slot(
                        tname, pwn, swn, slot,
                    )
                };
                self.queue_audio_event(
                    AudioEventRequest::new(fire_sound.as_str())
                        .with_object(attacker_id)
                        .with_position(muzzle_pos)
                        .with_priority(160),
                );

                // Capture weapon name before mut borrow for RETURN_TO_BASE peels.
                let fire_wname = self.objects.get(&attacker_id).and_then(|a| {
                    if slot == 1 {
                        a.thing
                            .template
                            .secondary_weapon_name
                            .clone()
                            .or_else(|| a.thing.template.primary_weapon_name.clone())
                    } else {
                        a.thing.template.primary_weapon_name.clone()
                    }
                });
                if let Some(attacker) = self.objects.get_mut(&attacker_id) {
                    if let Some(weapon) = attacker.weapon_slot_mut(slot) {
                        Object::consume_ammo_on_fire_named(
                            weapon,
                            current_time,
                            fire_wname.as_deref(),
                        );
                    }
                    if let Some(tid) = attacker.target {
                        attacker.record_shot_at_target(tid);
                        attacker.stamp_continuous_fire_coast(self.frame);
                        attacker.stamp_auto_reload_when_idle(self.frame);
                    }
                    // C++ STEALTH_NOT_WHILE_ATTACKING residual: combat fire breaks stealth.
                    if attacker.stealth_breaks_on_attack && attacker.status.stealthed {
                        attacker.break_stealth();
                    }
                }
            }
        }

        // AssistedTargeting residual: advance pending Patriot assist clips after
        // primary fire this combat pass (AssistingClipSize / DelayBetweenShots).
        // Wave 824: under coupled shadow, pending patriot assists sole-tick after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_pending_patriot_assists();
        }
        // BinaryDataStream laser residual: expire DeletionUpdate lifetime beams.
        // Wave 823: under coupled shadow, patriot assist lasers sole-tick after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_patriot_assist_lasers();
        }
        // Weapon.ini LaserName residual lifetime / scroll.
        crate::game_logic::host_weapon_laser::update_weapon_lasers(
            &mut self.weapon_lasers,
            self.frame,
        );
    }

    pub(super) fn find_ground_attack_victim(
        &self,
        attacker_id: ObjectId,
        target_location: Vec3,
    ) -> Option<ObjectId> {
        const GROUND_IMPACT_RADIUS: f32 = 12.0;

        let attacker = self.objects.get(&attacker_id)?;
        let force_attack = attacker.force_attack;
        let attacker_team = attacker.team;

        // Pure residual acquire: nearest attackable victim near ground impact (3D).
        let candidate_ids: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(&candidate_id, candidate)| {
                if candidate_id == attacker_id
                    || !candidate.is_alive()
                    || !candidate.is_attackable()
                {
                    return None;
                }
                if !force_attack && candidate.team == attacker_team {
                    return None;
                }
                Some(candidate_id)
            })
            .collect();

        let attacker = self.objects.get(&attacker_id)?;
        let candidates: Vec<_> = candidate_ids
            .into_iter()
            .filter_map(|id| {
                let candidate = self.objects.get(&id)?;
                if !attacker.can_target(candidate) {
                    return None;
                }
                Some(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id,
                        team: candidate.team,
                        position: candidate.get_position(),
                        is_alive: true,
                        is_neutral: candidate.team == Team::Neutral,
                        under_construction: candidate.status.under_construction,
                        combat_kind: true,
                        effectively_stealthed: candidate.is_effectively_stealthed(),
                        is_air: candidate.is_kind_of(KindOf::Aircraft)
                            || candidate.status.airborne_target,
                        eject_invulnerable: candidate.is_eject_invulnerable(),
                    },
                )
            })
            .collect();

        crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
            attacker_id,
            attacker_team,
            target_location,
            candidates,
            |_| GROUND_IMPACT_RADIUS,
            |_| true,
        )
        .map(|(id, _, _)| id)
    }

}
