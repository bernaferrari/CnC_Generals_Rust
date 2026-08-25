//! Host tick `impl GameLogic` — `step`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
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

    pub(in super::super) fn step_simulation(
        &mut self,
        delta_time: f32,
        absolute_time: Option<f32>,
    ) {
        self.step_simulation_with_budget(delta_time, absolute_time, None);
    }

    pub(in super::super) fn step_simulation_with_budget(
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

        self.commit_dirty_host_objects_to_gameworld();
        // C++ processDestroyList runs inside every GameLogic::update
        // (GameLogic.cpp:3762), not once after the fixed-step catch-up batch.
    }

    /// Live-host object fold mixed into leftover `GameLogic::getCRC`.
    /// C++ walks `m_objList`; leftover objects may be empty on the live tick.
    fn fold_live_host_logic_crc(&self) -> u32 {
        let mut hasher = game_engine::common::crc::Crc::new();
        hasher.compute_crc(&self.frame.to_le_bytes());
        let mut ids: Vec<u32> = self.objects.keys().map(|id| id.0).collect();
        ids.sort_unstable();
        for id in ids {
            let Some(obj) = self.objects.get(&ObjectId(id)) else {
                continue;
            };
            hasher.compute_crc(&id.to_le_bytes());
            hasher.compute_crc(&obj.position.x.to_bits().to_le_bytes());
            hasher.compute_crc(&obj.position.y.to_bits().to_le_bytes());
            hasher.compute_crc(&obj.position.z.to_bits().to_le_bytes());
            hasher.compute_crc(&obj.health.current.to_bits().to_le_bytes());
            hasher.compute_crc(&obj.health.maximum.to_bits().to_le_bytes());
        }
        hasher.get()
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
    /// Line 3625: getCRC + MSG_LOGIC_CRC               [replay CRC]
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
    pub(in super::super) fn update_simulation(&mut self, dt: f32) {
        // Presentation spawn/destruction events are logic-frame scoped. Keep
        // active particle systems themselves, but never replay old spawn events
        // forever on later presentation frames.
        self.combat_particles.clear_frame_events();
        // HashMap starts the tick as a GameWorld view (HP/pose/target/fat fields).
        self.sync_authoritative_view_from_gameworld();
        // Pathfinding dynamic obstacles rebuild once per host logic frame.
        self.pathfinding_system.note_logic_frame(self.frame as u64);
        self.refresh_pathfind_ally_masks();
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
        // C++ setWaterHeight: leftover scripts queue DAMAGE_WATER amounts.
        // Instant WATER_CHANGE_HEIGHT uses 999999.9; over-time uses scripted amount.
        for amount in gamelogic::terrain::take_pending_host_water_rise_damage() {
            let _ = self.apply_water_rise_damage(amount);
        }
        if gamelogic::terrain::take_pending_host_pathfind_recalculation() {
            self.seed_pathfinding_from_terrain();
        }

        // Refresh underwater/cliff flags only. Do not invent a 25 HP dry→wet chip.
        let _ = self.refresh_surface_cells_and_water_edge_damage(0.0);

        // -----------------------------------------------------------------------
        // Phase 3b: Logic CRC (C++ GameLogic.cpp:3625-3654)
        // -----------------------------------------------------------------------
        // C++: m_CRC = getCRC(CRC_RECALC); TheMessageStream->appendMessage(MSG_LOGIC_CRC);
        // then TheRecorder->UPDATE() inside processCommandList's recorder flush.
        crate::command_system::stamp_host_logic_frame(self.frame);
        crate::command_system::post_host_logic_crc_if_due(
            self.frame,
            self.fold_live_host_logic_crc(),
        );

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
        // Phase 6: Object Updates -- Sleepy heap (C++ GameLogic.cpp:3699-3740)
        // -----------------------------------------------------------------------
        // C++ ticks UpdateModules via m_sleepyUpdates (wake_frame, phase).
        // Host residuals are scheduled the same way: drain due modules, then
        // run the matching system tick. UPDATE_SLEEP_NONE → next frame.
        let now = self.frame.max(1);
        let due = self.host_sleepy.drain_due(now);
        for kind in due {
            match kind {
                super::HostSleepyKind::Construction => {
                    self.update_construction(&object_ids, dt);
                }
                super::HostSleepyKind::SellList => {
                    if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                        && crate::gameworld_shadow::shadow_coupled_tick_active())
                    {
                        self.update_sell_list();
                    }
                }
                super::HostSleepyKind::DozerBoredRepair => {
                    if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                        && crate::gameworld_shadow::shadow_coupled_tick_active())
                    {
                        self.update_dozer_bored_repair();
                    }
                }
                super::HostSleepyKind::RebuildHoles => {
                    self.update_rebuild_holes();
                }
                super::HostSleepyKind::Movement => {
                    self.update_movement(&object_ids, dt);
                }
                super::HostSleepyKind::SpecialPowers => {
                    update_special_powers();
                }
            }
        }

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

        // Host GLA Rebel Ambush residual: leftover OCLSpecialPower CreateObject
        // (science UpgradeOCL) plus FadeIn/DiesOnBadLand after the fire-frame spawn.
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
        // Fail-closed vs full AutoHealBehavior particle / world-anim pulse FX.
        self.update_ambulance_auto_heal(dt);
        self.update_default_auto_heal();
        // C++ TransportContain::update HealthRegen%PerSec on embarked riders.
        self.update_transport_health_regen(dt);

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

        // Host China vehicle HordeUpdate residual. Vehicles run every frame for
        // decal size (UPDATE_SLEEP_NONE) but membership is gated on UpdateRate.
        // Wave 812: under coupled shadow, horde status owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_battlemaster_horde_status();
        }

        // Host China infantry HordeUpdate residual. Infantry UPDATE_SLEEP(UpdateRate).
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
        self.update_cleanup_area_orders();
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
        crate::command_executor::tick_live_beacon_client_updates(self);

        // Host CIA Intelligence residual: expire vision-spied marks + FOW undos.
        // Fail-closed vs full SpyVisionUpdate setUnitsVisionSpied module path.
        self.update_satellite_hack_spy_vision();
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
        self.update_supply_warehouse_crippling();

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
        self.update_spectre_gunship_flights();
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

        // Host RailroadBehavior residual: locomotives follow track waypoints,
        // wait at stations, hitch carriages. Crate RailroadGuideAIUpdate is
        // gated empty (OBJECT_REGISTRY) so this is the live path.
        // C++ RailroadBehavior::update (RailroadGuideAIUpdate.cpp:652-832).
        self.update_railroads();
        // C++ RailedTransportAIUpdate::update — park at nearest End, open dock
        // when the current End is reached. Crate leftover is dual-world gated.
        self.update_railed_transports();

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
        self.mirror_overlord_addon_damage_after_combat();
        self.flush_pending_garrison_really_damaged_ejects();
        self.flush_subdual_passenger_orders();
        // Nested AttackStateMachine residual (privateAttackObject enter path).
        let frame = self.frame;
        let t = frame as f32 * LOGIC_FRAME_TIMESTEP;
        self.tick_nested_attack_machines(&object_ids, t, frame);
        self.tick_all_turret_state_machines(&object_ids, t, frame);
        self.sync_attack_priority_from_script_engine();
        self.tick_mood_auto_acquire(&object_ids);
        self.tick_attack_team_persist(&object_ids);
        self.tick_attack_area_persist(&object_ids);
        self.tick_out_of_ammo_jet_damage();
        self.tick_airfield_parking_heal();
        self.tick_airfield_runway_clear();
        self.tick_shock_stun_all();
        // C++ PartitionManager collide residual (broadphase fail-closed O(n²)).
        self.tick_physics_collisions_all();

        // Projectiles: drain global fire queue into host CombatSystem and step.
        // Sole ownership — engine must not maintain a second mid-frame CombatSystem.
        crate::game_logic::host_historic_bonus::set_logic_frame(self.frame);
        {
            let objects = &self.objects;
            crate::game_logic::combat::drain_pending_projectiles(&mut self.combat_system, objects);
        }
        crate::game_logic::combat::apply_ready_projectileless_delayed_damage(
            &mut self.combat_system,
            &mut self.objects,
            self.frame,
            Some(&self.players),
        );

        // C++ Weapon::fireWeaponTemplate runs FireOCL at shot acceptance,
        // independently of whether the projectile later finds a target.
        self.execute_pending_weapon_fire_ocls();
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
            let hits = self.combat_system.update_projectiles_with_relationships(
                dt,
                &mut self.objects,
                Some(&mut self.countermeasures),
                self.frame,
                Some(&self.players),
            );
            crate::game_logic::host_projectile_log::record_snapshot(
                self.combat_system.projectiles_snapshot(),
            );
            hits
        };
        for victim in self.combat_system.take_pending_under_attack() {
            let _ = self.try_under_attack_from_damage(victim);
        }
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
        // C++ Weapon.ini ProjectileDetonationFX/OCL residual at real impact
        // (not fire-time), for either host or GameWorld flight ownership.
        self.flush_projectile_impact_fx();
        // C++ Weapon.ini ProjectileExhaust: MissileAI attaches at IGNITION
        // (`createAttachedParticleSystemID`). Publish leftover host poses so
        // attached systems follow; world-space sync is the fallback when
        // leftover attach is unavailable.
        {
            let frame = self.frame;
            let snaps = self.combat_system.projectiles_snapshot();
            for p in &snaps {
                p.publish_attached_exhaust_pose();
            }
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
                .sync_projectile_exhausts(frame, &exhausts);
            self.sync_live_state_particles();
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
                        shooter.and_then(|o| o.weapon_name_for_slot(o.last_fire_slot))
                    };
                    let sname = shooter
                        .map(|o| {
                            crate::game_logic::weapon_bootstrap::host_projectile_stream_name_for_slots(
                                firing_name,
                                o.weapon_name_for_slot(0),
                                o.weapon_name_for_slot(1),
                                o.weapon_name_for_slot(2),
                            )
                        })
                        .unwrap_or_else(|| {
                            firing_name
                                .map(crate::game_logic::weapon_bootstrap::host_projectile_stream_name_for_weapon_name)
                                .unwrap_or_default()
                        });
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
                        let geom = &owner.thing.template.geometry_info;
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
            // 1. THE_AI.update only drains the crate Pathfinder queue (it no
            //    longer walks crate groups / ThePlayerList — that is AIManager).
            // Host objects live in GameLogic.objects, not OBJECT_REGISTRY, so
            // an empty crate world has no pathfinder work the host needs.
            // Skip to avoid pretending this is the live AI tick. Host
            // AIManager.update below remains the real TheAI residual.
            let crate_world_empty = gamelogic::object::registry::OBJECT_REGISTRY.is_empty();
            if !crate_world_empty {
                if let Ok(mut ai) = THE_AI.write() {
                    if let Err(e) = ai.update(self.frame) {
                        log::warn!("THE_AI update failed at frame {}: {:?}", self.frame, e);
                    }
                }
            } else {
                static SKIP_THE_AI: std::sync::Once = std::sync::Once::new();
                SKIP_THE_AI.call_once(|| {
                    log::info!(
                        "Skipping THE_AI.update: OBJECT_REGISTRY/crate world is empty; \
                         crate pathfinder never sees host objects (host AIManager still runs)"
                    );
                });
            }

            // 2. AiIntegrationManager (per-player crate AIPlayer / SkirmishPlayer).
            // Skip when no host-registered crate AI players exist — an empty
            // integration would dual-simulate nothing and must not replace
            // host AIManager.update below.
            let has_crate_ai_players = gamelogic::ai::integration::with_ai_integration(|mgr| {
                mgr.get_ai_player_count() > 0
            })
            .unwrap_or(false);
            if has_crate_ai_players {
                if let Some(result) = with_ai_integration_mut(|mgr| mgr.update_ai_players_only()) {
                    if let Err(e) = result {
                        log::warn!(
                            "AiIntegrationManager update failed at frame {}: {:?}",
                            self.frame,
                            e
                        );
                    }
                }
            } else {
                static SKIP_AI_INTEGRATION: std::sync::Once = std::sync::Once::new();
                SKIP_AI_INTEGRATION.call_once(|| {
                    log::info!(
                        "Skipping update_ai_players_only: no host-registered crate AI players \
                         (host AIManager still runs)"
                    );
                });
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
        // USA Battle Drone residual master repair: 12-unit weld SM, not instant 48-range.
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
        // Note: The gamelogic crate's full update_pipeline also runs its own
        // partition manager update (tick_gamelogic_crate in cnc_game_engine.rs).

        // -----------------------------------------------------------------------
        // Phase 13: Death/Cleanup (C++ line 3762)
        // -----------------------------------------------------------------------
        // C++: processDestroyList();
        // Same-frame cascades: iterator re-evaluates end() so objects queued
        // during deletion are processed this logic frame (GameLogic.cpp:2449-2510).
        self.fire_temporary_weapons_for_pending_deaths();
        self.process_destroy_list();
        // C++ BridgeBehavior/Tower onDamage/onHealing/onDie + scaffold rise tick.
        // Residual drain is same-frame after object combat/AI/repair and destroy.
        self.sync_host_bridge_rubble_and_scaffolds();

        // -----------------------------------------------------------------------
        // Phase 14: Weapon Store Update (C++ line 3767)
        // -----------------------------------------------------------------------
        // C++: TheWeaponStore->UPDATE();
        // Process delayed weapon damage that is now ready.
        crate::game_logic::combat::apply_ready_projectileless_delayed_damage(
            &mut self.combat_system,
            &mut self.objects,
            self.frame,
            Some(&self.players),
        );
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
        // Evaluate inside every logic frame, not only PresentationFrame::build.
        let _ = self.evaluate_victory_condition();

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
        // C++ Radar.cpp overlay queries live Object pose/stealth each update.
        self.host_radar_sync_live_objects();

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
}
